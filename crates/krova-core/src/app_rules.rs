//! Per-app rules: which programs Krova leaves alone, floats, or sends somewhere.
//!
//! A rule is keyed by the program's **executable file name** (`chrome.exe`, `Code -
//! Insiders.exe`, `Safari`), compared without regard to case. Never by window title,
//! and never by path: titles change and can be spoofed by any web page, paths differ
//! between machines. One rule per program, one action per rule:
//!
//! * [`RuleAction::NeverTouch`]: Krova ignores the program completely, also when the
//!   user drags a window with the modifier held;
//! * [`RuleAction::Float`]: never arranged automatically; manual snaps still work;
//! * [`RuleAction::PreferredZone`]: a new window of the program goes to this zone
//!   when it is free. The number is the 1-based zone number of the keyboard
//!   shortcuts (reading order), so it keeps meaning the same thing when the layout is
//!   edited or swapped;
//! * [`RuleAction::StartOnMonitor`]: new windows of the program are sent to this
//!   monitor profile first. The id is the profile id the config uses.
//!
//! On top of the rules, [`is_arrangeable`] decides from the *kind* of window whether
//! Krova may arrange it at all. The platform layer classifies; core decides. Dialogs,
//! tool windows, owned popups and the like are never arranged, whatever the rules say.
//!
//! Everything that arrives from a config file or the UI is untrusted: [`AppRules`]
//! can be checked strictly ([`AppRules::validate`], [`AppRules::set`]) or repaired
//! ([`AppRules::sanitized`]).

use serde::{Deserialize, Serialize};

use crate::zone::{MAX_NAME_LEN, MAX_ZONES};

/// Most rules a config may hold.
pub const MAX_RULES: usize = 256;
/// Longest executable file name, in characters.
pub const MAX_EXE_NAME_LEN: usize = 128;
/// Longest monitor profile id, in characters.
pub const MAX_MONITOR_ID_LEN: usize = MAX_NAME_LEN;

/// Characters that can never be part of a file name on Windows (and that mean a path
/// or a pattern anywhere else). Rejecting them keeps paths and wildcards out.
const NOT_IN_FILE_NAMES: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("the program name is empty")]
    EmptyName,
    #[error("the program name is too long (at most 128 characters)")]
    NameTooLong,
    #[error("use the program's file name, not a path or a pattern")]
    NotAFileName,
    #[error("the program name has spaces around it")]
    SurroundingSpace,
    #[error("the text has control, invisible or direction-changing characters")]
    HiddenCharacter,
    #[error("there is already a rule for {0}")]
    Duplicate(String),
    #[error("a zone number is between 1 and 64")]
    ZoneNumber,
    #[error("the monitor id is empty or too long (at most 64 characters)")]
    MonitorId,
    #[error("too many rules (at most 256)")]
    TooManyRules,
}

/// Control characters, line and paragraph separators, zero-width characters, the
/// direction overrides, embeddings and isolates, and the other invisible format
/// characters. They have no place in a program name or an id: they make two
/// different names look the same, or reorder what the user reads.
fn is_hidden(c: char) -> bool {
    c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(
            c,
            '\u{00AD}'            // soft hyphen
            | '\u{061C}'          // Arabic letter mark
            | '\u{180E}'          // Mongolian vowel separator
            | '\u{200B}'..='\u{200F}' // zero-width space, joiners, LRM, RLM
            | '\u{2028}' | '\u{2029}' // line and paragraph separators
            | '\u{202A}'..='\u{202E}' // embeddings and overrides
            | '\u{2060}'..='\u{2064}' // word joiner, invisible operators
            | '\u{2066}'..='\u{2069}' // isolates
            | '\u{206A}'..='\u{206F}' // deprecated format characters
            | '\u{FEFF}'          // byte order mark / zero-width no-break space
            | '\u{FFF9}'..='\u{FFFB}' // interlinear annotation
            | '\u{E0000}'..='\u{E007F}' // tag characters
        )
}

