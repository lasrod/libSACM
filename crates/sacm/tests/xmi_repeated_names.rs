//! Tests for a repeated `name` (SACM23-XMI-021; formal/23-05-08 §8.6
//! `name:LangString[1]`; XMI 2.5.1 §7.8.2, rule 4f; D-STRICT; D-PRESERVE;
//! ADR 0021 amendments 1 and 2).
//!
//! A `ModelElement` given two or more `name` elements in no namespace is refused
//! in both modes as `ReadError::RepeatedValue` (SACM-MULT-001) with property
//! `name`, even when the names are equal. `element` is the owner, `locations()`
//! the start tag of every `name` in document order, and `location()` the
//! second. Each later `name` is checked in full first, so a malformed later
//! name is refused as its own defect.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters.

use std::path::Path;

use sacm::diagnostic::{Code, SourceLocation};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-XMI-021";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// A declaration ended by CRLF, then a comment holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

// --- Inputs ------------------------------------------------------------------

/// A bare ArgumentPackage root `a` whose content, from line 2, is `content`
/// (the layout of `xmi_names.rs`).
fn root_holding(content: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\">\n{content}\n</sacm:ArgumentPackage>")
}

/// `content` in an ArgumentPackage root `p` named "P", after [`LEAD`].
fn in_ap(content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n  \
         {content}\r\n</sacm:ArgumentPackage>"
    )
}

/// `content` in an AssuranceCasePackage root `case` named "Case".
fn in_acp(content: &str) -> String {
    format!(
        "{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\r\n  <name content=\"Case\"/>\r\n  \
         {content}\r\n</sacm:AssuranceCasePackage>"
    )
}

/// A Claim `c1` holding `content`, in [`in_ap`].
fn in_claim(content: &str) -> String {
    in_ap(&format!(
        "<!-- \u{e9} --> <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">{content}</argumentElement>"
    ))
}

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

// --- Reading -----------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
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

/// Asserts that `template`, read in both modes, is `RepeatedValue` for the
/// `name` of `owner`, at every mark, with `location()` the second.
fn assert_repeated_name(what: &str, template: &str, owner: &str) -> ReadError {
    let (input, marks) = marked(template);
    assert!(
        marks.len() >= 2,
        "{ROW}: setup: {what} has {} marks, not two or more",
        marks.len()
    );
    let error = refused(what, &input);
    let ReadError::RepeatedValue {
        element,
        property,
        locations,
        ..
    } = &error
    else {
        panic!("{ROW}: {what}: expected RepeatedValue, got {error:?}");
    };
    assert_eq!(element.as_str(), owner, "{ROW}: {what}: the element");
    assert_eq!(property, "name", "{ROW}: {what}: the property");
    assert_eq!(
        locations.as_slice(),
        marks.as_slice(),
        "{ROW}: {what}: the locations field is not every name start tag in order"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: {what}: locations() is not every name start tag in order"
    );
    assert_eq!(
        error.location(),
        Some(marks[1]),
        "{ROW}: {what}: location() is not the second name"
    );
    assert_eq!(
        error.code(),
        Some(Code::MULTIPLICITY_VIOLATION),
        "{ROW}: {what}: the code is not SACM-MULT-001"
    );
    error
}

/// Asserts that `template`, read in both modes, is refused at its one mark
/// with a refusal `is_expected` accepts, and is not `RepeatedValue` for
/// `name`.
fn assert_refused_at_mark(
    what: &str,
    template: &str,
    expected: &str,
    is_expected: fn(&ReadError) -> bool,
) {
    let (input, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has {} marks, not one", marks.len());
    };
    let error = refused(what, &input);
    assert!(
        !matches!(&error, ReadError::RepeatedValue { property, .. } if property == "name"),
        "{ROW}: {what}: refused as a repeated name: {error:?}"
    );
    assert!(
        is_expected(&error),
        "{ROW}: {what}: expected {expected}, got {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(*at),
        "{ROW}: {what}: refused elsewhere: {error}"
    );
}

