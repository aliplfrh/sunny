//! Emoji picker. Port konsep modul emoji vicinae.
//!
//! Dataset tertanam (include_str) — tanpa unduhan runtime, instan loading.
//! Pencarian dilakukan di frontend (dataset kecil), backend hanya menyedia data.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EmojiItem {
    /// Karakter emoji (bisa lebih dari 1 codepoint, mis. 👨‍💻)
    pub emoji: String,
    /// Nama utama (Inggris, konvensi dataset umum)
    pub name: String,
    /// Kata kunci pencarian Tambahan (mis. "senyum", "happy", "grin")
    pub keywords: String,
}

const EMOJI_JSON: &str = include_str!("../assets/emojis.json");

/// Parse dataset sekali (cache sekali-parse via OnceLock), kembalikan semua.
pub fn all() -> &'static [EmojiItem] {
    static DATA: std::sync::OnceLock<Vec<EmojiItem>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Raw {
            emoji: String,
            name: String,
            keywords: String,
        }
        let raw: Vec<Raw> = serde_json::from_str(EMOJI_JSON).expect("emojis.json rusak");
        raw.into_iter()
            .map(|Raw {
                emoji,
                name,
                keywords,
            }| EmojiItem {
                emoji,
                name,
                keywords,
            })
            .collect()
    })
}
