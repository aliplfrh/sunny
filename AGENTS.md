# AGENTS.md

Sunny is a **Raycast-style desktop launcher for Windows**, written in **Rust + Tauri v2** with a
**vanilla HTML/CSS/JS** frontend. It is a GPL-3.0 port of [Vicinae](https://github.com/vicinaehq/vicinae)
(C++/Qt) — the *behaviour* is ported, the *implementation* is not.

This file is the contract for any AI agent (or human) contributing to this repository. Read it before
touching code. For everything else, read the codebase.

---

## 1. Non-negotiable constraints

These are architectural decisions, not preferences. Do **not** change them without an explicit
maintainer decision recorded in an issue.

| Constraint | Value | Why |
|---|---|---|
| Language (backend) | **Rust** (edition 2021, stable MSVC toolchain) | Safety, small binaries |
| App framework | **Tauri v2** | Lightweight WebView2 shell, native window control |
| Frontend | **Vanilla HTML/CSS/JS** — no bundler, no framework, no TypeScript | Zero build step; `frontendDist: "../ui"` |
| Target OS | **Windows 10/11 only** (for now) | Platform code is Windows-specific |
| License | **GPL-3.0** with NOTICE attribution to Vicinae | Derivative work |
| UI language | **English** (code, comments, commits, docs) | Consistency |

**Do not** introduce React, Vue, Svelte, Vite, Webpack, Tailwind, npm dependencies, or a
`package.json`-driven build. The UI is three files: `index.html`, `style.css`, `main.js`.

**Do not** add a second webview framework, a Node sidecar, or an extension runtime. Those are
explicitly out of scope (see `docs/ROADMAP.md`).

---

## 2. Repository layout

```
apps/sunny/                 Tauri application
  src-tauri/
    src/main.rs             ALL Tauri commands, window/tray/hotkey wiring, background threads
    tauri.conf.json         Window definitions (main + settings), updater config, bundle config
    capabilities/default.json  Tauri v2 permissions for BOTH windows
    icons/                  App + tray icons
  ui/
    index.html              Single document, serves both windows
    style.css               All styling, theme variables
    main.js                 All UI logic, serves both windows
crates/sun-core/            Pure Rust logic, no Tauri, no Windows APIs
  src/search.rs             Fuzzy ranking (SkimMatcherV2)
  src/apps.rs               Start Menu .lnk scan + icon cache
  src/files.rs              File index + fuzzy file search
  src/clipboard.rs          Clipboard history store
  src/emoji.rs              Emoji dataset
  src/calc.rs               Calculator (meval)
  src/settings.rs           Settings struct, load/save/normalize
crates/sun-platform/        Windows FFI only (windows-rs)
  src/lib.rs                Icon extraction, screen capture, clipboard, SendInput, ShellExecuteW, COM
docs/                       ROADMAP.md, PORT-MAP.md, assets/
scripts/                    Asset generation + release helper
```

### Layering rule

```
ui (JS)  →  main.rs (Tauri commands)  →  sun-core (logic)  →  sun-platform (OS)
```

- `sun-core` must **never** depend on `tauri` or `windows`.
- `sun-platform` must **never** depend on `tauri` or `sun-core`.
- `main.rs` is the only place that knows about both Tauri and the crates.
- The UI must **never** call OS APIs directly; it goes through a Tauri command.

If you need a new OS capability, add it to `sun-platform`, expose it through a `#[tauri::command]`
in `main.rs`, then call it from `main.js`.

---

## 3. The two-window model

One UI bundle serves **two** Tauri windows:

| Window | Label | URL | Purpose |
|---|---|---|---|
| Launcher | `main` | `index.html` | The palette itself |
| Settings | `settings` | `index.html?window=settings` | Separate settings window |

`main.js` detects its role at startup:

```js
const IS_SETTINGS_WINDOW =
  new URLSearchParams(location.search).get("window") === "settings";
```

Rules:

- **Never** assume a DOM element exists in both windows. Guard with `?.` or `if (IS_SETTINGS_WINDOW)`.
- Settings changes are broadcast with `app.emit("sunny:settings-updated", &settings)`; **both** windows
  listen and re-apply. If you add a setting, make sure the launcher reacts live — do not require a restart.
- The Settings window is created hidden (`visible: false`) and only shown by `settings_open`.
  Closing it **hides** it (`CloseRequested` is intercepted); it is never destroyed.
- Backdrops are per-window: `BackdropState` (launcher) and `SettingsBackdropState` (settings).
  Do not merge them — the capture rectangles differ.

---

## 4. Rust rules

- **No `unwrap()` on user-reachable paths.** Mutex locks in command handlers are the one accepted
  exception (a poisoned lock means a prior panic; the app is already broken). Prefer `?`, `if let`,
  `let ... else`.
- **No panics in background threads.** A panic there kills a feature silently.
- **Never hold a `MutexGuard` across an `.await`.** Clone the data out first.
- **Blocking work goes to `std::thread::spawn`**, not the command handler. File indexing, icon
  extraction, and screen capture are all blocking.
- **Settings writes are atomic**: temp file + rename (`Settings::save`). Do not replace this with a
  plain `fs::write` — a crash mid-write would reset the user's preferences.
- **`unsafe` lives only in `sun-platform`.** Every `unsafe` block needs a comment explaining the
  invariant. Free every Win32 handle you acquire (`DestroyIcon`, `DeleteObject`, `CloseHandle`,
  `CoUninitialize`).
- **Errors**: commands return `Result<T, String>` with a human-readable message. The UI shows it.
- **Formatting**: `cargo fmt --all` before every commit. **Lints**: `cargo clippy` should stay clean
  for new code.

### Naming

- Types: `PascalCase`. Functions/vars: `snake_case`. Constants: `UPPER_SNAKE_CASE`.
- Tauri commands: `snake_case` (they become the JS function name verbatim).
- Settings fields: `snake_case` in Rust, identical in JSON and in JS (`SETTINGS.include_files`).

---

## 5. Frontend rules

- **No framework, no build step.** If you need a component, write a function that returns a DOM node.
- **Never inject untrusted text with `innerHTML`.** File names, clipboard contents, and app names are
  untrusted. Use `textContent`, or `escapeHtml()` when markup is genuinely required.
- **All state lives in module-level variables** (`SETTINGS`, `mode`, `selected`, `items`). There is no
  store. Keep mutations explicit and call `render()` / `refresh()` afterwards.
- **Debounce disk writes.** Settings changes go through `onSettingChange(patch, immediate)`; the
  non-immediate path debounces 300 ms and flushes on blur/unload.
- **Theme via CSS variables only.** Never hardcode a colour in a rule; add a variable in `:root` and
  override it in `body.theme-light`. Both windows must look correct in light and dark.
- **The Sunny logo is pure white.** On light backgrounds it must be inverted with
  `filter: brightness(0)` (see `body.theme-light .logo`). Any new place you put the logo needs the
  same treatment.
- **Keyboard first.** Every action must be reachable without a mouse. New bindings go in the
  `keydown` handler and must be documented in the Shortcuts tab.

---

## 6. Adding a feature (checklist)

1. **Logic** → `crates/sun-core` with unit tests.
2. **OS access** (if any) → `crates/sun-platform`, with handle cleanup.
3. **Command** → `#[tauri::command]` in `main.rs`, registered in `generate_handler!`.
4. **Permission** → add the needed capability to `capabilities/default.json` if it is a plugin command.
5. **UI** → `main.js` + `style.css`; works in both windows; light and dark.
6. **Settings** (if user-configurable) → field in `Settings` + `normalize()` + a row in the right tab.
7. **Docs** → update `docs/ROADMAP.md`; update `README.md` if it is user-visible.
8. **Verify** → `cargo fmt --all`, `cargo test -p sun-core`, `cargo build --release`, run the app.

---

## 7. Build & run

```powershell
cargo build --release
target\release\sunny.exe
```

- UI assets are **embedded at compile time**. Any change to `ui/` requires a rebuild — editing the
  files on disk does nothing to a running binary.
- The build pipeline exits with code 1 even on success in some shells; check for `Finished` in the
  output rather than the exit code.
- Installer: `cd apps/sunny; npx @tauri-apps/cli@2 build` (NSIS, per-user).
- Full signed release: `powershell -File scripts/release.ps1`.

### Debugging the UI

Launch with the WebView2 remote debugging port and attach over CDP:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
Start-Process target\release\sunny.exe
# then: http://127.0.0.1:9222/json lists the page targets
```

Two page targets exist — `http://tauri.localhost/` (launcher) and
`http://tauri.localhost/index.html?window=settings` (settings). Target them explicitly.

---

## 8. Release & auto-update

- Version lives in **two** places and must match: `apps/sunny/src-tauri/tauri.conf.json` (`version`)
  and `Cargo.toml` (`[workspace.package] version`).
- Releases are signed with a minisign key. The **public** key is in `tauri.conf.json`
  (`plugins.updater.pubkey`); the **private** key is never committed.
- `latest.json` must be written **without a UTF-8 BOM** — the updater fails to decode a BOM-prefixed
  manifest. Both `scripts/release.ps1` and the GitHub workflow handle this; keep it that way.
- Publishing: push a `v*` tag (CI) or run `scripts/release.ps1` then `gh release create`.

---

## 9. What not to do

- Do not add a snippet/text-expander, an extension system, AI chat, or a store. Out of scope.
- Do not copy Vicinae's QML or Raycast's assets/text. Port behaviour, redesign the UI.
- Do not add telemetry, analytics, or network calls other than the update check.
- Do not commit `latest.json`, `target/`, `reference/`, or the updater private key.
- Do not "fix" the opacity implementation by making the window transparent — opacity is a desktop
  snapshot + tint overlay so that text and icons stay fully opaque.
- Do not reformat unrelated files in a feature commit.

---

## 10. AI-generated contributions

AI-generated code is held to exactly the same standard as hand-written code. A PR that was mostly
AI-generated should say which model/tool was used. Untested, unreviewed AI output will be rejected.
Keep PR descriptions short.
