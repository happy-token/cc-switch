# HappySwitch 发布流程

本 fork 属于 happy-token/cc-switch；保留 CC Switch 上游许可证和署名。

## 构建验证

使用 `.github/workflows/happy-build.yml` 的手动触发构建，指定确切提交或 main。macOS Universal 同时编译 Intel/Apple Silicon；Windows x64 编译 MSI 与 NSIS EXE。两个平台执行类型检查、账户回归测试、renderer 与 Rust release 构建，上传安装包和独立 SHA-256 清单。

macOS 默认启用 Developer ID 签名、公证，签名失败不得作为正式包发布。Secrets 仅供此仓库使用；临时证书与钥匙串在任务结束时清除。Windows 当前没有 Authenticode 证书，安装包不宣称已签名。Windows ARM64 不在首版验证范围。

## 发布

仅在两平台构建与 macOS 签名/公证检查成功后，下载同一次构建的产物，验证校验清单。以 `happy-v<version>` 标签创建 HappySwitch Release，标签必须指向构建提交；此标签不触发上游 `v*` 发布流水线。不生成或启用上游 updater 清单，当前仍采用手动下载安装。

发布说明应写清 HappyToken 浏览器登录、按接口协议自动导入、账户面板及已知兼容限制。GPT Web 与 Image 不自动导入编程助手；模型工具调用尚未全面实测。

## 网站下载

HappyToken 网站添加独立 HappySwitch 下载入口，保留官方原版 CC Switch 下载。网址与产物元数据以验证后的 Release 为准，不能把 fork 安装包标为上游官方原版。不因本文件或仓库修改自动部署网站/Worker；生产上线需要用户明确授权。

## 当前状态（2026-10-04）

用户要求 GitHub Actions 验证并发布 Release，已允许将 HappyRouter 对应 Apple 签名/公证凭据保存为该仓库 Actions Secrets。首版定为 3.20.5。初轮 Windows 发现 PowerShell 逗号参数解析问题，已切换 Bash；正式构建 37212480889 运行中，尚不能宣称成功或发布完成。

CI 全量测试发现分组清理绕过统一当前供应商入口，已改用 mode::current::is_referenced，保护设备直连指针、数据库引用和代理路由。需等待修正提交的 CI 与最终构建，旧产物不用于发布。
