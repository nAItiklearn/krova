//! The native string tables (`src-tauri/i18n/*.json`), compiled into the binary.
//!
//! English (`en.json`) is the source; the other files start as English copies
//! marked TODO and are translated by people, never by a machine. Lookups fall
//! back to English per key (`krova_core::i18n::lookup`).

use std::collections::HashMap;
use std::sync::OnceLock;

use krova_core::i18n::{fill, lookup, resolve, Table, ENGLISH};

use crate::model::{lock, Shared};

/// `(language id, file contents)` of every shipped table.
const FILES: [(&str, &str); 5] = [
    ("en", include_str!("../i18n/en.json")),
    ("es", include_str!("../i18n/es.json")),
    ("de", include_str!("../i18n/de.json")),
    ("ja", include_str!("../i18n/ja.json")),
    ("zh-Hans", include_str!("../i18n/zh-Hans.json")),
];

fn tables() -> &'static HashMap<&'static str, Table> {
    static TABLES: OnceLock<HashMap<&'static str, Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        FILES
            .iter()
            .filter_map(|(id, json)| match Table::parse(json) {
                Ok(t) => Some((*id, t)),
                Err(e) => {
                    eprintln!("krova: the {id} string table is unusable: {e}");
                    None
                }
            })
            .collect()
    })
}

/// The string `key` in `language` (a resolved id such as `"de"`), falling back
/// to English, then to the key itself.
pub fn tr(language: &str, key: &str) -> String {
    let all = tables();
    let empty = Table::default();
    let english = all.get(ENGLISH).unwrap_or(&empty);
    lookup(all.get(language), english, key).to_owned()
}

/// [`tr`] with `{name}` placeholders filled in. For the notification and overlay
/// strings (`notify.*`, `overlay.*`, `shortcut.*`), which have no native caller
/// yet: the shortcut sentences are still built in `krova-platform`, and the overlay
/// draws no text today.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tr_with(language: &str, key: &str, args: &[(&str, &str)]) -> String {
    fill(&tr(language, key), args)
}

/// The language the native strings are shown in right now: `Settings.language`,
/// or the Windows display language for `"system"`.
pub fn current_language(shared: &Shared) -> &'static str {
    let setting = lock(&shared.model).config.settings.language.clone();
    resolve(&setting, shared.platform.ui_language())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(id: &str) -> Vec<String> {
        let json = FILES.iter().find(|(l, _)| *l == id).unwrap().1;
        let mut k: Vec<String> = Table::parse(json)
            .unwrap()
            .keys()
            .map(str::to_owned)
            .collect();
        k.sort();
        k
    }

    #[test]
    fn every_shipped_table_parses_and_has_exactly_the_english_keys() {
        let english = keys("en");
        assert!(english.len() >= 10);
        for (id, _) in FILES {
            assert_eq!(keys(id), english, "{id} keys differ from en.json");
        }
        assert_eq!(tables().len(), FILES.len());
    }

    #[test]
    fn every_language_in_the_picker_has_a_table() {
        for (id, _) in krova_core::i18n::LANGUAGES {
            if id != krova_core::i18n::SYSTEM {
                assert!(FILES.iter().any(|(l, _)| *l == id), "{id}");
            }
        }
    }

    #[test]
    fn untranslated_tables_say_so() {
        for (id, json) in FILES {
            if id != ENGLISH {
                assert!(
                    json.contains("\"_status\": \"TODO"),
                    "{id} is not marked TODO"
                );
            }
        }
    }

    #[test]
    fn lookups_fall_back_and_fill_placeholders() {
        assert_eq!(tr("en", "tray.quit"), "Quit Krova");
        assert_eq!(
            tr("de", "tray.quit"),
            "Quit Krova",
            "English until translated"
        );
        assert_eq!(tr("xx", "tray.quit"), "Quit Krova");
        assert_eq!(tr("en", "no.such.key"), "no.such.key");
        assert_eq!(
            tr_with("en", "shortcut.in_use", &[("chord", "Ctrl+Alt+Shift+3")]),
            "Ctrl+Alt+Shift+3 is already used by another app"
        );
    }
}
