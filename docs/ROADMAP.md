# Sunny Roadmap

Port of Vicinae's core features (C++23/Qt) → Rust + Tauri v2. Windows-only, GPL-3.0.

## Phase 0 — Foundation ✅
- [x] License decision: port of Vicinae core features → GPL-3.0 (with NOTICE attribution to Vicinae)
- [x] UI stack decision: Tauri v2 (WebView2) for a Raycast-like appearance
- [x] Platform decision: Windows only
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
- [x] Run at startup (Settings → Open at Login)

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
- [x] **Search-only commands** — internal commands (Clipboard History, Emoji Picker, Search Files,
      Settings) are no longer pinned to the default list; they appear only when searched
- [x] **Branding** — Sunny logo pack wired in: multi-size `.ico` (app icon), custom tray icon, UI logo
- [x] **File search** (Raycast-style "Search Files")
  - [x] Background index of the user profile (Desktop, Documents, Downloads, Pictures, Music,
        Videos + home) with depth/size limits and skipped dev/system folders
  - [x] Fuzzy name matching (`SkimMatcherV2`) with path fallback, top 60 results
  - [x] Dedicated file mode: type-aware folder/file glyphs, parent path in the row, live indexing state
  - [x] `Enter` opens with the default app; `Ctrl+R` reveals the file in File Explorer
  - [x] Index built once per launch on a background thread; UI never blocks on disk IO
- [x] **Settings** (Raycast/Sonora-inspired), persisted to `%LOCALAPPDATA%\Sunny\settings.json`
  - [x] General: Open at Login (`HKCU\...\Run`), Show in System Tray, Sunny Hotkey
        (click-to-capture, requires a modifier, re-registers the global shortcut)
  - [x] Appearance: Font — **Sonora (Inter)** default or System; Font size 12–20 px
  - [x] Window: Opacity 40–100 % — **background only** (desktop snapshot + tint; text/icons stay
        opaque), Blur UI (blurs the snapshot), Window mode — **Raycast Compact** / **Raycast Expanded**
  - [x] Live preview while dragging; tray menu entry and `Ctrl+,`; search-only `Settings` command
  - [x] Reliable persistence: single-instance guard (no competing writers) and pending-save flush
        when the panel closes or the window hides
- [ ] Window switcher

## Porting Principles
1. **The UI is rewritten, not translated**: QML → Raycast-like web UI; C++ logic → idiomatic Rust (safe, without line-by-line 1:1 equivalence).
2. **Vicinae references are mapped** in `docs/PORT-MAP.md` — each module lists its upstream source file.
3. **Windows only**, with abstractions in `sun-platform` kept clean for maintainability.
4. Each phase must deliver a *usable* application, not a half-finished one.

## Out of Scope
Sunny is a focused, lightweight launcher. The following upstream Vicinae features are
**not planned**:

- Text expander / snippet
- Extension system (Vicinae `.figura` runtime, Raycast extension compatibility, extension store)
- AI chat / dictation
- Linux and macOS builds
