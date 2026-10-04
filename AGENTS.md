# HappySwitch 协作约定

开始工作先读 [CONTEXT.md](CONTEXT.md) 和 [登录实现说明](docs/happy-token/implementation.md)。

- 用户的新决定与代码事实及时更新上述文件；区分已确认需求、实现选择和未验证假设。
- 领域名词维护在 CONTEXT.md；接口依据、行为约束和验证状态维护在实现说明。
- 按用户要求直接提交并推送到 happy-token 仓库的 main；不创建 PR，也不向官方上游提交 PR。
- macOS 本机签名和公证获授权复用 ../config/env/env.shared 中 HappyRouter 的凭证；仅在构建进程内加载，不复制到仓库或日志。
- 本项目 fork 归 `happy-token`，保留上游许可证和原作者署名。
- 不输出或提交真实 Cookie、API Key、账户密码或生产配置。
- 仓库修改不包含生产部署授权。
- 保留已有与任务无关的修改。
