//! Tests for `assertionDeclaration` values that are not literals
//! (SACM23-VAL-006; formal/23-05-08 §11.8, §11.10; XMI 2.5.1 rule 8, rule 2i;
//! XML 1.0 §3.3.3; D-ENUM-EXACT; ADR 0021 amendment 2).
//!
//! `xmi::read`, in both modes, refuses an `assertionDeclaration` that is not
//! exactly one of the six literals as `ReadError::InvalidLiteral`
//! (SACM-ENUM-001), in the attribute form and in the element form. `value` is
//! the value as read: after attribute-value normalisation for the attribute,
//! the exact character content for the element. It is located at the
//! attribute's name or at the value element's `<`. Each occurrence is checked
//! in full before the repeat check, so the first defect in document order
//! decides.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters. Every Claim follows a
//! CRLF and a non-ASCII character on its own line.
//!
//! Not checked here: that `InvalidLiteral` is `#[non_exhaustive]` (a test
//! cannot show that a struct expression fails to compile; the patterns here
//! use `..`, as a user must).

use std::error::Error;
use std::fmt::{Debug, Display};
use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, SourceLocation};
use sacm::model::{AssertionDeclaration, Document, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-VAL-006";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// What comes before the root: a declaration ended by CRLF, then a comment
/// holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// The six literals, in §11.8 order.
const LITERALS: [&str; 6] = [
    "asserted",
    "needsSupport",
    "assumed",
    "axiomatic",
    "defeated",
    "asCited",
];

// --- Inputs ------------------------------------------------------------------

/// A named Claim `c1` with `attributes` after its `xmi:id` and `content` after
/// its name, in a named ArgumentPackage root `p`. The Claim is on line 4,
/// after a CRLF and a comment holding `é`.
fn claim(attributes: &str, content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n  \
         <!-- \u{e9} --> <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{attributes}>\
         <name content=\"C\"/>{content}</argumentElement>\r\n</sacm:ArgumentPackage>"
    )
}

/// `content` in a named ArgumentPackage root `p`.
fn in_ap(content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n  \
         <!-- \u{e9} --> {content}\r\n</sacm:ArgumentPackage>"
    )
}

/// `content` in a named AssuranceCasePackage root `case`.
fn in_acp(content: &str) -> String {
    format!(
        "{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\r\n  <name content=\"Case\"/>\r\n  \
         <!-- \u{e9} --> {content}\r\n</sacm:AssuranceCasePackage>"
    )
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
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

/// `template` without its one mark, and the mark's location.
fn marked_once(what: &str, template: &str) -> (String, SourceLocation) {
    let (text, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has {} marks, not one", marks.len());
    };
    (text, *at)
}

// --- Reading -----------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => panic!("{ROW}: {what}: {mode:?} read refused {input:?}: {error:?}"),
        }
    }
    let differences = semantic_compare(&documents[0], &documents[1]);
    assert!(
        differences.is_empty(),
        "{ROW}: {what}: the modes read different documents: {differences:?}"
    );
    documents.swap_remove(0)
}

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{ROW}: {what}: {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{ROW}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

/// Asserts that `error` is `InvalidLiteral` for the Claim `c1`'s
/// `assertionDeclaration`, holding `value`, at `at`, with SACM-ENUM-001.
fn assert_invalid_literal(what: &str, error: &ReadError, value: &str, at: SourceLocation) {
    let ReadError::InvalidLiteral {
        element,
        property,
        value: found,
        location,
        ..
    } = error
    else {
        panic!("{ROW}: {what}: expected InvalidLiteral, got {error:?}");
    };
    assert_eq!(element.as_str(), "c1", "{ROW}: {what}: the element");
    assert_eq!(
        property, "assertionDeclaration",
        "{ROW}: {what}: the property"
    );
    assert_eq!(
        found, value,
        "{ROW}: {what}: the value is not the value as read"
    );
    assert_eq!(*location, Some(at), "{ROW}: {what}: the location field");
    assert_eq!(error.location(), Some(at), "{ROW}: {what}: location()");
    assert_eq!(
        error.locations(),
        [at],
        "{ROW}: {what}: locations() is not [location]"
    );
    assert_eq!(
        error.code(),
        Some(Code::ENUM_INVALID_LITERAL),
        "{ROW}: {what}: the code"
    );
    assert_eq!(
        error.code().map(Code::as_str),
        Some("SACM-ENUM-001"),
        "{ROW}: {what}: the code is not SACM-ENUM-001"
    );
}

/// Asserts that `text` is one line that starts in lower case and does not end
/// in punctuation or white space (the `ReadError` Display rules).
fn assert_one_lowercase_line(what: &str, text: &str) {
    assert!(
        !text.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']),
        "{ROW}: {what}: Display {text:?} is not one line"
    );
    assert!(
        text.chars().next().is_some_and(char::is_lowercase),
        "{ROW}: {what}: Display {text:?} does not start in lower case"
    );
    assert!(
        text.chars()
            .last()
            .is_some_and(|c| !c.is_whitespace() && !matches!(c, '.' | ',' | ';' | ':' | '!' | '?')),
        "{ROW}: {what}: Display {text:?} ends in punctuation or white space"
    );
}

// --- Item 1: the attribute form ----------------------------------------------

#[test]
fn sacm23_val_006_each_bad_attribute_value_on_a_claim_is_invalid_literal_in_both_modes() {
    for value in [
        "foo",
        "Asserted",
        "ASSERTED",
        " asserted",
        "asserted ",
        "",
        "justification",
        "needs Support",
    ] {
        let what = format!("assertionDeclaration={value:?}");
        let (input, at) = marked_once(
            &what,
            &claim(&format!(" {MARK}assertionDeclaration=\"{value}\""), ""),
        );
        let error = refused(&what, &input);
        assert_invalid_literal(&what, &error, value, at);
    }
}

#[test]
fn sacm23_val_006_a_bad_attribute_value_is_refused_even_beside_a_valid_sibling() {
    // No document is returned, so neither Claim can be held with an unknown
    // value or read as `asserted`.
    let (input, at) = marked_once(
        "a bad value after a valid sibling",
        &in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"ok\" assertionDeclaration=\"assumed\">\
             <name content=\"OK\"/></argumentElement>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}assertionDeclaration=\"Assumed\">\
             <name content=\"C\"/></argumentElement>"
        )),
    );
    let error = refused("a bad value after a valid sibling", &input);
    assert_invalid_literal("a bad value after a valid sibling", &error, "Assumed", at);
}

