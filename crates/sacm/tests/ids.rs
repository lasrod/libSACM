//! Tests for element ids read from a file (SACM23-VAL-001, SACM23-VAL-020;
//! XMI 2.5.1 §7.6.1 `xmi:id` of type `xsd:ID`; ADR 0015 amendment 2; D-ID).
//!
//! - SACM23-VAL-001: a duplicate `xmi:id` is refused by `xmi::read` as
//!   `ReadError::DuplicateId` (SACM-ID-001), naming every element that carries
//!   it; the edit API refuses a caller id already in the document
//!   (SACM-CMD-004).
//! - SACM23-VAL-020: an id that is not an `NCName` is read as written and
//!   reported by `validation::validate` as SACM-ID-002; an empty id is refused
//!   as `ReadError::InvalidId` (SACM-ID-002). An id with white space around
//!   an `NCName` is read trimmed (SACM23-XMI-025, `xmi_padded_ids.rs`); one of
//!   white space alone is still held as written.
//!
//! Inputs are inline strings, LF-only, built line by line so that each
//! expected location is counted from the layout: an element at the `<` of
//! its start tag, an attribute at the first character of its name.
//!
//! Not checked here: that `DuplicateId` and `InvalidId` are
//! `#[non_exhaustive]` (a test cannot show that a struct expression fails to
//! compile; the patterns here use `..`, as a user must).

use std::path::Path;

use sacm::diagnostic::{self, Code, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError};
use sacm::model::{Document, ElementId, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const VAL_001: &str = "SACM23-VAL-001";
const VAL_020: &str = "SACM23-VAL-020";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The start tag of an XMI wrapper that declares `xmi` and `sacm`.
const WRAPPER_OPEN: &str = concat!(
    "<xmi:XMI",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
);

// --- Helpers ----------------------------------------------------------------

fn read_in(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

fn read_ok(row: &str, mode: Mode, input: &str) -> ReadOutput {
    match read_in(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {mode:?} read refused {input:?}: {error:?} ({error})"),
    }
}

fn refusal(row: &str, mode: Mode, input: &str) -> ReadError {
    match read_in(mode, input) {
        Ok(output) => panic!(
            "{row}: {mode:?} read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// An ArgumentPackage root `p`, named, holding `lines` from line 3, each
/// indented by two spaces.
fn ap(lines: &[String]) -> String {
    let mut text = format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n");
    for line in lines {
        text.push_str("  ");
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("</sacm:ArgumentPackage>");
    text
}

/// A named Claim `id` in `argumentElement`, on one line.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/></argumentElement>"
    )
}

/// A named ArgumentPackage root `id`, bare.
fn ap_root(id: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"{id}\"><name content=\"P\"/></sacm:ArgumentPackage>"
    )
}

/// Asserts that `text` is one line that starts in lower case and does not
/// end in punctuation or white space (the `ReadError` Display rules).
fn assert_one_lowercase_line(row: &str, text: &str) {
    assert!(
        !text.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']),
        "{row}: Display {text:?} is not one line"
    );
    assert!(
        text.chars().next().is_some_and(char::is_lowercase),
        "{row}: Display {text:?} does not start in lower case"
    );
    assert!(
        text.chars()
            .last()
            .is_some_and(|c| !c.is_whitespace() && !matches!(c, '.' | ',' | ';' | ':' | '!' | '?')),
        "{row}: Display {text:?} ends in punctuation or white space"
    );
}

/// Asserts that `error` is `DuplicateId` for `id` naming exactly `locations`,
/// with SACM-ID-001, `location()` the second of them, and a one-line
/// `Display` naming the id and every location.
fn assert_duplicate(
    row: &str,
    what: &str,
    error: &ReadError,
    id: &str,
    locations: &[SourceLocation],
) {
    let ReadError::DuplicateId {
        id: field_id,
        locations: field_locations,
        ..
    } = error
    else {
        panic!("{row}: {what}: expected DuplicateId, got {error:?}");
    };
    assert_eq!(
        field_id.as_str(),
        id,
        "{row}: {what}: the refusal names another id"
    );
    assert_eq!(
        field_locations.as_slice(),
        locations,
        "{row}: {what}: the locations field is not every start tag carrying {id:?}, in order"
    );
    assert_eq!(
        error.locations(),
        locations,
        "{row}: {what}: locations() is not every start tag carrying {id:?}, in order"
    );
    assert_eq!(
        error.location(),
        locations.get(1).copied(),
        "{row}: {what}: location() is not the first repeat"
    );
    assert_eq!(
        error.code(),
        Some(Code::ID_DUPLICATE),
        "{row}: {what}: the refusal does not carry Code::ID_DUPLICATE"
    );
    assert_eq!(
        error.code().map(Code::as_str),
        Some("SACM-ID-001"),
        "{row}: {what}: the refusal's code is not SACM-ID-001"
    );
    let shown = error.to_string();
    assert_one_lowercase_line(row, &shown);
    assert!(
        shown.contains(&format!("{id:?}")),
        "{row}: {what}: Display {shown:?} does not name {id:?}"
    );
    for location in locations {
        assert!(
            shown.contains(&location.to_string()),
            "{row}: {what}: Display {shown:?} does not name {location}"
        );
    }
}

/// Reads `input` in both modes and asserts the same `DuplicateId` in each.
fn assert_duplicate_in_both_modes(
    row: &str,
    what: &str,
    input: &str,
    id: &str,
    locations: &[SourceLocation],
) {
    let strict = refusal(row, Mode::Strict, input);
    let tolerant = refusal(row, Mode::Tolerant, input);
    assert_duplicate(row, &format!("{what} (strict)"), &strict, id, locations);
    assert_duplicate(row, &format!("{what} (tolerant)"), &tolerant, id, locations);
    assert_eq!(
        strict, tolerant,
        "{row}: {what}: the modes refuse differently"
    );
}

/// The validation findings for SACM-ID-002.
fn id_findings(document: &Document) -> Vec<diagnostic::Diagnostic> {
    validation::validate(document)
        .into_iter()
        .filter(|found| found.code() == Code::ID_INVALID)
        .collect()
}

/// The catalog row of `code`, which must be the only one.
fn catalog_row(row: &str, code: Code) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", code.as_str());
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&cell))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{row}: the catalog has {} rows for {code}, not one",
            rows.len()
        );
    };
    (*line).to_owned()
}

