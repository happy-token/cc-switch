//! HappyToken login reuses the Gateway/Casdoor flow without granting remote pages IPC access.
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use reqwest::{header, Client, Method};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{provider::Provider, store::AppState};

const GATEWAY: &str = "https://gateway.happy-token.cn";
const LOGIN_WINDOW: &str = "happy-token-login";
const CALLBACK_PATH: &str = "/__happy_switch_authenticated";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncResult {
    account: String,
    groups: Vec<String>,
    providers: usize,
    warnings: Vec<String>,
}

fn callback_uid(url: &url::Url, nonce: &str) -> Option<u64> {
    if url.origin().ascii_serialization() != GATEWAY || url.path() != CALLBACK_PATH {
        return None;
    }
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
    if pairs.get("state")?.as_ref() != nonce {
        return None;
    }
    pairs.get("uid")?.parse::<u64>().ok().filter(|id| *id > 0)
}

#[tauri::command]
pub async fn happy_token_login(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        window
            .set_focus()
            .map_err(|_| "无法打开登录窗口".to_string())?;
        return Ok(());
    }
    let nonce = uuid::Uuid::new_v4().to_string();
    // Only a verified numeric user ID crosses the navigation callback. Credentials never enter URLs.
    let script = include_str!("happy_token_login.js").replace("__HAPPY_NONCE__", &nonce);
    let busy = Arc::new(AtomicBool::new(false));
    let navigation_busy = busy.clone();
    let navigation_app = app.clone();
    let window = WebviewWindowBuilder::new(
        &app,
        LOGIN_WINDOW,
        WebviewUrl::External(
            format!("{GATEWAY}/sso?next=%2Fdashboard&lang=zh")
                .parse()
                .unwrap(),
        ),
    )
    .title("HappyToken · 登录并自动配置")
    .inner_size(980.0, 760.0)
    .initialization_script(script)
    .on_navigation(move |url| {
        if url.path() == CALLBACK_PATH {
            if let Some(uid) = callback_uid(url, &nonce) {
                if !navigation_busy.swap(true, Ordering::SeqCst) {
                    let handle = navigation_app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = handle.emit_to("main", "happy-token-syncing", ());
                        let result = sync_from_window(&handle, uid).await;
                        match result {
                            Ok(result) => {
                                let _ = handle.emit_to("main", "happy-token-synced", result);
                            }
                            Err(error) => {
                                let _ = handle.emit_to("main", "happy-token-error", error);
                            }
                        }
                        if let Some(window) = handle.get_webview_window(LOGIN_WINDOW) {
                            let _ = window.close();
                        }
                    });
                }
            }
            return false;
        }
        // Casdoor may redirect through external identity providers; none receive native permissions.
        url.scheme() == "https"
    })
    .build()
    .map_err(|_| "无法创建 HappyToken 登录窗口".to_string())?;
    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) && !busy.load(Ordering::SeqCst) {
            let _ = close_app.emit_to("main", "happy-token-cancelled", ());
        }
    });
    Ok(())
}