// --- Item 2: normalisation ---------------------------------------------------

#[test]
fn sacm23_val_006_an_attribute_value_is_normalised_and_never_trimmed() {
    for (written, as_read) in [
        ("asserted&#9;", "asserted\t"),
        ("asserted\t", "asserted "),
        ("\nassumed", " assumed"),
        ("&#xA;assumed", "\nassumed"),
    ] {
        let what = format!("assertionDeclaration=\"{}\"", written.escape_debug());
        let (input, at) = marked_once(
            &what,
            &claim(&format!(" {MARK}assertionDeclaration=\"{written}\""), ""),
        );
        let error = refused(&what, &input);
        assert_invalid_literal(&what, &error, as_read, at);
    }
}

// --- Item 3: the element form ------------------------------------------------

#[test]
fn sacm23_val_006_each_bad_element_value_is_invalid_literal_at_its_start_tag_with_the_exact_content()
 {
    for (element, as_read) in [
        ("<assertionDeclaration>foo</assertionDeclaration>", "foo"),
        (
            "<assertionDeclaration> asserted </assertionDeclaration>",
            " asserted ",
        ),
        ("<assertionDeclaration/>", ""),
        ("<assertionDeclaration></assertionDeclaration>", ""),
        (
            "<assertionDeclaration><![CDATA[Asserted]]></assertionDeclaration>",
            "Asserted",
        ),
        (
            "<assertionDeclaration>&#x41;sserted</assertionDeclaration>",
            "Asserted",
        ),
        (
            "<assertionDeclaration>assumed&#9;</assertionDeclaration>",
            "assumed\t",
        ),
        (
            "<assertionDeclaration>\r\nassumed</assertionDeclaration>",
            "\nassumed",
        ),
    ] {
        let (input, at) = marked_once(element, &claim("", &format!("{MARK}{element}")));
        let error = refused(element, &input);
        assert_invalid_literal(element, &error, as_read, at);
    }
}

#[test]
fn sacm23_val_006_a_comment_inside_a_literal_element_is_ignored_and_it_reads() {
    let doc = read_ok(
        "asser<!--x-->ted",
        &claim(
            "",
            "<assertionDeclaration>asser<!--x-->ted</assertionDeclaration>",
        ),
    );
    assert_eq!(
        doc.get("c1").and_then(SacmElement::assertion_declaration),
        Some(AssertionDeclaration::Asserted),
        "{ROW}: asser<!--x-->ted does not read as asserted"
    );
    let doc = read_ok(
        "needs<?pi?>Support",
        &claim(
            "",
            "<assertionDeclaration>needs<?pi x?>Support</assertionDeclaration>",
        ),
    );
    assert_eq!(
        doc.get("c1").and_then(SacmElement::assertion_declaration),
        Some(AssertionDeclaration::NeedsSupport),
        "{ROW}: needs<?pi x?>Support does not read as needsSupport"
    );
}

