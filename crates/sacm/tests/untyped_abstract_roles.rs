//! Tests for an untyped child under a role whose declared type is abstract
//! (LIB-COMPAT-010; XMI 2.5.1 §7.6.3; ADR 0022 and its amendment 1).
//!
//! An element under such a role with no `xmi:type` or `xsi:type` is refused
//! in both modes with SACM-XMI-005 at its start tag: its local name is the
//! role, so no class can be inferred, and none is guessed. In this release
//! the one readable role with an abstract declared type is `argumentElement`
//! of an `ArgumentPackage` (§11.4, `ArgumentationElement`).
//!
//! Assumptions this file makes about the design, which the lead confirms:
//! - The code is `Code::XMI_MISSING_TYPE` (SACM-XMI-005), named like
//!   `XMI_MISSING_ID` (SACM-XMI-004).
//! - The refusal is a coded `ReadError` variant of its own (ADR 0015
//!   amendment 2). Its name is not pinned: the tests use `code()`,
//!   `location()` and `locations()`, and that it is not `Unsupported`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::diagnostic::{Code, SourceLocation};
use sacm::model::{Document, ElementKind, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "LIB-COMPAT-010";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where the refusal is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// An ArgumentPackage root `p` named "P", holding `body` on line 3.
fn ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// An AssuranceCasePackage root `case` named "Case", holding `body` on line 3.
fn acp(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  {body}\n\
         </sacm:AssuranceCasePackage>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
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
    (text, marks)
}

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read to `==`
/// documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => documents.push(output.into_document()),
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `template` without its one mark in both modes and asserts that it
/// is refused the same in both, with SACM-XMI-005 at the mark.
fn assert_missing_type_at_mark(what: &str, template: &str) {
    let (input, marks) = marked(template);
    assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, &input) {
            Ok(output) => panic!(
                "{ROW}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{ROW}: {what}: the modes refuse differently"
    );
    let error = &errors[0];
    assert!(
        !matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: {what}: still Unsupported: {error:?} ({error})"
    );
    assert_eq!(
        (error.code(), error.location(), error.locations()),
        (
            Some(Code::XMI_MISSING_TYPE),
            Some(marks[0]),
            marks.as_slice()
        ),
        "{ROW}: {what}: not SACM-XMI-005 at the start tag: {error:?} ({error})"
    );
    let shown = error.to_string();
    assert!(
        shown.contains(&marks[0].to_string()) && !shown.contains('\n'),
        "{ROW}: {what}: the refusal {shown:?} is not one line showing {}",
        marks[0]
    );
}

#[test]
fn lib_compat_010_an_untyped_child_under_an_abstract_role_is_sacm_xmi_005_at_its_start_tag() {
    let cases = [
        (
            "an untyped argumentElement",
            ap(&format!("{MARK}<argumentElement xmi:id=\"c1\"/>")),
        ),
        (
            "an untyped argumentElement with a name",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"><name content=\"C\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement with children",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"><name content=\"C\"/>\
                 <description xmi:id=\"d\"/><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"/>\
                 </argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement without an xmi:id",
            ap(&format!(
                "{MARK}<argumentElement><name content=\"C\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement with attributes over two lines",
            ap(&format!(
                "{MARK}<argumentElement xmi:id=\"c1\"\n      assertionDeclaration=\"assumed\"/>"
            )),
        ),
        (
            "an untyped argumentElement after a typed one",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  \
                 {MARK}<argumentElement xmi:id=\"c2\"/>"
            )),
        ),
        (
            "an untyped argumentElement in an argumentPackage of an AssuranceCasePackage",
            acp(&format!(
                "<argumentPackage xmi:id=\"a\"><name content=\"A\"/>\n    \
                 {MARK}<argumentElement xmi:id=\"c1\"/></argumentPackage>"
            )),
        ),
        (
            "an untyped argumentElement in a nested ArgumentPackage",
            ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\">\
                 <name content=\"I\"/>\n    {MARK}<argumentElement xmi:id=\"c1\"/></argumentElement>"
            )),
        ),
        (
            "an untyped argumentElement in an xmi:XMI wrapper",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n\
                 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/>\n  \
                 {MARK}<argumentElement xmi:id=\"c1\"/></sacm:ArgumentPackage>\n</xmi:XMI>"
            ),
        ),
    ];
    for (what, template) in &cases {
        assert_missing_type_at_mark(what, template);
    }
}

