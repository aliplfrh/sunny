# Sunny Roadmap

Full port of Vicinae (C++23/Qt) → Rust + Tauri v2. Windows-first, GPL-3.0.

## Phase 0 — Foundation ✅
- [x] License decision: full port → GPL-3.0 (with NOTICE attribution to Vicinae)
- [x] UI stack decision: Tauri v2 (WebView2) for a Raycast-like appearance
- [x] Platform decision: Windows first
- [x] Repo, LICENSE, NOTICE, README, ROADMAP, PORT-MAP
- [x] Scaffold Cargo workspace (`crates/sun-core`, `crates/sun-platform`)
- [x] Scaffold Tauri application (`apps/sunny`, opaque overlay window)
- [x] Shallow-clone Vicinae repo as a porting reference (read-only, gitignored)
- [x] Raycast-style overlay window: frameless, always on top, centered, global hotkey (default: Alt+Space), close on Esc / focus loss

## Phase 1 — MVP (usable launcher) ✅
- [x] Fuzzy search engine (SkimMatcherV2 — concept ported from Vicinae)
- [x] Windows application index (recursive Start Menu `.lnk` scan)
- [x] In-memory app cache (no disk IO per keystroke)
- [x] Real application icons (`SHGetFileInfoW` → `HICON` → PNG, disk cache)
- [x] Calculator (`meval`; Enter copies the result)
- [x] Raycast-style list view UI (display + keyboard bindings ↑↓, Enter, Esc)
- [x] Application launching (proper process spawning)
- [x] System tray icon (left-click toggle, menu Show/Quit)
- [x] Release build: ~6 MB binary, ~28 MB RAM
- [ ] Run at startup (optional)

## Phase 2 — Core Vicinae Features 🚧
- [x] **Clipboard history** — polling watcher (`GetClipboardSequenceNumber`, 400 ms), no hooks
  - [x] Text capture, dedupe (existing entry moves to top), max 100 entries
  - [x] Disk persistence (`%LOCALAPPDATA%\Sunny\clipboard-history.json`)
  - [x] Entry metadata: type, size (bytes), MD5 checksum (backfilled for legacy entries)
  - [x] Raycast-style master–detail UI: entry list + preview/metadata pane
  - [x] Header: item count, **Pause clipboard** (the watcher genuinely stops recording)
  - [x] Type filter (`All` / `Text`) next to the search field
  - [x] **Paste to active window** (Enter) — sets clipboard, hides launcher, injects `Ctrl+V`
  - [x] Delete selected entry (`Ctrl+D`); copy without pasting (`Ctrl+B`)
  - [ ] Image / file clipboard capture (extension point already modelled via `kind`, `width`, `height`)
- [x] **Emoji picker** — 1,870 entries generated from gemoji (MIT)
  - [x] `:` prefix enters emoji mode; search by English name + extra keywords
  - [x] Enter copies the emoji
- [x] **Search-only commands** — internal commands (Clipboard History, Emoji Picker) are no longer
      pinned to the default list; they appear only when searched
- [x] **Branding** — Sunny logo pack wired in: multi-size `.ico` (app icon), custom tray icon, UI logo
- [ ] Text expander / snippet — *intentionally dropped (out of scope for this phase)*
- [ ] File search (fast index like Everything/ripgrep)
- [ ] Window switcher

## Phase 3 — Extension System ⏭️
- [ ] Design Sunny's own extension API
- [ ] Runtime for Vicinae `.figura` extensions (mapped from the upstream `figura/` runtime)
- [ ] Compatibility with a subset of Raycast script commands
- [ ] (Optional/gradual) Raycast React extension compatibility

## Phase 4 — Feature Parity with Vicinae ⏭️
- [ ] Extension store
- [ ] AI chat / dictation (integration stub)
- [ ] Volume control, font browser, etc.
- [ ] Linux (wlr/X11) and macOS

## Porting Principles
1. **The UI is rewritten, not translated**: QML → Raycast-like web UI; C++ logic → idiomatic Rust (safe, without line-by-line 1:1 equivalence).
2. **Vicinae references are mapped** in `docs/PORT-MAP.md` — each module lists its upstream source file.
3. **Windows first**, with abstractions in `sun-platform` so Linux/macOS can follow without a major refactor.
4. Each phase must deliver a *usable* application, not a half-finished one.
