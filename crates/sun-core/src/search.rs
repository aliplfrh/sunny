//! Mesin pencarian fuzzy. Port perilaku `src/lib/fuzzy` vicinae.
//!
//! Memo: ranking bisa disempurnakan (frekuensi + recency) di fase berikutnya;
//! untuk saat ini pakai SkimMatcherV2 (pengganti algoritma `fuzzy` vicinae).

use crate::apps::AppEntry;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::Serialize;

/// Satu hasil pencarian lengkap dengan skor.
#[derive(Debug, Clone, Serialize)]
pub struct Ranked<T> {
    pub item: T,
    pub score: i64,
}

/// Peringkatkan entri berdasarkan kueri fuzzy; terurut terbaik dulu.
/// Mengklon entri agar hasil milik si pemanggil (mudah dibawa ke IPC/UI).
pub fn rank(query: &str, items: &[AppEntry]) -> Vec<Ranked<AppEntry>> {
    let matcher = SkimMatcherV2::default().ignore_case();
    let mut hits: Vec<Ranked<AppEntry>> = items
        .iter()
        .filter_map(|item| {
            let score = matcher.fuzzy_match(&item.name, query)?;
            Some(Ranked {
                item: item.clone(),
                score,
            })
        })
        .collect();
    hits.sort_unstable_by(|a, b| b.score.cmp(&a.score));
    hits
}
