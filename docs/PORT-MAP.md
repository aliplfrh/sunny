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

| Modul Sunny | Asal di Vicinae | Strategi |
|---|---|---|
| `apps/sunny` (UI) | `src/*/qml` (UI QML) | DITULIS ULANG — UI web tapi bertampilan Raycast (webview, bukan Qt Quick) |
| `apps/sunny` (backend) | `src/main.cpp`, `src/main-window.*` | Rust + tauri v2: jendela frameless, overlay, hotkey, tray |
| `crates/sun-core` | `src/lib` (search, ranking, settings, storage, action, grid, list...) | Pindahkan ke Rust sesuai semantik, bukan baris per baris |
| `crates/sun-core: fuzzy` | `src/lib/search*` | Samakan perilaku dengan pustaka fuzzy Rust (kandidat: `fuzzy-matcher` / implen khusus) |
| `crates/sun-platform` | `src/lib/platform/` | Abstraksi Windows via pustaka ekosistem: `windows-rs`, `arboard`, `global-hotkey` |
| `crates/sun-core: system-run` | `src/lib/services/` (app service, window service...) | `windows-rs`: enumerasi jendela, enumerasi aplikasi Start Menu |
| `crates/sun-extensions` | `figura/` + TypeScript | **Pada fase lanjut** — runtime ekstensi; evaluasi jalur: subset command script dulu, lalu ekstensi React |
| Kalkulator | modul kalkulator (di macOS upstream pakai SoulverCore) | Rust: `meval`/`evalexpr` untuk Fase 1 |

## Prohibisi saat porting
- Jangan tiru arsitektur actor Qt (`QObject`/signal-slot) 1:1 — pakai kanal (channel) & `tokio`/`async` bawaan Rust.
- Jangan bawa plugin/dependensi Qt — semuanya ada padanannya di ekosistem Rust.
- Jangan salin teks UI/ikon berhak cipta dari Raycast — tampilan disusun ulang.
- Simpan sebagai referensi *desain & perilaku* — bukan salinan art.

## Langkah berikutnya
1. Clone shallow vicinae → `reference/vicinae` (hanya untuk baca, di-.gitignore)
2. Katalogisasi struktur `src/` & `figura/` untuk melengkapi peta ini
3. Scaffold workspace cargo + Tauri
