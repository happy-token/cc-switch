//! Browser authorization never imports browser cookies into the desktop application.
use crate::{provider::Provider, store::AppState};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
const GATEWAY: &str = "https://gateway.happy-token.cn";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResult {
    account: String,
    groups: Vec<String>,
    providers: usize,
    warnings: Vec<String>,
}
#[derive(Deserialize)]
struct BrowserGrant {
    id: String,
    code: String,
    #[serde(rename = "expiresIn")]
    expires_in: u64,
}
#[derive(Serialize)]
pub struct BrowserLogin {
    code: String,
}
#[derive(Deserialize)]
struct GroupSnapshot {
    name: String,
    key: String,
    models: Vec<String>,
}
#[derive(Deserialize)]
struct Snapshot {
    uid: u64,
    account: String,
    groups: Vec<GroupSnapshot>,
    warnings: Vec<String>,
}
struct PendingLogin {
    code: String,
    url: String,
    cancelled: Arc<AtomicBool>,
}
static PENDING: Lazy<Mutex<Option<PendingLogin>>> = Lazy::new(|| Mutex::new(None));
static STARTING: AtomicBool = AtomicBool::new(false);

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn valid_grant(grant: &BrowserGrant) -> bool {
    grant.id.len() == 32
        && grant
            .id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        && grant.code.len() == 8
        && grant
            .code
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
        && (1..=600).contains(&grant.expires_in)
}
fn client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "无法创建登录客户端".into())
}
fn open_browser(app: &tauri::AppHandle, url: &str) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|_| "无法打开默认浏览器".into())
}

