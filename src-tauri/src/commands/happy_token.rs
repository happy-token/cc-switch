//! Browser authorization never imports browser cookies into the desktop application.
use crate::store::AppState;
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
#[path = "happy_token_providers.rs"]
mod providers;
use providers::Protocol;
const GATEWAY: &str = "https://gateway.happy-token.cn";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResult {
    account: String,
    overview: Option<AccountOverview>,
    groups: Vec<String>,
    providers: usize,
    warnings: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverview {
    balance: f64,
    consumed: f64,
    symbol: String,
    updated_at: u64,
}
#[derive(Serialize, Deserialize)]
pub struct AccountSummary {
    account: String,
    overview: Option<AccountOverview>,
}
const ACCOUNT_SETTING: &str = "happy_token_account";
static ACCOUNT_WRITE: Lazy<tokio::sync::Mutex<()>> = Lazy::new(|| tokio::sync::Mutex::new(()));

#[tauri::command]
pub async fn happy_token_logout(app: tauri::AppHandle) -> Result<(), String> {
    if STARTING.load(Ordering::SeqCst) {
        return Err("正在打开授权页面，请稍后退出".into());
    }
    happy_token_cancel_login()?;
    let _guard = ACCOUNT_WRITE.lock().await;
    app.state::<AppState>()
        .db
        .set_setting(ACCOUNT_SETTING, "null")
        .map_err(|_| "无法退出 HappyToken 账户")?;
    Ok(())
}

fn config_hash(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}

fn fingerprint_value(provider: &crate::provider::Provider) -> Value {
    let mut value = json!({"settings": provider.settings_config, "meta": provider.meta,
        "name": provider.name, "notes": provider.notes, "category": provider.category,
        "website": provider.website_url, "icon": provider.icon, "iconColor": provider.icon_color});
    // This flag is maintained by the original app when it discovers live configuration.
    if value["meta"]["liveConfigManaged"] == false {
        if let Some(meta) = value["meta"].as_object_mut() {
            meta.remove("liveConfigManaged");
        }
    }
    value
}
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut fields: Vec<_> = object.iter().collect();
            fields.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                fields
                    .into_iter()
                    .map(|(k, v)| (k.clone(), canonical(v)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
        value => value.clone(),
    }
}
fn provider_hash(provider: &crate::provider::Provider) -> String {
    format!(
        "v2:{}",
        config_hash(&canonical(&fingerprint_value(provider)))
    )
}
fn matches_managed(entry: &ManagedConfig, provider: &crate::provider::Provider) -> bool {
    if entry.hash == provider_hash(provider) {
        return true;
    }
    if entry.hash.starts_with("v2:") {
        return false;
    }
    let mut value = fingerprint_value(provider);
    if entry.hash == config_hash(&value) {
        return true;
    }
    // v1 serialized the three generated Desktop routes in HashMap iteration order.
    // Accept the six historical orders without changing any field or route content.
    let routes = value["meta"]["claudeDesktopModelRoutes"]
        .as_object()
        .cloned();
    if let Some(routes) = routes.filter(|r| r.len() == 3) {
        let fields: Vec<_> = routes.into_iter().collect();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            value["meta"]["claudeDesktopModelRoutes"] =
                Value::Object(order.into_iter().map(|i| fields[i].clone()).collect());
            if entry.hash == config_hash(&value) {
                return true;
            }
        }
    }
    false
}

fn is_stale(
    entry: &ManagedConfig,
    current: &std::collections::HashSet<String>,
    refreshed: &[String],
    authorized: Option<&[String]>,
) -> bool {
    !current.contains(&entry.id)
        && (providers::excluded_group(&entry.group)
            || refreshed.contains(&entry.group)
            || authorized.is_some_and(|groups| !groups.contains(&entry.group)))
}

#[derive(Serialize, Deserialize)]
struct ManagedConfig {
    group: String,
    app: String,
    id: String,
    hash: String,
}