/// Whether `name` is an acceptable executable file name exactly as written (use
/// [`normalize_exe_name`] to trim first).
pub fn validate_exe_name(name: &str) -> Result<(), RuleError> {
    if name.is_empty() {
        return Err(RuleError::EmptyName);
    }
    if name.chars().take(MAX_EXE_NAME_LEN + 1).count() > MAX_EXE_NAME_LEN {
        return Err(RuleError::NameTooLong);
    }
    if name.chars().any(is_hidden) {
        return Err(RuleError::HiddenCharacter);
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(if name.chars().all(|c| c == ' ') {
            RuleError::EmptyName
        } else {
            RuleError::SurroundingSpace
        });
    }
    if name.chars().any(|c| NOT_IN_FILE_NAMES.contains(&c)) || name.chars().all(|c| c == '.') {
        return Err(RuleError::NotAFileName);
    }
    Ok(())
}

/// Trim the spaces around a typed name and check it. The result keeps its case, for
/// display; matching ignores case.
pub fn normalize_exe_name(raw: &str) -> Result<String, RuleError> {
    let name = raw.trim_matches(' ');
    validate_exe_name(name)?;
    Ok(name.to_owned())
}

/// The file name at the end of a Windows or Unix path: everything after the last `\`
/// or `/`. For platform code that has a full path and needs the key.
pub fn exe_file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Case-insensitive equality of two program names, without allocating.
fn same_exe(a: &str, b: &str) -> bool {
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// Check a monitor profile id: non-empty, short, nothing hidden.
fn validate_monitor_id(id: &str) -> Result<(), RuleError> {
    if id.is_empty() || id.chars().take(MAX_MONITOR_ID_LEN + 1).count() > MAX_MONITOR_ID_LEN {
        return Err(RuleError::MonitorId);
    }
    if id.chars().any(is_hidden) {
        return Err(RuleError::HiddenCharacter);
    }
    Ok(())
}

/// What to do with a program's windows. Serialised as `{"kind": "float"}` or, for
/// the two that carry a value, `{"kind": "preferred_zone", "value": 3}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuleAction {
    NeverTouch,
    Float,
    /// 1-based zone number in reading order.
    PreferredZone(u32),
    /// A monitor profile id.
    StartOnMonitor(String),
}

impl RuleAction {
    fn validate(&self) -> Result<(), RuleError> {
        match self {
            RuleAction::NeverTouch | RuleAction::Float => Ok(()),
            RuleAction::PreferredZone(n) => {
                if (1..=MAX_ZONES as u32).contains(n) {
                    Ok(())
                } else {
                    Err(RuleError::ZoneNumber)
                }
            }
            RuleAction::StartOnMonitor(id) => validate_monitor_id(id),
        }
    }

    /// Never placed by auto-arrange (`NeverTouch` and `Float`).
    pub fn excludes_from_arrange(&self) -> bool {
        matches!(self, RuleAction::NeverTouch | RuleAction::Float)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRule {
    /// The executable file name, as the user typed it.
    pub exe: String,
    pub action: RuleAction,
}

impl AppRule {
    pub fn new(exe: &str, action: RuleAction) -> Self {
        Self {
            exe: exe.to_owned(),
            action,
        }
    }

    /// The name must be a valid file name exactly as stored; the action must be
    /// within its limits.
    pub fn validate(&self) -> Result<(), RuleError> {
        validate_exe_name(&self.exe)?;
        self.action.validate()
    }
}

/// What kind of window the platform found. The platform only classifies; whether a
/// kind may be arranged is decided here by [`is_arrangeable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// An ordinary top-level window of an application.
    Normal,
    /// A dialog or message box.
    Dialog,
    /// A tool window: palette, floating toolbar, inspector.
    Tool,
    /// A window owned by another one: menus, drop-downs, tooltips, pop-ups.
    OwnedPopup,
    /// A splash screen or loading window.
    Splash,
    /// A toast, an on-screen display or another notification.
    Notification,
    /// A window covering its whole monitor: a game, a video, a presentation.
    Fullscreen,
    /// The desktop, the taskbar, the dock or another part of the shell.
    Shell,
    /// The platform could not tell. Treated with caution: not arranged.
    Unknown,
}

/// Whether windows of this kind may ever be arranged. Only ordinary windows.
pub fn is_arrangeable(kind: WindowKind) -> bool {
    match kind {
        WindowKind::Normal => true,
        WindowKind::Dialog
        | WindowKind::Tool
        | WindowKind::OwnedPopup
        | WindowKind::Splash
        | WindowKind::Notification
        | WindowKind::Fullscreen
        | WindowKind::Shell
        | WindowKind::Unknown => false,
    }
}

/// Why a window is not arranged. For the "Not arranged" chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Its kind is never arranged.
    Kind(WindowKind),
    /// A "never touch" rule.
    NeverTouch,
    /// A "float" rule.
    Float,
}