#[tauri::command]
pub async fn happy_token_login(app: tauri::AppHandle) -> Result<BrowserLogin, String> {
    {
        let pending = PENDING.lock().map_err(|_| "登录状态不可用")?;
        if let Some(pending) = pending.as_ref() {
            open_browser(&app, &pending.url)?;
            return Ok(BrowserLogin {
                code: pending.code.clone(),
            });
        }
    }
    if STARTING.swap(true, Ordering::SeqCst) {
        return Err("正在打开浏览器登录，请稍候".into());
    }
    let result = start_login(app).await;
    STARTING.store(false, Ordering::SeqCst);
    result
}
async fn start_login(app: tauri::AppHandle) -> Result<BrowserLogin, String> {
    // Two independent UUIDs provide 244 random bits; base64url produces a 43-character verifier.
    let mut bytes = Vec::from(*uuid::Uuid::new_v4().as_bytes());
    bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let client = client()?;
    let response = client
        .post(format!("{GATEWAY}/sso/desktop/start"))
        .json(&json!({"challenge": challenge(&verifier)}))
        .send()
        .await
        .map_err(|_| "无法连接 HappyToken，请检查网络")?;
    if !response.status().is_success() {
        return Err("浏览器授权服务尚不可用，请确认网关已发布桌面授权功能".into());
    }
    let grant: BrowserGrant = response
        .json()
        .await
        .map_err(|_| "网关尚未提供浏览器授权接口")?;
    if !valid_grant(&grant) {
        return Err("浏览器授权响应无效".into());
    }
    let url = format!("{GATEWAY}/sso/desktop?id={}", grant.id);
    if let Err(error) = open_browser(&app, &url) {
        let _ = client
            .post(format!("{GATEWAY}/sso/desktop/cancel"))
            .json(&json!({"id":grant.id,"verifier":verifier}))
            .send()
            .await;
        return Err(error);
    }
    let code = grant.code.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    *PENDING.lock().map_err(|_| "登录状态不可用")? = Some(PendingLogin {
        code: code.clone(),
        url,
        cancelled: cancelled.clone(),
    });
    tauri::async_runtime::spawn(async move {
        let result = poll_login(&app, &client, &grant, &verifier, &cancelled).await;
        if let Ok(mut pending) = PENDING.lock() {
            *pending = None;
        }
        match result {
            Ok(Some(result)) => {
                let _ = app.emit_to("main", "happy-token-synced", result);
            }
            Ok(None) => {
                let _ = app.emit_to("main", "happy-token-cancelled", ());
            }
            Err(error) => {
                let _ = app.emit_to("main", "happy-token-error", error);
            }
        }
    });
    Ok(BrowserLogin { code })
}
#[tauri::command]
pub fn happy_token_cancel_login() -> Result<(), String> {
    if let Some(pending) = PENDING.lock().map_err(|_| "登录状态不可用")?.as_ref() {
        pending.cancelled.store(true, Ordering::SeqCst);
    }
    Ok(())
}
async fn poll_login(
    app: &tauri::AppHandle,
    client: &Client,
    grant: &BrowserGrant,
    verifier: &str,
    cancelled: &AtomicBool,
) -> Result<Option<SyncResult>, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(grant.expires_in);
    loop {
        if cancelled.load(Ordering::SeqCst) || tokio::time::Instant::now() >= deadline {
            let _ = client
                .post(format!("{GATEWAY}/sso/desktop/cancel"))
                .json(&json!({"id":grant.id,"verifier":verifier}))
                .send()
                .await;
            return if cancelled.load(Ordering::SeqCst) {
                Ok(None)
            } else {
                Err("浏览器登录已超时，请重新登录".into())
            };
        }
        let response = client
            .post(format!("{GATEWAY}/sso/desktop/poll"))
            .json(&json!({"id":grant.id,"verifier":verifier}))
            .send()
            .await
            .map_err(|_| "无法获取浏览器授权结果，请重新登录")?;
        if cancelled.load(Ordering::SeqCst) {
            continue;
        }
        match response.status().as_u16() {
            202 | 429 => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            200 => {
                let value: Value = response.json().await.map_err(|_| "授权结果格式无效")?;
                if value["status"] != "ready" {
                    return Err("授权状态无效".into());
                }
                let snapshot: Snapshot = serde_json::from_value(value["data"].clone())
                    .map_err(|_| "授权配置格式无效")?;
                validate_snapshot(&snapshot)?;
                let _ = app.emit_to("main", "happy-token-syncing", ());
                return sync_snapshot(app, snapshot).await.map(Some);
            }
            410 => return Err("浏览器授权已过期、取消或同步失败，请重新登录".into()),
            _ => return Err("无法完成浏览器授权，请重新登录".into()),
        }
    }
}
fn validate_snapshot(snapshot: &Snapshot) -> Result<(), String> {
    if snapshot.uid == 0
        || snapshot.account.len() > 1024
        || snapshot.groups.is_empty()
        || snapshot.groups.len() > 32
        || snapshot.warnings.len() > 32
    {
        return Err("授权配置无效".into());
    }
    let mut names = std::collections::HashSet::new();
    for group in &snapshot.groups {
        if group.name.is_empty()
            || group.name.len() > 256
            || !names.insert(&group.name)
            || !group.key.starts_with("sk-")
            || group.key.len() <= 3
            || group.key.len() > 512
            || group.key.contains('*')
            || group.key.chars().any(char::is_control)
            || group.models.len() > 10000
            || group
                .models
                .iter()
                .any(|m| m.len() > 512 || m.chars().any(char::is_control))
        {
            return Err("授权分组配置无效".into());
        }
    }
    Ok(())
}

