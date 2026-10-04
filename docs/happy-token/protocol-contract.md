# 分组、模型与客户端协议契约

核对日期：2026-10-04。本文记录当前自动导入范围；客户端自身支持范围与本项目自动导入范围分别说明。模型目录快照见 [逐模型清单](model-protocol-inventory.json)，登录与配置生命周期见 [实现说明](implementation.md)。

## 协议命名与地址

| 标准名称 | 快照值 | CC Switch apiFormat | 原生客户端 api | 网关请求路径 |
| --- | --- | --- | --- | --- |
| OpenAI Responses | responses | openai_responses | openai-responses | POST /v1/responses |
| OpenAI Chat Completions | chat | openai_chat | openai-completions | POST /v1/chat/completions |
| Anthropic Messages | anthropic | anthropic | anthropic-messages | POST /v1/messages |
| Gemini GenerateContent | gemini | gemini_native | google-generative-ai | POST /v1beta/models/{model}:generateContent；流式用 streamGenerateContent |

路径与 base URL 不等价：OpenAI SDK 通常配置网关 /v1；Anthropic AI SDK 配置 /v1，Claude 的 ANTHROPIC_BASE_URL 配置网关根地址；Google AI SDK 配置 /v1beta，Gemini CLI 配置网关根地址。凭据仍由已有写入器处理，本文不记录真实 Key。

## 客户端契约与依据

