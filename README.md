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

## Lisensi

GPL-3.0 — lihat [LICENSE](LICENSE) dan [NOTICE](NOTICE).

Sunny adalah karya turunan (*derivative work*) dari Vicinae yang berlisensi GPL-3.0. Seluruh kode hasil port mengikuti GPL-3.0.
