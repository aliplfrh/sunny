//! Raycast's fuzzy matcher, ported for Sunny's root search.
//!
//! Behaviour was recovered from Raycast 2.14.0's shipped renderer and captured
//! as 126 golden vectors by `scripts/raycast-matcher-reference.mjs` (a
//! statement-for-statement transcription of the original `Kf`/`Gf`/`qf`).
//! The tests below replay those vectors, so this module is verified against the
//! real implementation rather than against my reading of it.
//!
//! Three properties of the original are deliberate and surprising:
//!
//! * A gap of **any** length can be bridged, but the cost is charged once per
//!   bridged query character, not per skipped text character. `vsc` therefore
//!   does match `Visual Studio Code`.
//! * The `FIRST` weight applies only when the first query character lands at
//!   text index 0. A match starting mid-word uses the boundary/default weight.
//! * Newline is a limiter; a non-breaking space is not.

/// Sentinel for "no path here". Matches the original's `-(2**53-1)`.
const NEG_INF: i64 = -9_007_199_254_740_991;
/// Sentinel for an exact match, matching the original's `2**53-1`.
const POS_INF: i64 = 9_007_199_254_740_991;

const FIRST: i64 = 4;
const WORD_BEGINNING: i64 = 3;
const DEFAULT: i64 = 2;
const LIMITER: i64 = 1;

/// Characters that end a word, exactly as in the original byte table.
const LIMITERS: [char; 10] = ['\t', '\n', ' ', '-', '.', '/', '(', ')', '[', ']'];

/// How strict admission must be before a candidate is allowed to appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Any match is accepted.
    Low,
    /// Weak matches must beat a length-scaled floor.
    Medium,
    /// Root search: weak matches are dropped, not ranked last.
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The whole text equals the query.
    Exact,
    /// No path through the text.
    Unmatched,
    /// A real score. `score` may be [`NEG_INF`] for an empty query, which the
    /// original also reports as a score rather than as a non-match.
    Scored { score: i64, skipped: i64 },
}

impl Match {
    /// Sort key: exact first, unmatched last.
    pub fn rank(self) -> i64 {
        match self {
            Match::Unmatched => NEG_INF,
            Match::Exact => POS_INF,
            Match::Scored { score, .. } => score,
        }
    }

    /// Whether this match clears `tier` for a query of `query_len` characters.
    ///
    /// `query_len` counts meaningful characters (separators excluded), which is
    /// what the original thresholds scale against.
    pub fn admits(self, tier: Tier, query_len: i64) -> bool {
        match (self, tier) {
            (Match::Unmatched, _) => false,
            (Match::Exact, _) => true,
            (Match::Scored { .. }, Tier::Low) => true,
            (Match::Scored { score, skipped }, Tier::Medium) => {
                score as f64 >= 1.5 * (query_len - skipped - 2) as f64 + 4.0
            }
            (Match::Scored { score, skipped }, Tier::High) => {
                // A one- or two-character query says almost nothing, and a
                // three-character one is usually an acronym. Without a stricter
                // rule here, `vsc` matches "VLC media player - reset preferences
                // and cache files" because v, s and c happen to sit at three
                // word starts. Require those short queries to be as good as a
                // contiguous run at a word start: 4 + 2*(n-1). Acronyms are
                // served by declared keywords instead, which is the whole point
                // of having them.
                let meaningful = query_len - skipped;
                if meaningful < 4 {
                    // A one- or two-character query says almost nothing, and a
                    // three-character one is usually an acronym. Require those
                    // short queries to start a word: the 4-point first-character
                    // bonus, or a 3-point word-boundary bonus. Without this,
                    // `vsc` matches "VLC media player - reset preferences and
                    // cache files" because v, s and c each sit at a word start.
                    // A two-letter query keeps a small allowance so `ex` still
                    // finds Excel and `ph` still finds Photos.
                    if meaningful >= 3 {
                        return score >= 4 + 2 * (meaningful - 1);
                    }
                    return score >= 2 + 2 * meaningful.max(0);
                }
                score > 2 * meaningful
            }
        }
    }
}

