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
use std::path::{Path, PathBuf};
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
    /// Serializes disk writes so a stale snapshot cannot overwrite a newer one.
    save_lock: Mutex<()>,
    /// Watcher berhenti merekam saat pause aktif (toggle ala Raycast).
    paused: AtomicBool,
}

impl Default for ClipboardStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardStore {
    pub fn new() -> Self {
        ClipboardStore {
            list: Mutex::new(Vec::new()),
            save_lock: Mutex::new(()),
            paused: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Vec<ClipEntry> {
        self.list
            .lock()
            .map(|list| list.clone())
            .unwrap_or_default()
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
                let normalized: Vec<ClipEntry> = list
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
                if let Ok(mut current) = self.list.lock() {
                    *current = normalized;
                }
            }
        }
    }

    /// Masukkan teks baru. Kembalikan true bila riwayat berubah.
    pub fn push(&self, text: &str) -> bool {
        if text.trim().is_empty() || text.len() > 100_000 {
            return false;
        }
        let bytes = text.as_bytes();
        let Ok(mut list) = self.list.lock() else {
            return false;
        };
        if list.first().map(|e| e.text == text).unwrap_or(false) {
            return false; // duplikat teratas
        }
        // Ids double as the delete key, so they must stay unique even when two
        // entries arrive inside the same millisecond.
        let latest = list.first().map(|e| e.id).unwrap_or(0);
        let entry = ClipEntry {
            id: now_ms().max(latest.saturating_add(1)),
            text: text.to_owned(),
            kind: "text".into(),
            size: bytes.len() as u64,
            hash: md5_hex(bytes),
        };
        list.retain(|e| e.text != text); // entri lama naik ke atas
        list.insert(0, entry);
        list.truncate(MAX_ENTRIES);
        true
    }

    /// Hapus entri berdasar id. Kembalikan true bila ada yang terhapus.
    pub fn delete(&self, id: u64) -> bool {
        let Ok(mut list) = self.list.lock() else {
            return false;
        };
        let before = list.len();
        list.retain(|e| e.id != id);
        list.len() != before
    }

    /// Simpan ke disk secara atomik (temp file + rename).
    pub fn save(&self) -> Result<(), String> {
        self.save_to(&history_path())
    }

    /// Same atomic write against an explicit path, so tests stay isolated.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        // Hold the write lock across snapshot + rename; otherwise an older
        // snapshot could land after a newer one and undo a deletion.
        let _guard = self
            .save_lock
            .lock()
            .map_err(|_| "Clipboard save lock poisoned")?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("Could not create clipboard folder: {e}"))?;
        }
        let data = serde_json::to_vec(&self.snapshot())
            .map_err(|e| format!("Could not encode clipboard history: {e}"))?;
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, &data).map_err(|e| format!("Could not write clipboard history: {e}"))?;
        if let Ok(file) = fs::File::open(&temp) {
            let _ = file.sync_all();
        }
        crate::fsutil::replace(&temp, path).map_err(|e| {
            let _ = fs::remove_file(&temp);
            format!("Could not replace clipboard history: {e}")
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("sunny-clipboard-tests")
            .join(format!("{name}-{}.json", now_ms()))
    }

    #[test]
    fn save_is_atomic_and_round_trips() {
        let store = ClipboardStore::new();
        assert!(store.push("first"));
        assert!(store.push("second"));
        let path = temp_path("round-trip");
        store.save_to(&path).expect("save");
        let data = fs::read(&path).expect("read back");
        let saved: Vec<ClipEntry> = serde_json::from_slice(&data).expect("valid json");
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].text, "second");
        assert!(
            !path.with_extension("json.tmp").exists(),
            "temp file left behind"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn delete_and_save_keeps_the_deletion_on_disk() {
        let store = ClipboardStore::new();
        store.push("keep");
        store.push("drop");
        let dropped = store.snapshot().into_iter().find(|e| e.text == "drop");
        assert!(store.delete(dropped.expect("entry").id));
        let path = temp_path("delete");
        store.save_to(&path).expect("save");
        let saved: Vec<ClipEntry> =
            serde_json::from_slice(&fs::read(&path).expect("read back")).expect("valid json");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].text, "keep");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn entries_pushed_in_the_same_millisecond_get_distinct_ids() {
        let store = ClipboardStore::new();
        assert!(store.push("a"));
        assert!(store.push("b"));
        let ids: Vec<u64> = store.snapshot().iter().map(|e| e.id).collect();
        assert_ne!(ids[0], ids[1], "duplicate ids would delete both entries");
    }

    #[test]
    fn duplicate_and_blank_text_do_not_change_history() {
        let store = ClipboardStore::new();
        assert!(store.push("same"));
        assert!(!store.push("same"));
        assert!(!store.push("   "));
        assert_eq!(store.snapshot().len(), 1);
    }
}
