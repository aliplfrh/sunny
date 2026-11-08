//! Indeks aplikasi. Port konsep dari `src/lib/services` vicinae (app scanner).
//!
//! Windows-first: scanning shortcut .lnk di Start Menu (ProgramData + AppData).

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
}

fn scan_dir(dir: &Path, source: &'static str, out: &mut Vec<AppEntry>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return, // direktori tidak ada — normal di banyak sistem
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, source, out);
        } else if path.extension() == Some(&OsString::from("lnk")) {
            let name = match path.file_stem().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            out.push(AppEntry {
                name,
                path: path.to_string_lossy().into_owned(),
                source,
            });
        }
    }
}

/// Kumpulkan semua aplikasi dari Start Menu Windows (system + user).
/// Port konsep app service di vicinae, menggunakan native .lnk enumeration.
pub fn scan_installed_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();
    let known = [
        (
            "start-menu-system",
            "%ProgramData%\\Microsoft\\Windows\\Start Menu\\Programs",
        ),
        (
            "start-menu-user",
            "%AppData%\\Microsoft\\Windows\\Start Menu\\Programs",
        ),
    ];
    for (source, raw) in known {
        let Some(dir) = expand_env(raw) else { continue };
        scan_dir(Path::new(&dir), source, &mut apps);
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name) && a.path == b.path);
    apps
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
        let Some(end) = rest.find('%') else { return None };
        let var = &rest[..end];
        if var.is_empty() {
            return None;
        }
        result.push_str(&std::env::var(var).ok()?);
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    let buf = PathBuf::from(&result);
    if buf.is_dir() { Some(result) } else { None }
}