/// The verdict for a new window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision<'a> {
    /// Arrange it. `preferred_zone` feeds `ArrangeEvent::WindowOpened`;
    /// `start_monitor` names the monitor profile to start on, if a rule says so.
    Arrange {
        preferred_zone: Option<u32>,
        start_monitor: Option<&'a str>,
    },
    Skip(SkipReason),
}

/// The rules, in the order the user made them. Serialised as a plain array.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppRules {
    rules: Vec<AppRule>,
}

impl AppRules {
    pub fn new() -> Self {
        Self::default()
    }

    /// Strictly checked construction, for rules typed in the UI.
    pub fn from_rules(rules: Vec<AppRule>) -> Result<Self, RuleError> {
        let rules = Self { rules };
        rules.validate()?;
        Ok(rules)
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, AppRule> {
        self.rules.iter()
    }

    pub fn as_slice(&self) -> &[AppRule] {
        &self.rules
    }

    /// Check the whole set: at most [`MAX_RULES`], every rule valid, no program twice
    /// (ignoring case).
    pub fn validate(&self) -> Result<(), RuleError> {
        if self.rules.len() > MAX_RULES {
            return Err(RuleError::TooManyRules);
        }
        for (i, rule) in self.rules.iter().enumerate() {
            rule.validate()?;
            if self.rules[..i].iter().any(|r| same_exe(&r.exe, &rule.exe)) {
                return Err(RuleError::Duplicate(rule.exe.clone()));
            }
        }
        Ok(())
    }

    /// Repair a set that came from a file: trim names, drop rules that are invalid
    /// and later duplicates of a program, keep the first [`MAX_RULES`]. Never fails.
    pub fn sanitized(self) -> Self {
        let mut out: Vec<AppRule> = Vec::new();
        for mut rule in self.rules {
            if out.len() >= MAX_RULES {
                break;
            }
            rule.exe = rule.exe.trim_matches(' ').to_owned();
            if rule.validate().is_err() || out.iter().any(|r| same_exe(&r.exe, &rule.exe)) {
                continue;
            }
            out.push(rule);
        }
        Self { rules: out }
    }

    /// The rule for a program, ignoring case. `exe` is a file name; a path never
    /// matches (use [`exe_file_name`] first).
    pub fn lookup(&self, exe: &str) -> Option<&AppRule> {
        self.rules.iter().find(|r| same_exe(&r.exe, exe))
    }

    /// Add a rule, or replace the program's existing rule in place. The name is
    /// trimmed and checked first; on an error nothing changes.
    pub fn set(&mut self, rule: AppRule) -> Result<(), RuleError> {
        let rule = AppRule {
            exe: normalize_exe_name(&rule.exe)?,
            action: rule.action,
        };
        rule.validate()?;
        match self.rules.iter().position(|r| same_exe(&r.exe, &rule.exe)) {
            Some(i) => self.rules[i] = rule,
            None => {
                if self.rules.len() >= MAX_RULES {
                    return Err(RuleError::TooManyRules);
                }
                self.rules.push(rule);
            }
        }
        Ok(())
    }

    /// Delete the program's rule. Returns whether there was one.
    pub fn remove(&mut self, exe: &str) -> bool {
        match self.rules.iter().position(|r| same_exe(&r.exe, exe)) {
            Some(i) => {
                self.rules.remove(i);
                true
            }
            None => false,
        }
    }

    /// May the user snap this program's windows by hand? False only for "never touch".
    pub fn allows_manual_snap(&self, exe: Option<&str>) -> bool {
        !exe.and_then(|e| self.lookup(e))
            .is_some_and(|r| r.action == RuleAction::NeverTouch)
    }

    /// Decide what to do with a new window of kind `kind` run by `exe` (`None` when
    /// the platform could not find the program).
    pub fn decide(&self, kind: WindowKind, exe: Option<&str>) -> Decision<'_> {
        if !is_arrangeable(kind) {
            return Decision::Skip(SkipReason::Kind(kind));
        }
        match exe.and_then(|e| self.lookup(e)).map(|r| &r.action) {
            Some(RuleAction::NeverTouch) => Decision::Skip(SkipReason::NeverTouch),
            Some(RuleAction::Float) => Decision::Skip(SkipReason::Float),
            Some(RuleAction::PreferredZone(n)) => Decision::Arrange {
                preferred_zone: Some(*n),
                start_monitor: None,
            },
            Some(RuleAction::StartOnMonitor(id)) => Decision::Arrange {
                preferred_zone: None,
                start_monitor: Some(id.as_str()),
            },
            None => Decision::Arrange {
                preferred_zone: None,
                start_monitor: None,
            },
        }
    }
}

