//! Portable layout files: export one layout as JSON, import one back.
//!
//! The file is a small envelope so it can be told apart from anything else and
//! versioned:
//!
//! ```json
//! { "krova_layout": 1, "layout": { "id": "...", "name": "...", "zones": [ ... ] } }
//! ```
//!
//! Zones are proportions of the work area (0..=1), so the same file works on
//! another monitor, another resolution and on the other operating system.
//!
//! Importing is defensive: the text is size-capped before it is parsed, the
//! envelope must have exactly these two keys and version 1, and the layout goes
//! through [`Layout::validate`] (zone count, text length, rectangles, duplicate
//! ids) before anything else sees it. Nothing here touches the disk.

use serde_json::{json, Value};

use crate::zone::{Layout, LayoutError, MAX_NAME_LEN};

/// The only envelope version this build reads and writes.
pub const LAYOUT_FILE_VERSION: u32 = 1;
/// Largest layout file accepted, in bytes. A 64-zone layout is about 5 KiB.
pub const MAX_LAYOUT_JSON_BYTES: usize = 64 * 1024;

const VERSION_KEY: &str = "krova_layout";
const LAYOUT_KEY: &str = "layout";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutIoError {
    #[error("the layout file is larger than 64 KiB")]
    TooLarge,
    #[error("this is not a valid Krova layout file: {0}")]
    Malformed(String),
    #[error("unsupported layout file version {0} (this build reads version 1)")]
    WrongVersion(String),
    #[error("the layout is not valid: {0}")]
    Invalid(#[from] LayoutError),
    #[error("the layout id or name contains control characters")]
    BadText,
}

fn has_control_chars(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// Serialise `layout` as a pretty-printed layout file. The layout is validated
/// first, so an invalid layout is never written out.
pub fn export_json(layout: &Layout) -> Result<String, LayoutIoError> {
    layout.validate()?;
    if has_control_chars(&layout.id) || has_control_chars(&layout.name) {
        return Err(LayoutIoError::BadText);
    }
    let envelope = json!({ VERSION_KEY: LAYOUT_FILE_VERSION, LAYOUT_KEY: layout });
    let text = serde_json::to_string_pretty(&envelope)
        .map_err(|e| LayoutIoError::Malformed(e.to_string()))?;
    if text.len() > MAX_LAYOUT_JSON_BYTES {
        return Err(LayoutIoError::TooLarge);
    }
    Ok(text)
}

/// `base` made unique among `existing` by a numeric suffix (`-2`, `-3`, ...). The
/// result never exceeds the id length limit.
pub fn unique_id(base: &str, existing: &[&str]) -> String {
    if !existing.contains(&base) {
        return base.to_owned();
    }
    for n in 2usize.. {
        let suffix = format!("-{n}");
        let keep = MAX_NAME_LEN.saturating_sub(suffix.chars().count());
        let candidate: String = base.chars().take(keep).chain(suffix.chars()).collect();
        if !existing.contains(&candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!("the suffix counter is unbounded")
}

/// Parse and validate a layout file. `existing_ids` are the layout ids already on
/// the target monitor; an id that is taken gets a numeric suffix.
pub fn import_json(json: &str, existing_ids: &[&str]) -> Result<Layout, LayoutIoError> {
    if json.len() > MAX_LAYOUT_JSON_BYTES {
        return Err(LayoutIoError::TooLarge);
    }
    // Editors on Windows like to put a byte order mark in front of UTF-8 text.
    let text = json.strip_prefix('\u{feff}').unwrap_or(json);
    let value: Value =
        serde_json::from_str(text).map_err(|e| LayoutIoError::Malformed(e.to_string()))?;
    let Value::Object(mut object) = value else {
        return Err(LayoutIoError::Malformed("expected a JSON object".into()));
    };

    match object.get(VERSION_KEY) {
        None => {
            return Err(LayoutIoError::Malformed(format!(
                "missing \"{VERSION_KEY}\""
            )))
        }
        Some(Value::Number(n)) if n.as_u64() == Some(u64::from(LAYOUT_FILE_VERSION)) => {}
        Some(other) => {
            // Show a number as given, anything else only by its type.
            let shown = match other {
                Value::Number(n) => n.to_string(),
                Value::String(_) => "(text)".to_owned(),
                _ => "(unknown)".to_owned(),
            };
            return Err(LayoutIoError::WrongVersion(shown));
        }
    }
    if object
        .keys()
        .any(|k| k.as_str() != VERSION_KEY && k.as_str() != LAYOUT_KEY)
    {
        return Err(LayoutIoError::Malformed("unexpected extra field".into()));
    }
    let Some(layout_value) = object.remove(LAYOUT_KEY) else {
        return Err(LayoutIoError::Malformed(format!(
            "missing \"{LAYOUT_KEY}\""
        )));
    };
    let mut layout: Layout = serde_json::from_value(layout_value)
        .map_err(|e| LayoutIoError::Malformed(e.to_string()))?;

    layout.validate()?;
    if has_control_chars(&layout.id) || has_control_chars(&layout.name) {
        return Err(LayoutIoError::BadText);
    }
    layout.id = layout.id.trim().to_owned();
    layout.name = layout.name.trim().to_owned();
    if layout.id.is_empty() {
        layout.id = "imported".to_owned();
    }
    if layout.name.is_empty() {
        layout.name = "Imported layout".to_owned();
    }
    layout.id = unique_id(&layout.id, existing_ids);
    // Trimming and suffixing cannot break the limits, but check rather than assume.
    layout.validate()?;
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rect;
    use crate::zone::{templates, MAX_ZONES};

    fn thirds() -> Layout {
        templates().remove(1)
    }

    fn envelope(version: &str, layout: &str) -> String {
        format!(r#"{{"krova_layout":{version},"layout":{layout}}}"#)
    }

    const ONE_ZONE: &str =
        r#"{"id":"mine","name":"Mine","zones":[{"id":1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#;

    #[test]
    fn export_is_a_pretty_envelope_with_version_one() {
        let text = export_json(&thirds()).unwrap();
        assert!(text.contains('\n'), "pretty printed");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["krova_layout"], 1);
        assert_eq!(v["layout"]["id"], "thirds");
        assert_eq!(v["layout"]["zones"].as_array().unwrap().len(), 3);
        assert_eq!(v.as_object().unwrap().len(), 2);
    }

    #[test]
    fn export_then_import_round_trips_every_template() {
        for layout in templates() {
            let text = export_json(&layout).unwrap();
            let back = import_json(&text, &[]).unwrap();
            assert_eq!(back, layout);
        }
    }

    #[test]
    fn export_refuses_an_invalid_layout() {
        let mut bad = thirds();
        bad.zones[0].rect = Rect::new(0.9, 0.0, 0.5, 1.0);
        assert!(matches!(
            export_json(&bad),
            Err(LayoutIoError::Invalid(LayoutError::InvalidRect(1)))
        ));
        let empty = Layout {
            id: "e".into(),
            name: "e".into(),
            zones: vec![],
        };
        assert!(matches!(
            export_json(&empty),
            Err(LayoutIoError::Invalid(LayoutError::Empty))
        ));
        let mut nan = thirds();
        nan.zones[1].rect.x = f64::NAN;
        assert!(export_json(&nan).is_err());
        let mut ctl = thirds();
        ctl.name = "a\nb".into();
        assert_eq!(export_json(&ctl), Err(LayoutIoError::BadText));
    }

    #[test]
    fn a_taken_id_gets_a_numeric_suffix() {
        let text = envelope("1", ONE_ZONE);
        let l = import_json(&text, &["mine", "other"]).unwrap();
        assert_eq!(l.id, "mine-2");
        assert_eq!(l.name, "Mine");
        let l = import_json(&text, &["mine", "mine-2", "mine-3"]).unwrap();
        assert_eq!(l.id, "mine-4");
        let l = import_json(&text, &["other"]).unwrap();
        assert_eq!(l.id, "mine");
    }

    #[test]
    fn the_suffix_never_pushes_the_id_past_the_length_limit() {
        let long = "x".repeat(MAX_NAME_LEN);
        let layout = ONE_ZONE.replace("\"mine\"", &format!("\"{long}\""));
        let l = import_json(&envelope("1", &layout), &[long.as_str()]).unwrap();
        assert_eq!(l.id.chars().count(), MAX_NAME_LEN);
        assert!(l.id.ends_with("-2"));
        assert!(l.validate().is_ok());
        // Multi-byte characters are cut on character boundaries.
        let wide = "é".repeat(MAX_NAME_LEN);
        assert_eq!(
            unique_id(&wide, &[wide.as_str()]).chars().count(),
            MAX_NAME_LEN
        );
    }

    #[test]
    fn unique_id_is_stable_when_nothing_clashes() {
        assert_eq!(unique_id("a", &[]), "a");
        assert_eq!(unique_id("a", &["b"]), "a");
        assert_eq!(unique_id("a", &["a"]), "a-2");
    }

    #[test]
    fn oversized_input_is_refused_before_parsing() {
        let huge = " ".repeat(MAX_LAYOUT_JSON_BYTES + 1);
        assert_eq!(import_json(&huge, &[]), Err(LayoutIoError::TooLarge));
        // Exactly at the limit is judged on its content (here: not JSON).
        let at_limit = " ".repeat(MAX_LAYOUT_JSON_BYTES);
        assert!(matches!(
            import_json(&at_limit, &[]),
            Err(LayoutIoError::Malformed(_))
        ));
    }

    #[test]
    fn ten_thousand_zones_are_refused() {
        let zone = r#"{"id":1,"rect":{"x":0,"y":0,"w":0.01,"h":0.01}}"#;
        let zones = vec![zone; 10_000].join(",");
        let layout = format!(r#"{{"id":"z","name":"z","zones":[{zones}]}}"#);
        let text = envelope("1", &layout);
        assert!(text.len() > MAX_LAYOUT_JSON_BYTES);
        assert_eq!(import_json(&text, &[]), Err(LayoutIoError::TooLarge));
    }

    #[test]
    fn more_than_the_zone_limit_is_refused_even_when_small() {
        let zones: Vec<String> = (1..=1000)
            .map(|i| format!(r#"{{"id":{i},"rect":{{"x":0,"y":0,"w":0.01,"h":0.01}}}}"#))
            .collect();
        let layout = format!(r#"{{"id":"z","name":"z","zones":[{}]}}"#, zones.join(","));
        let text = envelope("1", &layout);
        assert!(text.len() <= MAX_LAYOUT_JSON_BYTES);
        assert_eq!(
            import_json(&text, &[]),
            Err(LayoutIoError::Invalid(LayoutError::TooManyZones))
        );
        // Exactly the maximum is fine.
        let ok: Vec<String> = (1..=MAX_ZONES)
            .map(|i| format!(r#"{{"id":{i},"rect":{{"x":0,"y":0,"w":0.01,"h":0.01}}}}"#))
            .collect();
        let layout = format!(r#"{{"id":"z","name":"z","zones":[{}]}}"#, ok.join(","));
        assert!(import_json(&envelope("1", &layout), &[]).is_ok());
    }

    #[test]
    fn other_versions_and_missing_versions_are_refused() {
        for v in ["2", "0", "99", "4294967297", "-1", "1.5"] {
            assert!(
                matches!(
                    import_json(&envelope(v, ONE_ZONE), &[]),
                    Err(LayoutIoError::WrongVersion(_))
                ),
                "version {v}"
            );
        }
        // A string is not a number, even if it says "1".
        assert!(matches!(
            import_json(&envelope("\"1\"", ONE_ZONE), &[]),
            Err(LayoutIoError::WrongVersion(_))
        ));
        assert!(matches!(
            import_json(&envelope("null", ONE_ZONE), &[]),
            Err(LayoutIoError::WrongVersion(_))
        ));
        let no_version = format!(r#"{{"layout":{ONE_ZONE}}}"#);
        assert!(matches!(
            import_json(&no_version, &[]),
            Err(LayoutIoError::Malformed(_))
        ));
    }

    #[test]
    fn the_envelope_must_be_exactly_version_and_layout() {
        let extra = format!(r#"{{"krova_layout":1,"layout":{ONE_ZONE},"note":"hi"}}"#);
        assert!(matches!(
            import_json(&extra, &[]),
            Err(LayoutIoError::Malformed(_))
        ));
        let missing = r#"{"krova_layout":1}"#;
        assert!(matches!(
            import_json(missing, &[]),
            Err(LayoutIoError::Malformed(_))
        ));
        for not_an_object in ["[]", "null", "1", "\"x\"", "true", ""] {
            assert!(
                matches!(
                    import_json(not_an_object, &[]),
                    Err(LayoutIoError::Malformed(_))
                ),
                "{not_an_object:?}"
            );
        }
        // A bare layout without the envelope is not accepted.
        assert!(import_json(ONE_ZONE, &[]).is_err());
    }

    #[test]
    fn non_finite_and_out_of_range_numbers_are_refused() {
        // JSON has no NaN or Infinity literals; serde_json rejects them outright.
        for literal in ["NaN", "Infinity", "-Infinity", "1e999", "-1e999"] {
            let layout = ONE_ZONE.replace("\"w\":1", &format!("\"w\":{literal}"));
            assert!(
                import_json(&envelope("1", &layout), &[]).is_err(),
                "{literal}"
            );
        }
        // Finite but outside the unit square, or degenerate.
        for (x, w) in [
            ("0.9", "0.5"),
            ("-0.5", "1"),
            ("0", "0"),
            ("0", "-1"),
            ("0", "0.005"),
        ] {
            let layout = format!(
                r#"{{"id":"a","name":"a","zones":[{{"id":1,"rect":{{"x":{x},"y":0,"w":{w},"h":1}}}}]}}"#
            );
            assert!(
                matches!(
                    import_json(&envelope("1", &layout), &[]),
                    Err(LayoutIoError::Invalid(LayoutError::InvalidRect(1)))
                ),
                "x={x} w={w}"
            );
        }
    }

    #[test]
    fn duplicate_zone_ids_are_refused() {
        let layout = r#"{"id":"a","name":"a","zones":[
            {"id":1,"rect":{"x":0,"y":0,"w":0.5,"h":1}},
            {"id":1,"rect":{"x":0.5,"y":0,"w":0.5,"h":1}}]}"#;
        assert_eq!(
            import_json(&envelope("1", layout), &[]),
            Err(LayoutIoError::Invalid(LayoutError::DuplicateId(1)))
        );
    }

    #[test]
    fn empty_and_wrongly_typed_layouts_are_refused() {
        let empty = r#"{"id":"a","name":"a","zones":[]}"#;
        assert_eq!(
            import_json(&envelope("1", empty), &[]),
            Err(LayoutIoError::Invalid(LayoutError::Empty))
        );
        for bad in [
            "null",
            "[]",
            "\"layout\"",
            r#"{"id":1,"name":"a","zones":[]}"#,
            r#"{"id":"a","name":"a"}"#,
            r#"{"id":"a","name":"a","zones":"none"}"#,
            r#"{"id":"a","name":"a","zones":[{"id":-1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#,
            r#"{"id":"a","name":"a","zones":[{"id":4294967296,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#,
            r#"{"id":"a","name":"a","zones":[{"id":1}]}"#,
        ] {
            assert!(
                matches!(
                    import_json(&envelope("1", bad), &[]),
                    Err(LayoutIoError::Malformed(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn deeply_nested_garbage_is_refused_not_a_stack_overflow() {
        let depth = 5_000;
        let nested = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        assert!(matches!(
            import_json(&envelope("1", &nested), &[]),
            Err(LayoutIoError::Malformed(_))
        ));
        let objects = format!("{}1{}", r#"{"a":"#.repeat(depth), "}".repeat(depth));
        assert!(import_json(&objects, &[]).is_err());
        let in_envelope = format!(r#"{{"krova_layout":1,"layout":{objects}}}"#);
        assert!(import_json(&in_envelope, &[]).is_err());
        // Garbage that is not JSON at all.
        for junk in [
            "{{{{",
            "\u{0}\u{0}",
            "{\"krova_layout\":1,",
            "\u{feff}",
            "💥",
        ] {
            assert!(import_json(junk, &[]).is_err(), "{junk:?}");
        }
    }

    #[test]
    fn text_limits_and_control_characters_are_enforced() {
        let long_name = "n".repeat(MAX_NAME_LEN + 1);
        let layout = ONE_ZONE.replace("\"Mine\"", &format!("\"{long_name}\""));
        assert_eq!(
            import_json(&envelope("1", &layout), &[]),
            Err(LayoutIoError::Invalid(LayoutError::TextTooLong))
        );
        let ctl = ONE_ZONE.replace("\"Mine\"", "\"Mi\\u0007ne\"");
        assert_eq!(
            import_json(&envelope("1", &ctl), &[]),
            Err(LayoutIoError::BadText)
        );
        let newline = ONE_ZONE.replace("\"mine\"", "\"mi\\nne\"");
        assert_eq!(
            import_json(&envelope("1", &newline), &[]),
            Err(LayoutIoError::BadText)
        );
    }

    #[test]
    fn blank_ids_and_names_get_defaults_and_whitespace_is_trimmed() {
        let blank = r#"{"id":"  ","name":"","zones":[{"id":1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#;
        let l = import_json(&envelope("1", blank), &[]).unwrap();
        assert_eq!(l.id, "imported");
        assert_eq!(l.name, "Imported layout");
        let padded = ONE_ZONE.replace("\"mine\"", "\"  mine \"");
        let l = import_json(&envelope("1", &padded), &["x"]).unwrap();
        assert_eq!(l.id, "mine");
    }

    #[test]
    fn a_byte_order_mark_is_tolerated() {
        let text = format!("\u{feff}{}", envelope("1", ONE_ZONE));
        assert!(import_json(&text, &[]).is_ok());
    }

    #[test]
    fn unknown_fields_inside_the_layout_are_ignored() {
        let layout = r#"{"id":"a","name":"a","future":true,"zones":[{"id":1,"extra":1,"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#;
        let l = import_json(&envelope("1", layout), &[]).unwrap();
        assert_eq!(l.zones.len(), 1);
    }

    #[test]
    fn a_unicode_name_survives_the_round_trip() {
        let mut l = thirds();
        l.name = "Drei Spalten – 日本語 🪟".to_owned();
        let back = import_json(&export_json(&l).unwrap(), &[]).unwrap();
        assert_eq!(back.name, l.name);
    }
}
