//! Quick web search: a query ending in `?` is handed to a search engine.
//!
//! Sunny itself performs no network access here. The generated URL is opened in
//! the user's browser, so this stays outside the "no network calls" rule while
//! still being instant.

use serde::{Deserialize, Serialize};

/// One selectable search destination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchEngine {
    /// Stable identifier stored in settings.
    pub id: String,
    /// Human label shown in the launcher and Settings.
    pub name: String,
    /// URL template; `{query}` is replaced with the encoded query.
    pub template: String,
    /// Optional extra token appended to the query (for example `" site:reddit.com"`).
    #[serde(default)]
    pub suffix: String,
}

impl SearchEngine {
    /// Build the final URL. The query is percent-encoded, so spaces, `&`, `#`,
    /// and non-ASCII text cannot break the URL or inject parameters.
    pub fn url(&self, query: &str) -> String {
        let mut full = query.trim().to_string();
        if !self.suffix.trim().is_empty() {
            full.push_str(self.suffix.trim_end());
        }
        self.template.replace("{query}", &urlencoding::encode(&full))
    }
}

/// Engines offered out of the box, in the order they should be offered.
pub fn default_engines() -> Vec<SearchEngine> {
    vec![
        engine("google", "Google", "https://www.google.com/search?q={query}"),
        engine("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q={query}"),
        engine("bing", "Bing", "https://www.bing.com/search?q={query}"),
        engine(
            "youtube",
            "YouTube",
            "https://www.youtube.com/results?search_query={query}",
        ),
        engine(
            "github",
            "GitHub",
            "https://github.com/search?q={query}&type=repositories",
        ),
        engine(
            "stackoverflow",
            "Stack Overflow",
            "https://stackoverflow.com/search?q={query}",
        ),
        engine(
            "wikipedia",
            "Wikipedia",
            "https://en.wikipedia.org/w/index.php?search={query}",
        ),
    ]
}

fn engine(id: &str, name: &str, template: &str) -> SearchEngine {
    SearchEngine {
        id: id.into(),
        name: name.into(),
        template: template.into(),
        suffix: String::new(),
    }
}

/// Look up an engine by id, falling back to Google for unknown ids so a
/// hand-edited settings file cannot disable the feature entirely.
pub fn engine_by_id(id: &str) -> SearchEngine {
    default_engines()
        .into_iter()
        .find(|engine| engine.id.eq_ignore_ascii_case(id))
        .unwrap_or_else(|| {
            engine("google", "Google", "https://www.google.com/search?q={query}")
        })
}

/// Why a query is not a web search. Returned instead of `None` so the UI can
/// explain itself rather than silently doing nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotASearch {
    MissingQuestionMark,
    EmptyQuery,
    TooShort,
}

/// Parse a raw launcher query into `(text to search, had_question_mark)`.
///
/// `?` may appear at the end (`rust ownership?`) or at the start (`?rust`), which
/// is the more natural way to type it on a keyboard.
pub fn split_query(raw: &str) -> (String, bool) {
    let trimmed = raw.trim();
    let marked = trimmed.ends_with('?') || trimmed.starts_with('?');
    let text = trimmed.trim_matches('?').trim().to_string();
    (text, marked)
}

/// Validate a marked query and produce its URL.
pub fn prepare(raw: &str, engine: &SearchEngine) -> Result<(String, String), NotASearch> {
    let (text, marked) = split_query(raw);
    if !marked {
        return Err(NotASearch::MissingQuestionMark);
    }
    if text.is_empty() {
        return Err(NotASearch::EmptyQuery);
    }
    if text.chars().count() < 2 {
        return Err(NotASearch::TooShort);
    }
    Ok((text.clone(), engine.url(&text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_mark_works_at_either_end() {
        assert_eq!(split_query("rust ownership?"), ("rust ownership".into(), true));
        assert_eq!(split_query("?rust ownership"), ("rust ownership".into(), true));
        assert_eq!(split_query("  rust??  "), ("rust".into(), true));
        assert_eq!(split_query("rust"), ("rust".into(), false));
    }

    #[test]
    fn spaces_and_symbols_are_percent_encoded() {
        let google = engine_by_id("google");
        let (_, url) = prepare("c++ & rust #1?", &google).unwrap();
        assert_eq!(url, "https://www.google.com/search?q=c%2B%2B%20%26%20rust%20%231");
        // No raw separator can leak into the query string.
        assert!(!url.split("q=").nth(1).unwrap().contains('&'));
    }

    #[test]
    fn unicode_queries_survive_encoding() {
        let (_, url) = prepare("harga emas hari ini?", &engine_by_id("google")).unwrap();
        assert!(url.starts_with("https://www.google.com/search?q="));
        assert!(!url.contains(' '));
    }

    #[test]
    fn empty_or_too_short_queries_are_rejected() {
        let google = engine_by_id("google");
        assert_eq!(prepare("?", &google), Err(NotASearch::EmptyQuery));
        assert_eq!(prepare("?  ", &google), Err(NotASearch::EmptyQuery));
        assert_eq!(prepare("?a", &google), Err(NotASearch::TooShort));
        assert_eq!(prepare("rust", &google), Err(NotASearch::MissingQuestionMark));
    }

    #[test]
    fn every_default_engine_builds_a_usable_url() {
        for engine in default_engines() {
            let (_, url) = prepare("test query?", &engine).unwrap();
            assert!(url.starts_with("https://"), "{} must be https", engine.id);
            assert!(url.contains("test%20query"), "{} lost the query", engine.id);
            assert!(!url.contains("{query}"), "{} left a placeholder", engine.id);
        }
    }

    #[test]
    fn unknown_engine_falls_back_to_google() {
        assert_eq!(engine_by_id("nope").id, "google");
        assert_eq!(engine_by_id("DUCKDUCKGO").id, "duckduckgo");
    }

    #[test]
    fn suffix_is_appended_before_encoding() {
        let mut reddit = engine_by_id("google");
        reddit.suffix = " site:reddit.com".into();
        let (_, url) = prepare("tcoder?", &reddit).unwrap();
        assert!(url.contains("tcoder%20site%3Areddit.com"));
    }
}