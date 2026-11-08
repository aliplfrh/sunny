//! Sunny platform abstractions (Windows-first).
//!
//! Untuk fase scaffold, hotkey global dan window overlay ditangani langsung
//! oleh plugin Tauri di `apps/sunny`. Crate ini akan menyimpan logika
//! platform-level (enumerasi jendela, clipboard watcher, autostart)
//! pada Fase 1–2.

/// Placeholder — bukti crate terhubung.
pub const PLATFORM: &str = "windows";