fn is_unsupported(error: &ReadError) -> bool {
    matches!(error, ReadError::Unsupported { .. }) && error.code().is_none()
}

// --- Item 1: two names -------------------------------------------------------

#[test]
fn sacm23_xmi_021_two_names_on_a_root_or_a_child_are_repeated_value_in_both_modes() {
    let cases = [
        (
            "two different names on a root",
            root_holding(&format!(
                "  {MARK}<name content=\"A\"/>\n  {MARK}<name content=\"B\"/>"
            )),
            "a",
        ),
        (
            "two equal empty names on a root",
            root_holding(&format!("  {MARK}<name/>\n  {MARK}<name/>")),
            "a",
        ),
        (
            "an element-form name and an attribute-form name on a root",
            root_holding(&format!(
                "  {MARK}<name><content>A</content></name>{MARK}<name lang=\"en\"/>"
            )),
            "a",
        ),
        (
            "two equal names on a Claim",
            in_claim(&format!(
                "{MARK}<name content=\"A\"/>{MARK}<name content=\"A\"/>"
            )),
            "c1",
        ),
        (
            "two names on an ArgumentPackage child",
            in_acp(&format!(
                "<argumentPackage xmi:id=\"ap\">{MARK}<name/>\r\n  {MARK}<name/></argumentPackage>"
            )),
            "ap",
        ),
        (
            "two names on a wrapped root",
            format!(
                "{LEAD}<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\r\n  \
                 <sacm:ArgumentPackage xmi:id=\"w\">{MARK}<name content=\"W\"/>\
                 {MARK}<name content=\"W\"/></sacm:ArgumentPackage>\r\n</xmi:XMI>"
            ),
            "w",
        ),
    ];
    for (what, template, owner) in &cases {
        assert_repeated_name(what, template, owner);
    }
}

// --- Item 2: three names, and a malformed later name -------------------------

#[test]
fn sacm23_xmi_021_three_names_give_three_locations_and_the_second_is_the_location() {
    assert_repeated_name(
        "three names on a root",
        &root_holding(&format!(
            "  {MARK}<name content=\"A\"/>\n  {MARK}<name content=\"B\"/>\n  {MARK}<name/>"
        )),
        "a",
    );
    assert_repeated_name(
        "three names on a Claim",
        &in_claim(&format!("{MARK}<name/>{MARK}<name/>\r\n{MARK}<name/>")),
        "c1",
    );
}

#[test]
fn sacm23_xmi_021_a_malformed_second_name_is_refused_as_its_own_defect_first() {
    let unsupported: [(&str, String); 4] = [
        (
            "a second name with an unread attribute",
            in_claim(&format!("<name content=\"A\"/><name {MARK}foo=\"1\"/>")),
        ),
        (
            "a second name holding text",
            in_claim(&format!("<name content=\"A\"/><name>{MARK}x</name>")),
        ),
        (
            "a second name typed as another class",
            in_claim(&format!(
                "<name content=\"A\"/>{MARK}<name xmi:type=\"sacm:MultiLangString\"/>"
            )),
        ),
        (
            "a second name holding an element",
            in_claim(&format!("<name content=\"A\"/><name>{MARK}<value/></name>")),
        ),
    ];
    for (what, template) in &unsupported {
        assert_refused_at_mark(what, template, "Unsupported", is_unsupported);
    }
    // A second name whose own `content` is given twice: that repeat first.
    let (input, marks) = marked(&in_claim(&format!(
        "<name content=\"A\"/><name {MARK}content=\"B\">{MARK}<content>B</content></name>"
    )));
    let error = refused("a second name repeating its content", &input);
    assert!(
        matches!(&error, ReadError::RepeatedValue { property, .. } if property == "name.content"),
        "{ROW}: a second name repeating its content is not RepeatedValue for name.content: {error:?}"
    );
    assert_eq!(
        error.locations(),
        marks.as_slice(),
        "{ROW}: a second name repeating its content: the locations"
    );
    // A second name with an empty id: InvalidId at that name (SACM23-XMI-022).
    assert_refused_at_mark(
        "a second name with an empty xmi:id",
        &in_claim(&format!(
            "<name content=\"A\"/>{MARK}<name xmi:id=\"\" content=\"B\"/>"
        )),
        "InvalidId",
        |error| matches!(error, ReadError::InvalidId { id, .. } if id.is_empty()),
    );
}

