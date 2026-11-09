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
| Settings — separate window, tabbed (General, Launcher, Shortcuts, Keyboard, Advanced, About) with search | ✅ |
| Theme — System / Light / Dark | ✅ |
| Auto-update — signed releases, in-app check & install | ✅ |
| System tray icon | ✅ |
| Window switcher | ⏭️ planned |
| Linux and Mac Support | ⏭️ planned |

## Architecture

| Component | Description |
|---|---|
| `apps/sunny` | Tauri v2 application — web UI (Raycast-like interface) + Rust backend, global hotkey, tray |
| `crates/sun-core` | Search engine (fuzzy), ranking, calculator, application index, file index, emoji, clipboard store, settings |
| `crates/sun-platform` | Windows platform abstraction: global hotkey, window helpers, clipboard sequence, icon extraction, screen capture, paste injection |

## Platform

**Windows only for now.** Sunny targets Windows 10/11. Linux and macOS are planned later;
the `sun-platform` crate keeps the OS-specific code isolated so a port stays possible.

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

**Installer:**

```sh
cd apps/sunny
npx @tauri-apps/cli@2 build               # NSIS installer in target/release/bundle/nsis
```

Or, from the repo root, build + sign + generate `latest.json` in one step:

```powershell
powershell -File scripts/release.ps1
```

**Regenerating assets:**

```sh
powershell -File scripts/make-icon.ps1    # icons/icon.ico + icon.png from the logo pack
powershell -File scripts/make-emojis.ps1  # emojis.json from the gemoji dataset (MIT)
```

## Releasing & Auto-Update

Sunny updates itself through [`tauri-plugin-updater`](https://tauri.app/plugin/updater/). Releases are built and
signed by `.github/workflows/release.yml`:

1. Bump `version` in `apps/sunny/src-tauri/tauri.conf.json` (and `[workspace.package]` in `Cargo.toml`).
2. Push a tag: `git tag v1.0.1 && git push origin v1.0.1`.
3. The workflow builds the NSIS installer, signs it with `TAURI_SIGNING_PRIVATE_KEY`, writes
   `latest.json`, and publishes both to the GitHub release.

The app polls `https://github.com/aliplfrh/sunny/releases/latest/download/latest.json`; the public key that
verifies the signature lives in `tauri.conf.json` under `plugins.updater.pubkey`.

**Required repository secrets:**

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the minisign private key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Key password (empty if the key has none) |

> Keep the private key backed up. If it is lost, existing installs can no longer verify new updates and
> users must reinstall manually.

## License

GPL-3.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

Sunny is a derivative work of Vicinae, which is licensed under GPL-3.0. All ported code is licensed under GPL-3.0.