// --- Item 4: the first defect in document order ------------------------------

#[test]
fn sacm23_val_006_the_first_defect_in_document_order_decides() {
    let cases = [
        (
            "a bad attribute and a good element",
            claim(
                &format!(" {MARK}assertionDeclaration=\"Asserted\""),
                "<assertionDeclaration>assumed</assertionDeclaration>",
            ),
            "Asserted",
        ),
        (
            "a bad attribute and a bad element",
            claim(
                &format!(" {MARK}assertionDeclaration=\"Asserted\""),
                "<assertionDeclaration>bogus</assertionDeclaration>",
            ),
            "Asserted",
        ),
        (
            "a good attribute and a bad element",
            claim(
                " assertionDeclaration=\"assumed\"",
                &format!("{MARK}<assertionDeclaration>bogus</assertionDeclaration>"),
            ),
            "bogus",
        ),
        (
            "an equal good attribute and a bad element",
            claim(
                " assertionDeclaration=\"assumed\"",
                &format!("{MARK}<assertionDeclaration> assumed</assertionDeclaration>"),
            ),
            " assumed",
        ),
        (
            "a good element and a bad element",
            claim(
                "",
                &format!(
                    "<assertionDeclaration>assumed</assertionDeclaration>\
                     {MARK}<assertionDeclaration/>"
                ),
            ),
            "",
        ),
    ];
    for (what, template, value) in cases {
        let (input, at) = marked_once(what, &template);
        let error = refused(what, &input);
        assert!(
            !matches!(error, ReadError::RepeatedValue { .. }),
            "{ROW}: {what}: the repeat was counted before the value was checked: {error:?}"
        );
        assert_invalid_literal(what, &error, value, at);
    }
    // Two good occurrences stay a repeated value, at both.
    let (input, marks) = marked(&claim(
        &format!(" {MARK}assertionDeclaration=\"assumed\""),
        &format!("{MARK}<assertionDeclaration>assumed</assertionDeclaration>"),
    ));
    let error = refused("two good occurrences", &input);
    assert!(
        matches!(&error, ReadError::RepeatedValue { property, .. } if property == "assertionDeclaration"),
        "{ROW}: two good occurrences are not RepeatedValue: {error:?}"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: two good occurrences: the locations"
    );
}

// --- Item 5: unchanged refusals ----------------------------------------------

#[test]
fn sacm23_val_006_the_property_on_a_non_assertion_and_a_qualified_attribute_stay_unsupported() {
    let mut cases = Vec::new();
    for value in ["asserted", "foo"] {
        let attribute = format!("{MARK}assertionDeclaration=\"{value}\"");
        cases.push((
            format!("{value:?} on an ArgumentPackage root"),
            format!("{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\" {attribute}/>"),
        ));
        cases.push((
            format!("{value:?} on an ArgumentPackage child"),
            in_acp(&format!("<argumentPackage xmi:id=\"x\" {attribute}/>")),
        ));
        cases.push((
            format!("{value:?} on an AssuranceCasePackage child"),
            in_acp(&format!("<assuranceCasePackage xmi:id=\"x\" {attribute}/>")),
        ));
        cases.push((
            format!("sacm:assertionDeclaration={value:?} on a Claim"),
            claim(&format!(" {MARK}sacm:assertionDeclaration=\"{value}\""), ""),
        ));
        cases.push((
            format!("an element holding {value:?} on a package"),
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\">\
                 <name content=\"X\"/>{MARK}<assertionDeclaration>{value}</assertionDeclaration>\
                 </argumentElement>"
            )),
        ));
    }
    for (what, template) in &cases {
        let (input, at) = marked_once(what, template);
        let error = refused(what, &input);
        assert!(
            matches!(error, ReadError::Unsupported { .. }),
            "{ROW}: {what}: expected Unsupported, got {error:?}"
        );
        assert_eq!(error.code(), None, "{ROW}: {what}: Unsupported has a code");
        assert_eq!(
            error.location(),
            Some(at),
            "{ROW}: {what}: refused elsewhere: {error}"
        );
    }
}

// --- Item 6: the literals still read -----------------------------------------

