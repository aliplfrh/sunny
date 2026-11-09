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
    /// Path lengkap ke shortcut .lnk — dibuka langsung untuk meluncurkan aplikasi
    pub path: String,
    /// Sumber entri (mis. "start-menu-user", "start-menu-system")
    pub source: &'static str,
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
