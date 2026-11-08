# Peta Port: Vicinae → Sunny

Referensi upstream: https://github.com/vicinaehq/vicinae (GPL-3.0, 2.600+ commit)

Ukuran kode upstream (per Nov 2026):
| Bahasa | Ukuran | Fungsi |
|---|---|---|
| C++23 | ±6,8 MB | Core, daemon, framework, modul bawaan |
| QML | ±715 KB | Seluruh UI (akan ditulis ulang sebagai tampilan web mirip Raycast) |
| TypeScript/JS | ±330 KB | Runtime & API ekstensi (frontend), alat build ekstensi |
| Obj-C++/Swift/CMake/Shell | sisanya | macOS, build scripts |

## Peta modul berikut diteruskan (dilengkapi setelah clone referensi upstream)

Struktur upstream (shallow clone di `reference/vicinae`, per Nov 2026):

`src/`: `browser-extension, cli, data-control-server, file-indexer, inference, lib, server, snippet, typescript, wayland-protocols`

`src/lib/`: `common, crypto, figura, fuzzy, glyph, linux-utils, script-command, soulver, vicinae-ipc, xdgpp`

| Modul Sunny | Asal di Vicinae | Strategi |
|---|---|---|
| `apps/sunny` (UI) | UI QML vicinae | DITULIS ULANG — web UI bertampilan Raycast (WebView2, bukan Qt Quick) |
| `apps/sunny` (backend) | `src/server`, `src/main*` | Rust + Tauri v2: jendela frameless, overlay, hotkey, tray |
| `crates/sun-core` | `src/lib/common`, search/ranking, settings, storage | Port ke Rust sesuai semantik (bukan baris per baris) |
| `crates/sun-core: fuzzy` | `src/lib/fuzzy` | Perilaku dipetakan ke `SkimMatcherV2` (fuzzy-matcher) — ✅ scaffold awal |
| `crates/sun-core: apps` | app service (app scanner/launcher) | Windows: enumerasi .lnk Start Menu — ✅ scaffold awal |
| `crates/sun-core: calculator` | `src/lib/soulver` (backend Soulver) | Rust: `meval`/`evalexpr` untuk Fase 1 |
| `crates/sun-core: snippets` | `src/snippet` | Fase 2 |
| `crates/sun-core: file-indexer` | `src/file-indexer` | Fase 2 (file search) |
| `crates/sun-platform` | `src/lib/linux-utils`, `xdgpp` (platform layer) | Windows melalui `windows-rs`; Linux menyusul (Wayland/X11) |
| IPC ekstensi | `src/lib/vicinae-ipc` | Sistem IPC Sunny sendiri (desain lebih ringan) |
| Extension runtime | `src/lib/figura`, `src/typescript` | **Fase 3** — evaluasi: Node sidecar vs QuickJS |
| Kalender & UI-helper | `src/lib/glyph`, `src/inference` | Fase 4 / sesuai kebutuhan |

## Prohibisi saat porting
- Jangan tiru arsitektur actor Qt (`QObject`/signal-slot) 1:1 — pakai kanal (channel) & `tokio`/`async` bawaan Rust.
- Jangan bawa plugin/dependensi Qt — semuanya ada padanannya di ekosistem Rust.
- Jangan salin teks UI/ikon berhak cipta dari Raycast — tampilan disusun ulang.
- Simpan sebagai referensi *desain & perilaku* — bukan salinan art.

## Langkah berikutnya
1. Clone shallow vicinae → `reference/vicinae` (hanya untuk baca, di-.gitignore)
2. Katalogisasi struktur `src/` & `figura/` untuk melengkapi peta ini
3. Scaffold workspace cargo + Tauri
