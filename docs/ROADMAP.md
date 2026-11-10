# Sunny Roadmap

Port of Vicinae's core features (C++23/Qt) → Rust + Tauri v2. Windows-only, GPL-3.0.

## Release v1.0.7 — Search correctness and responsiveness

- **Open hotkey bug:** Win+Space can pass Space to the foreground app and pause YouTube.
  An attempted Space-only filter was rolled back after live user testing opened Windows
  Start/Search instead. Pure routing tests did not model Windows shell behavior. Preserve
  each user's configured accelerator; do not ship another hook change without testing
  YouTube, normal typing, the Start menu/input switcher, and both key-release orders.
- Preserve the marked query through web-search activation; regression tests pass actual UI
  arguments into the Rust URL parser rather than accepting a permissive browser mock.
- Explicit `?` queries bypass local providers. Coalesce typing bursts with a 40 ms debounce,
  invalidating stale results immediately so Enter cannot launch an older query's result.
- Run file ranking and browser opening off the native UI thread. Publish apps before files,
  preserve keyboard selection when files arrive, and keep apps usable if file search fails.
- Share immutable app/file index snapshots instead of deep-copying every app icon on each
  query. Cache empty app scans as initialized; serialize cold scans and icon refresh scans.
- Bound Lorem output to at most 500 words for every unit, including extreme paragraph counts.

Reproducible synthetic benchmark: `cargo run --release -p sun-core --example search_benchmark`.
On the development machine, ranking 400 entries with 32 KB icons took 8.61 ms/query with the
old deep index copy versus 1.92 ms borrowing the index. Ranking 60,000 synthetic files took
23–36 ms median depending on query; this work previously ran on the native UI thread.
These are core microbenchmarks, not installed-app typing/opening latency measurements.

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
  - [x] Search **Emoji** and open its command; no automatic prefix-driven mode changes
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
  - [x] Keyboard: Vim navigation (`Ctrl+J` / `Ctrl+K`), Backspace-to-app-search toggle;
        emoji picker is an explicit command (legacy prefix field remains load-compatible)
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
- [x] Root search groups **Results** above **Files**, omits empty groups, and honors a zero file limit
- [x] Launcher home groups **Favorites** before **Suggestions**, persists favorite order and
      successful launch counts locally, and ranks suggestions by frequency then recency
- [x] Settings changes are serialized with error reporting; atomic saves preserve the previous
      file on failure instead of deleting it or falling back to non-atomic writes
- [x] Startup reconciles Open at Login with the executable's current path and launches hidden
      with `--autostart`; Windows Startup apps disables are respected
- [x] Capture hidden window client rectangles on a background thread after compositor completion;
      already-open windows only focus, and cancelling a pending show prevents delayed reopens
- [x] About reads the running binary version, with validated release-label metadata rather than
      a hardcoded UI version
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

### Backlog ideas (ordered easiest first)

Not committed work. Ordered by implementation cost; see the two blockers below before starting
anything that needs the network.

| # | Idea | Notes |
|:--:|---|---|
| 1 | **Quick web search** | Append `?` to open the query in a browser. `shell_open` already exists, and Sunny itself makes no request, so the network policy is untouched. |
| 2 | **Lorem ipsum generator** | Pure `sun-core` module, no dependencies. |
| 3 | **System commands** | Lock, shutdown, restart, sleep. Small surface, but destructive actions need an explicit confirmation step. |
| 4 | **Windows & Control Panel settings search** | No enumerable API for `ms-settings:`; the page list must be curated by hand. Data work, not code work. |
| 5 | **QR code generator** | Needs a QR crate; custom styling (colors, module shape, center logo) means rendering SVG directly. |
| 6 | **Currency conversion and world clock** | World clock is pure timezone math. Currency needs an API, caching, and a parser for `1 usd to rupiah`. Free rate sources are typically daily, not real-time. |
| 7 | **YouTube download via OmniGet** | Delegates to the external `omniget-cli` instead of bundling yt-dlp and FFMPEG. Estimated 5–8 days; see the dedicated section below. |

### YouTube download via OmniGet (planned, unverified)

Sunny would not download video itself. It would detect a video URL, offer resolution
choices, and run the separately installed `omniget-cli` as a child process, reading its
progress output. OmniGet is Tauri 2 + Rust under GPL-3.0, so the stack and licence match.

Spike first: `omniget-cli` ships in every OmniGet release, but its progress output has not
been checked for machine parsing here. Estimated total **5–8 days**, broken down as:

