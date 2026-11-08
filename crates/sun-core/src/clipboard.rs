//! Clipboard history service. Port konsep clipboard vicinae (Fase 2).
//!
//! Strategi MVP: polling nomor urut clipboard tiap 400ms (murah, tanpa hook),
//! teks saja (gambar menyusul), dedupe, persist ke
//! %LOCALAPPDATA%\Sunny\clipboard-history.json.
//!
//! Tiap entri disimpan lengkap dengan metadata gaya Raycast:
//! jenis konten, ukuran byte, dan checksum MD5 (untuk panel detail).

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const MAX_ENTRIES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipEntry {
    /// Unix epoch ms saat entri dibuat (dipakai juga sebagai id)
    pub id: u64,
    /// Isi teks
    pub text: String,
    /// Jenis konten: "text" (nanti: "image", "file")
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Ukuran konten dalam byte (UTF-8)
    #[serde(default)]
    pub size: u64,
    /// Checksum MD5 hex — ditampilkan di panel detail
    #[serde(default)]
    pub hash: String,
}

fn default_kind() -> String {
    "text".into()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct ClipboardStore {
    list: Mutex<Vec<ClipEntry>>,
    /// Watcher berhenti merekam saat pause aktif (toggle ala Raycast).
    paused: AtomicBool,
}

impl ClipboardStore {
    pub fn new() -> Self {
        ClipboardStore {
            list: Mutex::new(Vec::new()),
            paused: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Vec<ClipEntry> {
        self.list.lock().unwrap().clone()
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Toggle pause. Kembalikan status terbaru.
    pub fn toggle_pause(&self) -> bool {
        self.paused.store(!self.is_paused(), Ordering::Relaxed);
        self.is_paused()
    }

    /// Load riwayat dari disk (dipanggil sekali di startup). Entri lama
    /// tanpa metadata diberi hash/size/kind secara lengkap (backfill).
    pub fn load(&self) {
        let path = history_path();
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(list) = serde_json::from_slice::<Vec<ClipEntry>>(&bytes) {
                *self.list.lock().unwrap() = list
                    .into_iter()
                    .map(|mut e| {
                        if e.kind.is_empty() {
                            e.kind = "text".into();
                        }
                        if e.size == 0 {
                            e.size = e.text.len() as u64;
                        }
                        if e.hash.is_empty() {
                            e.hash = md5_hex(e.text.as_bytes());
                        }
                        e
                    })
                    .collect();
            }
        }
    }

    /// Masukkan teks baru. Kembalikan true bila riwayat berubah.
    pub fn push(&self, text: &str) -> bool {
        if text.trim().is_empty() || text.len() > 100_000 {
            return false;
        }
        let bytes = text.as_bytes();
        let entry = ClipEntry {
            id: now_ms(),
            text: text.to_owned(),
            kind: "text".into(),
            size: bytes.len() as u64,
            hash: md5_hex(bytes),
        };
        let mut list = self.list.lock().unwrap();
        if list.first().map(|e| e.text == text).unwrap_or(false) {
            return false; // duplikat teratas
        }
        list.retain(|e| e.text != text); // entri lama naik ke atas
        list.insert(0, entry);
        list.truncate(MAX_ENTRIES);
        true
    }

    /// Hapus entri berdasar id. Kembalikan true bila ada yang terhapus.
    pub fn delete(&self, id: u64) -> bool {
        let mut list = self.list.lock().unwrap();
        let before = list.len();
        list.retain(|e| e.id != id);
        list.len() != before
    }

    /// Simpan ke disk.
    pub fn save(&self) {
        let path = history_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(data) = serde_json::to_vec(&self.snapshot()) {
            let _ = fs::write(&path, data);
        }
    }
}

fn md5_hex(data: &[u8]) -> String {
    let mut h = Md5::new();
    h.update(data);
    let d = h.finalize();
    let mut s = String::with_capacity(32);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn history_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("Sunny").join("clipboard-history.json")
}
