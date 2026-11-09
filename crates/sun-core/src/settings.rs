//! Preferensi aplikasi (Fase 2) — model setelan ala Raycast/Sonora.
//!
//! Disimpan sebagai JSON di `%LOCALAPPDATA%\Sunny\settings.json`. Nilai yang
//! tidak masuk akal dinormalisasi saat dimuat/diubah (clamp), supaya file yang
//! diedit manual tidak bisa membuat UI rusak.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SETTINGS_SAVE: AtomicU64 = AtomicU64::new(0);

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
    /// Mesin pencari default untuk "Quick Web Search" (kueri berakhiran `?`).
    pub search_engine: String,
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
            search_engine: "google".into(),
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

    /// Persist settings atomically, reporting failures without deleting the old file.
    pub fn save(&self) -> Result<(), String> {
        self.save_to(settings_path())
    }

    /// Save to an explicit path without accessing the production settings directory.
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), String> {
        self.save_to_with_writer(path.as_ref(), |file, bytes| file.write_all(bytes))
    }

    // Inject only the write step so tests can simulate partial disk failures safely.
    fn save_to_with_writer(
        &self,
        path: &Path,
        write: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
    ) -> Result<(), String> {
        let filename = path
            .file_name()
            .ok_or_else(|| "Choose a valid settings file path.".to_string())?;
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Could not encode settings: {error}"))?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create the settings directory: {error}"))?;
        }

        // Each writer exclusively creates its own same-directory temporary file.
        // Retry collisions with stale files without touching another writer's data.
        let (temporary, mut file) = loop {
            let mut name = filename.to_os_string();
            name.push(format!(
                ".{}.{}.tmp",
                std::process::id(),
                NEXT_SETTINGS_SAVE.fetch_add(1, Ordering::Relaxed)
            ));
            let temporary = path.with_file_name(name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "Could not create a temporary settings file: {error}"
                    ));
                }
            }
        };

        let result = write(&mut file, &bytes).and_then(|()| file.sync_all());
        // Close before rename or cleanup, including after partial writes on Windows.
        drop(file);
        // Rust's rename atomically replaces an existing file on Windows too.
        let result = result.and_then(|()| fs::rename(&temporary, path));
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "Could not save settings. Your previous settings were kept: {error}"
            ));
        }
        Ok(())
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
        // Mesin pencari tak dikenal kembali ke Google, supaya kueri `?` tetap jalan.
        if crate::websearch::engine_by_id(&self.search_engine).id != self.search_engine {
            self.search_engine = "google".into();
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
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir().join("opencode");
            fs::create_dir_all(&root).expect("create isolated test root");
            loop {
                let path = root.join(format!(
                    "sunny-settings-test-{}-{}",
                    std::process::id(),
                    NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create isolated settings test directory: {error}"),
                }
            }
        }

        fn settings_path(&self) -> PathBuf {
            self.0.join("settings.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

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

    #[test]
    fn save_returns_a_result_without_accessing_production_settings() {
        let _save: fn(&Settings) -> Result<(), String> = Settings::save;
    }

    #[test]
    fn save_creates_parent_directories_and_round_trips_the_existing_schema() {
        let directory = TestDirectory::new();
        let path = directory.0.join("nested").join("settings.json");
        let settings = Settings {
            open_at_login: true,
            opacity: 64,
            emoji_prefix: "!".into(),
            ..Settings::default()
        };

        settings.save_to(&path).expect("save isolated settings");

        let bytes = fs::read(&path).expect("read saved settings");
        let restored: Settings = serde_json::from_slice(&bytes).expect("decode saved settings");
        assert_eq!(
            serde_json::to_value(restored).unwrap(),
            serde_json::to_value(settings).unwrap()
        );
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn save_replaces_existing_settings_without_removing_unrelated_temporary_files() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        let unrelated = path.with_extension("json.tmp");
        fs::write(&path, "previous settings").unwrap();
        fs::write(&unrelated, "another writer's file").unwrap();
        let settings = Settings::default();

        settings.save_to(&path).expect("replace existing settings");

        assert_eq!(
            fs::read(&path).unwrap(),
            serde_json::to_vec_pretty(&settings).unwrap()
        );
        assert_eq!(
            fs::read_to_string(&unrelated).unwrap(),
            "another writer's file"
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    fn failed_partial_write_preserves_previous_settings_and_cleans_its_temporary_file() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        fs::write(&path, "previous settings").unwrap();

        let error = Settings::default()
            .save_to_with_writer(&path, |file, bytes| {
                file.write_all(&bytes[..bytes.len() / 2])?;
                Err(std::io::Error::other("simulated disk write failure"))
            })
            .expect_err("partial writes must fail");

        assert!(error.contains("simulated disk write failure"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous settings");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_replace_preserves_the_destination_and_cleans_its_temporary_file() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        fs::create_dir(&path).unwrap();
        let marker = path.join("keep.txt");
        fs::write(&marker, "keep existing destination").unwrap();

        assert!(Settings::default().save_to(&path).is_err());

        assert_eq!(
            fs::read_to_string(&marker).unwrap(),
            "keep existing destination"
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn blocked_parent_returns_an_error_without_changing_existing_files() {
        let directory = TestDirectory::new();
        let parent = directory.0.join("not-a-directory");
        fs::write(&parent, "keep existing parent").unwrap();

        assert!(Settings::default()
            .save_to(parent.join("settings.json"))
            .is_err());

        assert_eq!(fs::read_to_string(&parent).unwrap(), "keep existing parent");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn locked_destination_preserves_previous_settings_and_cleans_its_temporary_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = TestDirectory::new();
        let path = directory.settings_path();
        fs::write(&path, "previous settings").unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .expect("lock only the isolated destination");

        let result = Settings::default().save_to(&path);
        drop(lock);

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous settings");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_saves_use_distinct_temporary_files_and_leave_complete_json() {
        let directory = TestDirectory::new();
        let path = directory.settings_path();
        let barrier = std::sync::Barrier::new(8);

        std::thread::scope(|scope| {
            let mut writers = Vec::new();
            for opacity in 40..48 {
                let path = &path;
                let barrier = &barrier;
                writers.push(scope.spawn(move || {
                    let settings = Settings {
                        opacity,
                        ..Settings::default()
                    };
                    barrier.wait();
                    settings.save_to(path)
                }));
            }
            for writer in writers {
                writer
                    .join()
                    .expect("writer must not panic")
                    .expect("save concurrently");
            }
        });

        let restored: Settings = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!((40..48).contains(&restored.opacity));
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

#[cfg(test)]
mod search_engine_tests {
    use super::*;

    #[test]
    fn unknown_search_engine_falls_back_to_google() {
        let mut s = Settings { search_engine: "not-real".into(), ..Default::default() };
        s.normalize();
        assert_eq!(s.search_engine, "google");
    }

    #[test]
    fn known_search_engine_is_preserved() {
        let mut s = Settings { search_engine: "duckduckgo".into(), ..Default::default() };
        s.normalize();
        assert_eq!(s.search_engine, "duckduckgo");
    }

    #[test]
    fn legacy_settings_without_the_field_still_load() {
        let s: Settings = serde_json::from_str(r#"{"hotkey":"Ctrl+Space"}"#).unwrap();
        assert_eq!(s.search_engine, "google");
    }
}
