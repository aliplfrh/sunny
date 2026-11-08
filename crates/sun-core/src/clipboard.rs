//! Clipboard history service. Port konsep clipboard vicinae (Fase 2).
//!
//! Strategi MVP: polling nomor urut clipboard tiap 400ms (murah, tanpa hook),
//! teks saja (gambar menyusul), dedupe, persist ke
//! %LOCALAPPDATA%\Sunny\clipboard-history.json.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MAX_ENTRIES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    /// Unix epoch ms saat entri dibuat (dipakai juga sebagai id)
    pub id: u64,
    /// Isi teks
    pub text: String,
}

pub struct ClipboardStore(pub std::sync::Mutex<Vec<ClipEntry>>);

impl ClipboardStore {
    pub fn new() -> Self {
        ClipboardStore(std::sync::Mutex::new(Vec::new()))
    }

    /// Load riwayat dari disk (dipanggil sekali di startup).
    pub fn load(&self) {
        let path = history_path();
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(list) = serde_json::from_slice::<Vec<ClipEntry>>(&bytes) {
                *self.0.lock().unwrap() = list;
            }
        }
    }

    /// Masukkan teks baru. Kembalikan true bila riwayat berubah.
    pub fn push(&self, text: &str) -> bool {
        if text.trim().is_empty() || text.len() > 100_000 {
            return false;
        }
        let mut list = self.0.lock().unwrap();
        if list.first().map(|e| e.text == text).unwrap_or(false) {
            return false; // duplikat teratas
        }
        list.retain(|e| e.text != text); // entri lama naik ke atas
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        list.insert(0, ClipEntry { id: now, text: text.to_owned() });
        list.truncate(MAX_ENTRIES);
        true
    }

    /// Simpan ke disk.
    pub fn save(&self) {
        let path = history_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(data) = serde_json::to_vec(&self.0.lock().unwrap().clone()) {
            let _ = fs::write(&path, data);
        }
    }
}

fn history_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Sunny").join("clipboard-history.json")
}
