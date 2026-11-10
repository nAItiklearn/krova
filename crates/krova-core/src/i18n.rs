//! Native strings (tray menu, notifications, overlay labels) by id, with the
//! language choice and a per-key fallback to English.
//!
//! Tables are flat JSON objects `{"tray.quit": "Quit Krova", ...}`. Keys that
//! start with `_` are notes for translators and are never shown. A table that
//! lacks a key (or has an empty value for it) falls back to the English table,
//! and a key that English lacks too is shown as the key itself, so a missing
//! string is visible but never crashes anything.

use std::collections::HashMap;

/// The `Settings.language` value that follows the operating system.
pub const SYSTEM: &str = "system";
/// The source language every other table falls back to.
pub const ENGLISH: &str = "en";

/// Every value `Settings.language` may hold, with the name shown in the picker
/// (each language in its own script).
pub const LANGUAGES: [(&str, &str); 6] = [
    (SYSTEM, "System"),
    (ENGLISH, "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("ja", "日本語"),
    ("zh-Hans", "简体中文"),
];

/// Largest table accepted, and longest single string.
pub const MAX_TABLE_BYTES: usize = 256 * 1024;
pub const MAX_STRING_CHARS: usize = 500;

/// `Settings.language` forced to a known value (`"system"` otherwise).
pub fn sanitize_language(value: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(id, _)| *id == value)
        .map_or(SYSTEM, |(id, _)| id)
        .to_owned()
}

/// The table for a Windows UI language id (`GetUserDefaultUILanguage`): the
/// primary language in the low 10 bits, the sub-language above. Simplified
/// Chinese is China and Singapore; Traditional Chinese has no table yet and gets
/// English, like every other language Krova does not ship.
pub fn from_langid(langid: u16) -> &'static str {
    let primary = langid & 0x3ff;
    let sub = langid >> 10;
    match primary {
        0x0a => "es",
        0x07 => "de",
        0x11 => "ja",
        // SUBLANG_CHINESE_SIMPLIFIED (PRC) = 2, SUBLANG_CHINESE_SINGAPORE = 4.
        0x04 if sub == 0x02 || sub == 0x04 => "zh-Hans",
        _ => ENGLISH,
    }
}

/// The language the strings are shown in: the setting, or for `"system"` the
/// OS language (English when it is unknown or not shipped).
pub fn resolve(setting: &str, system_langid: Option<u16>) -> &'static str {
    match LANGUAGES.iter().find(|(id, _)| *id == setting) {
        Some((id, _)) if *id != SYSTEM => id,
        _ => system_langid.map_or(ENGLISH, from_langid),
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TableError {
    #[error("the string table is larger than {MAX_TABLE_BYTES} bytes")]
    TooLarge,
    #[error("the string table is not a JSON object: {0}")]
    NotAnObject(String),
}

/// One language's strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    strings: HashMap<String, String>,
}

impl Table {
    /// Parse a table. Non-string values, `_` notes, empty strings and control
    /// characters other than newlines are dropped; overlong strings are cut.
    pub fn parse(json: &str) -> Result<Self, TableError> {
        if json.len() > MAX_TABLE_BYTES {
            return Err(TableError::TooLarge);
        }
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| TableError::NotAnObject(e.to_string()))?;
        let serde_json::Value::Object(map) = value else {
            return Err(TableError::NotAnObject("top level".to_owned()));
        };
        let strings = map
            .into_iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .filter_map(|(k, v)| {
                let s: String = v
                    .as_str()?
                    .chars()
                    .filter(|c| *c == '\n' || !c.is_control())
                    .take(MAX_STRING_CHARS)
                    .collect();
                (!s.trim().is_empty()).then_some((k, s))
            })
            .collect();
        Ok(Self { strings })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(String::as_str)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.strings.keys().map(String::as_str)
    }
}

/// `table`'s string for `key`, else English's, else the key itself.
pub fn lookup<'a>(table: Option<&'a Table>, english: &'a Table, key: &'a str) -> &'a str {
    table
        .and_then(|t| t.get(key))
        .or_else(|| english.get(key))
        .unwrap_or(key)
}

/// Replace `{name}` placeholders with their values. Unknown placeholders stay as
/// they are, so a translation that names a wrong one still reads sensibly.
pub fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = text.to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_sanitised_to_the_known_list() {
        assert_eq!(sanitize_language("de"), "de");
        assert_eq!(sanitize_language("zh-Hans"), "zh-Hans");
        assert_eq!(sanitize_language("fr"), "system");
        assert_eq!(sanitize_language(""), "system");
        assert_eq!(sanitize_language("DE"), "system");
    }

    #[test]
    fn windows_language_ids_map_to_tables() {
        assert_eq!(from_langid(0x0409), "en"); // en-US
        assert_eq!(from_langid(0x0809), "en"); // en-GB
        assert_eq!(from_langid(0x0c0a), "es"); // es-ES
        assert_eq!(from_langid(0x080a), "es"); // es-MX
        assert_eq!(from_langid(0x0407), "de"); // de-DE
        assert_eq!(from_langid(0x0807), "de"); // de-CH
        assert_eq!(from_langid(0x0411), "ja");
        assert_eq!(from_langid(0x0804), "zh-Hans"); // zh-CN
        assert_eq!(from_langid(0x1004), "zh-Hans"); // zh-SG
        assert_eq!(from_langid(0x0404), "en"); // zh-TW: no Traditional table yet
        assert_eq!(from_langid(0x0c04), "en"); // zh-HK
        assert_eq!(from_langid(0x040c), "en"); // fr-FR
    }

    #[test]
    fn the_setting_wins_and_system_follows_the_os() {
        assert_eq!(resolve("ja", Some(0x0407)), "ja");
        assert_eq!(resolve("system", Some(0x0407)), "de");
        assert_eq!(resolve("system", None), "en");
        assert_eq!(resolve("klingon", Some(0x0411)), "ja");
    }

    #[test]
    fn tables_drop_notes_junk_and_fall_back_per_key() {
        let en = Table::parse(r#"{"_note": "x", "a": "Apple", "b": "Banana"}"#).unwrap();
        let de = Table::parse(r#"{"a": "Apfel", "b": "", "c": 5, "d": "Bell\u0007"}"#).unwrap();
        assert_eq!(en.get("_note"), None);
        assert_eq!(lookup(Some(&de), &en, "a"), "Apfel");
        assert_eq!(lookup(Some(&de), &en, "b"), "Banana", "empty falls back");
        assert_eq!(lookup(Some(&de), &en, "c"), "c", "nowhere: the key");
        assert_eq!(de.get("d"), Some("Bell"));
        assert_eq!(lookup(None, &en, "a"), "Apple");
    }

    #[test]
    fn bad_tables_are_refused_and_long_strings_cut() {
        assert!(Table::parse("[]").is_err());
        assert!(Table::parse("{").is_err());
        let huge = format!(r#"{{"a": "{}"}}"#, "x".repeat(MAX_TABLE_BYTES));
        assert_eq!(Table::parse(&huge), Err(TableError::TooLarge));
        let long = format!(r#"{{"a": "{}"}}"#, "y".repeat(MAX_STRING_CHARS + 50));
        assert_eq!(
            Table::parse(&long)
                .unwrap()
                .get("a")
                .unwrap()
                .chars()
                .count(),
            MAX_STRING_CHARS
        );
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(
            fill("{chord} is already used", &[("chord", "Ctrl+Alt+1")]),
            "Ctrl+Alt+1 is already used"
        );
        assert_eq!(fill("{other} stays", &[("chord", "x")]), "{other} stays");
    }
}
