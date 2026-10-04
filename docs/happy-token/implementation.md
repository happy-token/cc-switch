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