#[tauri::command]
pub fn happy_token_account(
    state: tauri::State<'_, AppState>,
) -> Result<Option<AccountSummary>, String> {
    state
        .db
        .get_setting(ACCOUNT_SETTING)
        .map_err(|_| "无法读取 HappyToken 账户")?
        .filter(|value| value != "null")
        .map(|value| serde_json::from_str(&value).map_err(|_| "HappyToken 账户记录无效".into()))
        .transpose()
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
#[derive(Clone, Deserialize)]
struct GroupSnapshot {
    name: String,
    key: String,
    models: Vec<String>,
    #[serde(default, rename = "modelProtocols")]
    model_protocols: std::collections::HashMap<String, Vec<Protocol>>,
}
#[derive(Deserialize)]
struct Snapshot {
    uid: u64,
    account: String,
    #[serde(default)]
    overview: Option<AccountOverview>,
    groups: Vec<GroupSnapshot>,
    warnings: Vec<String>,
    #[serde(default, rename = "authorizedGroups")]
    authorized_groups: Option<Vec<String>>,
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
            Ok(Some(_)) => {}
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
                let _guard = ACCOUNT_WRITE.lock().await;
                if cancelled.load(Ordering::SeqCst) {
                    return Ok(None);
                }
                let result = sync_snapshot(app, snapshot).await?;
                let _ = app.emit_to("main", "happy-token-synced", &result);
                return Ok(Some(result));
            }
            410 => return Err("浏览器授权已过期、取消或同步失败，请重新登录".into()),
            _ => return Err("无法完成浏览器授权，请重新登录".into()),
        }
    }
}
fn validate_snapshot(snapshot: &Snapshot) -> Result<(), String> {
    if snapshot.uid == 0
        || snapshot.account.is_empty()
        || snapshot.account.len() > 1024
        || snapshot.account.chars().any(char::is_control)
        || (snapshot.groups.is_empty() && snapshot.warnings.is_empty())
        || snapshot.groups.len() > 32
        || snapshot.warnings.len() > 32
    {
        return Err("授权配置无效".into());
    }
    if let Some(overview) = &snapshot.overview {
        if !overview.balance.is_finite()
            || !overview.consumed.is_finite()
            || overview.consumed < 0.0
            || overview.symbol.is_empty()
            || overview.symbol.len() > 32
            || overview.symbol.chars().any(char::is_control)
            || overview.updated_at == 0
            || overview.updated_at > 8_640_000_000_000_000
        {
            return Err("账户概况无效".into());
        }
    }
    if snapshot.authorized_groups.as_ref().is_some_and(|names| {
        names.len() > 32
            || names
                .iter()
                .any(|n| n.is_empty() || n.len() > 256 || n.chars().any(char::is_control))
            || names.iter().collect::<std::collections::HashSet<_>>().len() != names.len()
            || snapshot.groups.iter().any(|g| !names.contains(&g.name))
    }) {
        return Err("授权分组清单无效".into());
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
            || group.model_protocols.len() > group.models.len()
            || group
                .model_protocols
                .iter()
                .any(|(model, protocols)| !group.models.contains(model) || protocols.len() > 4)
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
    let manifest_key = format!("happy_token_managed_{}", snapshot.uid);
    let state = app.state::<AppState>();
    let mut managed: Vec<ManagedConfig> = state
        .db
        .get_setting(&manifest_key)
        .map_err(|_| "无法读取自动配置记录")?
        .map(|v| serde_json::from_str(&v).map_err(|_| "自动配置记录无效"))
        .transpose()?
        .unwrap_or_default();
    let refreshed: Vec<String> = snapshot
        .groups
        .iter()
        .filter(|g| g.models.is_empty() || !g.model_protocols.is_empty())
        .map(|g| g.name.clone())
        .collect();
    let mut current_ids = std::collections::HashSet::new();
    let mut result = SyncResult {
        account: snapshot.account,
        overview: snapshot.overview,
        groups: Vec::new(),
        providers: 0,
        warnings: snapshot.warnings,
    };
    for group in snapshot.groups {
        if group.name == "image" {
            result
                .warnings
                .push("image：按账户配置范围排除，不导入编程助手".into());
            continue;
        }
        let providers = providers::build_providers(snapshot.uid, &group);
        if providers.is_empty() {
            result.warnings.push(format!(
                "{}：缺少适用于编码助手的模型/协议声明，或不支持函数工具调用，未导入",
                group.name
            ));
            continue;
        }
        if group.models.iter().any(|m| {
            m.to_ascii_lowercase().starts_with("gemini-")
                && !group
                    .model_protocols
                    .get(m)
                    .is_some_and(|p| p.contains(&Protocol::Gemini))
        }) {
            result.warnings.push(format!(
                "{}：未声明 Gemini 原生接口，未配置 Gemini CLI；其他应用仍可使用兼容模型",
                group.name
            ));
        }
        if providers
            .iter()
            .any(|(_, p)| p.notes.as_ref().is_some_and(|n| n.contains("需要使用")))
        {
            result.warnings.push(format!(
                "{}：部分配置需要选择路由模式进行协议转换；函数工具调用仍需真实任务验收",
                group.name
            ));
        }
        for (app_type, provider) in &providers {
            current_ids.insert(provider.id.clone());
            let previous = managed.iter().find(|m| m.id == provider.id);
            if let Some(existing) = state
                .db
                .get_provider_by_id(&provider.id, app_type)
                .map_err(|_| "无法读取自动配置")?
            {
                if previous.is_some_and(|m| matches_managed(m, &existing)) {
                    let mut updated = existing;
                    updated.settings_config = provider.settings_config.clone();
                    updated.meta = provider.meta.clone();
                    updated.notes = provider.notes.clone();
                    crate::services::ProviderService::update(
                        state.inner(),
                        crate::app_config::AppType::from_str(app_type)
                            .map_err(|_| "助手类型无效")?,
                        None,
                        updated,
                    )
                    .map_err(|_| "无法更新自动协议配置")?;
                }
            }
        }
        let handle = app.clone();
        let import_group = group.clone();
        let uid = snapshot.uid;
        let (count, review_apps) = tauri::async_runtime::spawn_blocking(move || {
            let state = handle.state::<AppState>();
            let mut count = 0;
            let mut review_apps = Vec::new();
            for (app_type, mut provider) in providers {
                // Save to the library only; switching uses CC Switch's normal config writer.
                // Re-sync refreshes credentials while retaining user customizations.
                if let Some(mut existing) = state
                    .db
                    .get_provider_by_id(&provider.id, app_type)
                    .map_err(|_| "无法读取已有 HappyToken 配置")?
                {
                    providers::migrate_legacy(uid, &import_group, app_type, &mut existing);
                    let existing_format = providers::configured_format(app_type, &existing);
                    let generated_format = providers::configured_format(app_type, &provider);
                    if existing_format != generated_format && !review_apps.contains(&app_type) {
                        review_apps.push(app_type);
                    }
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
            Ok::<_, String>((count, review_apps))
        })
        .await
        .map_err(|_| "配置保存任务失败")??;
        for (app_type, provider) in providers::build_providers(snapshot.uid, &group) {
            managed.retain(|m| m.id != provider.id);
            managed.push(ManagedConfig {
                group: group.name.clone(),
                app: app_type.into(),
                id: provider.id.clone(),
                hash: provider_hash(&provider),
            });
        }
        result.providers += count;
        if !review_apps.is_empty() {
            result.warnings.push(format!(
                "{}：{} 的已有协议设置保留，请在供应商编辑器核对",
                group.name,
                review_apps.join("、")
            ));
        }
        result.groups.push(group.name);
    }
    let mut retained = Vec::new();
    for entry in managed {
        let stale = is_stale(
            &entry,
            &current_ids,
            &refreshed,
            snapshot.authorized_groups.as_deref(),
        );
        if stale {
            let app_type =
                crate::app_config::AppType::from_str(&entry.app).map_err(|_| "助手类型无效")?;
            if let Some(existing) = state
                .db
                .get_provider_by_id(&entry.id, &entry.app)
                .map_err(|_| "无法读取旧自动配置")?
            {
                // Additive applications may already have written this provider to live files.
                let active = existing.in_failover_queue
                    || existing
                        .meta
                        .as_ref()
                        .is_some_and(|m| m.live_config_managed == Some(true))
                    || ((app_type.is_additive_mode()
                        || app_type == crate::app_config::AppType::Pi)
                        && entry.group != "image")
                    || state
                        .db
                        .get_current_provider(&entry.app)
                        .map_err(|_| "无法读取当前配置")?
                        .as_deref()
                        == Some(&entry.id);
                if !active && matches_managed(&entry, &existing) {
                    crate::services::ProviderService::delete(state.inner(), app_type, &entry.id)
                        .map_err(|_| "无法清理旧自动配置")?;
                    continue;
                }
                result.warnings.push(format!(
                    "{} / {}：分组或协议已不可用，正在使用或自定义的配置已保留，请手动处理",
                    entry.group, entry.app
                ));
            } else {
                continue;
            }
        }
        retained.push(entry);
    }
    state
        .db
        .set_setting(
            &manifest_key,
            &serde_json::to_string(&retained).map_err(|_| "自动配置记录格式无效")?,
        )
        .map_err(|_| "无法保存自动配置记录")?;
    let summary = AccountSummary {
        account: result.account.clone(),
        overview: result.overview.clone(),
    };
    app.state::<AppState>()
        .db
        .set_setting(
            ACCOUNT_SETTING,
            &serde_json::to_string(&summary).map_err(|_| "账户概况格式无效")?,
        )
        .map_err(|_| "无法保存 HappyToken 账户概况")?;
    Ok(result)
}

fn refresh_key(existing: &mut Value, generated: &Value, app_type: &str) -> Result<(), String> {
    let (section, field) = match app_type {
        "codex" => ("auth", "OPENAI_API_KEY"),
        "claude-desktop" => ("env", "ANTHROPIC_AUTH_TOKEN"),
        "opencode" | "mcode" => ("options", "apiKey"),
        "openclaw" | "pi" => {
            existing
                .as_object_mut()
                .ok_or("已有配置格式无效")?
                .insert("apiKey".into(), generated["apiKey"].clone());
            return Ok(());
        }
        "hermes" => {
            existing
                .as_object_mut()
                .ok_or("已有配置格式无效")?
                .insert("api_key".into(), generated["api_key"].clone());
            return Ok(());
        }
        "grokbuild" => {
            let config = existing
                .get("config")
                .and_then(Value::as_str)
                .ok_or("已有 Grok 配置格式无效")?;
            let mut doc = config
                .parse::<toml_edit::DocumentMut>()
                .map_err(|_| "已有 Grok 配置格式无效")?;
            let profile = crate::grok_config::extract_model_config(config)
                .ok_or("已有 Grok 配置格式无效")?
                .profile;
            let key = crate::grok_config::extract_model_config(
                generated["config"].as_str().ok_or("Grok 配置格式无效")?,
            )
            .and_then(|c| c.api_key)
            .ok_or("Grok 配置缺少 Key")?;
            doc["model"][&profile]["api_key"] = toml_edit::value(key);
            existing["config"] = Value::String(doc.to_string());
            return Ok(());
        }
        "claude" => ("env", "ANTHROPIC_AUTH_TOKEN"),
        _ => ("env", "GEMINI_API_KEY"),
    };
    let settings = existing
        .get_mut(section)
        .and_then(Value::as_object_mut)
        .ok_or("已有配置格式无效，请修复后重新同步")?;
    let target = if field == "ANTHROPIC_AUTH_TOKEN"
        && !settings.contains_key(field)
        && settings.contains_key("ANTHROPIC_API_KEY")
    {
        "ANTHROPIC_API_KEY"
    } else {
        field
    };
    settings.insert(target.into(), generated[section][field].clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_ignore_map_order_and_internal_false_flag_but_preserve_user_edits() {
        let a = json!({"a": {"x": 1, "y": 2}, "b": 3});
        let b = json!({"b": 3, "a": {"y": 2, "x": 1}});
        assert_eq!(config_hash(&canonical(&a)), config_hash(&canonical(&b)));
        let group = GroupSnapshot {
            name: "default".into(),
            key: "sk-fixture-only".into(),
            models: vec!["gpt-5".into()],
            model_protocols: std::collections::HashMap::from([(
                "gpt-5".into(),
                vec![Protocol::Chat],
            )]),
        };
        let mut p = providers::build_providers(42, &group)
            .into_iter()
            .find(|(app, _)| *app == "claude-desktop")
            .unwrap()
            .1;
        let entry = ManagedConfig {
            group: group.name,
            app: "claude-desktop".into(),
            id: p.id.clone(),
            hash: config_hash(&fingerprint_value(&p)),
        };
        assert!(matches_managed(&entry, &p));
        let hash = provider_hash(&p);
        p.meta.as_mut().unwrap().live_config_managed = Some(false);
        assert_eq!(hash, provider_hash(&p));
        assert!(matches_managed(&entry, &p));
        p.name = "User customized".into();
        assert!(!matches_managed(&entry, &p));
    }

    #[test]
    fn reconciliation_requires_authoritative_absence_or_a_successful_refresh() {
        let entry = ManagedConfig {
            group: "pro".into(),
            app: "codex".into(),
            id: "managed".into(),
            hash: "fixture".into(),
        };
        let none = std::collections::HashSet::new();
        assert!(!is_stale(&entry, &none, &[], None));
        assert!(!is_stale(&entry, &none, &[], Some(&["pro".into()])));
        assert!(is_stale(&entry, &none, &[], Some(&[])));
        assert!(is_stale(&entry, &none, &["pro".into()], None));
        let present = std::collections::HashSet::from(["managed".into()]);
        assert!(!is_stale(&entry, &present, &["pro".into()], Some(&[])));
        assert_ne!(
            config_hash(&json!({"model": "user-edited"})),
            config_hash(&json!({"model": "generated"}))
        );
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
            authorized_groups: None,
            uid: 42,
            account: "Test".into(),
            overview: None,
            groups: vec![GroupSnapshot {
                name: "default".into(),
                key: "sk-test".into(),
                models: vec!["gpt-5".into()],
                model_protocols: Default::default(),
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
            model_protocols: Default::default(),
        });
        assert!(validate_snapshot(&snapshot).is_err());
    }

    #[test]
    fn resync_preserves_user_model_and_other_settings() {
        let generated = json!({"auth": {"OPENAI_API_KEY": "sk-new"}});
        let mut existing = json!({"auth": {"OPENAI_API_KEY": "sk-old", "other": true}, "config": "custom model config"});
        refresh_key(&mut existing, &generated, "codex").unwrap();
        assert_eq!(existing["auth"]["OPENAI_API_KEY"], "sk-new");
        assert_eq!(existing["auth"]["other"], true);
        assert_eq!(existing["config"], "custom model config");
    }

    #[test]
    fn protocol_snapshot_wire_format_is_validated_and_web_only_account_can_login() {
        let mut value = json!({"uid": 42, "account": "Fixture", "groups": [{"name": "default", "key": "sk-test-only", "models": ["gpt-5"], "modelProtocols": {"gpt-5": ["chat"]}}], "warnings": []});
        let snapshot: Snapshot = serde_json::from_value(value.clone()).unwrap();
        validate_snapshot(&snapshot).unwrap();
        assert_eq!(
            snapshot.groups[0].model_protocols["gpt-5"],
            vec![Protocol::Chat]
        );
        value["groups"][0]["modelProtocols"]["gpt-5"] = json!(["unknown"]);
        assert!(serde_json::from_value::<Snapshot>(value.clone()).is_err());
        value["groups"] = json!([]);
        value["warnings"] = json!(["gpt-web：不支持函数工具调用"]);
        validate_snapshot(&serde_json::from_value::<Snapshot>(value).unwrap()).unwrap();
    }

    #[test]
    fn resync_refreshes_native_client_credentials_without_changing_models() {
        for (app, mut existing, generated, pointer) in [
            (
                "opencode",
                json!({"options": {"apiKey": "old", "custom": true}, "models": {"custom-model": {}}}),
                json!({"options": {"apiKey": "new"}}),
                "/options/apiKey",
            ),
            (
                "mcode",
                json!({"options": {"apiKey": "old"}, "models": {"custom-model": {}}}),
                json!({"options": {"apiKey": "new"}}),
                "/options/apiKey",
            ),
            (
                "openclaw",
                json!({"apiKey": "old", "models": [{"id": "custom-model"}]}),
                json!({"apiKey": "new"}),
                "/apiKey",
            ),
            (
                "pi",
                json!({"apiKey": "old", "models": [{"id": "custom-model"}]}),
                json!({"apiKey": "new"}),
                "/apiKey",
            ),
            (
                "hermes",
                json!({"api_key": "old", "models": [{"id": "custom-model"}]}),
                json!({"api_key": "new"}),
                "/api_key",
            ),
        ] {
            let models = existing["models"].clone();
            refresh_key(&mut existing, &generated, app).unwrap();
            assert_eq!(existing.pointer(pointer).unwrap(), "new");
            assert_eq!(existing["models"], models);
        }
    }
}
