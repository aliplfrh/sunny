# Port Map: Vicinae → Sunny

Upstream reference: https://github.com/vicinaehq/vicinae (GPL-3.0, 2,600+ commits)

Upstream code size (as of Nov 2026):
| Language | Size | Purpose |
|---|---:|---|
| C++23 | ±6.8 MB | Core, daemon, framework, built-in modules |
| QML | ±715 KB | Entire UI (to be rewritten as a Raycast-like web view) |
| TypeScript/JS | ±330 KB | Extension runtime & API (frontend), extension build tools |
| Obj-C++/Swift/CMake/Shell | remainder | macOS, build scripts |

## Module map to carry forward (to be completed after cloning the upstream reference)

Upstream structure (shallow clone in `reference/vicinae`, as of Nov 2026):

`src/`: `browser-extension, cli, data-control-server, file-indexer, inference, lib, server, snippet, typescript, wayland-protocols`

`src/lib/`: `common, crypto, figura, fuzzy, glyph, linux-utils, script-command, soulver, vicinae-ipc, xdgpp`

| Sunny module | Origin in Vicinae | Strategy |
|---|---|---|
| `apps/sunny` (UI) | Vicinae QML UI | REWRITE — Raycast-like web UI (WebView2, not Qt Quick) |
| `apps/sunny` (backend) | `src/server`, `src/main*` | Rust + Tauri v2: frameless window, overlay, hotkey, tray |
| `crates/sun-core` | `src/lib/common`, search/ranking, settings, storage | Port to Rust with equivalent semantics (not line by line) |
| `crates/sun-core: fuzzy` | `src/lib/fuzzy` | Map behavior to `SkimMatcherV2` (`fuzzy-matcher`) — ✅ initial scaffold |
| `crates/sun-core: apps` | app service (app scanner/launcher) | Windows: enumerate Start Menu .lnk files — ✅ initial scaffold |
| `crates/sun-core: calculator` | `src/lib/soulver` (Soulver backend) | Rust: `meval`/`evalexpr` for Phase 1 |
| `crates/sun-core: settings` | settings/config service | Windows: JSON at `%LOCALAPPDATA%\Sunny`, single-instance guard, debounced UI writes — ✅ |
| `crates/sun-core: snippets` | `src/snippet` | Out of scope (dropped for Phase 2) |
| `crates/sun-core: file-indexer` | `src/file-indexer` | Windows: background recursive scan of user folders + fuzzy name search — ✅ (MVP; no USN journal yet) |
| `crates/sun-platform` | `src/lib/linux-utils`, `xdgpp` (platform layer) | Windows via `windows-rs`; other platforms out of scope |
| Extension IPC | `src/lib/vicinae-ipc` | Out of scope |
| Extension runtime | `src/lib/figura`, `src/typescript` | Out of scope |
| Calendar & UI helpers | `src/lib/glyph`, `src/inference` | Out of scope |

## Porting prohibitions
- Do not copy the Qt actor architecture (`QObject`/signal-slot) 1:1 — use Rust's built-in channels and `tokio`/`async`.
- Do not bring in Qt plugins/dependencies — equivalents exist in the Rust ecosystem.
- Do not copy Raycast's copyrighted UI text/icons — redesign the interface.
- Keep it as a *design and behavior* reference — not an art copy.

## Next steps
1. Window switcher (the last planned Phase 2 feature)
2. Optional clipboard image capture
3. Polish: ranking by frequency/recency
