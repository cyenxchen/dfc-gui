# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在本仓库中工作时提供指导。
`AGENTS.md` 是本文件的 symlink,两个入口遵循同一套规则。

## 项目概览

基于 GPUI 的 Rust 原生桌面客户端,用于设备群控 (Device Fleet Control)。通过 Pulsar / Redis (`fred`) 通信,解码 IoT Hub 的 protobuf 载荷。单二进制发布,跨平台目标为 macOS arm64 和 Windows **i686**(32 位,**不是 x64**)。

## 构建与工具链

- 需要 Rust **1.85+** — `Cargo.toml` 使用 `edition = "2024"`,不要"修"成 2021。
- `protoc` 通过 `protoc-bin-vendored` 自带,**不要**让用户去装系统级 protoc。
- `build.rs` 会用 `prost-build` 编译 `proto/*.proto`,并在 Windows 上嵌入 VERSIONINFO 和图标。修改 `.proto` 后请执行 `cargo clean -p dfc-gui`(或全量重建),增量构建可能跳过代码重生成。
- 发布目标:`cargo build --release --target aarch64-apple-darwin` 与 `cargo build --release --target i686-pc-windows-msvc`。CI 配置见 `.github/workflows/release.yml`。

## 编码规范

- `[lints.clippy] unwrap_used = "deny"` 已启用 — **禁止** `.unwrap()`。用 `?` 传播错误(错误类型基于 `snafu` / `anyhow`),只有在不变量可证明的地方才允许 `expect("理由")`。
- 日志统一走 `tracing`(`info!` / `warn!` / `error!` / `debug!`),应用代码中不要使用 `println!` / `eprintln!`。
- 无自定义 `rustfmt.toml`,直接 `cargo fmt` 即可。

## 国际化 (i18n)

翻译位于 `locales/en.toml` 与 `locales/zh.toml`。新增任何文案都必须**同步更新两个文件**。代码侧使用 `rust_i18n::t!("key")` 宏,catalog 在启动时由 `rust_i18n::i18n!("locales", fallback = "en")` 加载。

## UI / 状态架构

GPUI Entity 单向数据流:UI Action → State 方法 → Service 派发 → Event → State 更新 → UI 刷新。目录划分:`src/states/`(状态实体)、`src/services/`(后台任务)、`src/views/`(UI 组件)。**View 不允许直接修改 state**,必须通过 state 上的方法。

## 发布规则

- 创建或推送 git tag 之前,先把 `Cargo.toml` 的 package `version` 更新为发布版本。
- tag 之前先提交版本号变更的 commit。
- 如果 tag 版本与 `Cargo.toml` 版本不一致,**不要**创建或推送 tag。