| 客户端 | 本项目生成的客户端请求协议 | 可接入上游与条件 | 依据 |
| --- | --- | --- | --- |
| Claude Code | Messages | 首选 Messages；Chat / Responses / Gemini 经原有路由转换，需启用路由 | [官方网关文档](https://code.claude.com/docs/en/llm-gateway)、本地 proxy 转换器 |
| Claude Desktop | 本项目本地代理与 Claude 模型别名 | Messages / Chat / Responses / Gemini；走原有 Desktop Proxy 与模型映射 | 本地 claude_desktop 配置及代理实现；不能把本项目适配宣称为官方任意模型支持 |
| Codex | Responses，wire_api=responses | Responses 可直连；Chat / Messages 需原有路由转换 | [官方配置](https://developers.openai.com/codex/config-reference/)、本地 Codex 路由器 |
| Gemini CLI | Gemini 原生 | 只导入 Gemini 原生协议且模型 ID 为 gemini-；不把 Chat-only 当 Gemini 原生 | [官方认证说明](https://geminicli.com/docs/get-started/authentication/)、本地 Gemini 写入器 |
| Grok Build | 当前模板使用 Responses | Responses 直连，Chat / Messages 使用现有转换；支持自定义模型，不应只允许 grok- | [官方自定义模型源码文档](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md) |
| OpenCode | 按 AI SDK provider 选择 | 四协议分别生成配置，npm 为 openai / openai-compatible / anthropic / google | [官方 Providers 文档](https://opencode.ai/docs/providers/) |
| OpenClaw | 配置 api 指定原生协议 | 四协议分别生成配置 | [官方模型提供商文档](https://docs.openclaw.ai/concepts/model-providers)、本地 openclaw 写入器 |
| Pi | 配置 api 指定原生协议 | 四协议分别生成配置；不能仅凭 meta.apiFormat 判断，因为原写入器会归一化元数据 | [官方 models 文档](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/models.md)、本地 pi_config |
| Hermes | api_mode | chat_completions / codex_responses / anthropic_messages；不生成 Gemini 原生项 | [官方配置文档](https://hermes-agent.nousresearch.com/docs/user-guide/configuration/)、本地 hermes_config |
| MiniMax Code（MCode） | 配置 api 指定原生协议 | Chat / Responses / Messages；不生成 Gemini 原生项 | [官方源码及 README](https://github.com/MiniMax-AI/minimax-code)、本地 mcode_config 验证器 |

官方文档可能随客户端版本改变。上述“当前模板”是代码事实，不能把本项目保守导入规则当作客户端完整能力上限。Claude 非 Claude 模型属于第三方适配，官方网关文档不提供这种组合的支持承诺。

## 当前分组与逐模型证据

| 分组 | 清单模型数 | 证据 | 自动导入范围 |
| --- | --- | --- | --- |
| default | 41 | 真实分组 Key 的 /v1/models 与公开 pricing | 模型必须同时有可识别文本用途及协议声明；未知声明跳过 |
| gpt-pro | 24 | 真实分组 Key 的 /v1/models 与公开 pricing | 编程 GPT/o1/o3/o4 按明确服务策略使用 Responses，保留价目表原始声明供对照 |
| gpt-web | 11 | 仅公开 pricing，未用该组 Key 核对完整目录 | 当前编码导入排除；普通函数工具受后端限制 |
| image | 31 | 仅公开 pricing，未用该组 Key 核对完整目录 | 用户明确要求全部排除，不因出现文本模型而导入 |

[JSON 清单](model-protocol-inventory.json)逐项保存 model、pricingProtocols、importProtocols、protocolSource、toolCallingVerified；catalogVerified 表示是否确实读取分组目录。Default 当前有 7 个、Pro 有 2 个模型的导入协议仍未知；这些模型保守跳过。空协议数组表示未知，不能默认 Responses。快照会过期，运行时以每次授权的新目录为准，不能把本文件硬编码为账户权限。

NewAPI 的价目表可能汇总同名模型跨渠道接口，不能证明某个具体分组渠道实际接受全部列出的协议。gpt-pro 的 Responses 覆盖来自 HappyAPIWeb/docs/gpt-onboarding.md 服务策略，未通过付费请求逐模型确认。GPT Web 部署记录指向 ChatGPT2API revision e55aef2829e7bf1d7256d6ff3feb4b40b02743d2：[Chat 处理](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_chat_complete.py)、[Responses 处理](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_response.py)。源码存在 Responses 处理，不等于网关渠道开放该接口；不支持普通函数工具的限制也不会因协议转换消失。

## 可执行导入流程

1. 浏览器明确授权，服务端核实账户并读取完整授权分组清单。获取失败与权限删除分别记录。
2. 排除 image、gpt-web；收集每组专用 Key 与该 Key 的模型目录。
3. 目录与该组 pricing 取交集，映射四种已知协议；gpt-pro 使用上述明确策略。未知声明不猜测。
4. 原生校验快照边界，筛选文本模型；名称筛选仅作为保守用途过滤，不作为协议或工具能力证据。
5. `happy_token_providers::build_providers` 按客户端契约生成配置：单配置按协议优先顺序选模型；多协议客户端分别建立目录，模型不跨协议混放。
6. `happy_token::sync_snapshot` 比较管理指纹：未改动的自动配置可更新；用户自定义配置保留并仅刷新凭据；清理条件必须结合权威分组清单、目录成功状态及当前使用状态。
7. 使用原供应商服务保存；不自动启用、不写当前 CLI。用户手动启用时复用原写入器与协议路由。
8. 返回账户概况、数量与跳过/保留原因，不在事件或日志输出 Key。

执行入口：`src-tauri/src/commands/happy_token.rs` 与 `happy_token_providers.rs`；服务端协议映射在 HappyAPIWeb/gateway-sso/src/desktop-session.ts。协议未知或目录读取失败不能触发“接口已移除”的清理。

## 验收分级与审查发现

- 文档确认：客户端配置字段和协议选择有以上官方/源码依据；Desktop 是本项目适配证据。
- 声明确认：上述逐模型清单已保存；Default/Pro 目录已用真实账户 Key 读取，未保存 Key。
- 导入确认：dev 已验证 16 项、image 自动配置清理、账户更新及退出；这是 Grok 规则修正前的真实导入结果。
- 调用验收：尚未逐模型验证流式、结构化函数工具调用、工具结果回传及第二轮响应。`toolCallingVerified=false` 不表示一定不支持。
- 本轮发现并修正：旧 Grok Build 导入条件误认为只接受 Grok 模型；官方自定义模型文档明确支持其他模型，已去掉品牌限制并增加转换条件测试。
- Pi 协议警告已改按实际原生 api 比较，避免元数据归一化误报；管理指纹归一化差异仍待处理，不应覆盖用户配置来消除警告。

真实任务验收应逐个“分组×模型×协议”记录：请求端点、客户端版本、流式结束事件、函数名称/参数、工具结果回传后的回答、成功/失败及时间；只存脱敏结果。一次 HTTP 200 或普通文字回答不能升级为编码可用。
