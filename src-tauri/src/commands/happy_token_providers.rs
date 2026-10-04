//! HappyToken-only import policy. Existing client writers and protocol bridges stay unchanged.
use super::{GroupSnapshot, GATEWAY};
use crate::provider::{ClaudeDesktopMode, ClaudeDesktopModelRoute, Provider, ProviderMeta};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Protocol {
    Responses,
    Chat,
    Anthropic,
    Gemini,
}

impl Protocol {
    fn format(self) -> &'static str {
        match self {
            Self::Responses => "openai_responses",
            Self::Chat => "openai_chat",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini_native",
        }
    }
    fn native_api(self) -> &'static str {
        match self {
            Self::Responses => "openai-responses",
            Self::Chat => "openai-completions",
            Self::Anthropic => "anthropic-messages",
            Self::Gemini => "google-generative-ai",
        }
    }
    fn root(self) -> String {
        match self {
            Self::Chat | Self::Responses => format!("{GATEWAY}/v1"),
            _ => GATEWAY.into(),
        }
    }
}

fn text_model(model: &str) -> bool {
    let name = model.to_ascii_lowercase();
    [
        "gpt-", "o1", "o3", "o4", "claude-", "gemini-", "grok-", "qwen", "deepseek", "kimi-",
        "glm-", "minimax-", "doubao-",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        && ![
            "image",
            "seedream",
            "seededit",
            "audio",
            "omni",
            "realtime",
            "transcrib",
            "tts",
            "search",
            "instruct",
            "embedding",
            "rerank",
        ]
        .iter()
        .any(|part| name.contains(part))
}

fn available(group: &GroupSnapshot, protocol: Protocol) -> Vec<&String> {
    let mut models: Vec<_> = group
        .models
        .iter()
        .filter(|model| {
            text_model(model)
                && group
                    .model_protocols
                    .get(*model)
                    .is_some_and(|p| p.contains(&protocol))
        })
        .collect();
    models.sort();
    models.dedup();
    models
}

// Compare version numbers before coding suffixes: a legacy Codex alias must not
// outrank a newer general model. Only candidates with declared protocols reach here.
fn model_rank(model: &str, family: &str) -> (bool, Vec<u32>, bool, u8, bool, String) {
    let name = model.to_ascii_lowercase();
    let version = name
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u32>().ok())
        .take_while(|number| *number < 100)
        .take(2)
        .collect();
    let primary = if name.contains("sonnet") || name.ends_with("-sol") {
        4
    } else if name.contains("opus") || name.ends_with("-astra") {
        3
    } else if name.contains("luna") || name.contains("haiku") {
        0
    } else {
        2
    };
    (
        name.starts_with(family),
        version,
        !name.contains("nano") && !name.contains("mini"),
        primary,
        name.contains("codex") || name.contains("coder") || name.contains("code"),
        name,
    )
}

fn preferred<'a>(models: &[&'a String], family: &str) -> Option<&'a String> {
    models
        .iter()
        .copied()
        .max_by_key(|model| model_rank(model, family))
}

fn identity(uid: u64, group: &str, app: &str, protocol: Option<Protocol>) -> String {
    let hash = format!("{:x}", Sha256::digest(format!("{uid}\0{group}").as_bytes()));
    let suffix = protocol
        .map(|p| format!("-{}", p.native_api()))
        .unwrap_or_default();
    format!("happy-token-{hash}-{app}{suffix}")
}

fn provider(
    uid: u64,
    group: &str,
    app: &str,
    protocol: Protocol,
    settings: Value,
    multi: bool,
) -> Provider {
    let mut p = Provider::with_id(
        identity(uid, group, app, multi.then_some(protocol)),
        if multi {
            format!("HappyToken · {group} · {}", protocol.native_api())
        } else {
            format!("HappyToken · {group}")
        },
        settings,
        Some("https://www.happy-token.cn".into()),
    );
    p.category = Some("aggregator".into());
    p.created_at = Some(chrono::Utc::now().timestamp_millis());
    p.meta = Some(ProviderMeta {
        api_format: Some(protocol.format().into()),
        ..Default::default()
    });
    p.notes = Some(format!(
        "HappyToken 账户 {uid} · 分组 {group} · {}。按服务声明导入；模型目录不代表工具调用已验收。",
        protocol.native_api()
    ));
    p
}

