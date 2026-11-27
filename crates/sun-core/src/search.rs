//! Pencarian fuzzy aplikasi.
//!
//! Skor pencocokan memakai [`crate::match_rank`] — port matcher Raycast yang
//! tervalidasi 126 vektor emas, bukan lagi SkimMatcherV2. Urutan akhir
//! menambahkan dua hal yang membuat launcher terasa "hafal":
//!
//! * **Kecocokan keyword** — nama resmi tetap yang utama, tapi alias seperti
//!   "vsc" atau "code" bisa memunculkan aplikasi yang namanya tidak cocok.
//!   Ini meniru `keywords` milik Raycast.
//! * **Frecency** — aplikasi yang sering dan baru dipakai naik antar hasil yang
//!   skornya berdekatan, tanpa pernah mengalahkan hasil yang jauh lebih cocok.
//!
//! Kueri disaring [`Tier::High`]: kecocokan lemah dibuang, bukan ditaruh paling
//! bawah. "tti" tidak lagi memunculkan Settings dari tengah kata, dan "cfg" yang
//! dulu cocok dengan Microsoft Edge sekarang tidak memunculkan apa pun.
//!
//! Modul ini juga memegang [`write_searchable`], adaptor teks yang dipakai
//! bersama oleh pencarian aplikasi dan pencarian file ([`crate::files`]).

use crate::apps::AppEntry;
use crate::match_rank::{self, Match, Tier};
use serde::Serialize;
use std::borrow::Cow;

/// Satu hasil pencarian lengkap dengan skor.
#[derive(Debug, Clone, Serialize)]
pub struct Ranked<T> {
    pub item: T,
    pub score: i64,
}

/// Peringkatkan entri berdasarkan kueri fuzzy; terurut terbaik dulu.
pub fn rank(query: &str, items: &[AppEntry]) -> Vec<Ranked<AppEntry>> {
    rank_with_usage(query, items, &|_| 0)
}

/// Seperti [`rank`], tetapi `usage` memberi urutan tambahan saat skor seri.
/// `usage` menerima path aplikasi dan mengembalikan angka pemakaian (mis.
/// jumlah peluncuran). Nilainya tidak pernah dikirim ke frontend.
pub fn rank_with_usage(
    query: &str,
    items: &[AppEntry],
    usage: &dyn Fn(&str) -> i64,
) -> Vec<Ranked<AppEntry>> {
    // Matcher hanya menerima ASCII; kueri dan nama dilipat dengan aturan yang
    // sama supaya nama beraksen tetap bisa ditemukan (lihat [`write_searchable`]).
    let needle = searchable(query);
    let query_len = needle.chars().count() as i64;
    let mut hits: Vec<(bool, Ranked<AppEntry>)> = items
        .iter()
        .filter_map(|item| {
            let (matched_by_name, hit) = best_app_match(item, &needle, query_len)?;
            Some((
                matched_by_name,
                Ranked {
                    item: item.clone(),
                    // The matcher's own rank: exact first, every real score above
                    // an unmatched text.
                    score: hit.rank(),
                },
            ))
        })
        .collect();
    // Name matches first, then score, then usage as a tiebreak only. Sorting
    // this way keeps the order explainable: a habit never overrides a match.
    hits.sort_unstable_by(|(a_name, a), (b_name, b)| {
        b_name
            .cmp(a_name)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| usage(&b.item.path).cmp(&usage(&a.item.path)))
            .then_with(|| a.item.name.to_lowercase().cmp(&b.item.name.to_lowercase()))
            .then_with(|| a.item.path.cmp(&b.item.path))
    });
    hits.into_iter().map(|(_, hit)| hit).collect()
}

