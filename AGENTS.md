# Repository Guidelines

## Project Structure & Module Organization

- `src/`: Rust application code (edition 2024)
  - `src/views/`: GPUI view components (`Render` implementations)
  - `src/states/`: reactive state entities and global store
  - `src/services/`: async backend operations (event hub, Redis repo, supervisor)
  - `src/connection/`: server config + encrypted credential storage
  - `src/helpers/`: small utilities shared across the app
- `assets/`: bundled assets (for example `assets/icons/`)
- `locales/`: i18n translations (`locales/en.toml`, `locales/zh.toml`)

## Build, Test, and Development Commands

- `cargo run`: run locally (debug build)
- `cargo build --release`: optimized build
- `RUST_LOG=debug cargo run`: enable verbose logging (`tracing`)
- `cargo check`: fast compilation check
- `cargo fmt`: format with rustfmt
- `cargo clippy`: lint (note: `unwrap_used` is denied)
- `cargo test`: run unit tests

## Coding Style & Naming Conventions

- Prefer `cargo fmt` over manual formatting; keep diffs minimal and consistent.
- Avoid `.unwrap()`/`.expect()`; use `?` and structured error handling (`snafu`/`anyhow`).
- Use idiomatic Rust naming: modules/functions `snake_case`, types `PascalCase`.
- UI types commonly use a `Dfc*` prefix (for example `DfcSidebar`, `DfcTitleBar`).

## Testing Guidelines

- Tests are primarily unit tests colocated with the module (`#[cfg(test)] mod tests`).
- When changing services/runtime code, add tests around parsing, state transitions, and retry/backoff logic where feasible.

## Commit & Pull Request Guidelines

- Use Conventional Commits (observed in history): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`.
- PRs should include: a clear description, linked issue/ticket, and screenshots for UI changes.
- Do not commit secrets or machine-local files (`.env*`, `*.local.*`, logs); see `.gitignore`.

## Agent Notes (Optional)

- Keep changes focused; update `CLAUDE.md` when developer commands or architecture guidance changes.


## 参考项目
- 数据读取逻辑参考DFC:/Users/cyenx/Dropbox/work/goldwind/code/DFC
- 界面UI参考zedis:/Users/cyenx/Library/CloudStorage/Dropbox/code/tools/zedis

## Others
- 使用中文进行回复