// --- SACM23-VAL-020 ----------------------------------------------------------

/// Ids that are not `NCName`s: a digit, `-` and `.` first, a colon, a space,
/// and U+00B7 (a `NameChar` that is not a `NameStartChar`) first.
const NOT_NCNAMES: [&str; 6] = ["1c", "-c", ".c", "a:b", "a b", "\u{B7}c"];

#[test]
fn sacm23_val_020_a_non_ncname_id_reads_and_is_one_id_002_error_on_a_root_and_on_a_child() {
    for id in NOT_NCNAMES {
        let cases = [
            ("a root", "ArgumentPackage", ap_root(id)),
            ("a child", "Claim", ap(&[claim(id)])),
        ];
        for (place, kind, input) in &cases {
            for mode in MODES {
                let output = read_ok(VAL_020, mode, input);
                let document = output.document();
                assert!(
                    document.get(id).is_some(),
                    "{VAL_020}: the id {id:?} on {place} was not held as written ({mode:?})"
                );
                let found = validation::validate(document);
                let [finding] = found.as_slice() else {
                    panic!(
                        "{VAL_020}: validate gave {} findings, not one, for {id:?} on {place}: {found:?}",
                        found.len()
                    );
                };
                assert_eq!(
                    finding.code(),
                    Code::ID_INVALID,
                    "{VAL_020}: {id:?} on {place}: not SACM-ID-002"
                );
                assert_eq!(
                    finding.severity(),
                    Severity::Error,
                    "{VAL_020}: {id:?} on {place}: not an Error"
                );
                assert_eq!(
                    finding.requirement_id(),
                    VAL_020,
                    "{VAL_020}: {id:?} on {place}: wrong requirement"
                );
                assert_eq!(
                    finding.clause(),
                    Some("7.6.1"),
                    "{VAL_020}: {id:?} on {place}: wrong clause"
                );
                assert_eq!(
                    finding.affected(),
                    [ElementId::from(id)],
                    "{VAL_020}: {id:?} on {place}: affected is not [id]"
                );
                assert_eq!(
                    finding.location(),
                    None,
                    "{VAL_020}: {id:?} on {place}: has a location"
                );
                let message = finding.message();
                assert!(
                    message.contains(kind) && message.contains(&format!("{id:?}")),
                    "{VAL_020}: the message {message:?} does not name {kind} and {id:?}"
                );
            }
        }
    }
}