// --- Item 3: what separates the names ----------------------------------------

#[test]
fn sacm23_xmi_021_names_separated_by_children_white_space_comments_or_pis_still_count() {
    // The Claim's own name is the Claim's; the root's two are the root's.
    assert_repeated_name(
        "names around a child, comments and a processing instruction",
        &root_holding(&format!(
            "  {MARK}<name content=\"A\"/>\n  <!-- c -->\n  <?pi x?>\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>\n  \
             \t {MARK}<name content=\"B\"/>"
        )),
        "a",
    );
    assert_repeated_name(
        "names in a Claim around a comment",
        &in_claim(&format!(
            "{MARK}<name content=\"A\"/>\r\n  <!-- c --> <?pi?>\r\n  {MARK}<name content=\"A\"/>"
        )),
        "c1",
    );
}

// --- Item 4: what is not an occurrence ---------------------------------------

#[test]
fn sacm23_xmi_021_a_qualified_name_or_a_name_attribute_is_not_an_occurrence() {
    let cases = [
        (
            "sacm:name after a name on a root",
            root_holding(&format!(
                "  <name content=\"A\"/>\n  {MARK}<sacm:name content=\"B\"/>"
            )),
        ),
        (
            "sacm:name after a name on a Claim",
            in_claim(&format!(
                "<name content=\"A\"/>{MARK}<sacm:name content=\"B\"/>"
            )),
        ),
        (
            "a name in the SACM default namespace after a name",
            in_claim(&format!(
                "<name content=\"A\"/>{MARK}<name xmlns=\"http://www.omg.org/spec/SACM/20220301\" content=\"B\"/>"
            )),
        ),
        (
            "a name attribute on a root with a name element",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}name=\"x\">\n  <name content=\"A\"/>\n\
                 </sacm:ArgumentPackage>"
            ),
        ),
        (
            "a name attribute on a Claim with a name element",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {MARK}name=\"x\">\
                 <name content=\"A\"/></argumentElement>"
            )),
        ),
    ];
    for (what, template) in &cases {
        assert_refused_at_mark(what, template, "Unsupported", is_unsupported);
    }
}

// --- Item 5: Display, docs and the catalog -----------------------------------

#[test]
fn sacm23_xmi_021_a_repeated_name_display_names_the_property_the_element_and_every_location() {
    let error = refused(
        "two names",
        &root_holding("  <name content=\"A\"/>\n  <name content=\"B\"/>"),
    );
    assert_eq!(
        error.to_string(),
        "the single-valued property \"name\" of the element \"a\" is given 2 times, \
         at line 2, column 3; line 3, column 3",
        "{ROW}: the Display of a repeated name"
    );
}

#[test]
fn sacm23_xmi_021_the_catalog_mult_001_row_no_longer_waits_for_this_row() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("| SACM-MULT-001 |"))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: the catalog has {} SACM-MULT-001 rows, not one",
            rows.len()
        );
    };
    assert!(
        !line.contains("once SACM23-XMI-021 is built"),
        "{ROW}: the SACM-MULT-001 row still waits for SACM23-XMI-021: {line}"
    );
    assert!(
        line.contains("`ReadError::RepeatedValue`"),
        "{ROW}: the SACM-MULT-001 row does not name ReadError::RepeatedValue: {line}"
    );
}
