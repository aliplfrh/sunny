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
    /// Tampilkan baris petunjuk tombol di bawah launcher.
    pub show_footer: bool,
    /// Awalan untuk masuk mode emoji (default ":").
    pub emoji_prefix: String,
    /// Backspace saat kolom kosong kembali ke pencarian aplikasi.
    pub backspace_back: bool,
    /// Navigasi daftar dengan Ctrl+J / Ctrl+K (ala Vim).
    pub vim_nav: bool,
    /// Sertakan hasil file di pencarian utama (tanpa membuka "Search Files").
    pub include_files: bool,
    /// Jumlah maksimum hasil file yang digabung ke pencarian utama.
    pub file_results: u32,
    /// "system" | "light" | "dark".
    pub theme: String,
    /// Warna aksen (hex `#rrggbb`) untuk sorotan, tombol aktif, dan switch.
    pub accent: String,
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
            show_footer: true,
            emoji_prefix: ":".into(),
            backspace_back: true,
            vim_nav: false,
            include_files: true,
            file_results: 8,
            theme: "system".into(),
            accent: "#f08c28".into(),
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
    /// Ditulis ke file sementara lalu di-rename, supaya crash di tengah tulis
    /// tidak meninggalkan settings.json setengah jadi (yang akan terbaca
    /// sebagai default dan membuat setelan pengguna "hilang").
    pub fn save(&self) {
        let path = settings_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let Ok(data) = serde_json::to_vec_pretty(self) else {
            return;
        };
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, &data).is_err() {
            return;
        }
        // Windows: rename gagal bila target ada, jadi hapus dulu.
        let _ = fs::remove_file(&path);
        if fs::rename(&tmp, &path).is_err() {
            let _ = fs::write(&path, &data);
            let _ = fs::remove_file(&tmp);
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
        // Awalan emoji harus satu karakter yang tidak kosong.
        let prefix: String = self.emoji_prefix.trim().chars().take(1).collect();
        self.emoji_prefix = if prefix.is_empty() {
            ":".into()
        } else {
            prefix
        };
        self.file_results = self.file_results.clamp(0, 30);
        if !matches!(self.theme.as_str(), "light" | "dark") {
            self.theme = "system".into();
        }
        // Aksen harus hex #rrggbb yang valid; kalau tidak, kembali ke oranye Sunny.
        self.accent = normalize_hex_color(&self.accent).unwrap_or_else(|| "#f08c28".into());
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

/// Validasi warna hex `#rrggbb` (atau `#rgb`) dan kembalikan bentuk kanonik
/// `#rrggbb` huruf kecil. `None` bila bukan warna yang sah.
pub fn normalize_hex_color(input: &str) -> Option<String> {
    let s = input.trim().trim_start_matches('#');
    let expanded = match s.len() {
        3 => s.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => s.to_string(),
        _ => return None,
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", expanded.to_ascii_lowercase()))
}

/// Pecah `#rrggbb` menjadi komponen `(r, g, b)`.
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let s = normalize_hex_color(hex)?;
    let r = u8::from_str_radix(&s[1..3], 16).ok()?;
    let g = u8::from_str_radix(&s[3..5], 16).ok()?;
    let b = u8::from_str_radix(&s[5..7], 16).ok()?;
    Some((r, g, b))
}

fn settings_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Sunny").join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_hex_colors() {
        assert_eq!(normalize_hex_color("#F08C28").as_deref(), Some("#f08c28"));
        assert_eq!(normalize_hex_color("f08c28").as_deref(), Some("#f08c28"));
        assert_eq!(normalize_hex_color("#abc").as_deref(), Some("#aabbcc"));
        assert_eq!(
            normalize_hex_color("  #123456  ").as_deref(),
            Some("#123456")
        );
    }

    #[test]
    fn rejects_invalid_colors() {
        assert!(normalize_hex_color("").is_none());
        assert!(normalize_hex_color("#12345").is_none());
        assert!(normalize_hex_color("#gggggg").is_none());
        assert!(normalize_hex_color("orange").is_none());
    }

    #[test]
    fn splits_rgb() {
        assert_eq!(hex_to_rgb("#f08c28"), Some((240, 140, 40)));
        assert_eq!(hex_to_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("#ffffff"), Some((255, 255, 255)));
    }

    #[test]
    fn accent_falls_back_to_default() {
        let mut s = Settings::default();
        s.accent = "not-a-color".into();
        s.normalize();
        assert_eq!(s.accent, "#f08c28");
    }
}