impl<'a> IntoIterator for &'a AppRules {
    type Item = &'a AppRule;
    type IntoIter = std::slice::Iter<'a, AppRule>;

    fn into_iter(self) -> Self::IntoIter {
        self.rules.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(exe: &str, action: RuleAction) -> AppRule {
        AppRule::new(exe, action)
    }

    fn float(exe: &str) -> AppRule {
        rule(exe, RuleAction::Float)
    }

    fn rules(items: Vec<AppRule>) -> AppRules {
        AppRules { rules: items }
    }

    // ----- names -----

    #[test]
    fn ordinary_program_names_are_accepted() {
        for name in [
            "chrome.exe",
            "Code - Insiders.exe",
            "Safari",
            "7zFM.exe",
            "git-bash.exe",
            "notepad++.exe",
            "my app (x86).exe",
            "übung.exe",
            "メモ帳.exe",
            "a",
            "name.with.dots.exe",
        ] {
            assert_eq!(validate_exe_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn empty_and_blank_names_are_refused() {
        assert_eq!(validate_exe_name(""), Err(RuleError::EmptyName));
        assert_eq!(validate_exe_name("   "), Err(RuleError::EmptyName));
        assert_eq!(normalize_exe_name("   "), Err(RuleError::EmptyName));
        assert_eq!(normalize_exe_name(""), Err(RuleError::EmptyName));
    }

    #[test]
    fn surrounding_spaces_are_refused_by_validate_and_trimmed_by_normalize() {
        assert_eq!(
            validate_exe_name(" a.exe"),
            Err(RuleError::SurroundingSpace)
        );
        assert_eq!(
            validate_exe_name("a.exe "),
            Err(RuleError::SurroundingSpace)
        );
        assert_eq!(normalize_exe_name("  a.exe  ").as_deref(), Ok("a.exe"));
        assert_eq!(normalize_exe_name("a b.exe").as_deref(), Ok("a b.exe"));
    }

    #[test]
    fn the_length_limit_counts_characters_not_bytes() {
        assert_eq!(validate_exe_name(&"a".repeat(MAX_EXE_NAME_LEN)), Ok(()));
        assert_eq!(
            validate_exe_name(&"a".repeat(MAX_EXE_NAME_LEN + 1)),
            Err(RuleError::NameTooLong)
        );
        assert_eq!(validate_exe_name(&"é".repeat(MAX_EXE_NAME_LEN)), Ok(()));
        assert_eq!(
            validate_exe_name(&"é".repeat(MAX_EXE_NAME_LEN + 1)),
            Err(RuleError::NameTooLong)
        );
        assert_eq!(
            validate_exe_name(&"é".repeat(100_000)),
            Err(RuleError::NameTooLong)
        );
    }

    #[test]
    fn paths_and_patterns_are_not_file_names() {
        for name in [
            r"C:\Windows\notepad.exe",
            "/usr/bin/vim",
            "dir/app",
            r"dir\app",
            "app:stream",
            "*.exe",
            "app?.exe",
            "a\"b",
            "<app>",
            "a|b",
            ".",
            "..",
            "...",
        ] {
            assert_eq!(
                validate_exe_name(name),
                Err(RuleError::NotAFileName),
                "{name}"
            );
        }
    }

    #[test]
    fn control_characters_are_refused() {
        for c in [
            '\0', '\n', '\r', '\t', '\u{7f}', '\u{1b}', '\u{85}', '\u{9f}',
        ] {
            let name = format!("a{c}b.exe");
            assert_eq!(
                validate_exe_name(&name),
                Err(RuleError::HiddenCharacter),
                "{c:?}"
            );
        }
    }

    #[test]
    fn zero_width_and_direction_characters_are_refused() {
        let required = [
            '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}', '\u{202A}', '\u{202B}',
            '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
            '\u{2028}', '\u{2029}',
        ];
        let extra = [
            '\u{200E}',
            '\u{200F}',
            '\u{061C}',
            '\u{00AD}',
            '\u{180E}',
            '\u{2061}',
            '\u{2064}',
            '\u{206A}',
            '\u{206F}',
            '\u{FFF9}',
            '\u{E0041}',
            '\u{00A0}',
            '\u{3000}',
            '\u{2003}',
        ];
        for c in required.into_iter().chain(extra) {
            for name in [
                format!("a{c}b.exe"),
                format!("{c}a.exe"),
                format!("a.exe{c}"),
            ] {
                assert_eq!(
                    validate_exe_name(&name),
                    Err(RuleError::HiddenCharacter),
                    "U+{:04X} in {name:?}",
                    u32::from(c)
                );
            }
        }
    }

    #[test]
    fn a_right_to_left_override_cannot_disguise_an_extension() {
        // "photo" + RLO + "gpj.exe" would display as "photoexe.jpg".
        assert_eq!(
            validate_exe_name("photo\u{202E}gpj.exe"),
            Err(RuleError::HiddenCharacter)
        );
    }

    #[test]
    fn a_file_name_comes_out_of_a_path() {
        assert_eq!(exe_file_name(r"C:\Program Files\App\app.exe"), "app.exe");
        assert_eq!(
            exe_file_name("/Applications/Safari.app/Contents/MacOS/Safari"),
            "Safari"
        );
        assert_eq!(exe_file_name("plain.exe"), "plain.exe");
        assert_eq!(exe_file_name(""), "");
        assert_eq!(exe_file_name(r"C:\dir\"), "");
        assert_eq!(exe_file_name("a/b\\c.exe"), "c.exe");
    }

    // ----- lookup -----

    #[test]
    fn lookup_ignores_case() {
        let r = rules(vec![float("Chrome.EXE")]);
        assert!(r.lookup("chrome.exe").is_some());
        assert!(r.lookup("CHROME.EXE").is_some());
        assert!(r.lookup("Chrome.EXE").is_some());
        assert!(r.lookup("chrome").is_none(), "no extension guessing");
        assert!(r.lookup("chrome.exe ").is_none());
        let u = rules(vec![float("ÜBUNG.exe")]);
        assert!(u.lookup("übung.exe").is_some());
        assert!(AppRules::new().lookup("chrome.exe").is_none());
        assert!(r.lookup("").is_none());
    }

    #[test]
    fn lookup_never_matches_a_path_or_a_title() {
        let r = rules(vec![float("app.exe")]);
        assert!(r.lookup(r"C:\x\app.exe").is_none());
        assert!(r.lookup("app.exe - Untitled").is_none());
        assert!(r.lookup("Untitled - app.exe").is_none());
        assert!(r.lookup("xapp.exe").is_none());
    }

    #[test]
    fn lookup_returns_the_first_matching_rule() {
        // A hand-edited file can hold duplicates; the first one wins until sanitized.
        let r = rules(vec![
            rule("a.exe", RuleAction::NeverTouch),
            rule("A.EXE", RuleAction::Float),
        ]);
        assert_eq!(r.lookup("a.exe").unwrap().action, RuleAction::NeverTouch);
    }

    // ----- limits -----

    #[test]
    fn at_most_256_rules() {
        let at_limit: Vec<AppRule> = (0..MAX_RULES)
            .map(|i| float(&format!("app{i}.exe")))
            .collect();
        assert_eq!(
            AppRules::from_rules(at_limit.clone()).unwrap().len(),
            MAX_RULES
        );
        let mut over = at_limit;
        over.push(float("one-too-many.exe"));
        assert_eq!(
            AppRules::from_rules(over.clone()),
            Err(RuleError::TooManyRules)
        );
        assert_eq!(AppRules { rules: over }.sanitized().len(), MAX_RULES);
    }

    #[test]
    fn set_stops_at_the_limit_but_still_replaces() {
        let mut r = AppRules::new();
        for i in 0..MAX_RULES {
            r.set(float(&format!("app{i}.exe"))).unwrap();
        }
        assert_eq!(r.set(float("extra.exe")), Err(RuleError::TooManyRules));
        assert_eq!(r.len(), MAX_RULES);
        r.set(rule("APP0.exe", RuleAction::NeverTouch)).unwrap();
        assert_eq!(r.len(), MAX_RULES);
        assert_eq!(r.lookup("app0.exe").unwrap().action, RuleAction::NeverTouch);
    }

    #[test]
    fn duplicates_ignoring_case_are_refused() {
        let r = AppRules::from_rules(vec![float("a.exe"), float("A.exe")]);
        assert_eq!(r, Err(RuleError::Duplicate("A.exe".into())));
        assert!(AppRules::from_rules(vec![float("a.exe"), float("b.exe")]).is_ok());
    }

    #[test]
    fn zone_numbers_run_from_one_to_the_zone_limit() {
        for n in [1, 2, 64] {
            assert_eq!(
                rule("a.exe", RuleAction::PreferredZone(n)).validate(),
                Ok(())
            );
        }
        for n in [0, 65, 1000, u32::MAX] {
            assert_eq!(
                rule("a.exe", RuleAction::PreferredZone(n)).validate(),
                Err(RuleError::ZoneNumber),
                "{n}"
            );
        }
    }

    #[test]
    fn monitor_ids_must_be_short_and_visible() {
        let ok = |id: &str| rule("a.exe", RuleAction::StartOnMonitor(id.into())).validate();
        assert_eq!(ok("monitor-1"), Ok(()));
        assert_eq!(ok(&"m".repeat(MAX_MONITOR_ID_LEN)), Ok(()));
        assert_eq!(ok(""), Err(RuleError::MonitorId));
        assert_eq!(
            ok(&"m".repeat(MAX_MONITOR_ID_LEN + 1)),
            Err(RuleError::MonitorId)
        );
        assert_eq!(ok("mon\u{0}itor"), Err(RuleError::HiddenCharacter));
        assert_eq!(ok("mon\u{200B}itor"), Err(RuleError::HiddenCharacter));
        assert_eq!(ok("mon\u{202E}itor"), Err(RuleError::HiddenCharacter));
    }

    // ----- editing -----

    #[test]
    fn set_adds_trims_and_replaces_in_place() {
        let mut r = AppRules::new();
        r.set(float("  a.exe ")).unwrap();
        r.set(float("b.exe")).unwrap();
        assert_eq!(r.as_slice()[0].exe, "a.exe", "trimmed");
        r.set(rule("A.EXE", RuleAction::PreferredZone(2))).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r.as_slice()[0], rule("A.EXE", RuleAction::PreferredZone(2)));
        assert_eq!(
            r.as_slice()[1].exe,
            "b.exe",
            "the order of the others is kept"
        );
    }

    #[test]
    fn set_refuses_bad_rules_and_changes_nothing() {
        let mut r = AppRules::new();
        r.set(float("a.exe")).unwrap();
        let before = r.clone();
        assert!(r.set(float("")).is_err());
        assert!(r.set(float(r"C:\a.exe")).is_err());
        assert!(r.set(rule("b.exe", RuleAction::PreferredZone(0))).is_err());
        assert!(r
            .set(rule("b.exe", RuleAction::StartOnMonitor(String::new())))
            .is_err());
        assert_eq!(r, before);
    }

    #[test]
    fn remove_ignores_case_and_reports_whether_it_found_one() {
        let mut r = rules(vec![float("a.exe"), float("b.exe")]);
        assert!(r.remove("A.EXE"));
        assert!(!r.remove("a.exe"));
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
        assert!(r.remove("b.exe"));
        assert!(r.is_empty());
    }

    #[test]
    fn rules_are_listed_in_the_order_they_were_made() {
        let r = rules(vec![float("b.exe"), float("a.exe"), float("c.exe")]);
        let names: Vec<&str> = r.iter().map(|x| x.exe.as_str()).collect();
        assert_eq!(names, vec!["b.exe", "a.exe", "c.exe"]);
        let via_into: Vec<&str> = (&r).into_iter().map(|x| x.exe.as_str()).collect();
        assert_eq!(via_into, names);
    }

    // ----- repairing -----

    #[test]
    fn sanitizing_keeps_the_good_rules_in_order() {
        let messy = rules(vec![
            float("  keep.exe  "),
            float(""),
            float(r"C:\bad.exe"),
            rule("zone.exe", RuleAction::PreferredZone(0)),
            float("KEEP.exe"),
            float("evil\u{202E}.exe"),
            rule("mon.exe", RuleAction::StartOnMonitor("monitor-2".into())),
            float(&"x".repeat(500)),
        ]);
        let clean = messy.sanitized();
        let names: Vec<&str> = clean.iter().map(|r| r.exe.as_str()).collect();
        assert_eq!(names, vec!["keep.exe", "mon.exe"]);
        assert_eq!(clean.validate(), Ok(()));
        assert_eq!(
            clean.clone().sanitized(),
            clean,
            "sanitizing twice changes nothing"
        );
    }

    // ----- window kinds -----

    #[test]
    fn only_ordinary_windows_are_arrangeable() {
        use WindowKind::*;
        assert!(is_arrangeable(Normal));
        for kind in [
            Dialog,
            Tool,
            OwnedPopup,
            Splash,
            Notification,
            Fullscreen,
            Shell,
            Unknown,
        ] {
            assert!(!is_arrangeable(kind), "{kind:?}");
        }
    }

    #[test]
    fn window_kinds_travel_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&WindowKind::OwnedPopup).unwrap(),
            "\"owned_popup\""
        );
        let k: WindowKind = serde_json::from_str("\"fullscreen\"").unwrap();
        assert_eq!(k, WindowKind::Fullscreen);
        assert!(serde_json::from_str::<WindowKind>("\"balloon\"").is_err());
    }

    // ----- decisions -----

    #[test]
    fn a_window_without_a_rule_is_arranged_plainly() {
        let r = AppRules::new();
        assert_eq!(
            r.decide(WindowKind::Normal, Some("chrome.exe")),
            Decision::Arrange {
                preferred_zone: None,
                start_monitor: None
            }
        );
        assert_eq!(
            r.decide(WindowKind::Normal, None),
            Decision::Arrange {
                preferred_zone: None,
                start_monitor: None
            }
        );
    }

    #[test]
    fn dialogs_and_popups_are_skipped_whatever_the_rules_say() {
        let r = rules(vec![rule("chrome.exe", RuleAction::PreferredZone(2))]);
        for kind in [
            WindowKind::Dialog,
            WindowKind::Tool,
            WindowKind::OwnedPopup,
            WindowKind::Unknown,
        ] {
            assert_eq!(
                r.decide(kind, Some("chrome.exe")),
                Decision::Skip(SkipReason::Kind(kind))
            );
        }
    }

    #[test]
    fn never_touch_and_float_skip_with_their_own_reason() {
        let r = rules(vec![
            rule("a.exe", RuleAction::NeverTouch),
            rule("b.exe", RuleAction::Float),
        ]);
        assert_eq!(
            r.decide(WindowKind::Normal, Some("A.exe")),
            Decision::Skip(SkipReason::NeverTouch)
        );
        assert_eq!(
            r.decide(WindowKind::Normal, Some("b.exe")),
            Decision::Skip(SkipReason::Float)
        );
        assert!(RuleAction::NeverTouch.excludes_from_arrange());
        assert!(RuleAction::Float.excludes_from_arrange());
        assert!(!RuleAction::PreferredZone(1).excludes_from_arrange());
        assert!(!RuleAction::StartOnMonitor("m".into()).excludes_from_arrange());
    }

    #[test]
    fn a_preferred_zone_and_a_start_monitor_are_passed_on() {
        let r = rules(vec![
            rule("a.exe", RuleAction::PreferredZone(3)),
            rule("b.exe", RuleAction::StartOnMonitor("monitor-2".into())),
        ]);
        assert_eq!(
            r.decide(WindowKind::Normal, Some("a.exe")),
            Decision::Arrange {
                preferred_zone: Some(3),
                start_monitor: None
            }
        );
        assert_eq!(
            r.decide(WindowKind::Normal, Some("B.EXE")),
            Decision::Arrange {
                preferred_zone: None,
                start_monitor: Some("monitor-2")
            }
        );
    }

    #[test]
    fn only_never_touch_blocks_manual_snapping() {
        let r = rules(vec![
            rule("a.exe", RuleAction::NeverTouch),
            rule("b.exe", RuleAction::Float),
        ]);
        assert!(!r.allows_manual_snap(Some("a.exe")));
        assert!(r.allows_manual_snap(Some("b.exe")));
        assert!(r.allows_manual_snap(Some("other.exe")));
        assert!(r.allows_manual_snap(None));
    }

    // ----- serde -----

    #[test]
    fn rules_serialise_as_a_plain_array_with_tagged_actions() {
        let r = rules(vec![
            rule("a.exe", RuleAction::NeverTouch),
            rule("b.exe", RuleAction::Float),
            rule("c.exe", RuleAction::PreferredZone(3)),
            rule("d.exe", RuleAction::StartOnMonitor("monitor-2".into())),
        ]);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(
            json,
            r#"[{"exe":"a.exe","action":{"kind":"never_touch"}},{"exe":"b.exe","action":{"kind":"float"}},{"exe":"c.exe","action":{"kind":"preferred_zone","value":3}},{"exe":"d.exe","action":{"kind":"start_on_monitor","value":"monitor-2"}}]"#
        );
        let back: AppRules = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
        assert_eq!(serde_json::to_string(&AppRules::new()).unwrap(), "[]");
    }

    #[test]
    fn unknown_actions_and_wrong_values_do_not_deserialize() {
        for bad in [
            r#"[{"exe":"a.exe","action":{"kind":"explode"}}]"#,
            r#"[{"exe":"a.exe","action":{"kind":"preferred_zone","value":"three"}}]"#,
            r#"[{"exe":"a.exe","action":{"kind":"preferred_zone","value":-1}}]"#,
            r#"[{"exe":"a.exe"}]"#,
            r#"[{"action":{"kind":"float"}}]"#,
        ] {
            assert!(serde_json::from_str::<AppRules>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_loaded_file_is_repaired_by_sanitizing() {
        let json = r#"[
            {"exe":"ok.exe","action":{"kind":"float"}},
            {"exe":"..\\evil.exe","action":{"kind":"never_touch"}},
            {"exe":"OK.exe","action":{"kind":"never_touch"}},
            {"exe":"zone.exe","action":{"kind":"preferred_zone","value":99}}
        ]"#;
        let loaded: AppRules = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.len(), 4, "deserializing does not judge");
        assert!(loaded.validate().is_err());
        let clean = loaded.sanitized();
        assert_eq!(clean.len(), 1);
        assert_eq!(clean.as_slice()[0].action, RuleAction::Float);
    }
}
