//! Tests that every read diagnostic is located at the first character of the
//! attribute's name (SACM23-XMI-023; the location rules of SACM23-XMI-010;
//! XML 1.0 (Fifth Edition) §2.11; ADR 0011).
//!
//! The read diagnostics that exist are SACM-XMI-010 (`xsi:type`, on a root, a
//! child and a `name`) and SACM-XMI-011 (`xmi:version`, on `xmi:XMI` and on
//! a root), both from LIB-COMPAT-001. One table holds each site several
//! times: after a non-ASCII character on the attribute's line, after a CRLF
//! and after a lone CR, and once with a prefix other than `xsi` or `xmi`.
//!
//! The expected location is marked in the input with [`MARK`] (U+2038),
//! which the test removes before reading, and is counted here as
//! `xmi_read_locations.rs` counts it: lines end at LF, CRLF or a lone CR, and
//! columns count characters (an astral character is one column). Each row
//! also states its location literally, counted by hand, and the test checks
//! that the two agree, so a wrong count in either is caught.
//!
//! Each row is read in both modes, as both diagnostics exist in both:
//! SACM-XMI-010 is an Error in strict mode and a Warning in tolerant mode,
//! SACM-XMI-011 is Info in both. Each row gives exactly one diagnostic, and
//! its code, severity and affected ids are checked with the location.

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::xmi::{self, Mode, ReadOptions};

const ROW: &str = "SACM23-XMI-023";

/// Marks, in a test input, where the diagnostic is expected.
const MARK: char = '\u{2038}';

