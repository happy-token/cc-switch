# HappyToken 登录与自动配置

## 用户目标与边界

- 将 `farion1231/cc-switch` fork 至 GitHub `happy-token` 账户，在此 fork 上开发。
- 桌面端提供 HappyToken 登录，登录后自动配置账户的全部可用分组。
- 用户提及 Default、Pro、GPT Web；不硬编码分组名，不把未授权分组当作可用分组。
- 复用 HappyImage / HappyServices 的账户体系，不创建另一套账户密码。
- 维护根 `CONTEXT.md` 和本说明，减少后续重复沟通。
- 用户要求直接提交到 happy-token/main，不使用 PR，也不向官方上游提交 PR。此前 draft PR #1 仅创建在 happy-token fork 内，已关闭；登录代码与文档已直接推送到 main。
- 用户授权 macOS 本机签名与公证复用上级 config 的 HappyRouter 凭证；不将真实凭证复制到项目或提交到 Git。
- 当前授权为修改项目仓库及本机签名打包；生产部署、Casdoor 应用设置、网关服务配置不在范围内。

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

## 实现约束

- 外部登录网页不授予 Tauri IPC 权限，保留现有仅 main 窗口的 capability；额外在应用 invoke handler 拒绝登录窗口调用所有应用命令（Tauri 默认应用命令未受 capability 限制）。
- 登录回调只带一次性随机 state 和已验证的用户 ID；URL、日志和前端事件不携带 Cookie 或 API Key。
- 后端读取 Gateway Cookie，再次验证 `/api/user/self` 的账户 ID。
- Cookie 请求只发固定 Gateway origin，禁止自动跟随重定向；模型发现使用独立客户端，仅携带分组 API Key。
- 只创建 HappySwitch 专用令牌，按分组复用；不改变用户其他令牌。
- 专用令牌无独立额度上限、无到期时间，仍受账户余额和分组权限约束。用户可在网关撤销。
- 按每组返回的模型生成 Claude Code、Codex、Gemini 配置；不编造不可用模型。仅图片等非编程模型的分组报告跳过原因。
- 配置 ID 包含网关用户、分组和助手类型，重复同步不重复添加，切换账户不覆盖另一账户配置。
- 重复同步保留用户编辑过的配置，只刷新 Key；已启用的配置通过现有供应商更新服务刷新当前 CLI 配置；不会删除网关移除分组所对应的历史配置。
- API Key 按 CC Switch 原有机制保存在本地供应商库，启用时写入 CLI 配置。登录同步事件只返回数量、账户名称和同步结果；供应商编辑器沿用上游的 API Key 显示与管理行为。

## 尚待用户决定

登录后自动启用策略：已询问是否首次启用 Default、后续保留用户选择，或仅导入后手动启用。暂按仅导入、由用户选择分组启用实现；收到答复后更新本节和实现。

## fork 更新边界

- 桌面产品名为 HappySwitch，应用标识为 `cn.happytoken.happyswitch`；本地供应商库沿用 CC Switch 兼容格式。
- 仓库、关于页面和手动更新链接指向 happy-token fork，保留上游许可证与原作者署名。
- 尚无 fork 自有签名更新源，清空自动更新 endpoint、关闭签名更新产物生成，防止 fork 被上游包覆盖。正式发布需要另外配置 HappyToken 自有签名密钥与更新清单。

## 验证状态

- 前端 `pnpm typecheck` 与 `pnpm build:renderer` 已通过。
- 原有前端测试全量运行：182 个文件、2,112 项测试通过（包含新增登录桥接的 9 项测试）。
- 最终定向前端测试：登录桥接 9 项、登录按钮交互 3 项全部通过。
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

## 浏览器登录需求与待实现方案

### 用户已确认

- 用户希望登录在默认浏览器完成，使用浏览器的自动填充，体验类似 Codex / Claude Code。当前仍是内置 WebView；该需求尚未实现。

### 已查明的限制

- `../HappyAPIWeb/gateway-sso/src/target-policy.ts` 仅允许 `/dashboard`、`/wallet`、`/keys`、`/profile` 回跳。
- `callback-page.ts` 在 Gateway origin 建立 Cookie 会话并写入浏览器 localStorage；桌面端不能读取系统浏览器 Cookie。单纯打开浏览器不能替代当前登录后自动同步。

### 实现方案（待跨仓库范围确认）

- 桌面端生成随机请求与 PKCE 校验数据，在默认浏览器打开 Gateway 专用桌面授权页面。
- 页面复用现有 SSO，明确展示授权给 HappySwitch 的分组同步操作，用户确认后完成授权。
- 网关保存短期授权记录，桌面端使用只在本地保留的验证数据轮询并一次性兑换；不把 Cookie 或 API Key 放入回跳 URL，不暴露 Casdoor 客户端密钥。
- 优先采用设备授权式轮询，避免通用外部重定向或浏览器向 localhost 传递凭证；需明确过期、取消、重复兑换拒绝与账户绑定。
- 网关只为该已验证账户执行既有分组/专用令牌同步操作，并通过认证的兑换响应交接所需配置；保持用户手动启用分组的当前策略。
- 桌面端提供等待浏览器、重新打开、取消与失败重试状态，保留安全且明确的超时。
- 需要修改 HappySwitch 与 gateway-sso；本次没有更改生产网关、Casdoor 配置或部署。跨仓库源代码范围已向用户询问。
