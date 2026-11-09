<div align="center">

<img src="docs/assets/banner.png" alt="Sunny — a fast, lightweight launcher for Windows" width="100%" />

<br />

**A fast, lightweight launcher for Windows.** Press <kbd>Alt</kbd>+<kbd>Space</kbd> anywhere.

[![Release](https://img.shields.io/github/v/release/aliplfrh/sunny?style=flat-square&color=f5a623&label=release)](https://github.com/aliplfrh/sunny/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/aliplfrh/sunny/total?style=flat-square&color=f5a623)](https://github.com/aliplfrh/sunny/releases)
[![License](https://img.shields.io/badge/license-GPL--3.0-f5a623?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6?style=flat-square&logo=windows)](https://github.com/aliplfrh/sunny/releases/latest)
[![Built with](https://img.shields.io/badge/built%20with-Rust%20%2B%20Tauri%20v2-000000?style=flat-square&logo=rust)](https://tauri.app)

[**Download**](https://github.com/aliplfrh/sunny/releases/latest) · [Roadmap](docs/ROADMAP.md) · [Contributing](CONTRIBUTING.md) · [Agent guide](AGENTS.md)

</div>

---

Sunny is a command palette for Windows in the spirit of [Raycast](https://www.raycast.com/): one
keystroke away, keyboard-first, and fast enough that you stop thinking about it. It is a Rust port of
[Vicinae](https://github.com/vicinaehq/vicinae)'s core ideas — the behaviour is ported, the
implementation is new.

The executable is 10.5 MB: no Electron runtime bundled, no account, no telemetry. It renders through
the WebView2 already installed on Windows, so it costs a host process plus WebView2 helpers at
runtime rather than shipping its own browser engine — see [Memory footprint](#memory-footprint)
for the real numbers.

<br />

<div align="center">
  <img src="docs/assets/launcher-home.png" alt="Sunny's home view: Favorites above Suggestions ranked by usage" width="720" />
  <br />
  <sub><b>Favorites and Suggestions</b> — pinned apps first, then what you actually launch.</sub>
</div>

<br />

<div align="center">
  <img src="docs/assets/launcher-search.png" alt="Results grouped above Files when searching for Valorant" width="720" />
  <br />
  <sub><b>Grouped search</b> — Results always above Files, and Files-only when nothing else matches.</sub>
</div>

<br />

## Features

### 🔎 One search field for everything

Type and Sunny figures out what you mean. App names, file names, a math expression, or a command —
all in the same list, ranked by relevance. No modes to remember, no prefixes to learn.

- **Fuzzy app search** across everything in your Start Menu, with real icons
- **Grouped search** — apps and commands appear under **Results**, then matching **Files**;
  when no apps or commands match, Files is the first section
- **Favorites and Suggestions** — the home view shows pinned apps, then suggestions ranked by
  how often you successfully launch them; preferences and counts stay on your computer
- **Calculator** — type `12*7` and the answer is the first row; press <kbd>Enter</kbd> to copy it
- **Commands** — Clipboard History, Emoji Picker, Search Files, Settings. Each command appears
  only when you search for it; **Search Files** additionally stays hidden while
  *Include Files in Root Search* already covers it

### 📋 Clipboard history

Everything you copy, kept locally and searchable. A master–detail view shows the full content, size,
timestamp, and checksum before you paste.

- <kbd>Enter</kbd> pastes straight into the window you came from
- <kbd>Ctrl</kbd>+<kbd>B</kbd> copies without pasting, <kbd>Ctrl</kbd>+<kbd>D</kbd> deletes
- Pause recording at any time; history is capped and stored on disk

<div align="center">
  <img src="docs/assets/clipboard-history.png" alt="Clipboard history with preview and metadata" width="720" />
  <br />
  <sub>Search "Clipboard" and open the command. Sample entries shown.</sub>
</div>

### 😀 Emoji picker

Search for **Emoji**, open the **Emoji Picker**, then search 1,870 emoji by name.
<kbd>Enter</kbd> copies the one you picked. Typing `:` in a normal search does nothing special —
the picker is a command you choose, not a hidden mode.

<div align="center">
  <img src="docs/assets/emoji-picker.png" alt="Emoji picker searching for fire" width="720" />
</div>

### 🎨 A launcher that looks like it belongs

- **Theme** — System, Light, or Dark, applied to both the launcher and Settings
- **Accent color** — ten presets or any hex value; highlights, switches, and selections follow it
- **Microsoft Store apps** — not just Start Menu shortcuts: WhatsApp, Spotify, Netflix and friends
  are searchable and launchable too
- **Background-only opacity** — dim the backdrop without fading your text and icons
- **Blur** — soften the captured desktop behind the launcher without blurring text or icons
- **Compact or Expanded** window modes
- **Adjustable font size** and a bundled Inter typeface

### ⚙️ A real Settings window

Settings opens as its own window, with a sidebar and a search box — not a cramped panel inside the
palette. Every change applies live, and is saved atomically so a crash can never wipe your preferences.
About Sunny reads the running build version, including a separate release label when applicable.
**Open at Login** repairs missing startup registration when Sunny starts; login launches stay
in the background. A manual disable in Windows Startup apps is never silently overridden.

### 🔄 Updates itself

Sunny checks for a new release, verifies its signature, installs it, and restarts — from
**Settings → About Sunny**. No manual downloads after the first install.

### ⌨️ Keyboard first

Choose **Win + Space** in Settings → General → Sunny Hotkey to open Sunny after
releasing the combination. It is observed without blocking or replacing any keys;
Windows may also switch input languages. Other shortcuts still use native registration.
Reopening Sunny clears the previous search and returns to Favorites and Suggestions.

| Action | Shortcut |
|---|---|
| Show / hide Sunny | <kbd>Alt</kbd>+<kbd>Space</kbd> |
| Open Settings | <kbd>Ctrl</kbd>+<kbd>,</kbd> |
| Navigate results | <kbd>↑</kbd> <kbd>↓</kbd> |
| Open / run / copy | <kbd>Enter</kbd> |
| Emoji picker | Search **Emoji**, then <kbd>Enter</kbd> |
| Back to app search | <kbd>Backspace</kbd> |
| Reveal file in Explorer | <kbd>Ctrl</kbd>+<kbd>R</kbd> |
| Vim navigation (optional) | <kbd>Ctrl</kbd>+<kbd>J</kbd> / <kbd>Ctrl</kbd>+<kbd>K</kbd> |

<br />

## Install

1. Download **`Sunny_x.y.z_x64-setup.exe`** from the [latest release](https://github.com/aliplfrh/sunny/releases/latest).
2. Run it. Sunny installs **per-user** — no administrator prompt.
3. Press <kbd>Alt</kbd>+<kbd>Space</kbd>.

> **Requirements:** Windows 10 or 11 (x64). WebView2 is already present on current Windows builds.

<br />

## Status

See the [latest release](https://github.com/aliplfrh/sunny/releases/latest) for the published
version. About Sunny displays the version of the binary you are actually running.

| | Feature | State |
|:--:|---|:--:|
| 🔎 | App launcher with fuzzy search and real icons | ✅ |
| ⭐ | Persistent Favorites and usage-ranked Suggestions | ✅ |
| 🏪 | Microsoft Store apps (WhatsApp, Spotify, …) | ✅ |
| 📁 | File search, open or reveal in Explorer | ✅ |
| 🧮 | Calculator | ✅ |
| 😀 | Emoji picker (1,870 emoji) | ✅ |
| 📋 | Clipboard history with paste-to-active-window | ✅ |
| ⚙️ | Separate Settings window with search | ✅ |
| 🎨 | Theme: System / Light / Dark | ✅ |
| 🌈 | Accent color (presets or custom hex) | ✅ |
| 🔄 | Signed auto-update | ✅ |
| 🖥️ | System tray icon | ✅ |
| 🪟 | Window switcher | 🚧 planned |
| 🐧 | Linux and macOS builds | 🚧 planned |

See [docs/ROADMAP.md](docs/ROADMAP.md) for the full picture, including what is explicitly out of scope.

<br />

## Memory footprint

Sunny is small on disk, but it is honest to say it is **not** the smallest thing in RAM. The
executable is 10.5 MB, yet it renders through WebView2, so at runtime you own a host process plus
its Chromium helper processes.

Measured with **private working set** — the number Task Manager shows in its *Memory* column — on
Windows 11 x64, Sunny 1.0.5 freshly launched and idle, five samples:

| | Process count | Memory (private working set) |
|---|:--:|---|
| `sunny.exe` alone (what one Task Manager row shows) | 1 | **11.3 MB** |
| Sunny, full process tree (host + WebView2 helpers) | 8 | **114.6 MB** |

The single row is misleading. If you collapse Sunny in Task Manager you are only looking at the
Rust host; the seven `msedgewebview2.exe` children hold the rest.

### Compared with Raycast

| Launcher | Processes | Memory (Task Manager *Memory* column) |
|---|:--:|---|
| **Sunny** (full tree) | 8 | **114.6 MB** |
| Raycast (expanded row) | 10 | 341.9 MB |

Both readings are from Task Manager on the same machine; Raycast was already running and reported
341.9 MB across its 10 processes. On this comparison Sunny uses roughly **a third** of Raycast's
memory.

A fair caveat: I measured Sunny directly and sampled five times, but the Raycast figure is the total
its own Task Manager row reported in that session rather than a controlled re-run here. Treat it as
a same-machine observation, not a benchmark. Raycast may cache more, keep more extensions warm, or
behave differently after a fresh start.

### Why it is not lower

Electron-style runtimes share one constraint: the UI is HTML. Sunny avoids bundling a browser
engine — it uses the WebView2 that already ships with Windows — which is why the download is
10.5 MB instead of ~100 MB. Avoiding a bundled engine does not remove the cost of having *a*
engine, only the duplication.

<br />

## Build from source

**Prerequisites**

1. [Rust](https://rustup.rs) — `stable-x86_64-pc-windows-msvc`
2. [Node.js](https://nodejs.org) ≥ 20 (only for the Tauri CLI)
3. **Visual Studio Build Tools** with the *Desktop development with C++* workload (MSVC linker + Windows SDK)

   ```powershell
   winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

**Run it**

```powershell
cargo build --release
target\release\sunny.exe
```

**Build the installer**

```powershell
cd apps/sunny
npx @tauri-apps/cli@2 build        # → target/release/bundle/nsis
```

**Build a signed release** (installer + signature + `latest.json`)

```powershell
powershell -File scripts/release.ps1
```

**Regenerate assets**

```powershell
powershell -File scripts/make-icon.ps1     # icons from the logo pack
powershell -File scripts/make-emojis.ps1   # emojis.json from the gemoji dataset (MIT)
```

> UI files are embedded at compile time — rebuild after editing anything in `apps/sunny/ui/`.

<br />

## Architecture

Sunny is three layers, and each one only knows about the layer below it.

```
apps/sunny/ui          Vanilla HTML/CSS/JS — one bundle, two windows
        │  Tauri commands
apps/sunny/src-tauri   Rust: commands, windows, tray, hotkey, background threads
        │
crates/sun-core        Pure logic: search, apps, files, clipboard, emoji, calc, settings
        │
crates/sun-platform    Windows FFI: icons, screen capture, clipboard, SendInput, ShellExecuteW
```

| Crate | Responsibility |
|---|---|
| `apps/sunny` | Tauri v2 app — the launcher and Settings windows, global hotkey, tray |
| `crates/sun-core` | Fuzzy ranking, app index, file index, clipboard store, emoji, calculator, settings |
| `crates/sun-platform` | All `unsafe` Windows code, isolated behind a safe API |

No frontend framework, no bundler, no TypeScript. The UI is `index.html`, `style.css`, and `main.js`.

<br />

## Contributing

Contributions are welcome — bug reports, fixes, docs, and ideas.

- Read [**CONTRIBUTING.md**](CONTRIBUTING.md) for the process.
- Read [**AGENTS.md**](AGENTS.md) if you are contributing with an AI agent — it is the architecture
  contract, and it exists so an agent can be productive without breaking the design.

<br />

## Support the project

Sunny is free, open source, and has no ads, telemetry, or paid tier. If it saves you time, you can
buy me a coffee — entirely optional, always appreciated.

<div align="center">

<!-- Replace the link below with your own donation page. -->
[![Saweria](https://img.shields.io/badge/Saweria-Support%20Sunny-FFA500?style=for-the-badge&logoColor=white)](https://saweria.co/aliplfrh)
[![GitHub Sponsors](https://img.shields.io/badge/GitHub-Sponsor-ea4aaa?style=for-the-badge&logo=githubsponsors&logoColor=white)](https://github.com/sponsors/aliplfrh)

</div>

Other ways to help that cost nothing: **star the repo**, report bugs, and tell someone about it.

<br />

## License

Sunny is licensed under **GPL-3.0**. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

It is a derivative work of [Vicinae](https://github.com/vicinaehq/vicinae), which is also GPL-3.0.
All ported code remains under GPL-3.0.

<div align="center">
<br />
<sub>Built with Rust and Tauri. Not affiliated with Raycast or Vicinae.</sub>
</div>