#[test]
fn sacm23_val_006_the_six_literals_and_an_absent_attribute_still_read_the_same_in_both_modes() {
    let absent = read_ok("no declaration", &claim("", ""));
    assert_eq!(
        absent
            .get("c1")
            .and_then(SacmElement::assertion_declaration),
        Some(AssertionDeclaration::Asserted),
        "{ROW}: a Claim with no declaration does not read as asserted"
    );
    for (literal, expected) in LITERALS.into_iter().zip(AssertionDeclaration::ALL) {
        for (form, input) in [
            (
                "attribute",
                claim(&format!(" assertionDeclaration=\"{literal}\""), ""),
            ),
            (
                "element",
                claim(
                    "",
                    &format!("<assertionDeclaration>{literal}</assertionDeclaration>"),
                ),
            ),
        ] {
            let what = format!("{literal} as an {form}");
            let doc = read_ok(&what, &input);
            assert_eq!(
                doc.get("c1").and_then(SacmElement::assertion_declaration),
                Some(*expected),
                "{ROW}: {what} does not read as {expected}"
            );
        }
    }
}

// --- Item 7: Display and traits ----------------------------------------------

#[test]
fn sacm23_val_006_invalid_literal_display_is_one_line_naming_the_element_property_value_location_and_literals()
 {
    let cases = [
        (
            "an attribute",
            claim(&format!(" {MARK}assertionDeclaration=\"Asserted\""), ""),
            "Asserted",
        ),
        (
            "an attribute holding a line feed",
            claim(
                &format!(" {MARK}assertionDeclaration=\"needs&#xA;Support\""),
                "",
            ),
            "needs\nSupport",
        ),
        (
            "an element holding quotes and a backslash",
            claim(
                "",
                &format!("{MARK}<assertionDeclaration>\"a\\b\"</assertionDeclaration>"),
            ),
            "\"a\\b\"",
        ),
    ];
    for (what, template, value) in cases {
        let (input, at) = marked_once(what, &template);
        let error = refused(what, &input);
        assert_invalid_literal(what, &error, value, at);
        let shown = error.to_string();
        assert_one_lowercase_line(what, &shown);
        for part in [
            "\"c1\"".to_owned(),
            "assertionDeclaration".to_owned(),
            format!("{value:?}"),
            at.to_string(),
        ] {
            assert!(
                shown.contains(&part),
                "{ROW}: {what}: Display {shown:?} does not contain {part:?}"
            );
        }
        for literal in LITERALS {
            assert!(
                shown.contains(literal),
                "{ROW}: {what}: Display {shown:?} does not list the literal {literal}"
            );
        }
    }
}

fn assert_error_type<T: Error + Clone + Eq + Debug + Display + Send + Sync + 'static>() {}

#[test]
fn sacm23_val_006_invalid_literal_is_clone_eq_debug_and_an_error_with_no_source() {
    assert_error_type::<ReadError>();
    let (input, at) = marked_once(
        "Asserted",
        &claim(&format!(" {MARK}assertionDeclaration=\"Asserted\""), ""),
    );
    let error = refused("Asserted", &input);
    assert_invalid_literal("Asserted", &error, "Asserted", at);
    assert_eq!(error.clone(), error, "{ROW}: a clone is not equal");
    assert!(error.source().is_none(), "{ROW}: the refusal has a source");
    assert!(
        format!("{error:?}").contains("InvalidLiteral"),
        "{ROW}: Debug does not name the variant: {error:?}"
    );
    assert_eq!(
        refused("Asserted again", &input),
        error,
        "{ROW}: reading the same input again gives another refusal"
    );
    let boxed: Box<dyn Error + Send + Sync> = Box::new(error.clone());
    assert_eq!(
        boxed.downcast_ref::<ReadError>(),
        Some(&error),
        "{ROW}: the boxed refusal does not downcast to itself"
    );
}

// --- Item 8: the catalog -----------------------------------------------------

/// The catalog row of `code`, which must be the only one.
fn catalog_row(code: Code) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", code.as_str());
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&cell))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: the catalog has {} rows for {code}, not one",
            rows.len()
        );
    };
    (*line).to_owned()
}

#[test]
fn sacm23_val_006_the_catalog_enum_001_row_names_the_code_and_the_refusal() {
    assert_eq!(
        Code::ENUM_INVALID_LITERAL.as_str(),
        "SACM-ENUM-001",
        "{ROW}: Code::ENUM_INVALID_LITERAL is not SACM-ENUM-001"
    );
    let line = catalog_row(Code::ENUM_INVALID_LITERAL);
    assert!(
        line.starts_with("| SACM-ENUM-001 | Error |"),
        "{ROW}: the row is not an Error: {line}"
    );
    for part in [
        "`Code::ENUM_INVALID_LITERAL`",
        "`ReadError::InvalidLiteral`",
        "SACM23-VAL-006",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-ENUM-001 row does not name {part}: {line}"
        );
    }
}
