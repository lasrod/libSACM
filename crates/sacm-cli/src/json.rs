//! The `--format json` report of `sacm validate` (ADR 0023), written by hand
//! so that the binary needs no serialisation library.
//!
//! The report is one compact object on one line, with its keys in a fixed
//! order:
//!
//! ```text
//! {"format":1,"file":…,"mode":…,"result":…,"refusal":…,"diagnostics":[…]}
//! ```
//!
//! A refusal is `{"code","location","locations","message"}`, a diagnostic
//! `{"code","severity","requirement","clause","location","affected","message"}`,
//! and a location `{"line","column"}`. Every key is always present, with
//! `null` where a value is absent. Strings are escaped per RFC 8259 §7.

use std::fmt::Write as _;

use sacm::diagnostic::{Diagnostic, SourceLocation};

/// Why a file could not be checked: a refusal by `xmi::read`, or an I/O
/// error, which has no code and no location.
pub(crate) struct Refusal {
    /// The catalog code, if the refusal has one.
    pub(crate) code: Option<&'static str>,
    /// Where the refusal is, if it is located.
    pub(crate) location: Option<SourceLocation>,
    /// Every location the refusal names.
    pub(crate) locations: Vec<SourceLocation>,
    /// What went wrong, on one line.
    pub(crate) message: String,
}

/// The report of one `validate` run, ending with a newline.
pub(crate) fn report(
    file: &str,
    mode: &str,
    result: &str,
    refusal: Option<&Refusal>,
    diagnostics: &[Diagnostic],
) -> String {
    let mut out = String::from("{\"format\":1,\"file\":");
    string(&mut out, file);
    out.push_str(",\"mode\":");
    string(&mut out, mode);
    out.push_str(",\"result\":");
    string(&mut out, result);
    out.push_str(",\"refusal\":");
    match refusal {
        None => out.push_str("null"),
        Some(refusal) => {
            out.push_str("{\"code\":");
            optional(&mut out, refusal.code);
            out.push_str(",\"location\":");
            location(&mut out, refusal.location);
            out.push_str(",\"locations\":[");
            for (index, &at) in refusal.locations.iter().enumerate() {
                comma(&mut out, index);
                location(&mut out, Some(at));
            }
            out.push_str("],\"message\":");
            string(&mut out, &refusal.message);
            out.push('}');
        }
    }
    out.push_str(",\"diagnostics\":[");
    for (index, found) in diagnostics.iter().enumerate() {
        comma(&mut out, index);
        diagnostic(&mut out, found);
    }
    out.push_str("]}\n");
    out
}

/// Appends `found` as a diagnostic object.
fn diagnostic(out: &mut String, found: &Diagnostic) {
    out.push_str("{\"code\":");
    string(out, found.code().as_str());
    out.push_str(",\"severity\":");
    string(out, found.severity().as_str());
    out.push_str(",\"requirement\":");
    string(out, found.requirement_id());
    out.push_str(",\"clause\":");
    optional(out, found.clause());
    out.push_str(",\"location\":");
    location(out, found.location());
    out.push_str(",\"affected\":[");
    for (index, id) in found.affected().iter().enumerate() {
        comma(out, index);
        string(out, id.as_str());
    }
    out.push_str("],\"message\":");
    string(out, found.message());
    out.push('}');
}

/// Appends a `,` before every item but the first.
fn comma(out: &mut String, index: usize) {
    if index > 0 {
        out.push(',');
    }
}

/// Appends `at` as `{"line":L,"column":C}`, or `null`.
fn location(out: &mut String, at: Option<SourceLocation>) {
    match at {
        // Writing to a `String` cannot fail.
        Some(at) => {
            let _ = write!(out, "{{\"line\":{},\"column\":{}}}", at.line(), at.column());
        }
        None => out.push_str("null"),
    }
}

/// Appends `text` as a string, or `null`.
fn optional(out: &mut String, text: Option<&str>) {
    match text {
        Some(text) => string(out, text),
        None => out.push_str("null"),
    }
}

/// Appends `text` as a JSON string, escaped per RFC 8259 §7: `"` and `\`,
/// the short forms of backspace, form feed, line feed, carriage return and
/// tab, and `\u00xx` for every other character below U+0020. Everything else
/// is written as it is.
fn string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Writing to a `String` cannot fail.
            c if c < '\u{20}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escaped(text: &str) -> String {
        let mut out = String::new();
        string(&mut out, text);
        out
    }

    #[test]
    fn strings_are_escaped_per_rfc_8259() {
        assert_eq!(escaped("plain é"), "\"plain é\"");
        assert_eq!(escaped("q\"b\\s"), r#""q\"b\\s""#);
        assert_eq!(
            escaped("\u{8}\u{c}\n\r\t\u{0}\u{1}\u{1f} \u{7f}"),
            r#""\b\f\n\r\t\u0000\u0001\u001f "#.to_owned() + "\u{7f}\""
        );
    }

    #[test]
    fn an_io_refusal_has_null_code_and_location_and_no_locations() {
        let refusal = Refusal {
            code: None,
            location: None,
            locations: Vec::new(),
            message: "cannot read the file".to_owned(),
        };
        assert_eq!(
            report("f", "tolerant", "unreadable", Some(&refusal), &[]),
            "{\"format\":1,\"file\":\"f\",\"mode\":\"tolerant\",\"result\":\"unreadable\",\
             \"refusal\":{\"code\":null,\"location\":null,\"locations\":[],\
             \"message\":\"cannot read the file\"},\"diagnostics\":[]}\n"
        );
    }
}