// Pi's original writer may drop apiFormat; the native api field is authoritative.
pub(super) fn configured_format<'a>(app: &str, provider: &'a Provider) -> Option<&'a str> {
    if app == "pi" {
        match provider.settings_config["api"].as_str() {
            Some("openai-responses") => return Some("openai_responses"),
            Some("openai-completions") => return Some("openai_chat"),
            Some("anthropic-messages") => return Some("anthropic"),
            Some("google-generative-ai") => return Some("gemini_native"),
            _ => {}
        }
    }
    provider
        .meta
        .as_ref()
        .and_then(|meta| meta.api_format.as_deref())
}

pub(super) fn excluded_group(name: &str) -> bool {
    matches!(name, "image" | "gpt-web")
}

pub(super) fn build_providers(uid: u64, group: &GroupSnapshot) -> Vec<(&'static str, Provider)> {
    // The deployed ChatGPT2API source substitutes text for unsupported function tools.
    // A protocol bridge cannot repair that; also guard snapshots from older Workers.
    if excluded_group(&group.name) {
        return vec![];
    }
    let mut result = Vec::new();
    for app in ["claude", "claude-desktop", "codex", "gemini", "grokbuild"] {
        let order: &[Protocol] = match app {
            "claude" | "claude-desktop" => &[
                Protocol::Anthropic,
                Protocol::Chat,
                Protocol::Responses,
                Protocol::Gemini,
            ],
            "gemini" => &[Protocol::Gemini],
            _ => &[Protocol::Responses, Protocol::Chat, Protocol::Anthropic],
        };
        for &protocol in order {
            let models: Vec<_> = available(group, protocol)
                .into_iter()
                .filter(|model| {
                    // Gemini CLI requires its native model family; Grok supports custom models.
                    match app {
                        "gemini" => model.to_ascii_lowercase().starts_with("gemini-"),
                        _ => true,
                    }
                })
                .collect();
            let family = match app {
                "claude" | "claude-desktop" => "claude-",
                "gemini" => "gemini-",
                "grokbuild" => "grok-",
                _ => "gpt-",
            };
            let Some(model) = preferred(&models, family) else {
                continue;
            };
            let key = &group.key;
            let root = protocol.root();
            let settings = match app {
                "claude" | "claude-desktop" => json!({"env": {
                    "ANTHROPIC_BASE_URL": root, "ANTHROPIC_AUTH_TOKEN": key, "ANTHROPIC_MODEL": model,
                    "ANTHROPIC_DEFAULT_HAIKU_MODEL": model, "ANTHROPIC_DEFAULT_SONNET_MODEL": model, "ANTHROPIC_DEFAULT_OPUS_MODEL": model }}),
                "gemini" => {
                    json!({"env": {"GOOGLE_GEMINI_BASE_URL": GATEWAY, "GEMINI_API_KEY": key, "GEMINI_MODEL": model}})
                }
                "grokbuild" => json!({"config": format!(
                    "[models]\ndefault = \"happy_token\"\n\n[model.happy_token]\nname = \"HappyToken\"\nmodel = {}\nbase_url = {}\napi_key = {}\napi_backend = \"responses\"\ncontext_window = {}\n",
                    json!(model), json!(root), json!(key), crate::grok_config::DEFAULT_CONTEXT_WINDOW)}),
                _ => json!({"auth": {"OPENAI_API_KEY": key}, "config": format!(
                    "model_provider = \"happy_token\"\nmodel = {}\n\n[model_providers.happy_token]\nname = \"HappyToken\"\nbase_url = {}\nwire_api = \"responses\"\nrequires_openai_auth = true\n", json!(model), json!(root))}),
            };
            let mut p = provider(uid, &group.name, app, protocol, settings, false);
            if app == "claude-desktop" {
                let meta = p.meta.as_mut().unwrap();
                meta.claude_desktop_mode = Some(ClaudeDesktopMode::Proxy);
                for route in ["claude-sonnet-5", "claude-opus-5", "claude-haiku-4-5"] {
                    meta.claude_desktop_model_routes.insert(
                        route.into(),
                        ClaudeDesktopModelRoute {
                            model: model.clone(),
                            label_override: Some(model.clone()),
                            supports_1m: None,
                        },
                    );
                }
                p.settings_config["env"]["ANTHROPIC_MODEL"] = json!("claude-sonnet-5");
            }
            if matches!(app, "claude" | "codex" | "grokbuild")
                && protocol
                    != if app == "claude" {
                        Protocol::Anthropic
                    } else {
                        Protocol::Responses
                    }
            {
                p.notes
                    .as_mut()
                    .unwrap()
                    .push_str(" 需要使用 CC Switch 的路由模式完成协议转换。");
            }
            result.push((app, p));
            break;
        }
    }
    // One entry per protocol prevents mixed catalogs from being sent to a wrong endpoint.
    for protocol in [
        Protocol::Responses,
        Protocol::Chat,
        Protocol::Anthropic,
        Protocol::Gemini,
    ] {
        let models = available(group, protocol);
        if models.is_empty() {
            continue;
        }
        let dict: serde_json::Map<_, _> = models
            .iter()
            .map(|m| ((*m).clone(), json!({"name": m})))
            .collect();
        let list: Vec<_> = models.iter().map(|m| json!({"id": m, "name": m})).collect();
        let root = protocol.root();
        for app in ["opencode", "openclaw", "hermes", "pi", "mcode"] {
            if (app == "hermes" || app == "mcode") && protocol == Protocol::Gemini {
                continue;
            }
            let id = identity(uid, &group.name, app, Some(protocol));
            let settings = match app {
                "opencode" => {
                    json!({"name": format!("HappyToken · {}", group.name), "npm": match protocol {
                    Protocol::Responses => "@ai-sdk/openai", Protocol::Chat => "@ai-sdk/openai-compatible",
                    Protocol::Anthropic => "@ai-sdk/anthropic", Protocol::Gemini => "@ai-sdk/google" },
                    "options": {"baseURL": match protocol { Protocol::Gemini => format!("{GATEWAY}/v1beta"), Protocol::Anthropic => format!("{GATEWAY}/v1"), _ => root.clone() }, "apiKey": group.key}, "models": dict})
                }
                "mcode" => {
                    json!({"name": format!("HappyToken · {}", group.name), "kind": "custom", "enabled": true,
                    "api": protocol.native_api(), "options": {"baseURL": root, "apiKey": group.key}, "models": dict})
                }
                "hermes" => {
                    json!({"name": id, "base_url": root, "api_key": group.key, "api_mode": match protocol {
                    Protocol::Responses => "codex_responses", Protocol::Anthropic => "anthropic_messages", _ => "chat_completions" }, "models": list})
                }
                _ => {
                    json!({"name": format!("HappyToken · {}", group.name), "baseUrl": root, "apiKey": group.key,
                    "api": protocol.native_api(), "models": list})
                }
            };
            result.push((
                app,
                provider(uid, &group.name, app, protocol, settings, true),
            ));
        }
    }
    result
}

