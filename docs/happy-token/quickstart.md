# 开发与使用

## 本地开发

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Rust 工具链由根目录 `rust-toolchain.toml` 固定；Tauri 桌面构建仍需平台原生编译环境。

## 使用登录自动配置

1. 打开助手供应商页面，点击「登录 HappyToken」。
2. 在弹出的官方 Gateway / Casdoor 窗口完成登录。
3. 等待自动同步结果。账户实际可用的每个分组都会尝试同步；部分失败或没有编程模型时显示原因。
4. 在对应 Claude Code、Codex、Gemini 页面选择 `HappyToken · 分组名称` 配置启用。
5. 需要刷新令牌或获取新分组时，再次点击「同步 HappyToken」。会复用有效的专用分组令牌，并更新已有配置。

登录后会为缺少专用令牌的分组创建 HappySwitch 专用 API Key。令牌没有独立额度上限或到期时间，受账户余额和分组权限约束，可在 Gateway 控制台撤销。

当前「同步」复用登录窗口中的现有 Gateway 会话；独立的切换账户或退出登录入口尚未实现。

## 本次验证命令

```sh
pnpm typecheck
pnpm build:renderer
pnpm exec vitest run tests/utils/happyTokenLogin.test.ts tests/components/HappyTokenLoginButton.test.tsx
cargo check --manifest-path src-tauri/Cargo.toml --locked
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib commands::happy_token::tests
pnpm tauri build --debug --bundles app
```

构建桌面应用不等于已经通过真实账户登录和模型调用验证。验证边界记录在[实现说明](implementation.md)。