async fn sync_snapshot(app: &tauri::AppHandle, snapshot: Snapshot) -> Result<SyncResult, String> {
    let mut result = SyncResult {
        account: snapshot.account,
        groups: Vec::new(),
        providers: 0,
        warnings: snapshot.warnings,
    };
    for group in snapshot.groups {
        let providers = build_providers(snapshot.uid, &group.name, &group.key, &group.models);
        if providers.is_empty() {
            result.warnings.push(format!(
                "{}：没有适用于 Claude Code、Codex 或 Gemini 的模型",
                group.name
            ));
            continue;
        }
        let handle = app.clone();
        let count = tauri::async_runtime::spawn_blocking(move || {
            let state = handle.state::<AppState>();
            let mut count = 0;
            for (app_type, mut provider) in providers {
                // Save to the library only; switching uses CC Switch's normal config writer.
                // Re-sync refreshes credentials while retaining user customizations.
                if let Some(mut existing) = state
                    .db
                    .get_provider_by_id(&provider.id, app_type)
                    .map_err(|_| "无法读取已有 HappyToken 配置")?
                {
                    refresh_key(
                        &mut existing.settings_config,
                        &provider.settings_config,
                        app_type,
                    )?;
                    provider = existing;
                    crate::services::ProviderService::update(
                        state.inner(),
                        crate::app_config::AppType::from_str(app_type)
                            .map_err(|_| "助手类型无效")?,
                        None,
                        provider,
                    )
                    .map_err(|_| "无法更新 HappyToken 配置")?;
                    count += 1;
                    continue;
                }
                state
                    .db
                    .save_provider(app_type, &provider)
                    .map_err(|_| "无法保存 HappyToken 配置")?;
                count += 1;
            }
            Ok::<_, String>(count)
        })
        .await
        .map_err(|_| "配置保存任务失败")??;
        result.providers += count;
        result.groups.push(group.name);
    }
    if result.providers == 0 {
        return Err(format!("未导入任何配置。{}", result.warnings.join("；")));
    }
    Ok(result)
}

fn build_providers(
    uid: u64,
    group: &str,
    key: &str,
    models: &[String],
) -> Vec<(&'static str, Provider)> {
    let mut result = Vec::new();
    for app_type in ["claude", "codex", "gemini"] {
        let model = models.iter().rev().find(|model| {
            let name = model.to_ascii_lowercase();
            match app_type {
                "claude" => name.starts_with("claude-"),
                "gemini" => {
                    name.starts_with("gemini-")
                        && !name.contains("image")
                        && !name.contains("embedding")
                }
                _ => {
                    (name.starts_with("gpt-")
                        || name.starts_with("o1")
                        || name.starts_with("o3")
                        || name.starts_with("o4"))
                        && ![
                            "image",
                            "audio",
                            "realtime",
                            "transcribe",
                            "tts",
                            "search",
                            "instruct",
                        ]
                        .iter()
                        .any(|part| name.contains(part))
                }
            }
        });
        let Some(model) = model else {
            continue;
        };
        let settings = match app_type {
            "claude" => {
                json!({"env": {"ANTHROPIC_BASE_URL": GATEWAY, "ANTHROPIC_AUTH_TOKEN": key, "ANTHROPIC_MODEL": model,
                "ANTHROPIC_DEFAULT_HAIKU_MODEL": model, "ANTHROPIC_DEFAULT_SONNET_MODEL": model, "ANTHROPIC_DEFAULT_OPUS_MODEL": model}})
            }
            "gemini" => {
                json!({"env": {"GOOGLE_GEMINI_BASE_URL": GATEWAY, "GEMINI_API_KEY": key, "GEMINI_MODEL": model}})
            }
            _ => {
                // JSON string quoting is valid TOML basic-string quoting for model identifiers.
                let model_literal = serde_json::to_string(model).unwrap();
                json!({"auth": {"OPENAI_API_KEY": key}, "config": format!(
                    "model_provider = \"happy_token\"\nmodel = {model_literal}\n\n[model_providers.happy_token]\nname = \"HappyToken\"\nbase_url = \"{GATEWAY}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n")})
            }
        };
        let hash = format!("{:x}", Sha256::digest(format!("{uid}\0{group}").as_bytes()));
        let mut provider = Provider::with_id(
            format!("happy-token-{hash}-{app_type}"),
            format!("HappyToken · {group}"),
            settings,
            Some("https://www.happy-token.cn".into()),
        );
        provider.category = Some("aggregator".into());
        provider.created_at = Some(chrono::Utc::now().timestamp_millis());
        provider.notes = Some(format!(
            "HappyToken 账户 {uid} · 分组 {group}，登录后自动同步"
        ));
        result.push((app_type, provider));
    }
    result
}