/// Pilih kecocokan terbaik untuk satu aplikasi, atau `None` bila tidak ada yang
/// cukup kuat untuk tampil di root search.
///
/// Aturannya: nama resmi diperiksa lebih dulu; alias hanya dipakai bila nama
/// sendiri tidak lolos [`Tier::High`]. Alias ada untuk *memunculkan* aplikasi,
/// bukan untuk mendorongnya melewati aplikasi yang namanya cocok.
///
/// Keyword dinilai satu per satu, bukan lewat [`match_rank::score_with_keywords`]:
/// fungsi itu memilih skor tertinggi, sedangkan ambang [`Tier::High`] bergantung
/// pada `skipped`, jadi skor tertinggi belum tentu lolos. Menyaring per kandidat
/// memastikan alias yang lolos tidak tersembunyi di balik nama yang tidak lolos.
fn best_app_match(item: &AppEntry, needle: &str, query_len: i64) -> Option<(bool, Match)> {
    let name_match = match_rank::score(&searchable(&item.name), needle);
    if name_match.admits(Tier::High, query_len) {
        return Some((true, name_match));
    }
    item.keywords
        .iter()
        .map(|keyword| match_rank::score(&searchable(keyword), needle))
        .filter(|candidate| candidate.admits(Tier::High, query_len))
        .max_by_key(|candidate| candidate.rank())
        .map(|matched| (false, matched))
}

/// Tulis `text` ke `buffer` dalam bentuk yang bisa dicocokkan matcher.
///
/// Matcher menolak teks non-ASCII (`Unmatched`) alih-alih mentransliterasi,
/// karena kueri Sunny dianggap ASCII. Nama aplikasi dan path Windows nyata tetap
/// sering memuat huruf beraksen ("Jürgen", "Café") atau CJK; dibiarkan apa
/// adanya, seluruh entri seperti itu hilang dari hasil. Setiap karakter
/// non-ASCII karena itu menjadi `/` — limiter yang sudah dikenal matcher —
/// sehingga entri masih bisa ditemukan lewat bagian ASCII-nya dan huruf
/// sesudahnya tetap dianggap awal kata. Aturan yang sama dipakai untuk kueri,
/// jadi kueri beraksen tetap cocok persis dengan namanya sendiri.
///
/// `\` (pemisah path Windows) juga menjadi `/`, satu-satunya pemisah yang
/// dikenal matcher. Tanpa itu seluruh path dianggap satu kata, sehingga nama
/// folder di tengah path tidak pernah mendapat bobot word-beginning.
///
/// Buffer dipanggil ulang agar pass path 60 ribu entri tidak mengalokasi per
/// kueri.
pub(crate) fn write_searchable(text: &str, buffer: &mut String) {
    buffer.clear();
    buffer.extend(
        text.chars()
            .map(|ch| if ch.is_ascii() && ch != '\\' { ch } else { '/' }),
    );
}