/// What comes before every row: line 1 ends in CRLF, line 2 in a lone CR,
/// and line 3 starts with 12 characters, one of them `é`, so the root's `<`
/// is at line 3, column 13.
const LEAD: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
    "<!-- line 2 -->\r",
    "<!-- \u{e9} -->  ",
);

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`. The
/// rows put them after the attribute, so that the column is short to count.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The namespace declarations of an `xmi:XMI` element: `xmi`, `sacm` and
/// `xsi`.
const WRAP_NS: &str = concat!(
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The diagnostic a row must give.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// SACM-XMI-010, `xsi:type`.
    XsiType,
    /// SACM-XMI-011, `xmi:version`.
    XmiVersion,
}

/// One diagnostic site.
struct Row {
    what: &'static str,
    /// The input, after [`LEAD`], with [`MARK`] where the diagnostic is.
    text: String,
    kind: Kind,
    /// The ids the diagnostic affects.
    affected: &'static [&'static str],
    /// The location, counted by hand: (line, column).
    at: (u32, u32),
}

fn row(
    what: &'static str,
    text: String,
    kind: Kind,
    affected: &'static [&'static str],
    at: (u32, u32),
) -> Row {
    Row {
        what,
        text,
        kind,
        affected,
        at,
    }
}

/// Every diagnostic site, each after a CRLF, a lone CR and a non-ASCII
/// character.
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn table() -> Vec<Row> {
    use Kind::{XmiVersion, XsiType};
    vec![
        // --- SACM-XMI-010 on a root ------------------------------------------
        // `<sacm:ArgumentPackage` (21), ` xmi:id="Ünï"` (13), a space: 13 + 35.
        row(
            "xsi:type on a bare root, after a non-ASCII id on its line",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"\u{dc}n\u{ef}\" \u{2038}xsi:type=\"sacm:ArgumentPackage\"{NS}/>"
            ),
            XsiType,
            &["\u{dc}n\u{ef}"],
            (3, 48),
        ),
        row(
            "xsi:type on a bare root, after a CRLF in its start tag",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"\r\n    \u{2038}xsi:type=\"sacm:ArgumentPackage\"{NS}/>"
            ),
            XsiType,
            &["a"],
            (4, 5),
        ),
        row(
            "xsi:type on a bare root, after a lone CR in its start tag",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"\r  \u{2038}xsi:type=\"sacm:ArgumentPackage\"{NS}/>"
            ),
            XsiType,
            &["a"],
            (4, 3),
        ),
        // ` xmi:id="é"` (11), a space: 1 + 12.
        row(
            "xsi:type on a bare root, after a CRLF and a non-ASCII id on its line",
            format!(
                "<sacm:AssuranceCasePackage\r\n xmi:id=\"\u{e9}\" \u{2038}xsi:type=\"sacm:AssuranceCasePackage\"{NS}/>"
            ),
            XsiType,
            &["\u{e9}"],
            (4, 13),
        ),
        // `<sacm:ArgumentPackage` (21), ` xmi:id="Ünï"` (13), a space: 1 + 35.
        row(
            "xsi:type on a wrapped root, after a lone CR and a non-ASCII id",
            format!(
                "<xmi:XMI{WRAP_NS}>\r<sacm:ArgumentPackage xmi:id=\"\u{dc}n\u{ef}\" \u{2038}xsi:type=\"sacm:ArgumentPackage\"/></xmi:XMI>"
            ),
            XsiType,
            &["\u{dc}n\u{ef}"],
            (4, 36),
        ),
        row(
            "a type under another prefix bound to xsi, at the prefix",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"\r\n \u{2038}i:type=\"sacm:ArgumentPackage\" xmlns:i=\"http://www.w3.org/2001/XMLSchema-instance\"{NS}/>"
            ),
            XsiType,
            &["a"],
            (4, 2),
        ),
        // --- SACM-XMI-010 on a child -----------------------------------------
        // Two spaces, `<argumentElement` (16), ` xmi:id="Ünï"` (13), a space: 1 + 32.
        row(
            "xsi:type on a Claim, after a CRLF and a non-ASCII id on its line",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r\n  <argumentElement xmi:id=\"\u{dc}n\u{ef}\" \u{2038}xsi:type=\"sacm:Claim\"/></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["\u{dc}n\u{ef}"],
            (4, 33),
        ),
        row(
            "xsi:type on a Claim, after a lone CR and a CRLF",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r  <argumentElement xmi:id=\"c1\"\r\n      \u{2038}xsi:type=\"sacm:Claim\"/></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["c1"],
            (5, 7),
        ),
        row(
            "xsi:type on a Claim, at column 1 after a lone CR",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r\n<argumentElement\r\u{2038}xsi:type=\"sacm:Claim\" xmi:id=\"c1\"/></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["c1"],
            (5, 1),
        ),
        // Two spaces, `<argumentPackage` (16), ` xmi:id="é"` (11), a space: 1 + 30.
        row(
            "a redundant xsi:type on argumentPackage, after a CRLF and a non-ASCII id",
            format!(
                "<sacm:AssuranceCasePackage xmi:id=\"p\"{NS}>\r\n  <argumentPackage xmi:id=\"\u{e9}\" \u{2038}xsi:type=\"sacm:ArgumentPackage\"/></sacm:AssuranceCasePackage>"
            ),
            XsiType,
            &["\u{e9}"],
            (4, 31),
        ),
        // --- SACM-XMI-010 on a name ------------------------------------------
        // Two spaces, `<name` (5), ` content="😀"` (12), a space: 1 + 20.
        row(
            "xsi:type on a root's name, after a CRLF and an astral character",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r\n  <name content=\"\u{1F600}\" \u{2038}xsi:type=\"sacm:LangString\"/></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["p"],
            (4, 21),
        ),
        // Two spaces, `<name` (5), ` content="é"` (12), a space: 1 + 20.
        row(
            "xsi:type on a root's name, after a lone CR and a non-ASCII character",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r  <name content=\"\u{e9}\" \u{2038}xsi:type=\"sacm:LangString\"/></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["p"],
            (4, 21),
        ),
        row(
            "xsi:type on a Claim's name, after a lone CR, a CRLF and a lone CR",
            format!(
                "<sacm:ArgumentPackage xmi:id=\"p\"{NS}>\r<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\r\n    <name\r \u{2038}xsi:type=\"sacm:LangString\" content=\"C\"/></argumentElement></sacm:ArgumentPackage>"
            ),
            XsiType,
            &["c1"],
            (6, 2),
        ),
        // --- SACM-XMI-011 on xmi:XMI -----------------------------------------
        // `<xmi:XMI` (8), a space: 13 + 9.
        row(
            "xmi:version on xmi:XMI, after the non-ASCII character of the lead",
            format!(
                "<xmi:XMI \u{2038}xmi:version=\"2.0\"{WRAP_NS}><sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
            ),
            XmiVersion,
            &[],
            (3, 22),
        ),
        row(
            "xmi:version on xmi:XMI, after a CRLF in its start tag",
            format!(
                "<xmi:XMI\r\n  \u{2038}xmi:version=\"2.1\"{WRAP_NS}><sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
            ),
            XmiVersion,
            &[],
            (4, 3),
        ),
        row(
            "xmi:version on xmi:XMI, at column 1 after a lone CR",
            format!(
                "<xmi:XMI{WRAP_NS}\r\u{2038}xmi:version=\"2.0\"><sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
            ),
            XmiVersion,
            &[],
            (4, 1),
        ),
        row(
            "a version under another prefix bound to XMI, at the prefix",
            String::from(concat!(
                "<x:XMI xmlns:x=\"http://www.omg.org/spec/XMI/20131001\"",
                " xmlns:s=\"http://www.omg.org/spec/SACM/20220301\"\r\n",
                " \u{2038}x:version=\"2.0\"><s:ArgumentPackage x:id=\"a\"/></x:XMI>",
            )),
            XmiVersion,
            &[],
            (4, 2),
        ),
        // --- SACM-XMI-011 on a root ------------------------------------------
        // `<sacm:AssuranceCasePackage` (26), ` xmi:id="Ünï"` (13), a space: 13 + 40.
        row(
            "xmi:version on a bare root, after a non-ASCII id on its line",
            format!(
                "<sacm:AssuranceCasePackage xmi:id=\"\u{dc}n\u{ef}\" \u{2038}xmi:version=\"2.0\"{NS}/>"
            ),
            XmiVersion,
            &["\u{dc}n\u{ef}"],
            (3, 53),
        ),
        row(
            "xmi:version on a bare root, after a CRLF in its start tag",
            format!("<sacm:ArgumentPackage{NS}\r\n\t\u{2038}xmi:version=\"2.0\" xmi:id=\"a\"/>"),
            XmiVersion,
            &["a"],
            (4, 2),
        ),
        // `<sacm:ArgumentPackage` (21), ` xmi:id="Ünï"` (13), a space: 1 + 35.
        row(
            "xmi:version on a wrapped root, after a lone CR and a non-ASCII id",
            format!(
                "<xmi:XMI{WRAP_NS}>\r<sacm:ArgumentPackage xmi:id=\"\u{dc}n\u{ef}\" \u{2038}xmi:version=\"2.0\"/></xmi:XMI>"
            ),
            XmiVersion,
            &["\u{dc}n\u{ef}"],
            (4, 36),
        ),
        row(
            "xmi:version on a wrapped root, at column 1 after a CRLF and a lone CR",
            format!(
                "<xmi:XMI{WRAP_NS}>\r\n  <sacm:AssuranceCasePackage\r\u{2038}xmi:version=\"\" xmi:id=\"n1\"/></xmi:XMI>"
            ),
            XmiVersion,
            &["n1"],
            (5, 1),
        ),
    ]
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The location of the character after `before`: lines end at LF, CRLF or a
/// lone CR, and columns count characters.
fn location_after(before: &str) -> SourceLocation {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    for c in before.chars() {
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    loc(line, column)
}

/// The input of `row`, without its mark, and where the mark was.
fn input_of(row: &Row) -> (String, SourceLocation) {
    let text = format!("{LEAD}{}", row.text);
    let Some((before, after)) = text.split_once(MARK) else {
        panic!("{ROW}: setup: the row {:?} has no mark", row.what);
    };
    assert!(
        !after.contains(MARK),
        "{ROW}: setup: the row {:?} has two marks",
        row.what
    );
    (format!("{before}{after}"), location_after(before))
}

/// The severity of `kind` read in `mode`.
fn severity(kind: Kind, mode: Mode) -> Severity {
    match (kind, mode) {
        (Kind::XsiType, Mode::Strict) => Severity::Error,
        (Kind::XsiType, _) => Severity::Warning,
        (Kind::XmiVersion, _) => Severity::Info,
    }
}

/// The code of `kind`.
fn code(kind: Kind) -> Code {
    match kind {
        Kind::XsiType => Code::XMI_XSI_TYPE,
        Kind::XmiVersion => Code::XMI_VERSION_IGNORED,
    }
}

#[test]
fn sacm23_xmi_023_every_read_diagnostic_is_located_at_the_attribute_name() {
    let rows = table();
    let mut failures = Vec::new();
    for row in &rows {
        let (input, marked) = input_of(row);
        let at = loc(row.at.0, row.at.1);
        assert_eq!(
            marked, at,
            "{ROW}: setup: {}: the mark and the hand count disagree",
            row.what
        );
        for mode in [Mode::Strict, Mode::Tolerant] {
            let output = match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
            {
                Ok(output) => output,
                Err(error) => {
                    failures.push(format!(
                        "{}: {mode:?}: refused: {error} (input {input:?})",
                        row.what
                    ));
                    continue;
                }
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|diagnostic| {
                    (
                        diagnostic.code(),
                        diagnostic.severity(),
                        diagnostic
                            .affected()
                            .iter()
                            .map(|id| id.as_str().to_owned())
                            .collect::<Vec<_>>(),
                        diagnostic.location(),
                    )
                })
                .collect();
            let expected = vec![(
                code(row.kind),
                severity(row.kind, mode),
                row.affected
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect::<Vec<_>>(),
                Some(at),
            )];
            if found != expected {
                failures.push(format!(
                    "{}: {mode:?}: found {found:?}, expected {expected:?}",
                    row.what
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} reads of the {} rows are not located at the attribute name:\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}