#[test]
fn sacm23_val_020_boundary_ncnames_and_edit_built_ids_give_no_id_002() {
    for id in ["_", "a-b.c", "x\u{B7}y", "\u{E0}", "\u{10000}"] {
        for input in [ap_root(id), ap(&[claim(id)])] {
            let output = read_ok(VAL_020, Mode::Tolerant, &input);
            let found = validation::validate(output.document());
            assert!(
                found.is_empty(),
                "{VAL_020}: the NCName {id:?} gave findings: {found:?}"
            );
        }
    }
    // Caller and generated ids, from edits only.
    let mut doc = Document::new();
    let steps: [Result<_, EditError>; 4] = [
        edit::apply(
            &mut doc,
            CreateAssuranceCasePackage::root(LangString::new("Case")).with_id("case"),
        ),
        edit::apply(
            &mut doc,
            CreateArgumentPackage::new("case", LangString::new("Args")),
        ),
        edit::apply(
            &mut doc,
            CreateArgumentPackage::new("case", LangString::new("More")).with_id("more"),
        ),
        edit::apply(&mut doc, CreateClaim::new("more", LangString::new("Safe"))),
    ];
    for step in steps {
        assert!(
            step.is_ok(),
            "{VAL_020}: setup: an edit was refused: {step:?}"
        );
    }
    let found = id_findings(&doc);
    assert!(
        found.is_empty(),
        "{VAL_020}: an edit-built document gave SACM-ID-002: {found:?}"
    );
}

#[test]
fn sacm23_val_020_two_bad_ids_give_two_findings_in_diagnostic_order_and_valid_ids_give_none() {
    // In document order "a b" comes first; in Diagnostic order "1c" does.
    let input = ap(&[claim("a b"), claim("ok"), claim("1c")]);
    let output = read_ok(VAL_020, Mode::Tolerant, &input);
    let found = validation::validate(output.document());
    let affected: Vec<Vec<ElementId>> = found.iter().map(|d| d.affected().to_vec()).collect();
    assert_eq!(
        affected,
        [vec![ElementId::from("1c")], vec![ElementId::from("a b")]],
        "{VAL_020}: expected one finding for each bad id, in Diagnostic order: {found:?}"
    );
    assert!(
        found.iter().all(|d| d.code() == Code::ID_INVALID),
        "{VAL_020}: a finding is not SACM-ID-002: {found:?}"
    );
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(
        found, sorted,
        "{VAL_020}: the findings are not in Diagnostic order"
    );
}

