//! Lorem ipsum generator for the launcher.
//!
//! Deterministic and dependency-free: the same seed always produces the same
//! text, so a query can be re-run and copied again without surprises.

const WORDS: [&str; 60] = [
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do",
    "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna", "aliqua", "enim",
    "ad", "minim", "veniam", "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure", "in", "reprehenderit",
    "voluptate", "velit", "esse", "cillum", "eu", "fugiat", "nulla", "pariatur", "excepteur",
    "sint", "occaecat", "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
    "deserunt", "mollit", "anim",
];

/// Opening sentence every Lorem ipsum text starts with, kept intact so the
/// output is recognisable.
const OPENING: [&str; 18] = [
    "Lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "do",
    "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Words,
    Sentences,
    Paragraphs,
}

impl Unit {
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "word" | "words" | "kata" => Some(Unit::Words),
            "sentence" | "sentences" | "kalimat" => Some(Unit::Sentences),
            "paragraph" | "paragraphs" | "paragraf" => Some(Unit::Paragraphs),
            _ => None,
        }
    }
}

/// Upper bound on generated text so a typo such as `lorem 999999 words`
/// cannot block the UI thread or allocate wildly.
const MAX_WORDS: usize = 500;

/// Parse `lorem 3 paragraphs`, `lorem 5 sentences`, `lorem 20 words`, or a bare
/// `lorem` (one paragraph). Returns the amount and the requested unit.
pub fn parse_query(query: &str) -> Option<(usize, Unit)> {
    let mut parts = query.split_whitespace();
    let first = parts.next()?;
    if !first.eq_ignore_ascii_case("lorem") && !first.eq_ignore_ascii_case("ipsum") {
        return None;
    }
    let rest: Vec<&str> = parts.collect();
    match rest.as_slice() {
        [] => Some((1, Unit::Paragraphs)),
        [amount] => Unit::parse(amount).map(|unit| (1, unit)),
        [amount, unit] => {
            let value = amount.parse::<usize>().ok().filter(|n| *n > 0)?;
            Unit::parse(unit).map(|unit| (value, unit))
        }
        // Anything longer is a different intent; do not guess.
        _ => None,
    }
}

/// Generate text for a parsed request.
pub fn generate(amount: usize, unit: Unit) -> String {
    match unit {
        Unit::Words => words(amount),
        Unit::Sentences => sentences(amount),
        Unit::Paragraphs => (0..amount)
            .map(|_| sentences(4))
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

/// Convenience for the launcher: `None` when the query is not a lorem request.
pub fn evaluate(query: &str) -> Option<String> {
    let (amount, unit) = parse_query(query)?;
    Some(generate(amount, unit))
}

fn words(count: usize) -> String {
    let count = count.clamp(1, MAX_WORDS);
    let mut out: Vec<&str> = Vec::with_capacity(count);
    for (i, word) in OPENING.iter().enumerate().take(count) {
        out.push(word);
        let _ = i;
    }
    let mut index = 0;
    while out.len() < count {
        out.push(WORDS[index % WORDS.len()]);
        index += 1;
    }
    let mut text = out.join(" ");
    text.push('.');
    text
}

fn sentences(count: usize) -> String {
    let count = count.clamp(1, MAX_WORDS / 4);
    let mut out = Vec::with_capacity(count);
    let mut cursor = 0usize;
    for i in 0..count {
        let len = 8 + (i % 5); // 8..12 words, stable without an RNG
        let mut sentence: Vec<&str> = Vec::with_capacity(len);
        if i == 0 {
            for word in OPENING.iter().take(len.min(OPENING.len())) {
                sentence.push(word);
            }
            cursor = sentence.len();
        }
        while sentence.len() < len {
            sentence.push(WORDS[cursor % WORDS.len()]);
            cursor += 1;
        }
        let mut text = sentence.join(" ");
        text.push('.');
        // Capitalise the first character of every sentence.
        let mut chars = text.chars();
        if let Some(first) = chars.next() {
            text = format!("{}{}", first.to_uppercase(), chars.as_str());
        }
        out.push(text);
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_supported_forms() {
        assert_eq!(parse_query("lorem"), Some((1, Unit::Paragraphs)));
        assert_eq!(parse_query("Lorem Ipsum"), None);
        assert_eq!(parse_query("lorem 3 paragraphs"), Some((3, Unit::Paragraphs)));
        assert_eq!(parse_query("lorem 5 sentences"), Some((5, Unit::Sentences)));
        assert_eq!(parse_query("lorem 20 words"), Some((20, Unit::Words)));
        assert_eq!(parse_query("lorem paragraphs"), Some((1, Unit::Paragraphs)));
    }

    #[test]
    fn rejects_other_queries_instead_of_guessing() {
        for query in [
            "lorem 0 words",
            "lorem abc words",
            "lorem 3 pages",
            "lorem 3 words extra",
            "hello 20 words",
            "",
            "   ",
        ] {
            assert!(parse_query(query).is_none(), "should reject {query:?}");
        }
    }

    #[test]
    fn word_count_is_exact_and_starts_with_the_classic_opening() {
        let text = generate(20, Unit::Words);
        assert_eq!(text.split_whitespace().count(), 20);
        assert!(text.starts_with("Lorem ipsum dolor sit amet"));
        assert!(text.ends_with('.'));
    }

    #[test]
    fn sentences_are_capitalised_and_terminated() {
        let text = generate(3, Unit::Sentences);
        assert_eq!(text.matches('.').count(), 3);
        assert!(text.starts_with("Lorem ipsum"));
        for sentence in text.split(". ").filter(|s| !s.trim().is_empty()) {
            let first = sentence.trim_start().chars().next().unwrap();
            assert!(first.is_uppercase(), "sentence should start uppercase: {sentence}");
        }
    }

    #[test]
    fn paragraphs_are_separated_by_a_blank_line() {
        let text = generate(3, Unit::Paragraphs);
        assert_eq!(text.split("\n\n").count(), 3);
    }

    #[test]
    fn output_is_bounded_so_a_huge_request_cannot_stall_the_launcher() {
        let text = generate(999_999, Unit::Words);
        assert!(text.split_whitespace().count() <= MAX_WORDS);
    }

    #[test]
    fn repeated_requests_are_deterministic() {
        assert_eq!(generate(4, Unit::Sentences), generate(4, Unit::Sentences));
        assert_eq!(evaluate("lorem 2 paragraphs"), evaluate("lorem 2 paragraphs"));
    }
}