// Only repair the exact old generated template. Edited configurations stay user-owned.
pub(super) fn migrate_legacy(uid: u64, group: &GroupSnapshot, app: &str, existing: &mut Provider) {
    if existing.meta.is_some() {
        return;
    }
    let settings = &existing.settings_config;
    let (model, key) = match app {
        "codex" => {
            let Some(config) = settings["config"]
                .as_str()
                .and_then(|s| s.parse::<toml::Value>().ok())
            else {
                return;
            };
            let (Some(model), Some(key)) = (
                config.get("model").and_then(toml::Value::as_str),
                settings["auth"]["OPENAI_API_KEY"].as_str(),
            ) else {
                return;
            };
            (model.to_owned(), key.to_owned())
        }
        "claude" => {
            let (Some(model), Some(key)) = (
                settings["env"]["ANTHROPIC_MODEL"].as_str(),
                settings["env"]["ANTHROPIC_AUTH_TOKEN"].as_str(),
            ) else {
                return;
            };
            (model.to_owned(), key.to_owned())
        }
        _ => return,
    };
    let legacy = if app == "codex" {
        json!({"auth": {"OPENAI_API_KEY": key}, "config": format!(
            "model_provider = \"happy_token\"\nmodel = {}\n\n[model_providers.happy_token]\nname = \"HappyToken\"\nbase_url = \"{GATEWAY}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n", json!(model))})
    } else {
        json!({"env": {"ANTHROPIC_BASE_URL": GATEWAY, "ANTHROPIC_AUTH_TOKEN": key, "ANTHROPIC_MODEL": model,
            "ANTHROPIC_DEFAULT_HAIKU_MODEL": model, "ANTHROPIC_DEFAULT_SONNET_MODEL": model, "ANTHROPIC_DEFAULT_OPUS_MODEL": model}})
    };
    if *settings != legacy {
        return;
    }
    let single = GroupSnapshot {
        name: group.name.clone(),
        key: group.key.clone(),
        models: vec![model],
        model_protocols: group.model_protocols.clone(),
    };
    if let Some((_, replacement)) = build_providers(uid, &single)
        .into_iter()
        .find(|(id, _)| *id == app)
    {
        existing.settings_config = replacement.settings_config;
        existing.meta = replacement.meta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(name: &str, entries: &[(&str, Protocol)]) -> GroupSnapshot {
        let mut group = GroupSnapshot {
            name: name.into(),
            key: "sk-test-only".into(),
            models: vec![],
            model_protocols: Default::default(),
        };
        for &(model, protocol) in entries {
            if !group.models.contains(&model.to_string()) {
                group.models.push(model.into());
            }
            group
                .model_protocols
                .entry(model.into())
                .or_default()
                .push(protocol);
        }
        group
    }
    fn find<'a>(providers: &'a [(&str, Provider)], app: &str) -> &'a Provider {
        &providers.iter().find(|(a, _)| *a == app).unwrap().1
    }
    #[test]
    fn chat_only_group_uses_existing_codex_and_claude_bridges() {
        let providers = build_providers(42, &group("default", &[("gpt-5", Protocol::Chat)]));
        let codex = find(&providers, "codex");
        let config: toml::Value = codex.settings_config["config"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            config["model_providers"]["happy_token"]["wire_api"].as_str(),
            Some("responses")
        );
        assert!(
            crate::proxy::providers::should_convert_codex_responses_to_chat(codex, "/v1/responses")
        );
        assert!(crate::tray::provider_needs_routing(
            &crate::app_config::AppType::Codex,
            codex
        ));
        assert!(crate::tray::provider_needs_routing(
            &crate::app_config::AppType::Claude,
            find(&providers, "claude")
        ));
        assert!(!providers.iter().any(|(app, _)| *app == "gemini"));
        crate::claude_desktop_config::validate_provider(find(&providers, "claude-desktop"))
            .unwrap();
        assert_eq!(
            find(&providers, "opencode").settings_config["npm"],
            "@ai-sdk/openai-compatible"
        );
        assert_eq!(
            find(&providers, "hermes").settings_config["api_mode"],
            "chat_completions"
        );
        assert_eq!(
            find(&providers, "pi").settings_config["api"],
            "openai-completions"
        );
        crate::mcode_config::validate_provider(
            &find(&providers, "mcode").id,
            &find(&providers, "mcode").settings_config,
        )
        .unwrap();
    }
    #[test]
    fn response_group_is_native_for_codex_and_converted_for_claude() {
        let providers = build_providers(42, &group("gpt-pro", &[("gpt-5.6", Protocol::Responses)]));
        let codex = find(&providers, "codex");
        assert!(!crate::tray::provider_needs_routing(
            &crate::app_config::AppType::Codex,
            codex
        ));
        assert_eq!(
            find(&providers, "claude")
                .meta
                .as_ref()
                .unwrap()
                .api_format
                .as_deref(),
            Some("openai_responses")
        );
        assert_eq!(
            find(&providers, "opencode").settings_config["npm"],
            "@ai-sdk/openai"
        );
    }
    #[test]
    fn newer_versions_outrank_legacy_coding_aliases() {
        let providers = build_providers(
            42,
            &group(
                "gpt-pro",
                &[
                    ("gpt-5.3-codex", Protocol::Responses),
                    ("gpt-5.6-sol", Protocol::Responses),
                    ("gpt-6-astra", Protocol::Responses),
                    ("gpt-6-luna", Protocol::Responses),
                    ("gpt-6-sol", Protocol::Responses),
                ],
            ),
        );
        let config = find(&providers, "codex").settings_config["config"]
            .as_str()
            .unwrap()
            .parse::<toml::Value>()
            .unwrap();
        assert_eq!(config["model"].as_str(), Some("gpt-6-sol"));
        let candidates = ["gpt-9".to_string(), "gpt-10".to_string()];
        assert_eq!(
            preferred(&candidates.iter().collect::<Vec<_>>(), "gpt-").map(String::as_str),
            Some("gpt-10")
        );
        let claude = build_providers(
            42,
            &group(
                "default",
                &[
                    ("claude-sonnet-4-6", Protocol::Chat),
                    ("claude-opus-5", Protocol::Chat),
                    ("claude-sonnet-5", Protocol::Chat),
                ],
            ),
        );
        assert_eq!(
            find(&claude, "claude").settings_config["env"]["ANTHROPIC_MODEL"],
            "claude-sonnet-5"
        );
    }
    #[test]
    fn pi_native_protocol_survives_metadata_normalization() {
        let providers = build_providers(42, &group("default", &[("gpt-5", Protocol::Chat)]));
        let mut pi = find(&providers, "pi").clone();
        pi.meta = None;
        assert_eq!(configured_format("pi", &pi), Some("openai_chat"));
        pi.settings_config["api"] = json!("openai-responses");
        assert_eq!(configured_format("pi", &pi), Some("openai_responses"));
    }
    #[test]
    fn grok_build_accepts_custom_models_with_declared_protocols() {
        let providers = build_providers(42, &group("default", &[("qwen-coder", Protocol::Chat)]));
        let grok = find(&providers, "grokbuild");
        assert!(grok.settings_config["config"]
            .as_str()
            .unwrap()
            .contains("qwen-coder"));
        assert_eq!(
            grok.meta.as_ref().unwrap().api_format.as_deref(),
            Some("openai_chat")
        );
        assert!(crate::tray::provider_needs_routing(
            &crate::app_config::AppType::GrokBuild,
            grok
        ));
    }
    #[test]
    fn mixed_protocol_catalogs_never_send_models_to_wrong_endpoint() {
        let g = group(
            "default",
            &[
                ("gpt-5", Protocol::Responses),
                ("claude-sonnet-4", Protocol::Anthropic),
                ("gemini-pro", Protocol::Gemini),
                ("qwen-coder", Protocol::Chat),
            ],
        );
        let providers = build_providers(42, &g);
        assert_eq!(
            find(&providers, "gemini").settings_config["env"]["GEMINI_MODEL"],
            "gemini-pro"
        );
        assert_eq!(
            find(&providers, "claude")
                .meta
                .as_ref()
                .unwrap()
                .api_format
                .as_deref(),
            Some("anthropic")
        );
        let opencode: Vec<_> = providers.iter().filter(|(a, _)| *a == "opencode").collect();
        assert_eq!(opencode.len(), 4);
        for (_, p) in opencode {
            assert_eq!(p.settings_config["models"].as_object().unwrap().len(), 1);
            if p.settings_config["npm"] == "@ai-sdk/anthropic" {
                assert_eq!(
                    p.settings_config["options"]["baseURL"],
                    format!("{GATEWAY}/v1")
                );
            }
        }
        assert_eq!(providers.iter().filter(|(a, _)| *a == "mcode").count(), 3);
    }
    #[test]
    fn web_only_unknown_and_non_text_models_do_not_produce_agent_configs() {
        assert!(build_providers(42, &group("image", &[("gpt-5", Protocol::Responses)])).is_empty());
        assert!(build_providers(
            42,
            &group(
                "gpt-web",
                &[("gpt-5", Protocol::Responses), ("gpt-5", Protocol::Chat)]
            )
        )
        .is_empty());
        let mut unknown = group("unknown", &[("gpt-5", Protocol::Chat)]);
        unknown.model_protocols.clear();
        assert!(build_providers(42, &unknown).is_empty());
        assert!(build_providers(
            42,
            &group(
                "image",
                &[
                    ("gpt-image-2", Protocol::Chat),
                    ("gemini-image", Protocol::Gemini),
                    ("doubao-seedream-5", Protocol::Chat),
                    ("gpt-audio", Protocol::Chat)
                ]
            )
        )
        .is_empty());
    }
    #[test]
    fn stable_ids_are_account_group_app_and_protocol_scoped() {
        let g = group(
            "default",
            &[("gpt-5", Protocol::Chat), ("gpt-5", Protocol::Responses)],
        );
        let first = build_providers(42, &g);
        assert_eq!(
            first.iter().map(|(_, p)| &p.id).collect::<Vec<_>>(),
            build_providers(42, &g)
                .iter()
                .map(|(_, p)| &p.id)
                .collect::<Vec<_>>()
        );
        assert_ne!(
            find(&first, "codex").id,
            find(&build_providers(43, &g), "codex").id
        );
        let ids: std::collections::HashSet<_> = first.iter().map(|(_, p)| &p.id).collect();
        assert_eq!(ids.len(), first.len());
    }
    #[test]
    fn grok_build_prefers_grok_models_and_writes_native_toml() {
        let providers = build_providers(42, &group("custom", &[("grok-4.5", Protocol::Responses)]));
        let grok = find(&providers, "grokbuild");
        crate::grok_config::validate_config_toml(grok.settings_config["config"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            crate::grok_config::extract_credentials(
                grok.settings_config["config"].as_str().unwrap()
            )
            .unwrap()
            .1,
            "sk-test-only"
        );
    }
    #[test]
    fn legacy_claude_template_is_repaired_but_custom_fields_are_preserved() {
        let g = group("default", &[("claude-sonnet-4", Protocol::Chat)]);
        let mut old = find(&build_providers(42, &g), "claude").clone();
        old.meta = None;
        old.settings_config["env"]["ANTHROPIC_BASE_URL"] = json!(GATEWAY);
        let mut customized = old.clone();
        customized.settings_config["env"]["MY_OPTION"] = json!(true);
        migrate_legacy(42, &g, "claude", &mut customized);
        assert!(customized.meta.is_none());
        assert_eq!(customized.settings_config["env"]["MY_OPTION"], true);
        migrate_legacy(42, &g, "claude", &mut old);
        assert_eq!(old.meta.unwrap().api_format.as_deref(), Some("openai_chat"));
        assert_eq!(
            old.settings_config["env"]["ANTHROPIC_BASE_URL"],
            format!("{GATEWAY}/v1")
        );
        let mut codex = find(&build_providers(42, &g), "codex").clone();
        codex.meta = None;
        codex.settings_config["config"] = json!("custom_setting = true\n");
        let original = codex.settings_config.clone();
        migrate_legacy(42, &g, "codex", &mut codex);
        assert_eq!(codex.settings_config, original);
    }
}