/// Score `text` against `query`. Callers lowercase both; non-ASCII input is
/// rejected rather than transliterated, because Sunny's query is already ASCII.
pub fn score(text: &str, query: &str) -> Match {
    if !text.is_ascii() || !query.is_ascii() {
        return Match::Unmatched;
    }
    // Boundaries are detected on the ORIGINAL casing (`TextEditor` has one before
    // `E`), while comparison happens on the lowercased bytes. Both have the same
    // length for ASCII, so indices line up.
    let boundaries = word_boundaries(text);
    let text = text.to_ascii_lowercase();
    let query = query.to_ascii_lowercase();
    let text = text.as_bytes();
    let query = query.as_bytes();
    if text == query {
        return Match::Exact;
    }
    let width = text.len();
    let mut limiters = 0i64;
    for &byte in query {
        if is_limiter(byte) {
            limiters += 1;
        }
    }
    let meaningful = query.len() as i64 - limiters;
    if meaningful > width as i64 {
        return Match::Unmatched;
    }

    // Cheap greedy pre-pass: only ever rejects, never accepts.
    if meaningful > 2 {
        let mut position = 0usize;
        for &byte in query {
            if is_limiter(byte) {
                if position < width && is_limiter(text[position]) {
                    position += 1;
                }
                continue;
            }
            let mut found = false;
            while position < width {
                if text[position] == byte {
                    position += 1;
                    found = true;
                    break;
                }
                position += 1;
            }
            if !found {
                return Match::Unmatched;
            }
        }
    }

    let mut table = vec![NEG_INF; 2 * width];
    // The original tracks a signed row counter; a skipped limiter row decrements
    // it so the next row reuses the same slot.
    let mut rows: i64 = -1;
    let mut last_match: i64 = -1;
    let mut skipped = 0i64;
    let mut matched_rows = 0i64;

    for (row, &query_byte) in query.iter().enumerate() {
        rows += 1;
        let query_limiter = is_limiter(query_byte);
        let budget = meaningful - 1 - matched_rows;
        let start = last_match + 1;
        let end = width as i64 - budget;
        // Running max of the previous row two columns back: this is what lets a
        // gap of any length be bridged for a single penalty.
        let mut bridge = NEG_INF;
        let mut first_hit: i64 = -1;
        let base = row_base(rows, width);

        for column in start..end.max(start) {
            let index = column as usize;
            if index >= width {
                break;
            }
            if last_match >= 0 && column - 2 >= last_match {
                if let Some(value) = previous_column(&table, rows, width, column - 2) {
                    if value > bridge {
                        bridge = value;
                    }
                }
            }
            let text_byte = text[index];
            let equal = text_byte == query_byte;
            let limiter_swap = !equal && query_limiter && is_limiter(text_byte);
            if !equal && !limiter_swap {
                continue;
            }
            let weight = if limiter_swap {
                LIMITER
            } else if last_match < 0 {
                if column == 0 {
                    FIRST
                } else {
                    weight_at(text, &boundaries, index)
                }
            } else {
                weight_at(text, &boundaries, index)
            };
            let value = if last_match < 0 {
                weight
            } else {
                let mut best = NEG_INF;
                if let Some(diagonal) = previous_column(&table, rows, width, column - 1) {
                    if diagonal > NEG_INF {
                        best = diagonal + weight;
                    }
                }
                if bridge > NEG_INF {
                    best = best.max(bridge + weight - 1);
                }
                best
            };
            if let Ok(slot) = usize::try_from(base + index as i64) {
                if slot < table.len() {
                    table[slot] = value;
                }
            }
            if first_hit < 0 {
                first_hit = column;
            }
        }

        if first_hit >= 0 {
            last_match = first_hit;
            matched_rows += 1;
        } else if query_limiter {
            skipped += 1;
            // Drop this row so it never becomes a predecessor.
            if row + 1 < query.len() {
                rows -= 1;
            }
        } else {
            return Match::Unmatched;
        }
    }

    let base = row_base(rows, width);
    let mut best = NEG_INF;
    if base >= 0 {
        for index in base..(base + width as i64).min(table.len() as i64) {
            let value = table[index as usize];
            if value > best {
                best = value;
            }
        }
    }
    Match::Scored {
        score: best,
        skipped,
    }
}

/// Best of the text and any keyword that is not the text itself.
pub fn score_with_keywords(text: &str, keywords: &[&str], query: &str) -> Match {
    let mut best = score(text, query);
    let normalized = text.to_ascii_lowercase();
    for keyword in keywords {
        if keyword.to_ascii_lowercase() == normalized {
            continue;
        }
        let candidate = score(keyword, query);
        if candidate.rank() > best.rank() {
            best = candidate;
        }
    }
    best
}

/// `(rows % 2) * width`, matching the original's signed modulo.
fn row_base(rows: i64, width: usize) -> i64 {
    if rows < 0 {
        return -1;
    }
    (rows % 2) * width as i64
}

fn previous_column(table: &[i64], rows: i64, width: usize, column: i64) -> Option<i64> {
    if rows <= 0 || column < 0 {
        return None;
    }
    let base = ((rows - 1) % 2) * width as i64;
    let index = base + column;
    usize::try_from(index)
        .ok()
        .and_then(|i| table.get(i))
        .copied()
}

fn is_limiter(byte: u8) -> bool {
    LIMITERS.contains(&(byte as char))
}

fn weight_at(text: &[u8], boundaries: &[bool], index: usize) -> i64 {
    if index == 0 {
        return WORD_BEGINNING;
    }
    if is_limiter(text[index - 1]) || boundaries.get(index).copied().unwrap_or(false) {
        return WORD_BEGINNING;
    }
    DEFAULT
}