| Step | Estimate |
|---|:--:|
| Spike: fetch `omniget-cli`, confirm `omniget info <url>` lists qualities and that progress lines are parseable | 0.5 day |
| Detect installed CLI, pin a minimum version, and explain how to install it when missing | 0.5–1 day |
| `sun-core` URL/format parsing with tests | 1 day |
| Job subsystem: spawn, stream progress, cancel, clean up child processes | 1.5–2 days |
| Launcher UI: quality picker, per-download progress, cancel, and error states | 1.5–2 days |
| Optional integration instead of a hard dependency; documentation and disclosure | 1 day |

Decisions needed before starting:

- **Not bundled.** The installer must stay ~3.4 MB; ffmpeg and yt-dlp remain OmniGet's job.
- **Configuration, not code.** Prefer calling the CLI; do not compile OmniGet's plugins, which are
  pinned to their toolchain and would tie Sunny's releases to OmniGet's.
- **Legal position.** Sunny would pass a URL to a tool the user installed rather than process the
  video itself, but the repository is public and the feature still accepts YouTube URLs. State this
  plainly in the README and make the feature opt-in.
- **Network policy.** Invoking an external tool is not a request from Sunny, so `AGENTS.md` is not
  technically violated, but its intent shifts. Add an explicit clause covering delegated tools.

### Blockers for network-backed ideas

1. **Network policy.** `AGENTS.md` forbids network calls other than the update check. Currency
   conversion needs a documented allowlist, CSP restrictions (currently `csp: null`), timeouts,
   offline caching, and a decision about whether user queries may leave the machine — that is a
   privacy change for a project that advertises no telemetry.
2. **Long-running job subsystem.** Only the updater reports progress today. Future background
   work needs progress, cancellation, and cleanup instead of ad-hoc threads.

### Refactor queues ahead of the backlog

- `refresh()` in `main.js` is a five-branch `if/else` chain. Each answer type added inline makes it
  harder to test; introduce an answer-provider abstraction before adding a third provider.
- Releases are built and signed locally because GitHub Actions is blocked on an account billing
  issue. Restore CI before the release cadence grows.

### Considered and rejected

- **Internet speed test** — rejected. Needs external servers, careful throughput measurement, and a
  steady-state UI that does not fit a query-driven launcher.
- **Bundling YouTube download with FFMPEG in Sunny** — rejected. It would grow the installer from
  ~3.4 MB to roughly 80–120 MB and tie Sunny to a constantly breaking extractor. Delegating to an
  external tool (see the OmniGet entry above) is the alternative under consideration.

## Release v1.0.6 — Quick web search and Lorem ipsum

- **Quick web search** — end a query with `?` (or start it with one) to search the web instead of
  the local index, for example `rust ownership?`. Choose the engine in **Settings → Launcher →
  Quick Web Search**; Google, DuckDuckGo, Bing, YouTube, GitHub, Stack Overflow, and Wikipedia are
  included. Sunny builds the URL and hands it to your browser, so the app itself makes no request.
- **Lorem ipsum** — `lorem`, `lorem 5 sentences`, `lorem 3 paragraphs`, or `lorem 20 words`
  produces text you can copy with <kbd>Enter</kbd>. Deterministic and bounded, so a large request
  cannot stall the launcher.

## Release v1.0.5 — Search Files command

- With **Include Files in Root Search** enabled, the **Search Files** command no longer appears
  for unrelated queries (it duplicated the Files section); it still appears when the query names
  it, such as `search files` or `files`.
- With root file search disabled, the command is always available.
- The **File Results** stepper is labelled in files instead of pixels.

## Release v1.0.4 — Launcher UI fixes

- Launcher home shows **Favorites** then **Suggestions**, ranked by successful launch
  frequency then recency; preferences and counts stay in `launcher-history.json`.
- Root search groups **Results** above **Files**, hides empty groups, and honors a zero file limit.
- The Emoji Picker opens only from its command; the legacy prefix no longer changes modes.
- Opacity and blur use the real client-area capture; Settings no longer floors opacity at 90%.
- About reads the running binary version instead of a hardcoded label.
- Open at Login reconciles the current executable path on every start and launches hidden with
  `--autostart`; a disable in Windows Startup apps is surfaced instead of overridden.
- Pending window shows are cancellable, so closing during a slow capture cannot reopen a window.
- Settings saves are serialized and atomic, with native state rolled back on failure.

## Release v1.0.2.1 — Bug fixes

- The GitHub tag is `v1.0.2.1`; the application and updater use SemVer `1.0.3` so
  existing `1.0.2` installations recognize it as an update.
- Clear previous launcher queries and results on reopen, including late async responses.
- Exclude hidden Windows package entries such as File Picker (`1527…`).
- Offer Win+Space through a passive observer, with no suppressed or synthesized key events.
  Windows may still switch input languages. Live desktop hotkey testing is not yet performed.
- Allow the release script and workflow to use a tag distinct from the internal version.
