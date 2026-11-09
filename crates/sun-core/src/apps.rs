//! Indeks aplikasi. Port konsep dari `src/lib/services` vicinae (app scanner).
//!
//! Windows-first: scanning shortcut .lnk di Start Menu (ProgramData + AppData),
//! dengan cache ikon di disk agar scan berikutnya instan.

use base64::Engine;
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct AppEntry {
    /// Nama tampilan (nama file tanpa ekstensi)
    pub name: String,
    /// Path lengkap ke shortcut .lnk — dibuka langsung untuk meluncurkan aplikasi.
    /// Untuk aplikasi Microsoft Store, ini berisi AppUserModelID.
    pub path: String,
    /// Sumber entri: "start-menu" atau "store".
    pub source: &'static str,
    /// Kategori yang ditampilkan di baris hasil ("Application", "Game", ...).
    pub category: &'static str,
    /// Ikon aplikasi sebagai data URL PNG (None bila belum ada cache).
    /// Ekstraksi asli dilakukan `build_icon_cache` di background.
    pub icon: Option<String>,
}

fn scan_dir(dir: &Path, source: &'static str, out: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, source, out);
        } else if path.extension() == Some(&OsString::from("lnk")) {
            if let Some(p) = path.to_str() {
                out.push(p.to_string());
            }
        }
    }
}

/// Kumpulkan path semua shortcut dari Start Menu Windows (system + user).
fn collect_shortcuts() -> Vec<String> {
    let mut paths = Vec::new();
    let sources = [
        (
            "start-menu-system",
            "%ProgramData%\\Microsoft\\Windows\\Start Menu\\Programs",
        ),
        (
            "start-menu-user",
            "%AppData%\\Microsoft\\Windows\\Start Menu\\Programs",
        ),
    ];
    for (_, raw) in &sources {
        if let Some(dir) = expand_env(raw) {
            scan_dir(Path::new(&dir), "start-menu", &mut paths);
        }
    }
    paths
}

/// Kategori yang ditampilkan pada baris hasil.
///
/// Windows menaruh shortcut game di folder `Games`, dan utilitas sistem mudah
/// dikenali dari namanya. Sisanya dianggap aplikasi biasa.
fn detect_category(name: &str, path: &str) -> &'static str {
    let lower_name = name.trim().to_lowercase();
    let lower_path = path.to_lowercase();
    if lower_path.contains("\\games\\") || lower_name.contains("xbox") {
        return "Game";
    }
    match lower_name.as_str() {
        "settings" | "control panel" => "Windows Settings",
        _ => "Application",
    }
}

