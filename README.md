# Sunny

*Sunny* adalah launcher / command palette desktop yang cepat dan ringan — terinspirasi dari [Raycast](https://www.raycast.com/), merupakan port penuh dari [Vicinae](https://github.com/vicinaehq/vicinae) (C++/Qt) ke **Rust**.

## Status Proyek

🚧 **Fase 0 — Fondasi** sedang berjalan. Lihat [docs/ROADMAP.md](docs/ROADMAP.md).

## Arsitektur

| Komponen | Keterangan |
|---|---|
| `apps/sunny` | Aplikasi Tauri v2 — UI web (tampilan Raycast) + backend Rust |
| `crates/sun-core` | Mesin pencarian (fuzzy), ranking, settings, storage |
| `crates/sun-platform` | Abstraksi platform Windows: hotkey global, enumerasi jendela, clipboard |
| `crates/sun-extensions` | (Fase lanjut) Runtime ekstensi, kompatibilitas API Raycast |

## Platform

**Windows** diprioritaskan lebih dulu, disusul Linux dan macOS.

## Build (Windows)

**Prasyarat:**
1. [Rust](https://rustup.rs) (toolchain `stable-x86_64-pc-windows-msvc`)
2. [Node.js](https://nodejs.org) ≥ 20
3. **Microsoft Visual Studio Build Tools** dengan workload *Desktop development with C++* (linker MSVC + Windows SDK). Untuk mesin tanpa VS: `winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
4. Jalankan `cargo` dari terminal biasa — karena `link.exe` milik Git for Windows bisa muncul di PATH, repo ini memakai `.cargo/config.toml` yang menunjuk linker MSVC secara eksplisit.

**Perintah:**

```sh
cargo build --release                    # dari repo root
target\release\sunny.exe                 # jalankan (Alt+Space untuk tampilkan/sembunyikan)
```

## Lisensi

GPL-3.0 — lihat [LICENSE](LICENSE) dan [NOTICE](NOTICE).

Sunny adalah karya turunan (*derivative work*) dari Vicinae yang berlisensi GPL-3.0. Seluruh kode hasil port mengikuti GPL-3.0.