#[test]
fn sacm23_val_020_strict_write_refuses_and_tolerant_write_keeps_the_id_byte_for_byte() {
    for id in ["1c", "a:b", "a b", "\u{B7}c"] {
        let input = ap(&[claim(id)]);
        let document = read_ok(VAL_020, Mode::Tolerant, &input).into_document();
        let error = match xmi::write(&document, &WriteOptions::default()) {
            Ok(text) => panic!("{VAL_020}: a strict write of {id:?} succeeded: {text}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, WriteError::Invalid { .. }),
            "{VAL_020}: a strict write of {id:?} is not Invalid: {error:?}"
        );
        assert!(
            error
                .diagnostics()
                .iter()
                .any(|d| d.code() == Code::ID_INVALID && d.affected() == [ElementId::from(id)]),
            "{VAL_020}: the strict refusal of {id:?} does not hold its SACM-ID-002: {:?}",
            error.diagnostics()
        );
        let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
        let text = match xmi::write(&document, &tolerant) {
            Ok(text) => text,
            Err(error) => panic!("{VAL_020}: a tolerant write of {id:?} refused: {error:?}"),
        };
        assert!(
            text.contains(&format!("xmi:id=\"{id}\"")),
            "{VAL_020}: the tolerant output does not keep {id:?} byte for byte: {text}"
        );
        let again = read_ok(VAL_020, Mode::Tolerant, &text).into_document();
        assert!(
            again.get(id).is_some(),
            "{VAL_020}: reading the tolerant output back lost {id:?}"
        );
        assert_eq!(
            again, document,
            "{VAL_020}: the tolerant output of {id:?} reads back different"
        );
    }
}

#[test]
fn sacm23_val_020_an_empty_id_is_invalid_id_at_the_element_in_both_modes() {
    let bare_root = format!("<sacm:ArgumentPackage{NS} xmi:id=\"\"/>");
    let wrapped_root = format!("{WRAPPER_OPEN}\n  <sacm:ArgumentPackage xmi:id=\"\"/>\n</xmi:XMI>");
    let child = ap(&["<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>".to_owned()]);
    let cases = [
        ("a bare root", bare_root, loc(VAL_020, 1, 1)),
        ("a wrapped root", wrapped_root, loc(VAL_020, 2, 3)),
        ("a child", child, loc(VAL_020, 3, 3)),
    ];
    for (what, input, at) in &cases {
        for mode in MODES {
            let error = refusal(VAL_020, mode, input);
            let ReadError::InvalidId { id, location, .. } = &error else {
                panic!("{VAL_020}: {what} ({mode:?}): expected InvalidId, got {error:?}");
            };
            assert_eq!(id, "", "{VAL_020}: {what}: the refusal names another id");
            assert_eq!(
                *location,
                Some(*at),
                "{VAL_020}: {what}: the location field is not the start tag"
            );
            assert_eq!(
                error.location(),
                Some(*at),
                "{VAL_020}: {what}: location() is not the start tag"
            );
            assert_eq!(
                error.locations(),
                [*at],
                "{VAL_020}: {what}: locations() is not [location]"
            );
            assert_eq!(
                error.code(),
                Some(Code::ID_INVALID),
                "{VAL_020}: {what}: not SACM-ID-002"
            );
            let shown = error.to_string();
            assert_one_lowercase_line(VAL_020, &shown);
            assert_eq!(
                shown,
                format!("the xmi:id \"\" is not an NCName at {at}"),
                "{VAL_020}: {what} ({mode:?}): the Display is not the id and its location"
            );
        }
    }
    // The bare root pinned literally, so the location's own Display is
    // covered too.
    assert_eq!(
        refusal(VAL_020, Mode::Strict, &cases[0].1).to_string(),
        "the xmi:id \"\" is not an NCName at line 1, column 1",
        "{VAL_020}: a bare root: the Display is not the id and its location"
    );
}

#[test]
fn sacm23_val_020_an_id_of_white_space_alone_is_read_as_written_and_reported() {
    // White space alone trims to nothing, which is no NCName: read as
    // written, and reported. A padded NCName is read trimmed instead
    // (SACM23-XMI-025).
    for mode in MODES {
        let output = read_ok(VAL_020, mode, &ap(&[claim(" ")]));
        assert!(
            output.document().get(" ").is_some(),
            "{VAL_020}: the id \" \" was not held as written ({mode:?})"
        );
        let found = id_findings(output.document());
        assert_eq!(
            found
                .iter()
                .map(|d| d.affected().to_vec())
                .collect::<Vec<_>>(),
            [vec![ElementId::from(" ")]],
            "{VAL_020}: the id \" \" is not one SACM-ID-002 ({mode:?}): {found:?}"
        );
    }
}

#[test]
fn sacm23_val_020_the_catalog_id_002_row_covers_read_ids_and_names_the_row() {
    assert_eq!(
        Code::ID_INVALID.as_str(),
        "SACM-ID-002",
        "{VAL_020}: Code::ID_INVALID moved"
    );
    let line = catalog_row(VAL_020, Code::ID_INVALID);
    for part in [
        "SACM23-VAL-020",
        "`ReadError::InvalidId`",
        "`EditError::InvalidId`",
    ] {
        assert!(
            line.contains(part),
            "{VAL_020}: the catalog's SACM-ID-002 row does not name {part}: {line}"
        );
    }
}

// --- SACM23-VAL-001 ----------------------------------------------------------

#[test]
fn sacm23_val_001_two_claims_with_one_id_are_duplicate_id_in_both_modes() {
    let input = ap(&[claim("c1"), claim("c1")]);
    let locations = [loc(VAL_001, 3, 3), loc(VAL_001, 4, 3)];
    assert_duplicate_in_both_modes(VAL_001, "two Claims c1", &input, "c1", &locations);
    for mode in MODES {
        assert_eq!(
            refusal(VAL_001, mode, &input).to_string(),
            "the xmi:id \"c1\" is carried by 2 elements, at line 3, column 3; line 4, column 3",
            "{VAL_001}: two Claims c1 ({mode:?}): the Display is not the count and every location"
        );
    }
}

#[test]
fn sacm23_val_001_a_repeat_across_roots_into_a_descendant_and_under_its_parent_is_duplicate_id() {
    let two_roots = format!(
        "{WRAPPER_OPEN}\n  <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"A\"/></sacm:ArgumentPackage>\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"B\"/></sacm:ArgumentPackage>\n</xmi:XMI>"
    );
    let descendant = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  <argumentPackage xmi:id=\"b\">\n    \
         <name content=\"B\"/>\n    {}\n  </argumentPackage>\n</sacm:AssuranceCasePackage>",
        claim("a")
    );
    let under_parent = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  <argumentPackage xmi:id=\"q\">\n    \
         <name content=\"Q\"/>\n    {}\n  </argumentPackage>\n</sacm:AssuranceCasePackage>",
        claim("q")
    );
    let cases = [
        (
            "two roots in a wrapper",
            two_roots,
            "p",
            [loc(VAL_001, 2, 3), loc(VAL_001, 3, 3)],
        ),
        (
            "a root and its descendant",
            descendant,
            "a",
            [loc(VAL_001, 1, 1), loc(VAL_001, 5, 5)],
        ),
        (
            "a child and its parent",
            under_parent,
            "q",
            [loc(VAL_001, 3, 3), loc(VAL_001, 5, 5)],
        ),
    ];
    for (what, input, id, locations) in &cases {
        assert_duplicate_in_both_modes(VAL_001, what, input, id, locations);
    }
}

