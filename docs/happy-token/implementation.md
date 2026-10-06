# HappyToken 登录与自动配置

## 用户目标与边界

- 将 `farion1231/cc-switch` fork 至 GitHub `happy-token` 账户，在此 fork 上开发。
- 桌面端提供 HappyToken 登录，登录后自动配置账户的全部可用分组。
- 用户提及 Default、Pro、GPT Web；不硬编码分组名，不把未授权分组当作可用分组。
- 复用 HappyImage / HappyServices 的账户体系，不创建另一套账户密码。
- 维护根 `CONTEXT.md` 和本说明，减少后续重复沟通。
- 用户要求直接提交到 happy-token/main，不使用 PR，也不向官方上游提交 PR。此前 draft PR #1 仅创建在 happy-token fork 内，已关闭；登录代码与文档已直接推送到 main。
- 用户授权 macOS 本机签名与公证复用上级 config 的 HappyRouter 凭证；不将真实凭证复制到项目或提交到 Git。
- 用户已授权同时修改 HappySwitch 与 HappyAPIWeb/gateway-sso 源码，实现浏览器授权；生产部署与 Casdoor 应用设置仍未授权。
- 用户随后授权使用 HappyServices 中的真实账户进行验证；此授权覆盖账户测试，未授权生产 Worker 发布。

## 已查明的依据（2026-10-04）

