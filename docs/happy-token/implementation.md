# HappyToken 登录与自动配置

## 用户目标与边界

- 将 `farion1231/cc-switch` fork 至 GitHub `happy-token` 账户，在此 fork 上开发。
- 桌面端提供 HappyToken 登录，登录后自动配置账户的全部可用分组。
- 用户提及 Default、Pro、GPT Web；不硬编码分组名，不把未授权分组当作可用分组。
- 复用 HappyImage / HappyServices 的账户体系，不创建另一套账户密码。
- 维护根 `CONTEXT.md` 和本说明，减少后续重复沟通。
- 用户要求直接提交到 happy-token/main，不使用 PR，也不向官方上游提交 PR。此前 draft PR 仅创建在 happy-token fork 内，正在关闭。
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
- 当前签名、公证与 Gatekeeper 检查结果待本轮构建完成后更新。