#[test]
fn sacm23_val_001_three_elements_with_one_id_give_three_locations() {
    let input = ap(&[claim("x"), claim("x"), claim("x")]);
    let locations = [loc(VAL_001, 3, 3), loc(VAL_001, 4, 3), loc(VAL_001, 5, 3)];
    assert_duplicate_in_both_modes(VAL_001, "three Claims x", &input, "x", &locations);
    for mode in MODES {
        assert_eq!(
            refusal(VAL_001, mode, &input).to_string(),
            "the xmi:id \"x\" is carried by 3 elements, at line 3, column 3; line 4, column 3; line 5, column 3",
            "{VAL_001}: three Claims x ({mode:?}): the Display is not the count and every location"
        );
    }
}

#[test]
fn sacm23_val_001_with_two_repeated_ids_the_first_repeat_decides_and_always_the_same() {
    let cases = [
        // b repeats on line 5, before a repeats on line 6.
        (
            ap(&[claim("a"), claim("b"), claim("b"), claim("a")]),
            "b",
            [loc(VAL_001, 4, 3), loc(VAL_001, 5, 3)],
        ),
        // a repeats on line 5, before b repeats on line 6.
        (
            ap(&[claim("a"), claim("b"), claim("a"), claim("b")]),
            "a",
            [loc(VAL_001, 3, 3), loc(VAL_001, 5, 3)],
        ),
    ];
    for (input, id, locations) in &cases {
        assert_duplicate_in_both_modes(VAL_001, "two repeated ids", input, id, locations);
        let first = refusal(VAL_001, Mode::Tolerant, input);
        for _ in 0..3 {
            assert_eq!(
                refusal(VAL_001, Mode::Tolerant, input),
                first,
                "{VAL_001}: the same input gave another refusal"
            );
        }
    }
}