- fork：`https://github.com/happy-token/cc-switch`。
- 本地：`/Users/forever/workspace/HappySwitch`；上游 remote 为 `upstream`。
- 参考仓库实际名称为 `../HappyServices`，另参阅 `../HappyImage`、`../HappyAPIWeb/gateway-sso`。
- HappyImage 通过后端 OIDC 登录，并通过数据库绑定网关用户和令牌；桌面端不能照搬数据库访问或生产密钥。
- Gateway `https://gateway.happy-token.cn/api/status` 只读检查显示版本 `v1.0.0-rc.21`、OIDC 已启用。
- 统一登录入口：`https://gateway.happy-token.cn/sso?next=%2Fdashboard&lang=zh`。
- 现有登录页建立 Gateway Cookie，并在 Gateway origin 的 localStorage 写入 `uid` 和 `user`。
- 公开 `/api/pricing` 确认分组 ID 为 `default`、`gpt-pro`、`gpt-web`、`image`；公开价目表不能替代账户授权分组。
- 分组 API：`GET /api/user/self/groups`，返回分组名称映射；每项有 `desc`、`ratio`。
- 令牌 API：`GET /api/token/?p=N&page_size=100`；`POST /api/token/`；`POST /api/token/{id}/key`。
- 列表中的 Key 已脱敏，必须通过认证的 key 接口取完整值。
- 模型 API：用对应分组 Key 调用 `GET /v1/models`。
- 协议依据：[NewAPI rc.21 路由](https://github.com/QuantumNous/new-api/blob/v1.0.0-rc.21/router/api-router.go)、[令牌接口](https://github.com/QuantumNous/new-api/blob/v1.0.0-rc.21/controller/token.go)。

## 当前实现约束

- 默认浏览器完成 Gateway/Casdoor 登录与明确的 HappySwitch 授权确认；不再创建内置登录 WebView。
- 本机生成 43 字符随机 verifier，只向网关提交 SHA-256/base64url challenge；浏览器 URL 仅包含随机请求 ID，不携带 Cookie、verifier 或 API Key。
- 授权页显示已验证的网关账户、验证码与将执行的分组令牌同步范围；用户应核对桌面端相同验证码后确认。
- 创建、轮询、取消接口拒绝浏览器 Origin/Sec-Fetch-Site 请求；确认接口要求同源 Origin，验证 Cookie 对应 `/api/user/self` 的 ID，拒绝跨站请求。
- 授权记录由独立 SQLite Durable Object 保存，10 分钟到期并通过 alarm 清理；创建按 IP 哈希限流（每分钟 10 次），每个请求的轮询间隔至少 1 秒。
- 兑换要求原 verifier 且只允许一次；消费、取消、审批占用使用事务，重复或并发领取仅一个成功。
- 浏览器 Cookie 仅用于网关侧固定同源 API 调用，不写入授权存储、不返回桌面端；不跟随重定向。
- 网关在用户确认后只创建/复用 HappySwitch 专用分组令牌，不改变其他令牌；密钥接口取得完整值，独立无 Cookie 请求发现分组模型。
- 授权快照使用与 SSO_FLOW_SECRET 域隔离派生的 AES-GCM 密钥加密保存；兑换或取消后清除快照，过期清除记录。底层平台备份可能保留历史密文，不将应用层删除表述为平台物理擦除。
- 只处理当前账户实际可用分组，最多 32 个；快照大小最多 90,000 字节，避免 Durable Object 单值上限。未知或图片等非编程模型的分组报告跳过原因。
- API Key 按原有机制保存到本地供应商库；桌面端事件仅返回账户名称、账户金额快照、数量、分组和警告。供应商编辑器沿用上游显示与管理行为。
- 配置 ID 包含用户、分组和助手类型；重复同步保留用户编辑、刷新 Key；已启用配置通过既有服务刷新当前 CLI。不会删除网关移除分组对应的历史配置。
- 登录等待界面显示验证码、重新打开浏览器和取消入口；最多 10 分钟，成功后刷新列表。新导入仍由用户手动启用。

## 尚待用户决定

### 最小界面增量（2026-10-04 用户最新要求）

- 已确认：尽量减少项目文件改动；保留上游页面结构及其他功能，用户仍可自行添加供应商。
- 已确认：新增功能范围为登录、自动导入 API Key、账户余额、累计消费、控制台和充值入口；沿用 CC Switch 已有用量统计，不另加请求数、Token 或趋势统计。
- 已确认：用户核对左下角位置后要求“改动”，已授权实施账户入口；生产部署仍未授权。
- 代码事实：右上角登录组件已移除，App.tsx 恢复初始 fork 的布局代码；Sidebar.tsx 仅引入账户组件并在全局底部增加独立账户行。供应商列表及编辑器保持原样。
- 本机原版 CC Switch 与 fork 均为 3.20.4；测试使用独立数据目录，不能将其空列表当作页面结构改变，也不能将用户观察到的所有差异归因于数据目录。
- 已核对：CC Switch 已有侧栏全局用量统计页，支持会话日志和路由请求日志，展示费用、请求数与 Token 等指标；账户累计消费取网关记录，不由本机统计推算。
- 历史位置：原应用页顶部 actions 内，ProfileSwitcher 后、添加供应商前；此入口现已移除。
- 已确认位置：用户选择左下角，账户入口置于全局侧栏底部，替代当前应用页右上角登录按钮；登录前后使用同一位置，所有页面可访问。
- 当前展示：登录前显示“登录 HappyToken”，登录后原位显示账户名和展开标识；点击向上展开余额、累计消费、充值、控制台和同步配置。沿用侧栏宽度及现有底部功能，长账户名截断并提供完整名称提示；侧栏折叠时显示图标与提示。
- 链接源码依据：HappyAPIWeb/lib/site-data/navigation.ts 已定义控制台 /dashboard、充值 /wallet，通过 gatewayAccountEntryUrl 统一登录跳转并用浏览器打开；真实页面链路待验证。
- 金额依据：认证后的 /api/user/self 返回 quota（余额）、used_quota（累计消费），/api/status 返回 quota_per_unit、quota_display_type 和汇率。遵循 NewAPI rc.21 配置换算 USD/CNY/CUSTOM，不硬编码固定汇率。配置未知、TOKENS 展示或接口失败时不伪造金额，显示不可用并引导控制台。
- 最小授权选择：不新增长期桌面管理令牌或携带浏览器 Cookie。金额为最近授权快照，显示同步时间；“同步配置”重新在浏览器授权并更新金额和配置。充值后需重新同步，不能表述为实时余额。
- 账户名称和可选金额快照保存在既有本机数据库 settings 的 happy_token_account 键，重启可恢复。此记录不含 Cookie、API Key、密码或额外登录凭证；恢复账户显示不等于新的有效网关会话。
- 协议兼容：旧授权快照缺少 overview 时仍可导入配置并显示账户名，金额显示不可用。

### 本轮验证

- 前端 typecheck、renderer 构建通过；账户组件 7 项与 App 集成 16 项测试通过，覆盖账户恢复、延迟缓存不覆盖新同步账户、金额缺失、充值/控制台跳转、折叠入口及切换全局页面时入口保留。
- 使用实际 Sidebar 与账户组件的独立本地预览检查向上展开和折叠状态；账户名/金额为明确标记的演示数据，非真实账户链路。预览文件位于系统临时目录，不向项目增加预览页面。
- Rust release cargo check 通过；本轮未重新运行 Rust 单元测试或生成新版 macOS 安装包。
- gateway-sso 工作区 61 项测试与类型检查通过；新增货币换算测试，授权兑换测试验证余额/消费快照。Wrangler dry-run 通过，HappyAPIWeb 根类型检查与构建通过。
- 本轮桌面提交 d312897e、网关提交 762e021 均已直接推送各自 main；网关独立已提交副本的 60 项测试、类型检查与 dry-run 通过，未混入用户既有 start-page.ts / worker.test.ts 修改。此前浏览器版安装包及其源码提交记录是历史产物，不包含本轮账户菜单。
- 真实账户金额、充值跳转和浏览器授权端到端未验证，生产 Worker 未部署；此前 macOS 包不包含本轮账户入口改动。

### 真实账户检查（2026-10-04）

- HappyServices 根 .env 存在 Casdoor 管理员配置，未找到专用普通测试账户凭证；仅检查配置存在，不输出或复制其值。
- 本次复用 Chrome 已有真实管理员会话，通过实际账户菜单确认身份；未使用配置密码重新登录，不能表述为密码登录验证通过。
- 与桌面菜单相同的控制台和充值 SSO 链接分别到达 /dashboard/overview、/wallet；两个页面余额和累计消费显示一致。只查看钱包，不提交充值或支付。
- 只读查看现有令牌列表，确认存在 default、gpt-pro、gpt-web 等分组令牌；这不是 /api/user/self/groups 的完整权限验证。未读取完整 API Key、未创建、修改或删除令牌。
- 公开 /api/status 的版本仍为 v1.0.0-rc.21，货币展示为 CNY，并提供换算参数；实际账户页面单位与现有换算设计一致，尚未验证原始账户 API 数值到桌面浮层的传递。
- 原生形式 POST /sso/desktop/start 使用随机 challenge 探测，线上返回 HTTP 404，未生成授权请求。完整 HappySwitch 浏览器授权、分组 Key 导入与账户金额同步仍待 Worker 发布后验证；不能用上述只读验证替代端到端通过。
- 最新源码已用 `pnpm tauri build --no-bundle --ci` 完成 release 构建，包含左下角账户入口；本地测试副本使用 HappyRouter 对应 Developer ID 签名，严格签名校验通过，本轮未重新公证或生成 DMG。
- 原生测试副本位于用户 Library/Application Support 下的 HappySwitch-real-account-test，使用独立 `CC_SWITCH_TEST_HOME`。实际启动后确认主界面与设置页均有左下角登录入口；点击后请求失败，与线上 start 接口 404 一致。既有安装包与用户配置未覆盖。

登录后自动启用策略：已询问是否首次启用 Default、后续保留用户选择，或仅导入后手动启用。暂按仅导入、由用户选择分组启用实现；收到答复后更新本节和实现。

## 按应用和协议导入（2026-10-04）

### 已确认需求与依据

- 用户要求覆盖侧栏每个 Agent，区分分组模型实际使用的协议，并调查 GPT Web 能否用于 Codex；不改变原有页面或新建协议代理。
- 当前 Codex 官方配置只接受 `wire_api = "responses"`，不能通过写 `chat` 让新版 Codex 直连 Chat Completions。依据：[官方配置](https://developers.openai.com/codex/config-reference/)。
- 原 CC Switch 已实现 Codex Responses ↔ Chat Completions / Anthropic、Claude Messages ↔ Chat / Responses / Gemini 转换，并通过现有路由模式启用；导入仅使用其已有 `meta.apiFormat` 与客户端配置结构，不修改转换器。
- 只读实测公开 `/api/pricing`：当前 default / gpt-pro / gpt-web 文本模型的 supported_endpoint_types 多为 openai（Chat），缺少其他接口声明；不能把它当作每个分组实际仅支持 Chat 的结论。NewAPI rc.21 对同名模型跨渠道汇总协议，不能替代逐渠道验证。依据：[pricing.go](https://github.com/QuantumNous/new-api/blob/v1.0.0-rc.21/model/pricing.go)、[接口类型](https://github.com/QuantumNous/new-api/blob/v1.0.0-rc.21/constant/endpoint_type.go)。
- `../HappyAPIWeb/docs/gpt-onboarding.md` 已明确 gpt-pro 使用 Responses、default 按模型区分、gpt-web 仅供网页助手。gpt-pro 的编程 GPT 模型使用这一明确服务策略，其他分组不按名称推断接口。
- `../HappyServices/chatgpt2api/README.md` 部署记录对应源码 revision e55aef2829e7bf1d7256d6ff3feb4b40b02743d2：ChatGPT2API 实际有 Responses 兼容处理，但普通函数工具被替换成“工具不可用”的文字提示，文本流不生成编码所需的普通 function/tool_calls 结果。不能把它描述为“只有 Chat 接口”，也不能把 HTTP 成功或普通文字回答当成编码 Agent 验收。依据：[Chat 处理](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_chat_complete.py)、[Responses 处理](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_response.py)。本轮未连接服务器确认运行容器 revision，源码结论结合现有业务约束使用。

### 实现选择

- Worker 快照新增可选 modelProtocols（模型 → 受支持协议列表）。目录与账户授权分组、该分组 Key 的 `/v1/models` 取交集；公开价目表请求不带 Cookie 或 Key。不对未知协议默认写 Responses。
- Worker 对 gpt-web 给出原因，不创建编码用专用 Key；桌面也拦截旧快照中的 gpt-web。仅有被跳过分组的账户仍可保存账户概况，显示 0 配置和跳过原因。
- 旧快照没有 modelProtocols 时，不猜测协议、不生成新配置；需使用更新后的 Worker 重新授权。此前关于 overview 的兼容规则仍有效。
- 一个分组可为支持多协议的客户端生成多个条目，每个条目只包含该协议的模型，避免把混合目录发送到错误接口。文本模型筛选排除图像、音频、embedding、rerank 等；未知模型不自动作为编码模型。
- 单一配置应用保持原稳定 ID；新增多协议应用的 ID 包含账户、分组、应用和协议。重复同步仅刷新 Key；精确匹配未修改的旧 Claude/Codex 自动模板时修正协议元数据，已编辑配置保留并提示核对。
- 新导入只保存本地供应商库，不写用户 CLI 当前配置、不自动启动路由、不自动切换默认模型。启用仍使用原项目按钮和写入器。

| 应用 | 自动导入规则 |
| --- | --- |
| Claude Code | 优先 Messages；其他已声明协议通过原路由转换 |
| Claude Desktop | 原有本地路由模式及安全 Claude 模型路由映射 |
| Codex | 客户端始终 Responses；上游 Chat / Messages 以 apiFormat 标记供原路由转换 |
| Gemini CLI | 必须有 Gemini 模型及 Gemini 原生协议声明；Chat-only 不导入 |
| Grok Build | 必须有 Grok 模型及可转换协议；写 Grok 原生 TOML，context_window 沿用上游默认值，不声称是实测模型上限 |
| OpenCode | 按协议选择现有 AI SDK provider，每协议独立模型目录 |
| OpenClaw / Pi | 使用原生 api 字段，每协议独立模型目录 |
| Hermes | Chat / Messages / Responses 的原生 api_mode，不导入 Gemini 原生协议 |
| MCode（侧栏 MiniMax） | Chat / Messages / Responses 的原生 api，每协议独立模型目录 |

### 未验证状态

- 协议配置可生成不等于模型工具调用、流式返回或多轮工具结果均已验收。除已知 gpt-web 限制，其余条目使用服务声明作为候选配置，备注明确真实任务待验证。
- 生产桌面授权仍返回 404，尚未发布 Worker，也未用真实 Key 发起付费模型请求。不能宣称已经给当前真实账户完成各应用可用性配置。
- 当前已启动本地测试 App 是本轮协议改动前的二进制；本轮提交源码不自动替换安装包。

### 本轮验证结果

- 桌面 release 定向测试 11 项通过：协议转换触发、混合目录分离、Gemini/Grok 限制、旧模板迁移与用户编辑保留、各原生客户端 Key 刷新、协议快照校验及仅有网页分组的账户登录。
- 最后补验用户自定义 TOML 无 model 字段时不崩溃；以临时 Cargo package opt-level=0 / codegen-units=16 参数重跑上述 11 项全部通过，未修改项目编译配置。
- 最终生产配置下 `cargo check --release --locked` 与两份修改 Rust 文件的格式检查通过。
- 原项目未改动的 Responses → Chat 请求转换 103 项、Chat → Responses 流式转换 30 项测试通过，覆盖函数工具调用参数、工具结果与流式事件；测试均为虚构数据，不代表上游模型真实任务通过。
- 网关工作区测试 63 项通过；从独立已提交源码 1b74f25 复查 62 项通过（未混入用户既有 start-page.ts / worker.test.ts 修改），TypeScript 与 Wrangler dry-run 通过。未执行生产发布。
- HappyAPIWeb 根 TypeScript 检查与 Next 构建通过；本轮不改官网页面。

## fork 更新边界

- 桌面产品名为 HappySwitch，应用标识为 `cn.happytoken.happyswitch`；本地供应商库沿用 CC Switch 兼容格式。
- 仓库、关于页面和手动更新链接指向 happy-token fork，保留上游许可证与原作者署名。
- 尚无 fork 自有签名更新源，清空自动更新 endpoint、关闭签名更新产物生成，防止 fork 被上游包覆盖。正式发布需要另外配置 HappyToken 自有签名密钥与更新清单。

## 验证状态

- 前端 `pnpm typecheck` 与 `pnpm build:renderer` 已通过。
- 内置登录版本曾全量运行前端测试：182 个文件、2,112 项测试通过（包含新增登录桥接的 9 项测试）。
- 内置登录版本定向前端测试曾通过登录桥接 9 项、按钮交互 3 项；浏览器版本已移除旧桥接脚本，当前验证见文末。
- Rust `cargo check --locked` 已通过；最终登录后端定向测试 5 项全部通过。
- macOS debug 桌面打包已成功，实际启动可显示登录按钮并打开 Gateway → Casdoor 登录页；取消登录恢复按钮已验证。调试数据目录使用 `CC_SWITCH_TEST_HOME`。
- 最后一轮编译曾因磁盘空间不足失败；使用 `cargo clean -p cc-switch` 清理本项目构建缓存后，以关闭增量缓存的方式重跑 Rust 测试成功。
- 未输入真实账户凭证，尚未实测真实账户令牌同步、各分组模型调用、Windows/Linux 窗口行为；不将模拟测试或登录页可达视为这些链路通过。
- API key 与会话相关测试仅使用虚构数据。

## macOS 签名与公证（2026-10-04）

- 凭证源为 `../config/env/env.shared`；只记录变量名，不记录值。
- 使用 Keychain 已安装的 Developer ID Application 证书。
- 将 `CSC_NAME` 映射为构建进程的 `APPLE_SIGNING_IDENTITY`，将 `HAPPYROUTER` 映射为 `APPLE_PASSWORD`，同时使用 `APPLE_ID` 与 `APPLE_TEAM_ID`。
- Apple Developer ID 签名/公证凭证与 Tauri 自动更新签名密钥是不同用途；本次复用前者，自动更新仍保持关闭。
- 构建命令为 `pnpm tauri build --bundles app,dmg`，仅生成本机包，不自动发布 GitHub Release。
- 依据：[Tauri macOS 签名文档](https://v2.tauri.app/distribute/sign/macos/)、上级 config 的 macOS 发布说明。
- arm64 Release 编译完成，`HappySwitch.app` 的 `codesign --verify --deep --strict` 验证通过。
- 首次 Apple 公证曾返回协议 HTTP 403；账户协议处理后已解除，最终公证通过。构建凭证未写入仓库。
- 由于公证阶段中断 Tauri 打包，使用 `hdiutil` 生成本机 DMG 并以同一 Developer ID 签名；产物位于 `src-tauri/target/release/bundle/dmg/HappySwitch_3.20.4_aarch64.dmg`。
- DMG 签名与磁盘校验通过；最终 Apple 公证为 Accepted，DMG 票据装订及 `stapler validate` 通过，应用票据装订通过，`spctl --assess` 返回 accepted / Notarized Developer ID。没有发布 GitHub Release。

### 认证方式复核

- HappyRouter 的 `../tmp/HappyRouter/scripts/notarize.cjs` 使用 `@electron/notarize` 调用 `notarytool`，认证参数是 Apple ID、应用专用密码和 Team ID。HappyCode、TokenUsage 使用同类流程。
- 本轮使用的 Apple ID、Team ID 与 HappyRouter 应用专用密码经本地比对均一致；没有把这些值输出到日志或文档。Tauri 的 `APPLE_PASSWORD` 对应 Electron 的 `APPLE_APP_SPECIFIC_PASSWORD`。
- 绕过 Tauri，直接使用 HappyRouter、HappyCode、TokenUsage 各自的本地凭证执行只读 `notarytool history`，三者均返回相同的协议 HTTP 403。错误不是 HappySwitch 独有，也不依赖其 Bundle ID。
- Apple 返回的错误指向团队协议/账户资格；尚未登录开发者后台核实具体协议，不能确定是尚未签署、过期，还是 Apple 后台状态未同步。历史项目采用何种流程已确认，但过去每个安装包是否公证成功未核实。

### 用户同意协议后的复查（2026-10-04）

- 用户确认已同意协议。使用原 HappyRouter 凭证重查 `notarytool history` 并直接提交已签名 DMG，两次均仍返回同一协议 HTTP 403，尚未获得公证提交 ID。
- 已同意协议是用户确认；Apple 公证服务当前仍拒绝请求是实测事实。是否存在状态同步延迟、其他待签协议或团队账户资格问题，尚未确认；不据此断言用户未签署协议。

### 最终公证结果（2026-10-04）

- Chrome 中核实登录账户与构建账户一致，团队 BL67GP4S58，会员有效至 2027-04-21，Developer Program 协议已接受。App Store Connect 曾出现独立 Terms of Service 待同意弹窗；未代用户接受。随后页面无弹窗，Free Apps Agreement 显示 Active。
- 复查 `notarytool history` 成功，协议 403 已解除；不能仅凭前后状态确定具体是哪份协议导致原错误。
- 提交现有 arm64 DMG，Apple 返回 Accepted，提交 ID 为 `76163a49-b9ec-4282-85ee-e974ff2753de`。
- DMG 与本地 .app 的公证票据均已装订；DMG 票据验证通过，本地 .app 的签名验证与 Gatekeeper 检查通过。

## 浏览器登录实现与验证（2026-10-04）

- 用户同意跨仓库开发；HappySwitch 已改为默认浏览器，gateway-sso 增加 `/sso/desktop` 与 start/approve/poll/cancel 接口。
- 桌面旧内置登录脚本与对应桥接测试已删除；当前运行的旧本地测试进程仍为内置登录版本，新浏览器登录安装包见下节。
- 本轮前端类型检查、renderer 构建、等待窗口 4 项交互测试通过；Rust release cargo check 通过，Rust 定向测试 4 项通过。
- gateway-sso 工作区 60 项测试通过（含本轮 7 项协议测试及用户此前未提交的测试）；从独立的已提交版本复查 59 项测试通过，TypeScript 检查与 Wrangler dry-run 打包通过。部署应使用该独立提交版本，保留工作区用户既有修改。
- Workerd/Miniflare SQLite Durable Object 实际运行验证了创建、授权页、pending、取消与拒绝已取消兑换。
- 浏览器本地模拟账户授权页可显示当前账户、验证码和授权范围，确认后显示返回 HappySwitch 提示；模拟验证不代表真实网关 Cookie、令牌同步或真实账户链路已通过。
- HappyAPIWeb 根 TypeScript 检查与 Next.js 构建通过；未改官网页面，不将 Worker 页验证视为官网全部交互回归。
- 未部署 Worker，未改变生产 Casdoor 或网关配置；真实浏览器登录仍等待发布。部署与回滚见 `../HappyAPIWeb/gateway-sso/DESKTOP_LOGIN.md`。

### 浏览器登录版 macOS 包

- 桌面源码提交 `e6f9cb63`，网关可部署源码提交 `d7e17f9`（HappyAPIWeb）；均已直接推送至各自 main。
- 新 .app 已签名、公证 Accepted 并装订票据，公证 ID `0db1ee31-7614-4e72-b19f-f378707cd98f`；签名与 Gatekeeper 检查通过。
- Tauri 内置 DMG 脚本失败后改用 hdiutil；新 DMG 已签名、公证 Accepted 并装订票据，ID `1ec05d69-ca44-4976-8827-146dec2de7af`。DMG 签名、stapler validate、hdiutil verify 全部通过。
- 最终稳定副本：`release/HappySwitch_3.20.4_aarch64_browser_login.dmg`；SHA-256 `cf87443ddda8b689ed9b5afad61dbec0c42e4016e99f54d4cf29061e399cad81`。
- 先前旧包备份放在 Tauri 构建目录，被打包过程清理，当前不宣称保留了旧二进制；可从历史源码重建。最终包已移出构建目录，release 被 Git 忽略。
- 当前授权不包括生产 Worker 部署；尚未上线新接口或实测真实账户浏览器授权，不自动重启用户当前测试进程。


## 账户操作与分组变更（2026-10-04）

已确认：账户入口保持左下角，增加“更新配置”和“退出登录”；更新配置通过浏览器再次授权，取得最新余额、消费、分组、Key 和模型协议。有效浏览器会话直接显示授权页面，仍需点击确认；不自动批准授权。

实现选择：分组按接口声明匹配各个 Agent，不自动切换当前供应商。保存按账户隔离的自动配置指纹（仅摘要，不另外存 Key）；未自定义的自动配置可以刷新模型和协议，用户改动的配置只刷新凭据并提示协议差异。最新授权分组清单与成功同步的分组分开：获取某组失败不表示该组被删除。缺失分组/协议的未修改、未启用、未加入故障转移队列配置可通过原供应商服务删除；正在使用、用户修改、以及可能已写入外部文件的追加型应用配置保留并提示。旧版本未跟踪的配置不自动删除。

退出登录清除 HappySwitch 本地账户概况并取消待授权请求；保留导入配置、已有分组 Key 和浏览器会话，不撤销网关令牌。账户写入与退出串行，避免晚到的授权结果恢复账户。更新配置是账户配置更新，应用软件更新继续使用上游功能。

验证：前端全量单元测试 2112 项通过（账户组件 9 项），类型检查与 renderer 构建通过；网关类型检查、官网类型检查及构建、Worker dry-run 通过。Rust 定向测试 12 项与标准 release 类型检查通过；Worker 工作区 64 项、独立已提交版本 63 项测试通过，覆盖权威分组清单、零分组和旧配置识别。生产 Worker 尚未发布，真实账户整条授权/导入链路仍未通过；浏览器端口 18765 为示例数据 UI 预览，不能作为真实账户验证。


## 生产授权发布验收（2026-10-04，进行中）

用户在发布 72f9b8d 的明确询问后回复“继续”，本次授权仅包含 gateway-sso Worker 与真实账户测试。使用独立已提交副本部署，未混入 HappyAPIWeb 工作区其他修改。原版本 38344659-2d10-4fe7-a65a-6b671bda4699；新版本 a5ecde70-aaac-4150-9139-c1a6f4a6f97d。既有 SSO_FLOW_SECRET 存在，未读取或轮换其内容。线上无账户凭证的 start/pending/cancel/已取消兑换检查分别为 200/202/200/410。

首次添加 Durable Object 类后，普通版本回滚可能受类生命周期限制；异常时应恢复旧 SSO 行为并保留新增类与存储，不删除 namespace。依据：[Cloudflare 回滚限制](https://developers.cloudflare.com/workers/versions-and-deployments/rollbacks/)。Chrome 既有真实网关会话有效；新版本地应用正在构建，真实账户授权与导入尚未宣布通过。


### 真实账户首轮结果

Chrome 实际授权页面已显示“授权完成”，本地原生应用收到 Root User 账户并成功同步 24 项：Claude Code、Claude Desktop、Codex、OpenCode、OpenClaw、Hermes、Pi、MiniMax Code 各 3 项，分别来自 default、gpt-pro、image。gpt-web 未创建编码令牌并显示不支持函数工具调用的原因；Gemini CLI / Grok Build 未找到符合协议和模型条件的配置，未强行导入。账户余额和累计消费与控制台按显示精度一致。仅授权、目录读取和配置同步已验收，不代表真实编码任务/工具调用验收。

按用户最新要求，后续普通功能测试优先 Tauri dev / Vite 热更新，不重复完整打包。使用独立 CC_SWITCH_TEST_HOME 数据，renderer 18766；当前改用 release 依赖缓存加仅主包低优化参数运行 dev，避免新建完整 debug 依赖缓存。未修改 Cargo.toml 或默认 Tauri 配置；仅本地 dev 测试壳使用 ad-hoc 签名，无 Apple 证书签名、公证或 DMG 步骤。此前刚构建的签名测试副本保留在 Library 下，dev 运行实例需区分。


### image 排除与 GPT Web 能力核对

用户明确要求过滤 image，原生生成器与网关收集器均增加排除条件；网关收集器不为该组创建桌面专用 Key。原生还识别旧 Worker 快照，按管理记录清理未启用的自动 image 配置，不触及手动供应商与已启用配置。网关此排除改动尚未发布，生产仍是 72f9b8d；原生 dev 已过滤旧快照。

真实 dev 更新发现旧指纹对 HashMap 序列化顺序、原应用自动维护的 liveConfigManaged=false 敏感；改为规范化字段顺序的 v2 指纹，兼容旧 Desktop 三个默认路由的六种字段顺序，并忽略该内部 false 标记。true 仍视为已写入外部配置，保留；其他配置值及用户自定义名称/备注照常比较。

GPT Web 依据为本地 HappyServices/chatgpt2api/README.md 记录的线上后端 revision e55aef2829e7bf1d7256d6ff3feb4b40b02743d2：[Chat 处理源码](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_chat_complete.py)、[Responses 处理源码](https://github.com/basketikun/chatgpt2api/blob/e55aef2829e7bf1d7256d6ff3feb4b40b02743d2/services/protocol/openai_v1_response.py)。普通工具定义触发不支持本地工具的提示；源码包含 Responses 实现，但不能据此认定网关 gpt-web 渠道已开放该接口或能运行函数工具。未执行 gpt-web 付费模型调用，未部署或替换其后端；自动编程仍需结构化工具调用、结果回传和流式验收。


## 标准协议落档与最新状态

标准入口：[协议契约与可执行流程](protocol-contract.md)、[逐模型证据快照](model-protocol-inventory.json)。2026-10-04 使用真实 Default/Pro 分组 Key 只读获取 /v1/models，分别 41/24 项；GPT Web 11 项与 Image 31 项仅为公开 pricing 清单。真实 Key 只在内存用于固定网关目录请求，没有写入文档或输出。

dev 完成更新、退出、重新授权；image 自动配置已全部清理，账户恢复与 16 项导入通过。Grok 自定义模型限制在本轮 review 中更正，新规则尚未在真实 dev 同步；模型调用及函数工具往返仍未验收。Pi 原生 api 与元数据归一化差异作为后续审查项记录，不能把该警告直接认定为用户设置错误。

本轮协议审查后的 Rust 定向测试 14 项通过，覆盖 Grok 自定义模型、协议转换、image 排除、指纹归一化与用户配置保留；git diff --check 通过。


### 同步提示精简

前端显示边界合并 Default/Pro 路由提示并去重 Image，缩短 GPT Web、Gemini 和协议核对文案；未知错误原样保留。Pi 实际 api 与生成协议一致时不再提示核对，即使原写入器去掉 meta.apiFormat；确实不一致仍提示。仅修正警告比较，管理指纹的归一化差异仍需另行处理。本轮不改生产 Worker。


### 默认模型更新

再次只读核对真实 Default/Pro /v1/models。自动模板选择改为数值版本优先，避免旧 gpt-5.3-codex 凭名称标签胜过 GPT-6；同代优先 Sol/Sonnet，再考虑编码标签。多模型目录与客户端自行管理的默认设置保持原有机制。不认为数值排序等于发布日期或质量保证，只在本次已声明协议候选中采用该确定性选择策略。Default 的 gpt-6-astra 协议目前未知，不强行使用；最新上游 Claude 5.5 当前组目录未提供，不写不存在的型号。现有自动配置在新版重新授权更新时迁移，用户编辑过的配置保留。


### 认证与账户界面设计（2026-10-04）

使用 frontend-design 技能，保留桌面现有设计 token。浏览器授权采用桌面两栏、手机单栏：应用/账户身份、验证码、两项权限、主授权按钮；校验、准备、成功和失败有独立状态，继续使用原账户检查、nonce CSP、明确授权和一次性凭证。未新加账户权限或自动批准。

账户面板分身份、金额、外部入口、同步/退出四个区域；新增独立账户设置对话框，复用同一账户组件与原生命令，提供明确关闭按钮，进入浏览器登录前关闭设置对话框，避免叠加弹层。等待页统一验证码卡片、状态与主次按钮。四语言新增账户设置标题与说明；余额显示两位小数，原始值不变。

验证：前端组件 11 项、类型检查、renderer 构建通过；Worker 工作区 64 项、类型检查通过。实际 Tauri dev 可看到真实账户新版面板与设置。浏览器以实际 desktopPage 源码提供独立临时预览，模拟身份明确标为演示账户，本地成功状态及 390px 宽度无横向溢出已检查；模拟预览不代表生产新版授权通过。未发布 Worker，未重新打包应用。范围澄清尚无回复，网页控制台个人设置与原全局设置不在本轮完成范围。


### 单栏与账户入口收敛

用户要求传统单栏授权，已改为 480px 居中单栏：身份、验证码、权限、主操作自上而下。上一节独立账户设置对话框与菜单重复，本轮移除对话框及对应翻译和入口，仅保留左下角账户面板；已有同步、退出、最近同步时间及外部链接保留。前端 11 项测试与类型检查、Worker 工作区 64 项测试与类型检查通过，模拟授权预览已刷新。未发布生产 Worker。


用户纠正单栏目标为左下角账户弹出面板。本次将充值/控制台入口从两列改为单列全宽，更新配置/退出继续纵向排列；只调整布局，不修改原生命令、账户数据或授权行为。通过 Tauri dev 的 Vite 热更新查看，不重新打包、不部署 Gateway。

验证：账户组件 11 项测试及 TypeScript 类型检查通过；实际 Tauri dev 真实账户面板截图确认充值/控制台上下排列，面板无裁切。


账户面板设计对比：使用独立 localhost:18811 静态示例页面展示三个方案，示例账户/金额不读取真实凭据。A 信息行+一致菜单操作；B 余额重点+单个充值按钮；C 设置标题+服务/本地配置分组。已浏览器检查与截图，尚未选择，不作为功能或生产验证。


C 方案已实现：沿用 quiet 按钮与原生命令，金额不再放大，外部链接图标右置，操作按服务/本地配置/退出分区。四语言补齐标题与分组标签，同步时间文案精简。账户组件 11 项测试和 TypeScript 检查通过，实际 Tauri dev 的真实账户面板截图确认；未重新打包、未部署 Gateway。


发布验证见 [release.md](release.md)：跨平台 Actions 手动构建，macOS Developer ID 签名、公证及 stapler 校验，Windows MSI/NSIS；网站需独立区分 HappySwitch fork 与上游原版。验证状态以实际 Actions 结果更新，不把启动构建视为已通过。


2026-10-05：跨平台 CI 与 macOS/Windows 安装包构建通过，正式 Release 为 HappySwitch 3.20.5。分组清理改用 mode::current::is_referenced，保留设备直连、数据库和代理路由引用的供应商；全量测试通过。具体构建、签名、公证与网站上线状态见 [发布验证记录](release.md)。


2026-10-05：用户明确授权官网生产上线。HappyAPIWeb 提交 7152ad5c 的已验证 OpenNext 构建已部署至 happy-api-web-next，版本 7fcf4d22-bac6-4d92-b783-72ab2ccc0849。中英文下载页与原有首页/博客/模型/定价检查通过，三个 HappySwitch 3.20.5 安装包公开链接 HTTP 200；浏览器确认下载卡片和原版 CC Switch/Codex 入口保留。此次没有部署 Gateway 或 installer-sync。


## 用户教程（2026-10-05）

确认需求：简单图文，按用户实际操作说明。实现：仓库 user-guide.md 与官网中英文 /docs/happyswitch，三张可放大的示意图分别说明左下角登录/浏览器确认、助手选择/启用、账户面板。内容核对当前登录、手动启用、金额快照、更新配置及退出保留 Key 的真实行为，不宣称工具调用验收通过。保留原版 CC Switch 手动教程与录屏，GPT 入门减少技术术语并链接 HappySwitch。官网源码验证后直接提交 main；新教程随后已获明确授权并上线，见下方发布记录。


2026-10-05：用户明确回复“允许”，授权新版图文教程生产发布。网站提交 458be8c9 的 OpenNext 构建完成，预览检查通过后发布至 happy-api-web-next；生产版本 fa1b0f0b-7150-487f-ba35-04f5feae196b，前版本 7fcf4d22-bac6-4d92-b783-72ab2ccc0849。中英文教程、GPT/Codex/下载页入口、六张 SVG、首页、管理会话与 sitemap 均 HTTP 200 且内容核对通过；浏览器已确认线上中文图文教程。此记录取代此前教程“未授权/未发布”状态。未部署 Gateway 或 installer-sync。


2026-10-06：用户明确授权发布新版 Gateway 单栏授权页。从 HappyAPIWeb 已提交 b0a44d3c 副本发布（授权页源码 1b1ce4b），未带入未提交的 desktop-session/start-page/测试改动。Gateway 版本由 a5ecde70-aaac-4150-9139-c1a6f4a6f97d 更新为 e117d932-e871-4de0-aecf-aa8b1e3c3c8d；63 项测试、类型检查和 dry-run 通过。线上新版页面 HTTP 200、未授权轮询 202；Chrome 已有账户显示及授权按钮正常，测试未批准授权。单栏样式已生产发布，取代历史“未发布”记录；成功状态收起权限与验证码的代码已部署，本次未重新执行真实账户完整授权/兑换。


## 历史配置清理与重复同步（2026-10-06）

用户明确授权移除 Codex 的 HappyToken-Default/Pro。只读核对发现旧项与自动项使用不同 Key 和配置，且旧项未启用、不在故障转移队列；关闭应用后先本机备份，再删除指定旧项，重新启动后界面确认完成。未撤销网关令牌、未修改当前启用供应商，未提交本机数据库。

源码依据：happy_token_providers::identity 按账户、分组、助手及多协议后缀生成稳定 ID；同步通过 get_provider_by_id 查找并更新已有项。同一身份重复同步不会因次数新增配置；不同账户、手动导入及多协议项保持独立。此结论为源码核对，本次未再次进行连续两次真实授权验收。
