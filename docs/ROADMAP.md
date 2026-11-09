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
  - [x] **Separate Settings window** (like Raycast): its own frameless window with an integrated
        titlebar (logo, title, minimize/maximize/close), opened with `Ctrl+,` or the tray menu;
        closing it only hides it, so reopening is instant
  - [x] Tabbed panel with a Raycast-style sidebar: **General**, **Launcher**, **Shortcuts**,
        **Keyboard**, **Advanced**, **About Sunny**, plus a settings search box
  - [x] Consistent inline-SVG tab icons (Launcher uses the Sunny mark)
  - [x] **Theme** — System / Light / Dark, applied to both the launcher and the Settings window
  - [x] Opacity and blur are previewed inside the Settings window too (desktop snapshot backdrop)
  - [x] General: Open at Login (`HKCU\...\Run`), Show in System Tray, Sunny Hotkey
        (click-to-capture, requires a modifier, re-registers the global shortcut)
  - [x] Launcher: Font — **Sonora (Inter)** default or System; Font size 12–20 px;
        Opacity 40–100 % — **background only** (desktop snapshot + tint; text/icons stay
        opaque); Blur UI (blurs the snapshot); Window mode — **Raycast Compact** /
        **Raycast Expanded**; Show footer hints
  - [x] Shortcuts: read-only reference of every global and in-launcher binding
  - [x] Keyboard: Vim navigation (`Ctrl+J` / `Ctrl+K`), Backspace-to-app-search toggle,
        configurable emoji prefix
  - [x] Advanced: open the settings folder, reset all settings, file-index status and rebuild
  - [x] About Sunny: version, license, bundled font, emoji data source, platform,
        **Check for Updates** (signed auto-update via `tauri-plugin-updater`)
  - [x] Live preview while dragging; tray menu entry and `Ctrl+,`; search-only `Settings` command
  - [x] Reliable persistence: single-instance guard (no competing writers) and pending-save flush
        when the panel closes or the window hides
- [x] **Auto-update** — signed NSIS updater artifacts published by GitHub Actions
  - [x] `latest.json` manifest at `releases/latest/download/latest.json`
  - [x] In-app check, download with progress, install and relaunch
  - [x] Release workflow (`.github/workflows/release.yml`) triggered by a `v*` tag
- [x] **Root file search** — files appear directly in the main search field
      (`include_files`, `file_results`), no need to open the "Search Files" command
- [x] **Clean app icons** — `.lnk` shortcuts are resolved to their target executable before icon
      extraction, so the shortcut-arrow overlay badge is gone
- [x] **Microsoft Store apps** — AppX/MSIX packages from the AppModel registry are merged into the
      app index (manifest parsing, `Square44x44Logo` variant resolution, launch through
      `IApplicationActivationManager`), so Store-only apps like WhatsApp are searchable
- [x] **Result categories** — app rows show what they are: *Application*, *Game*, or
      *Windows Settings* (folder- and name-based detection)
- [x] Respect `AppListEntry="none"` in Store manifests, excluding hidden Windows components
      such as File Picker instead of showing their package identifiers as applications
- [x] Reopening the launcher clears its query, results, selection and specialized search mode;
      outstanding requests are invalidated so stale results cannot reappear
- [x] **Win + Space option** — passive observation after key release, without suppressing or
      injecting keyboard input; Windows may still switch input languages. Explicit preset in
      General and Shortcuts; other accelerators keep native global registration
- [x] **Accent color** — Launcher → Accent Color: ten curated presets, a native color picker, and a
      hex field; all highlights, switches, and selection bars derive from it (`--accent*` variables)
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

## Planned Later

- Window switcher
- Linux and macOS builds (the `sun-platform` abstraction is kept clean so a port stays
  possible; Windows remains the only supported target for now)

## Release v1.0.2.1 — Bug fixes

- The GitHub tag is `v1.0.2.1`; the application and updater use SemVer `1.0.3` so
  existing `1.0.2` installations recognize it as an update.
- Clear previous launcher queries and results on reopen, including late async responses.
- Exclude hidden Windows package entries such as File Picker (`1527…`).
- Offer Win+Space through a passive observer, with no suppressed or synthesized key events.
  Windows may still switch input languages. Live desktop hotkey testing is not yet performed.
- Allow the release script and workflow to use a tag distinct from the internal version.