/// [`write_searchable`] untuk satu nilai. `Cow::Borrowed` bila teks sudah aman,
/// sehingga nama ASCII murni tidak dialokasi ulang.
pub(crate) fn searchable(text: &str) -> Cow<'_, str> {
    if text.is_ascii() && !text.contains('\\') {
        return Cow::Borrowed(text);
    }
    let mut folded = String::with_capacity(text.len());
    write_searchable(text, &mut folded);
    Cow::Owned(folded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, path: &str, keywords: &[&str]) -> AppEntry {
        AppEntry {
            name: name.into(),
            path: path.into(),
            source: "start-menu",
            category: "Other",
            icon: None,
            keywords: keywords.iter().map(|word| (*word).to_string()).collect(),
        }
    }

    #[test]
    fn a_keyword_alias_finds_an_app_whose_name_does_not_match() {
        let items = [
            app("Visual Studio Code", "code.lnk", &["vsc", "code"]),
            app("Notepad", "notepad.lnk", &[]),
        ];
        let hits = rank_with_usage("vsc", &items, &|_| 0);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item.name, "Visual Studio Code");
    }

    #[test]
    fn a_name_match_beats_a_keyword_match_of_the_same_quality() {
        // "Text Tool" only matches through its "editor" alias.
        let items = [
            app("Code Editor", "a.lnk", &[]),
            app("Text Tool", "b.lnk", &["editor"]),
        ];
        let hits = rank_with_usage("editor", &items, &|_| 0);
        assert_eq!(hits[0].item.name, "Code Editor");
        // The alias "editor" is an exact match while the name only contains the
        // word, so the alias scores higher in isolation. Ordering therefore
        // comes from the name-first rule, not from the score.
        assert!(
            hits[1].score > hits[0].score,
            "the alias alone scores higher"
        );
    }

    #[test]
    fn weak_matches_are_dropped_instead_of_ranked_last() {
        // Golden vector: "Settings" vs "tti" is a real score (6) that Raycast
        // refuses at root-search strictness, because it sits mid-word.
        let items = [
            app("Settings", "settings.lnk", &[]),
            app("Terminal", "t.lnk", &[]),
        ];
        assert!(rank_with_usage("tti", &items, &|_| 0).is_empty());

        // A query that no name contains still yields nothing rather than noise.
        assert!(rank_with_usage("cfg", &items, &|_| 0).is_empty());
    }

    #[test]
    fn usage_breaks_ties_without_ever_beating_a_better_match() {
        // Two different names with the same score: usage decides the order.
        let items = [
            app("Codec", "codec.lnk", &[]),
            app("Codes", "codes.lnk", &[]),
        ];
        let plain = rank_with_usage("code", &items, &|_| 0);
        assert_eq!(plain[0].item.name, "Codec");
        assert_eq!(plain[0].score, plain[1].score, "a genuine score tie");
        let used = rank_with_usage("code", &items, &|path| {
            if path == "codes.lnk" {
                4
            } else {
                0
            }
        });
        assert_eq!(used[0].item.name, "Codes", "a habit wins an exact tie");

        // A weaker match cannot overtake a stronger one, however often it is used.
        let items = [
            app("Code", "code.lnk", &[]),
            app("Colour Decoder", "decoder.lnk", &[]),
        ];
        let exact = rank_with_usage("code", &items, &|path| {
            if path == "decoder.lnk" {
                50
            } else {
                0
            }
        });
        assert_eq!(exact[0].item.name, "Code", "score outranks usage");
    }

    #[test]
    fn equal_scores_are_ordered_deterministically() {
        // "Phones" and "Photos" both score 8 for "pho": the name tiebreak decides.
        let items = [app("Photos", "p.lnk", &[]), app("Phones", "h.lnk", &[])];
        let first = rank_with_usage("pho", &items, &|_| 0);
        let second = rank_with_usage("pho", &items, &|_| 0);
        let names = |hits: &[Ranked<AppEntry>]| {
            hits.iter()
                .map(|hit| hit.item.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(first[0].score, first[1].score, "a genuine score tie");
        assert_eq!(names(&first), vec!["Phones", "Photos"]);
        assert_eq!(names(&first), names(&second));
    }

    #[test]
    fn a_non_ascii_name_stays_findable() {
        // Matcher menolak teks non-ASCII, jadi nama seperti ini dulu hilang
        // seluruhnya. Aturan lipat yang sama dipakai untuk nama dan kueri.
        let items = [app("Café", "cafe.lnk", &[]), app("Notes", "notes.lnk", &[])];
        assert_eq!(rank_with_usage("café", &items, &|_| 0)[0].item.name, "Café");
        assert_eq!(rank_with_usage("caf", &items, &|_| 0)[0].item.name, "Café");
    }

    #[test]
    fn an_empty_query_admits_nothing() {
        // `match_rank` reports an empty query as a score, not as a non-match, so
        // the tier gate is what keeps the root search empty. The UI never asks
        // for an empty query, but the core must not depend on that.
        let items = [app("Notepad", "notepad.lnk", &[])];
        assert!(rank_with_usage("", &items, &|_| 0).is_empty());
    }

    #[test]
    fn a_weak_alias_does_not_admit_the_app() {
        // Alias tetap harus lolos Tier::High. "Settings" vs "tti" adalah skor
        // nyata (6) yang ditolak Raycast di root search karena di tengah kata,
        // jadi alias seperti itu tidak boleh memunculkan aplikasi apa pun.
        let items = [app("Text Tool", "b.lnk", &["Settings"])];
        assert!(rank_with_usage("tti", &items, &|_| 0).is_empty());
    }
}