fn refresh_key(existing: &mut Value, generated: &Value, app_type: &str) -> Result<(), String> {
    let (section, field) = match app_type {
        "codex" => ("auth", "OPENAI_API_KEY"),
        "claude" => ("env", "ANTHROPIC_AUTH_TOKEN"),
        _ => ("env", "GEMINI_API_KEY"),
    };
    let settings = existing
        .get_mut(section)
        .and_then(Value::as_object_mut)
        .ok_or("已有配置格式无效，请修复后重新同步")?;
    settings.insert(field.into(), generated[section][field].clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_groups_have_distinct_stable_account_scoped_provider_ids() {
        let models = vec!["gpt-5.6".to_string()];
        let default = build_providers(42, "default", "sk-test", &models);
        let again = build_providers(42, "default", "sk-new", &models);
        let pro = build_providers(42, "Pro", "sk-test", &models);
        let web = build_providers(42, "GPT Web", "sk-test", &models);
        let other_account = build_providers(43, "default", "sk-test", &models);
        assert_eq!(default[0].1.id, again[0].1.id);
        for other in [pro, web, other_account] {
            assert_ne!(default[0].1.id, other[0].1.id);
        }
        assert_eq!(default[0].0, "codex");
        let config: toml::Value =
            toml::from_str(default[0].1.settings_config["config"].as_str().unwrap()).unwrap();
        assert_eq!(config["model"].as_str(), Some("gpt-5.6"));
        assert_eq!(
            config["model_providers"]["happy_token"]["wire_api"].as_str(),
            Some("responses")
        );
    }

    #[test]
    fn models_are_filtered_by_app_and_non_coding_models_are_skipped() {
        let models = [
            "claude-sonnet-4-6",
            "gemini-2.5-pro",
            "gpt-5.6",
            "gpt-image-2",
            "gpt-audio",
            "gemini-image",
        ]
        .map(str::to_string);
        let providers = build_providers(42, "Pro", "sk-test", &models);
        assert_eq!(providers.len(), 3);
        assert_eq!(
            providers[0].1.settings_config["env"]["ANTHROPIC_MODEL"],
            "claude-sonnet-4-6"
        );
        assert_eq!(
            providers[2].1.settings_config["env"]["GEMINI_MODEL"],
            "gemini-2.5-pro"
        );
        assert!(build_providers(
            42,
            "Image",
            "sk-test",
            &["gpt-image-2".into(), "gemini-image".into()]
        )
        .is_empty());
    }

    #[test]
    fn browser_grants_and_snapshots_reject_untrusted_values() {
        let verifier = URL_SAFE_NO_PAD.encode([7u8; 32]);
        assert_eq!(verifier.len(), 43);
        assert_eq!(challenge(&verifier).len(), 43);
        assert_ne!(challenge(&verifier), verifier);
        let mut grant = BrowserGrant {
            id: "a".repeat(32),
            code: "1234ABCD".into(),
            expires_in: 600,
        };
        assert!(valid_grant(&grant));
        grant.id = "https://evil.test".into();
        assert!(!valid_grant(&grant));
        let mut snapshot = Snapshot {
            uid: 42,
            account: "Test".into(),
            groups: vec![GroupSnapshot {
                name: "default".into(),
                key: "sk-test".into(),
                models: vec!["gpt-5".into()],
            }],
            warnings: vec![],
        };
        assert!(validate_snapshot(&snapshot).is_ok());
        snapshot.groups[0].key = "sk-****".into();
        assert!(validate_snapshot(&snapshot).is_err());
        snapshot.groups[0].key = "sk-test".into();
        snapshot.groups.push(GroupSnapshot {
            name: "default".into(),
            key: "sk-test".into(),
            models: vec![],
        });
        assert!(validate_snapshot(&snapshot).is_err());
    }

    #[test]
    fn resync_preserves_user_model_and_other_settings() {
        let generated = build_providers(42, "default", "sk-new", &["gpt-5.6".into()]);
        let mut existing = json!({"auth": {"OPENAI_API_KEY": "sk-old", "other": true}, "config": "custom model config"});
        refresh_key(&mut existing, &generated[0].1.settings_config, "codex").unwrap();
        assert_eq!(existing["auth"]["OPENAI_API_KEY"], "sk-new");
        assert_eq!(existing["auth"]["other"], true);
        assert_eq!(existing["config"], "custom model config");
    }
}
