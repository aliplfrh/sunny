//! Indeks & pencarian file (Fase 2) — pengganti "Search Files" ala Raycast.
//!
//! Strategi MVP: indeks nama file dari folder pengguna (Desktop, Documents,
//! Downloads, Pictures, Music, Videos, dan home) dibangun di background,
//! lalu dicari dengan fuzzy matcher yang sama seperti aplikasi.
//!
//! Tidak ada driver/USN journal — cukup `fs::read_dir` rekursif dengan batas
//! kedalaman, jumlah entri, dan daftar folder yang dilewati (AppData,
//! node_modules, .git, target, dsb) agar tetap ringan.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Batas atas entri indeks (RAM ±10–15 MB pada batas ini).
pub const MAX_FILES: usize = 60_000;
const MAX_DEPTH: usize = 14;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RankedFile {
    pub item: FileEntry,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSearchResult {
    pub items: Vec<RankedFile>,
    /// Indeks selesai dibangun?
    pub ready: bool,
    pub total: usize,
}

/// Folder yang tidak perlu diindeks (nama komponen path, case-insensitive).
fn skip_dir(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    matches!(
        name.to_ascii_lowercase().as_str(),
        "appdata"
            | "node_modules"
            | "target"
            | "venv"
            | ".venv"
            | "__pycache__"
            | "$recycle.bin"
            | "system volume information"
            | "program files"
            | "program files (x86)"
            | "programdata"
            | "windows"
            | "obj"
            | ".cargo"
            | ".rustup"
            | ".cache"
            | ".npm"
            | ".gradle"
            | ".m2"
            | "dist"
    )
}

fn skip_file(name: &str) -> bool {
    if name.starts_with('.') || name.starts_with("~$") {
        return true;
    }
    matches!(
        name.to_ascii_lowercase().as_str(),
        "desktop.ini"
            | "thumbs.db"
            | "ehthumbs.db"
            | "iconcache.db"
            | "ntuser.dat"
            | "ntuser.dat.log1"
            | "ntuser.dat.log2"
    )
}

/// Kumpulan root awal. Folder known-user yang berada di dalam home tidak
/// diduplikasi — home sudah mencakup semuanya.
pub fn roots() -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let mut out: Vec<PathBuf> = Vec::new();

    let known = [
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
        dirs::picture_dir(),
        dirs::audio_dir(),
        dirs::video_dir(),
    ];
    for dir in known.into_iter().flatten() {
        let inside_home = home.as_ref().map(|h| dir.starts_with(h)).unwrap_or(false);
        if !inside_home && dir.is_dir() {
            out.push(dir);
        }
    }
    if let Some(h) = home {
        if h.is_dir() {
            out.push(h);
        }
    }
    out
}

/// Bangun indeks file (blocking; panggil dari thread background).
pub fn build_index() -> Vec<FileEntry> {
    let mut out = Vec::with_capacity(MAX_FILES.min(20_000));
    let mut visited_dirs: HashSet<String> = HashSet::new();
    let mut seen_paths: HashSet<String> = HashSet::new();
    for root in roots() {
        walk(&root, 0, &mut out, &mut visited_dirs, &mut seen_paths);
        if out.len() >= MAX_FILES {
            break;
        }
    }
    out
}

fn walk(
    dir: &Path,
    depth: usize,
    out: &mut Vec<FileEntry>,
    visited_dirs: &mut HashSet<String>,
    seen_paths: &mut HashSet<String>,
) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    let key = dir.to_string_lossy().to_ascii_lowercase();
    if !visited_dirs.insert(key) {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };

    for entry in rd.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = ft.is_dir();
        if if is_dir {
            skip_dir(&name)
        } else {
            skip_file(&name)
        } {
            continue;
        }
        if let Some(p) = path.to_str() {
            let lower = p.to_ascii_lowercase();
            if seen_paths.insert(lower) {
                out.push(FileEntry {
                    name,
                    path: p.to_string(),
                    is_dir,
                });
            }
        }

        if is_dir {
            walk(&path, depth + 1, out, visited_dirs, seen_paths);
        }
    }
}

/// Cari file/folder berdasarkan nama (fallback ke path) — hasil terbaik dulu.
///
/// Optimasi: pencocokan path lengkap hanya dijalankan bila hasil nama masih
/// sedikit. Untuk kueri pendek yang umum (mis. "doc"), pass kedua dilewati
/// sehingga 60 ribu entri tetap responsif.
pub fn search(index: &[FileEntry], query: &str, limit: usize) -> Vec<RankedFile> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let matcher = SkimMatcherV2::default().ignore_case();
    // (index, skor, cocok-di-nama?) — hasil nama selalu di atas fallback path.
    let mut scored: Vec<(usize, i64, bool)> = Vec::new();
    let mut name_hit = vec![false; index.len()];

    for (i, item) in index.iter().enumerate() {
        // Nama file lebih relevan daripada path lengkap.
        if let Some(s) = matcher.fuzzy_match(&item.name, q) {
            name_hit[i] = true;
            scored.push((i, s + 15, true));
        }
    }

    let path_query = q.contains('\\') || q.contains('/');
    if path_query || scored.len() < limit {
        for (i, item) in index.iter().enumerate() {
            if name_hit[i] {
                continue;
            }
            if let Some(s) = matcher.fuzzy_match(&item.path, q) {
                scored.push((i, s, false));
            }
        }
    }

    scored.sort_unstable_by(|a, b| b.2.cmp(&a.2).then_with(|| b.1.cmp(&a.1)));
    scored.truncate(limit);
    scored
        .into_iter()
        .map(|(i, score, _)| RankedFile {
            item: index[i].clone(),
            score,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            name: name.into(),
            path: path.into(),
            is_dir,
        }
    }

    fn sample() -> Vec<FileEntry> {
        vec![
            entry("notes.txt", r"C:\Users\me\Documents\notes.txt", false),
            entry("Report.pdf", r"C:\Users\me\Downloads\Report.pdf", false),
            entry("Documents", r"C:\Users\me\Documents", true),
            entry("sunny.rs", r"C:\code\sunny\src\sunny.rs", false),
        ]
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(search(&sample(), "   ", 10).is_empty());
    }

    #[test]
    fn matches_by_name_case_insensitive() {
        let hits = search(&sample(), "report", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item.name, "Report.pdf");
    }

    #[test]
    fn falls_back_to_path() {
        // "code" tidak ada di nama file, jadi hasilnya dari pencocokan path.
        let hits = search(&sample(), "code", 10);
        assert!(hits.iter().any(|h| h.item.name == "sunny.rs"));
    }

    #[test]
    fn respects_limit() {
        let hits = search(&sample(), "s", 2);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn skip_rules() {
        assert!(skip_dir("node_modules"));
        assert!(skip_dir(".git"));
        assert!(skip_dir("AppData"));
        assert!(!skip_dir("Documents"));
        assert!(skip_file("desktop.ini"));
        assert!(!skip_file("notes.txt"));
    }
}
