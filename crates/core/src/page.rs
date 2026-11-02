//! The reader-visible strings. They live in `data/strings.json`; the core
//! checks the file before the page draws anything, and fills each
//! `{placeholder}` with a value a rule decides.

use std::collections::BTreeMap;

/// Every key the game reads. A missing one is refused by name at start.
pub const REQUIRED: &[&str] = &[
    "title",
    "intro",
    "menu.play",
    "menu.settings",
    "menu.back",
    "select.heading",
    "select.difficulty",
    "select.start",
    "select.by",
    "select.facts",
    "select.targets",
    "select.best",
    "select.unplayed",
    "difficulty.easy",
    "difficulty.normal",
    "difficulty.hard",
    "difficulty.expert",
    "difficulty.master",
    "loading",
    "loading.failed",
    "play.score",
    "play.accuracy",
    "play.combo",
    "judgement.perfect",
    "judgement.great",
    "judgement.good",
    "judgement.miss",
    "pause.heading",
    "pause.resume",
    "pause.retry",
    "pause.quit",
    "results.heading",
    "results.score",
    "results.accuracy",
    "results.combo",
    "results.counts",
    "results.holds",
    "results.timing.early",
    "results.timing.late",
    "results.timing.none",
    "results.best",
    "results.retry",
    "results.songs",
    "settings.heading",
    "settings.audio",
    "settings.visual",
    "settings.volume",
    "settings.name",
    "settings.name.note",
    "settings.less",
    "settings.more",
    "settings.calibrate",
    "calibrate.heading",
    "calibrate.intro",
    "calibrate.count",
    "calibrate.result",
    "calibrate.done",
    "calibrate.again",
];

pub type Strings = BTreeMap<String, String>;

/// Why a strings file cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringsError {
    NotJson(String),
    Missing(&'static str),
    Empty(String),
}

impl StringsError {
    pub fn message(&self) -> String {
        match self {
            StringsError::NotJson(detail) => format!("The strings file could not be read: {detail}"),
            StringsError::Missing(key) => format!("The strings file has no \"{key}\"."),
            StringsError::Empty(key) => format!("The strings file has an empty \"{key}\"."),
        }
    }
}

/// Parses and checks the strings file. A string that is only whitespace
/// counts as empty, because the page would draw nothing.
pub fn parse_strings(json: &str) -> Result<Strings, StringsError> {
    let strings: Strings = serde_json::from_str(json).map_err(|e| StringsError::NotJson(e.to_string()))?;
    for key in REQUIRED {
        match strings.get(*key) {
            None => return Err(StringsError::Missing(key)),
            Some(s) if s.trim().is_empty() => return Err(StringsError::Empty(key.to_string())),
            Some(_) => {}
        }
    }
    Ok(strings)
}

/// The string at `key` with each `{name}` replaced by its value.
pub fn fill(strings: &Strings, key: &str, values: &[(&str, String)]) -> String {
    let mut s = strings.get(key).cloned().unwrap_or_else(|| key.to_string());
    for (name, value) in values {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

/// A whole number with thin groups of three: 1234567 is "1,234,567".
pub fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Hundredths of a percent as "97.25%".
pub fn percent(hundredths: i64) -> String {
    format!("{}.{:02}%", hundredths / 100, hundredths % 100)
}

/// Microseconds as "m:ss".
pub fn minutes(us: i64) -> String {
    let s = us / 1_000_000;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete() -> String {
        let pairs: Vec<String> = REQUIRED.iter().map(|k| format!("\"{k}\": \"x\"")).collect();
        format!("{{{}}}", pairs.join(", "))
    }

    #[test]
    fn a_complete_file_is_accepted() {
        assert!(parse_strings(&complete()).is_ok());
    }

    #[test]
    fn an_empty_title_is_rejected_by_name() {
        let e = parse_strings(&complete().replace("\"title\": \"x\"", "\"title\": \"  \"")).unwrap_err();
        assert_eq!(e.message(), "The strings file has an empty \"title\".");
    }

    #[test]
    fn a_missing_key_is_rejected_by_name() {
        let e = parse_strings(&complete().replace("\"intro\": \"x\", ", "")).unwrap_err();
        assert_eq!(e, StringsError::Missing("intro"));
    }

    #[test]
    fn text_that_is_not_json_is_rejected() {
        assert!(matches!(parse_strings("title: T"), Err(StringsError::NotJson(_))));
    }

    #[test]
    fn the_shipped_strings_file_is_complete() {
        assert!(parse_strings(include_str!("../../../data/strings.json")).is_ok());
    }

    #[test]
    fn numbers_are_formatted_by_hand_worked_cases() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped(-4500), "-4,500");
        assert_eq!(percent(10_000), "100.00%");
        assert_eq!(percent(9705), "97.05%");
        assert_eq!(minutes(75_400_000), "1:15");
        let s: Strings = [("a".to_string(), "{n} of {total}".to_string())].into_iter().collect();
        assert_eq!(fill(&s, "a", &[("n", "3".into()), ("total", "8".into())]), "3 of 8");
    }
}