/// Kumpulkan aplikasi lengkap dengan ikon dari cache (tanpa ekstraksi = cepat).
pub fn scan_installed_apps() -> Vec<AppEntry> {
    let mut apps: Vec<AppEntry> = collect_shortcuts()
        .into_iter()
        .map(|path| {
            let name = Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let icon = cached_icon(&path)
                .map(|png| format!("data:image/png;base64,{}", base64_encode(&png)));
            AppEntry {
                category: detect_category(&name, &path),
                name,
                path,
                source: "start-menu",
                icon,
            }
        })
        .collect();
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Aplikasi Microsoft Store dalam bentuk netral, supaya `sun-core` tidak perlu
/// bergantung pada `sun-platform`. `main.rs` yang mengonversinya.
#[derive(Debug, Clone)]
pub struct StoreAppInput {
    pub name: String,
    /// AppUserModelID, mis. `5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App`.
    pub app_id: String,
    /// Isi file logo paket (PNG), bila ada.
    pub icon_png: Option<Vec<u8>>,
}

/// Gabungkan aplikasi Start Menu dengan aplikasi Microsoft Store.
pub fn merge_store_apps(mut apps: Vec<AppEntry>, store: Vec<StoreAppInput>) -> Vec<AppEntry> {
    let mut existing: std::collections::HashSet<String> =
        apps.iter().map(|a| a.name.to_lowercase()).collect();

    for app in store {
        // Jangan duplikasi aplikasi yang sudah punya shortcut Start Menu.
        if !existing.insert(app.name.to_lowercase()) {
            continue;
        }
        let icon = app
            .icon_png
            .map(|png| format!("data:image/png;base64,{}", base64_encode(&png)));
        apps.push(AppEntry {
            category: detect_category(&app.name, &app.app_id),
            name: app.name,
            path: app.app_id,
            source: "store",
            icon,
        });
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Ekstraksi ikon yang belum ada di cache. Dipanggil dari thread background.
/// `extractor` = `sun_platform::extract_icon_png`. Kembalikan jumlah ikon baru.
pub fn build_icon_cache(extractor: fn(&str) -> Option<Vec<u8>>) -> usize {
    let dir = icon_cache_dir();
    let mut created = 0usize;
    for path in collect_shortcuts() {
        let target = dir.join(format!("{}.png", fnv64(&path)));
        if target.exists() {
            continue;
        }
        if let Some(png) = extractor(&path) {
            if png.len() > 64 {
                if fs::write(&target, &png).is_ok() {
                    created += 1;
                }
            }
        } else {
            // Tandai gagal dengan file kosong agar tidak dipanggil lagi
            let _ = fs::write(&target, []);
        }
    }
    created
}

fn icon_cache_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    // v2: ikon diekstrak dari target `.lnk` (tanpa overlay panah shortcut),
    // jadi cache lama yang masih berbadge tidak boleh dipakai lagi.
    let dir = base.join("Sunny").join("icon-cache-v2");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn cached_icon(path: &str) -> Option<Vec<u8>> {
    let file = icon_cache_dir().join(format!("{}.png", fnv64(path)));
    let bytes = fs::read(&file).ok()?;
    // File kosong = marka "gagal diekstrak"
    if bytes.is_empty() {
        return None;
    }
    Some(bytes)
}

/// Hash FNV-1a 64-bit untuk nama file cache.
fn fnv64(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in input.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    STANDARD.encode(data)
}

/// Expand `%VAR%` di path (tanpa dependensi; cukup untuk pola Start Menu).
fn expand_env(raw: &str) -> Option<String> {
    if !raw.contains('%') {
        return Some(raw.to_string());
    }
    let mut result = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        let Some(end) = rest.find('%') else {
            return None;
        };
        let var = &rest[..end];
        if var.is_empty() {
            return None;
        }
        result.push_str(&std::env::var(var).ok()?);
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    if !Path::new(&result).is_dir() {
        return None;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> AppEntry {
        AppEntry {
            name: name.into(),
            path: path.into(),
            source: "start-menu",
            category: "Application",
            icon: None,
        }
    }

    #[test]
    fn categories_follow_folder_and_name() {
        assert_eq!(
            detect_category("Hades", r"C:\Start Menu\Programs\Games\Hades.lnk"),
            "Game"
        );
        assert_eq!(
            detect_category("Settings", r"C:\Start Menu\Programs\Settings.lnk"),
            "Windows Settings"
        );
        assert_eq!(
            detect_category("Notepad", r"C:\Start Menu\Programs\Notepad.lnk"),
            "Application"
        );
    }

    #[test]
    fn store_apps_are_merged_without_duplicates() {
        let base = vec![entry("WhatsApp", r"C:\x\WhatsApp.lnk")];
        let merged = merge_store_apps(
            base,
            vec![
                StoreAppInput {
                    name: "WhatsApp".into(),
                    app_id: "5319275A.WhatsAppDesktop_cv1!App".into(),
                    icon_png: None,
                },
                StoreAppInput {
                    name: "Spotify".into(),
                    app_id: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify".into(),
                    icon_png: None,
                },
            ],
        );
        assert_eq!(merged.len(), 2);
        assert!(merged
            .iter()
            .any(|a| a.name == "Spotify" && a.source == "store"));
        assert!(merged
            .iter()
            .filter(|a| a.name == "WhatsApp")
            .all(|a| a.source == "start-menu"));
    }
}