#[test]
fn sacm23_val_001_ids_that_differ_in_case_are_not_duplicates() {
    let input = ap(&[claim("c1"), claim("C1")]);
    for mode in MODES {
        let output = read_ok(VAL_001, mode, &input);
        let document = output.document();
        for id in ["c1", "C1"] {
            assert_eq!(
                document.get(id).map(|element| element.id().as_str()),
                Some(id),
                "{VAL_001}: the id {id:?} was not kept exactly ({mode:?})"
            );
        }
        assert_eq!(
            document.element_count(),
            3,
            "{VAL_001}: an element was dropped"
        );
    }
}

#[test]
fn sacm23_val_001_a_duplicate_is_duplicate_id_before_later_content_that_is_not_read() {
    // The repeat carries an unread attribute, or is followed by an unread
    // element: the duplicate, earlier in the document, decides.
    let cases = [
        (
            "an unread attribute on the repeat",
            ap(&[
                claim("x"),
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" isCitation=\"false\"><name content=\"C\"/></argumentElement>"
                    .to_owned(),
            ]),
        ),
        ("an unread element after the repeat", ap(&[claim("x"), claim("x"), "<foo/>".to_owned()])),
    ];
    for (what, input) in &cases {
        let locations = [loc(VAL_001, 3, 3), loc(VAL_001, 4, 3)];
        assert_duplicate_in_both_modes(VAL_001, what, input, "x", &locations);
    }
}

#[test]
fn sacm23_val_001_locations_of_every_other_refusal_is_its_one_location_or_none() {
    let malformed = refusal(VAL_001, Mode::Tolerant, "<r></s>");
    assert_eq!(
        malformed.locations(),
        [loc(VAL_001, 1, 4)],
        "{VAL_001}: locations() of a Malformed refusal is not [location()]"
    );
    let unsupported = refusal(
        VAL_001,
        Mode::Tolerant,
        &format!("<sacm:ArtifactPackage{NS} xmi:id=\"a\"/>"),
    );
    assert_eq!(
        unsupported.locations(),
        unsupported.location().as_slice(),
        "{VAL_001}: locations() of an Unsupported refusal is not [location()]"
    );
}

#[test]
fn sacm23_val_001_the_catalog_id_001_row_names_the_code_and_the_refusal() {
    assert_eq!(
        Code::ID_DUPLICATE.as_str(),
        "SACM-ID-001",
        "{VAL_001}: Code::ID_DUPLICATE is not SACM-ID-001"
    );
    let line = catalog_row(VAL_001, Code::ID_DUPLICATE);
    assert!(
        line.starts_with("| SACM-ID-001 | Error |"),
        "{VAL_001}: the row is not an Error: {line}"
    );
    for part in [
        "`Code::ID_DUPLICATE`",
        "`ReadError::DuplicateId`",
        "SACM23-VAL-001",
    ] {
        assert!(
            line.contains(part),
            "{VAL_001}: the catalog's SACM-ID-001 row does not name {part}: {line}"
        );
    }
}

#[test]
fn sacm23_val_001_edit_refuses_an_id_already_in_the_document_and_changes_nothing() {
    let mut document = read_ok(VAL_001, Mode::Tolerant, &ap(&[claim("c1")])).into_document();
    let before = document.clone();
    let refusals = [
        edit::apply(
            &mut document,
            CreateClaim::new("p", LangString::new("Again")).with_id("c1"),
        ),
        edit::apply(
            &mut document,
            CreateArgumentPackage::new("p", LangString::new("Again")).with_id("p"),
        ),
        edit::apply(
            &mut document,
            CreateAssuranceCasePackage::root(LangString::new("Again")).with_id("c1"),
        ),
    ];
    for (result, id) in refusals.into_iter().zip(["c1", "p", "c1"]) {
        let error = match result {
            Ok(mutation) => panic!("{VAL_001}: creating {id:?} again succeeded: {mutation:?}"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, EditError::DuplicateId { id: field, .. } if field.as_str() == id),
            "{VAL_001}: creating {id:?} again is not DuplicateId: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_DUPLICATE_ID,
            "{VAL_001}: not SACM-CMD-004"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-CMD-004",
            "{VAL_001}: not SACM-CMD-004"
        );
    }
    assert_eq!(
        document, before,
        "{VAL_001}: a refused edit changed the document"
    );
}
