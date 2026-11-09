//! Preferensi aplikasi (Fase 2) — model setelan ala Raycast/Sonora.
//!
//! Disimpan sebagai JSON di `%LOCALAPPDATA%\Sunny\settings.json`. Nilai yang
//! tidak masuk akal dinormalisasi saat dimuat/diubah (clamp), supaya file yang
//! diedit manual tidak bisa membuat UI rusak.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Jalankan Sunny saat user login (HKCU\...\Run).
    pub open_at_login: bool,
    /// Tampilkan ikon tray.
    pub show_in_tray: bool,
    /// Accelerator global (format Tauri, mis. "Alt+Space").
    pub hotkey: String,
    /// "sonora" (Inter, default) atau "system".
    pub font: String,
    /// Ukuran font dasar UI (px), 12–20.
    pub font_size: u32,
    /// Opasitas jendela dalam persen, 40–100.
    pub opacity: u32,
    /// Efek blur UI dekoratif (di dalam halaman).
    pub blur_ui: bool,
    /// "compact" (Raycast Compact) atau "expanded" (Raycast Expanded).
    pub window_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            open_at_login: false,
            show_in_tray: true,
            hotkey: "Alt+Space".into(),
            font: "sonora".into(),
            font_size: 14,
            opacity: 100,
            blur_ui: false,
            window_mode: "expanded".into(),
        }
    }
}

impl Settings {
    /// Muat dari disk; nilai hilang/rusak jatuh ke default.
    pub fn load() -> Self {
        let mut s = fs::read(settings_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .unwrap_or_default();
        s.normalize();
        s
    }

    /// Simpan ke disk (best-effort, tidak mengganggu alur UI).
    pub fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(data) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(&path, data);
        }
    }

    /// Jaga semua nilai dalam rentang yang didukung UI.
    pub fn normalize(&mut self) {
        if self.hotkey.trim().is_empty() {
            self.hotkey = "Alt+Space".into();
        }
        if self.font != "system" {
            self.font = "sonora".into();
        }
        self.font_size = self.font_size.clamp(12, 20);
        self.opacity = self.opacity.clamp(40, 100);
        if self.window_mode != "compact" {
            self.window_mode = "expanded".into();
        }
    }

    /// Tinggi jendela (logical px) untuk mode yang aktif.
    pub fn window_height(&self) -> f64 {
        if self.window_mode == "compact" {
            356.0
        } else {
            464.0
        }
    }
}

fn settings_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Sunny").join("settings.json")
}
