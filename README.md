# Sunny

*Sunny* is a fast, lightweight desktop launcher / command palette inspired by [Raycast](https://www.raycast.com/), and a Rust port of [Vicinae](https://github.com/vicinaehq/vicinae) (C++/Qt) core features. Press `Alt+Space` anywhere to summon it. Press `Ctrl+,` for Settings.

## Project Status

🚧 **Phase 2 — Core Vicinae features** is in progress. See [docs/ROADMAP.md](docs/ROADMAP.md).

| Feature | State |
|---|---|
| App launcher with fuzzy search + real icons | ✅ |
| Calculator (`12*7` → Enter copies) | ✅ |
| Emoji picker (`:` prefix, 1,870 emoji) | ✅ |
| Clipboard history (master–detail, metadata, pause, paste to active window) | ✅ |
| File search — fuzzy index of your files, open or reveal in Explorer | ✅ |
| Settings — tabbed panel (General, Launcher, Shortcuts, Keyboard, Advanced, About) with search | ✅ |
| System tray icon | ✅ |
| Window switcher | ⏭️ planned |

## Architecture

| Component | Description |
|---|---|
| `apps/sunny` | Tauri v2 application — web UI (Raycast-like interface) + Rust backend, global hotkey, tray |
| `crates/sun-core` | Search engine (fuzzy), ranking, calculator, application index, file index, emoji, clipboard store, settings |
| `crates/sun-platform` | Windows platform abstraction: global hotkey, window helpers, clipboard sequence, icon extraction, screen capture, paste injection |

## Platform

**Windows only.** Sunny targets Windows 10/11; Linux and macOS are out of scope.

## Build (Windows)

**Prerequisites:**
1. [Rust](https://rustup.rs) (toolchain `stable-x86_64-pc-windows-msvc`)
2. [Node.js](https://nodejs.org) ≥ 20
3. **Microsoft Visual Studio Build Tools** with the *Desktop development with C++* workload (MSVC linker + Windows SDK). For machines without VS: `winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
4. Run `cargo` from a regular terminal — because Git for Windows' `link.exe` can appear in PATH, this repo uses `.cargo/config.toml` to explicitly select the MSVC linker.

**Commands:**

```sh
cargo build --release                    # from the repo root
target\release\sunny.exe                 # run (Alt+Space to show/hide)
```

**Regenerating assets:**

```sh
powershell -File scripts/make-icon.ps1    # icons/icon.ico + icon.png from the logo pack
powershell -File scripts/make-emojis.ps1  # emojis.json from the gemoji dataset (MIT)
```

## License

GPL-3.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

Sunny is a derivative work of Vicinae, which is licensed under GPL-3.0. All ported code is licensed under GPL-3.0.