#[test]
fn lib_compat_010_a_reference_form_element_under_an_abstract_role_is_not_missing_type() {
    // A role element with `xmi:idref` or `href` is the reference form of a
    // composite (XMI 2.5.1 §7.10.1, a proxy), not an untyped object: no
    // class is missing, so it is no SACM-XMI-005. This version does not read
    // it yet (the maintainer's decision): `Unsupported`, at its start tag,
    // in both modes, naming the row that reads it, SACM23-XMI-019 for
    // `xmi:idref` and SACM23-XMI-027 for `href`.
    const IDREF: &str = "(a limit of this version; SACM23-XMI-019 reads it)";
    const HREF: &str = "(a limit of this version; SACM23-XMI-027 reads it)";
    let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  ";
    let cases = [
        (
            "an argumentElement xmi:idref in an ArgumentPackage",
            ap(&format!("{claim}{MARK}<argumentElement xmi:idref=\"c1\"/>")),
            IDREF,
        ),
        (
            "an argumentElement href=\"#c1\" in an ArgumentPackage",
            ap(&format!("{claim}{MARK}<argumentElement href=\"#c1\"/>")),
            HREF,
        ),
        (
            "an argumentElement href to another document in an ArgumentPackage",
            ap(&format!(
                "{claim}{MARK}<argumentElement href=\"o.xmi#c1\"/>"
            )),
            HREF,
        ),
        (
            "an argumentElement xmi:idref in an argumentPackage of an AssuranceCasePackage",
            acp(&format!(
                "<argumentPackage xmi:id=\"a\"><name content=\"A\"/>{claim}\
                 {MARK}<argumentElement xmi:idref=\"c1\"/></argumentPackage>"
            )),
            IDREF,
        ),
    ];
    for (what, template, label) in &cases {
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
        for mode in MODES {
            let error = match read(mode, &input) {
                Ok(output) => panic!(
                    "{ROW}: {what}: a {mode:?} read returned a document of {} elements",
                    output.document().element_count()
                ),
                Err(error) => error,
            };
            let ReadError::Unsupported {
                location, reason, ..
            } = &error
            else {
                panic!("{ROW}: {what}: {mode:?}: not Unsupported: {error:?} ({error})");
            };
            assert!(
                error.code().is_none()
                    && *location == Some(marks[0])
                    && reason.ends_with(label)
                    && !reason.contains('\n'),
                "{ROW}: {what}: {mode:?}: not Unsupported at the start tag ending {label:?}: \
                 {error:?}"
            );
        }
    }
}

#[test]
fn lib_compat_010_the_same_element_typed_as_a_concrete_admitted_class_reads_as_before() {
    for (what, body, kind) in [
        (
            "xmi:type Claim",
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>",
            ElementKind::Claim,
        ),
        (
            "xsi:type Claim",
            "<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>",
            ElementKind::Claim,
        ),
        (
            "xmi:type ArgumentReasoning",
            "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"c1\"/>",
            ElementKind::ArgumentReasoning,
        ),
        (
            "xmi:type ArgumentPackage",
            "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"c1\"/>",
            ElementKind::ArgumentPackage,
        ),
    ] {
        let doc = read_ok(what, &ap(body));
        let Some(c1) = doc.get("c1") else {
            panic!("{ROW}: {what}: the element was not read");
        };
        assert_eq!(
            (c1.kind(), c1.parent().map(|parent| parent.id().as_str())),
            (kind, Some("p")),
            "{ROW}: {what}: not read as a {kind} in p"
        );
    }
}

#[test]
fn lib_compat_010_an_untyped_child_under_a_concrete_role_still_reads() {
    let doc = read_ok(
        "an untyped argumentPackage",
        &acp("<argumentPackage xmi:id=\"a\"><name content=\"A\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/></argumentPackage>\n  \
             <assuranceCasePackage xmi:id=\"sub\"/>"),
    );
    for (id, kind, parent) in [
        ("a", ElementKind::ArgumentPackage, "case"),
        ("c1", ElementKind::Claim, "a"),
        ("sub", ElementKind::AssuranceCasePackage, "case"),
    ] {
        let Some(found) = doc.get(id) else {
            panic!("{ROW}: the untyped child {id:?} was not read");
        };
        assert_eq!(
            (found.kind(), found.parent().map(|p| p.id().as_str())),
            (kind, Some(parent)),
            "{ROW}: the untyped child {id:?} is not read as a {kind} in {parent}"
        );
    }
    // A UtilityElement role is concrete too.
    let doc = read_ok(
        "an untyped description",
        &ap("<description xmi:id=\"d\"/><note xmi:id=\"n\"/>"),
    );
    assert_eq!(
        (
            doc.get("d").map(SacmElement::kind),
            doc.get("n").map(SacmElement::kind)
        ),
        (Some(ElementKind::Description), Some(ElementKind::Note)),
        "{ROW}: untyped utility elements are not read"
    );
}

#[test]
fn lib_compat_010_the_catalog_row_of_sacm_xmi_005_is_live_and_an_error_in_both_modes() {
    assert_eq!(
        Code::XMI_MISSING_TYPE.as_str(),
        "SACM-XMI-005",
        "{ROW}: Code::XMI_MISSING_TYPE is not SACM-XMI-005"
    );
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .filter(|line| line.trim_start().starts_with("| SACM-XMI-005 |"))
        .collect();
    let [row] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-XMI-005, not one",
            rows.len()
        );
    };
    let cells: Vec<&str> = row.split('|').map(str::trim).collect();
    assert_eq!(
        cells.get(2).copied(),
        Some("Error"),
        "{ROW}: the severity of SACM-XMI-005 is not Error in both modes: {row}"
    );
    for needle in ["`Code::XMI_MISSING_TYPE`", "LIB-COMPAT-010"] {
        assert!(
            row.contains(needle),
            "{ROW}: the catalog row of SACM-XMI-005 does not name {needle}: {row}"
        );
    }
}