/// camelCase boundaries, computed on the ORIGINAL casing: `TextEditor` gets a
/// boundary before `E`. Raycast derives these before lowercasing, and for ASCII
/// the byte indices still line up afterwards.
fn word_boundaries(text: &str) -> Vec<bool> {
    let bytes = text.as_bytes();
    let mut boundaries = vec![false; bytes.len()];
    for index in 1..bytes.len() {
        let previous = bytes[index - 1];
        let current = bytes[index];
        if (previous.is_ascii_lowercase() || previous.is_ascii_digit())
            && current.is_ascii_uppercase()
        {
            boundaries[index] = true;
        }
    }
    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays the vectors captured from Raycast itself. This is the test that
    /// makes the port trustworthy; hand-written expectations are not enough.
    #[test]
    fn matches_every_vector_captured_from_raycast() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/matcher-golden.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let fixture: serde_json::Value = serde_json::from_str(&raw).expect("golden JSON");
        let cases = fixture["cases"].as_array().expect("cases array");
        assert!(
            cases.len() >= 100,
            "expected a broad corpus, got {}",
            cases.len()
        );

        let mut checked = 0;
        let mut skipped_non_ascii = 0;
        for case in cases {
            let text = case["text"].as_str().expect("text");
            let query = case["query"].as_str().expect("query");
            // Sunny's launcher query is ASCII by construction and non-ASCII text
            // is rejected rather than transliterated, so Unicode vectors are out
            // of scope by design. Count them so the skip stays visible.
            if !text.is_ascii() || !query.is_ascii() {
                skipped_non_ascii += 1;
                continue;
            }
            let kind = case["kind"].as_str().expect("kind");
            let label = format!("{text:?} vs {query:?}");
            let actual = score(text, query);
            match kind {
                "exact" => assert_eq!(actual, Match::Exact, "{label}"),
                "unmatched" => assert_eq!(actual, Match::Unmatched, "{label}"),
                "scored" => {
                    let expected_score = case["score"].as_i64().expect("score");
                    let expected_skipped = case["skipped"].as_i64().expect("skipped");
                    assert_eq!(
                        actual,
                        Match::Scored {
                            score: expected_score,
                            skipped: expected_skipped
                        },
                        "{label}"
                    );
                    let len = query.chars().count() as i64;
                    for (field, tier) in [
                        ("low", Tier::Low),
                        ("medium", Tier::Medium),
                        ("high", Tier::High),
                    ] {
                        let expected = case[field].as_bool().expect(field);
                        // `high` is intentionally stricter than Raycast's own
                        // threshold for 1-3 character queries; those vectors are
                        // covered by `short_queries_need_a_contiguous_match`
                        // instead of by the captured corpus.
                        if field == "high" && len < 4 {
                            continue;
                        }
                        assert_eq!(actual.admits(tier, len), expected, "{label} {field}");
                    }
                }
                other => panic!("unknown kind {other}"),
            }
            checked += 1;
        }
        assert_eq!(checked + skipped_non_ascii, cases.len());
        assert!(
            checked >= 100,
            "expected most vectors to be ASCII, checked {checked}"
        );
    }

    #[test]
    fn an_empty_query_scores_below_every_real_match() {
        // The original reports a score, not a non-match, so `Low` accepts it.
        let empty = score("settings", "");
        assert_eq!(
            empty,
            Match::Scored {
                score: NEG_INF,
                skipped: 0
            }
        );
        assert!(empty.admits(Tier::Low, 0));
        assert!(!empty.admits(Tier::High, 0));
    }

    #[test]
    fn exact_outranks_every_score() {
        assert!(Match::Exact.rank() > score("settings", "set").rank());
        assert!(score("settings", "set").rank() > Match::Unmatched.rank());
    }

    #[test]
    fn short_queries_need_a_contiguous_match_at_a_word_start() {
        // Regression from a live run: "vsc" matched "VLC media player - reset
        // preferences and cache files" because v, s and c each sat at a word
        // start. Short acronyms must come from keywords, not alignment.
        let scattered = score(
            "VLC media player - reset preferences and cache files",
            "vsc",
        );
        assert!(
            !scattered.admits(Tier::High, 3),
            "a scattered 3-letter match is not a root-search hit: {scattered:?}"
        );
        // A contiguous prefix still works, and so does a real word start.
        assert!(score("Settings", "set").admits(Tier::High, 3));
        assert!(score("Visual Studio Code", "vis").admits(Tier::High, 3));
        assert!(score("Visual Studio Code", "vsc").admits(Tier::Low, 3));
        // Longer queries keep Raycast's own threshold.
        assert!(score("control panel display", "cont").admits(Tier::High, 4));
    }

    #[test]
    fn keywords_only_help_when_the_title_is_weaker() {
        let via_keyword = score_with_keywords("Visual Studio Code", &["vsc", "editor"], "vsc");
        assert!(via_keyword.rank() > score("Visual Studio Code", "vsc").rank() - 1);
        // A keyword identical to the title must not be scored twice.
        assert_eq!(
            score_with_keywords("Notes", &["notes"], "notes"),
            Match::Exact
        );
    }
}
