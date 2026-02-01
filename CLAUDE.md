# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Build
cargo build                  # Debug build
cargo build --release        # Release build (optimized with LTO)

# Run
cargo run                    # Run in debug mode
RUST_LOG=debug cargo run     # Run with debug logging

# Check & Lint
cargo check                  # Fast compilation check
cargo clippy                 # Run lints (unwrap_used is denied)

# Format
cargo fmt                    # Format code

# Clean
cargo clean                  # Clean build artifacts
```

## Architecture Overview

DFC-GUI is a native desktop application for device fleet control, built with **GPUI** (Rust GUI framework) and follows a **unidirectional data flow** architecture.

### Layer Structure

```
┌─────────────────────────────────────┐
│   Views (src/views/)                │  UI components, renders state
├─────────────────────────────────────┤
│   States (src/states/)              │  Entity-based reactive state
├─────────────────────────────────────┤
│   Services (src/services/)          │  Async backend operations
├─────────────────────────────────────┤
│   Connection (src/connection/)      │  Server configs, credentials
└─────────────────────────────────────┘
```

### Data Flow

```
UI Action → State Mutation → Service Call → ServiceEvent (crossbeam) → State Update → cx.notify() → UI Re-render
```

### Key Modules

- **states/**: GPUI Entity-based state management
  - `DfcAppState` - Persisted app config (route, theme, locale, server list)
  - `FleetState` - Runtime device/telemetry state with event ingestion
  - `ConfigState` - Redis configuration management
  - `DfcGlobalStore` - Wrapper combining all states, set via `cx.set_global()`

- **services/**: Background async operations
  - `ServiceHub` - Orchestrates all services, event multiplexing
  - `RedisRepo` - Redis operations (uses fred client with Tokio runtime)
  - `PulsarBus` - Message bus for device events
  - `Supervisor` - Connection health with exponential backoff retry

- **views/**: GPUI components implementing `Render` trait
  - `DfcSidebar` (80px fixed) - Navigation
  - `DfcContent` - Route-based content switching
  - `ConfigView` - Server configuration UI
  - `DfcTitleBar` - Custom macOS titlebar

- **connection/**: Configuration and encrypted credential storage
  - Server configs in `servers.toml`
  - AES-GCM encryption for passwords/tokens

### Runtime Notes

- GPUI uses smol-like async runtime, **not** Tokio
- fred Redis client requires Tokio context: use `runtime_handle().enter()` before Redis operations
- Events flow through crossbeam channels from services to states
- State changes trigger UI updates via `cx.notify()`

### Constants (src/constants.rs)

- Window: 1200x750 default, 800x500 minimum
- Sidebar: 80px width
- Bounded caches: 200 events/alarms, 1000 telemetry, 5000 logs
- Command timeout: 30 seconds

### Platform-Specific Paths

- **macOS**: `~/Library/Application Support/com.goldwind.dfc-gui/`
- **Windows**: `%APPDATA%\goldwind\dfc-gui\`
- **Linux**: `~/.config/dfc-gui/`

Logs: macOS `~/Library/Logs/`, Windows `%LOCALAPPDATA%\...\logs\`, Linux `~/.local/share/.../logs/`

### i18n

Translations in `locales/` (en.toml, zh.toml). Use `rust_i18n::t!()` macro with locale parameter.

### Clippy

`unwrap_used = "deny"` - Use `?` operator or proper error handling instead of `.unwrap()`.