async fn sync_from_window(app: &tauri::AppHandle, uid: u64) -> Result<SyncResult, String> {
    let window = app
        .get_webview_window(LOGIN_WINDOW)
        .ok_or("登录窗口已关闭")?;
    if window
        .url()
        .map_err(|_| "无法验证登录来源")?
        .origin()
        .ascii_serialization()
        != GATEWAY
    {
        return Err("登录来源无效，请重新登录".into());
    }
    // Cookie APIs must run away from the UI thread to avoid WebView2 deadlocks on Windows.
    let cookies = tauri::async_runtime::spawn_blocking(move || {
        window.cookies_for_url(format!("{GATEWAY}/api/user/self").parse().unwrap())
    })
    .await
    .map_err(|_| "无法读取登录会话")?
    .map_err(|_| "无法读取登录会话")?;
    let cookie = cookies
        .iter()
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
        .collect::<Vec<_>>()
        .join("; ");
    if cookie.is_empty() {
        return Err("未取得 Gateway 登录会话，请重新登录".into());
    }
    let mut headers = header::HeaderMap::new();
    let mut cookie_header =
        header::HeaderValue::from_str(&cookie).map_err(|_| "登录会话格式错误")?;
    cookie_header.set_sensitive(true);
    headers.insert(header::COOKIE, cookie_header);
    headers.insert(
        "New-Api-User",
        header::HeaderValue::from_str(&uid.to_string()).unwrap(),
    );
    headers.insert(header::ORIGIN, header::HeaderValue::from_static(GATEWAY));
    let client = Client::builder()
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "无法创建 Gateway 客户端")?;
    let user = api(&client, Method::GET, "/api/user/self", None).await?;
    if user["id"].as_u64() != Some(uid) {
        return Err("登录账户校验失败".into());
    }
    let groups = api(&client, Method::GET, "/api/user/self/groups", None).await?;
    let groups = groups.as_object().ok_or("网关返回的分组格式无效")?;
    if groups.is_empty() {
        return Err("此账户没有可用分组".into());
    }
    let mut result = SyncResult {
        account: user["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| user["username"].as_str())
            .unwrap_or("HappyToken")
            .to_string(),
        groups: Vec::new(),
        providers: 0,
        warnings: Vec::new(),
    };
    let mut tokens = list_tokens(&client).await?;
    // Sort for stable import order; every advertised group is considered, including future groups.
    let mut group_names: Vec<_> = groups.keys().cloned().collect();
    group_names.sort();
    for group in group_names {
        match prepare_group(&client, uid, &group, &mut tokens).await {
            Ok(providers) => {
                if providers.is_empty() {
                    result.warnings.push(format!(
                        "{group}：没有适用于 Claude Code、Codex 或 Gemini 的模型"
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
                result.groups.push(group);
            }
            Err(_) => result.warnings.push(format!(
                "{group}：同步失败，请检查分组权限、余额或网关状态后重试"
            )),
        }
    }
    if result.providers == 0 {
        return Err(format!("未导入任何配置。{}", result.warnings.join("；")));
    }
    Ok(result)
}

// Errors are deliberately bounded and exclude response bodies, cookies, and API keys.
async fn api(
    client: &Client,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut request = client.request(method, format!("{GATEWAY}{path}"));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|_| "Gateway 请求失败，请检查网络")?;
    if !response.status().is_success() {
        return Err(format!(
            "Gateway 请求失败（HTTP {}）",
            response.status().as_u16()
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|_| "Gateway 返回了无效数据")?;
    if value["success"] != true {
        return Err("Gateway 拒绝了请求，请检查账户权限后重试".into());
    }
    Ok(value["data"].clone())
}

async fn list_tokens(client: &Client) -> Result<Vec<Value>, String> {
    let mut tokens = Vec::new();
    for page in 1..=100 {
        let data = api(
            client,
            Method::GET,
            &format!("/api/token/?p={page}&page_size=100"),
            None,
        )
        .await?;
        let items = data["items"].as_array().ok_or("Gateway 令牌列表格式无效")?;
        tokens.extend(items.iter().cloned());
        if items.is_empty()
            || tokens.len() as u64 >= data["total"].as_u64().ok_or("Gateway 令牌分页格式无效")?
        {
            return Ok(tokens);
        }
    }
    Err("账户令牌数量超出同步上限".into())
}

fn token_name(group: &str) -> String {
    // NewAPI caps names at 50 characters; hashing also avoids collisions after truncation.
    format!("HappySwitch-{:x}", Sha256::digest(group.as_bytes()))[..44].to_string()
}

fn reusable_token<'a>(tokens: &'a [Value], group: &str, now: i64) -> Option<&'a Value> {
    let name = token_name(group);
    tokens.iter().find(|token| {
        token["name"].as_str() == Some(&name)
            && token["group"].as_str() == Some(group)
            && token["status"] == 1
            && (token["expired_time"] == -1
                || token["expired_time"]
                    .as_i64()
                    .is_some_and(|expiry| expiry > now))
            && (token["unlimited_quota"] == true
                || token["remain_quota"]
                    .as_i64()
                    .is_some_and(|quota| quota > 0))
            && token["model_limits_enabled"] == false
            && token["allow_ips"].as_str().is_none_or(|ips| ips.is_empty())
    })
}

async fn prepare_group(
    client: &Client,
    uid: u64,
    group: &str,
    tokens: &mut Vec<Value>,
) -> Result<Vec<(&'static str, Provider)>, String> {
    if reusable_token(tokens, group, chrono::Utc::now().timestamp()).is_none() {
        api(
            client,
            Method::POST,
            "/api/token/",
            Some(json!({
                "name": token_name(group), "group": group, "expired_time": -1,
                "unlimited_quota": true, "remain_quota": 0,
                "model_limits_enabled": false, "model_limits": "", "allow_ips": "",
            })),
        )
        .await?;
        *tokens = list_tokens(client).await?;
    }
    let token =
        reusable_token(tokens, group, chrono::Utc::now().timestamp()).ok_or("未找到分组令牌")?;
    let id = token["id"].as_u64().ok_or("令牌 ID 无效")?;
    // The list endpoint masks keys; always call the authenticated key endpoint.
    let key = api(client, Method::POST, &format!("/api/token/{id}/key"), None).await?;
    let key = key["key"]
        .as_str()
        .filter(|key| !key.is_empty() && !key.contains('*'))
        .ok_or("未取得完整 API Key")?;
    let key = if key.starts_with("sk-") {
        key.to_string()
    } else {
        format!("sk-{key}")
    };
    // Do not send Gateway session cookies to model discovery: only this group's API key.
    let model_client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|_| "无法创建模型客户端")?;
    let response = model_client
        .get(format!("{GATEWAY}/v1/models"))
        .bearer_auth(&key)
        .send()
        .await
        .map_err(|_| "无法读取分组模型")?;
    if !response.status().is_success() {
        return Err("无法读取分组模型".into());
    }
    let models: Value = response.json().await.map_err(|_| "分组模型格式无效")?;
    let mut models: Vec<String> = models["data"]
        .as_array()
        .ok_or("分组模型格式无效")?
        .iter()
        .filter_map(|model| model["id"].as_str().map(str::to_string))
        .collect();
    models.sort();
    Ok(build_providers(uid, group, &key, &models))
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
    fn callback_requires_origin_nonce_and_positive_uid() {
        let parse = |s: &str| url::Url::parse(s).unwrap();
        assert_eq!(
            callback_uid(
                &parse(&format!("{GATEWAY}{CALLBACK_PATH}?state=nonce&uid=42")),
                "nonce"
            ),
            Some(42)
        );
        for bad in [
            format!("https://evil.example{CALLBACK_PATH}?state=nonce&uid=42"),
            format!("http://gateway.happy-token.cn{CALLBACK_PATH}?state=nonce&uid=42"),
            format!("{GATEWAY}{CALLBACK_PATH}?state=wrong&uid=42"),
            format!("{GATEWAY}{CALLBACK_PATH}?state=nonce&uid=0"),
            format!("{GATEWAY}{CALLBACK_PATH}?state=nonce&uid=abc"),
            format!("{GATEWAY}/dashboard?state=nonce&uid=42"),
        ] {
            assert_eq!(callback_uid(&parse(&bad), "nonce"), None);
        }
    }

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
    fn only_valid_unrestricted_owned_group_tokens_are_reused() {
        let valid = json!({"id": 1, "name": token_name("Pro"), "group": "Pro", "status": 1,
            "expired_time": -1, "unlimited_quota": true, "model_limits_enabled": false, "allow_ips": ""});
        assert!(reusable_token(&[valid.clone()], "Pro", 100).is_some());
        assert!(reusable_token(&[valid.clone()], "Default", 100).is_none());
        for (field, value) in [
            ("name", json!("User's existing token")),
            ("status", json!(2)),
            ("expired_time", json!(50)),
            ("model_limits_enabled", json!(true)),
            ("allow_ips", json!("127.0.0.1")),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert!(reusable_token(&[invalid], "Pro", 100).is_none());
        }
        let mut exhausted = valid;
        exhausted["unlimited_quota"] = json!(false);
        exhausted["remain_quota"] = json!(0);
        assert!(reusable_token(&[exhausted], "Pro", 100).is_none());
        assert!(token_name(&"很长的分组".repeat(100)).len() <= 50);
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
