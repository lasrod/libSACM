//! Tests for the locations and the `Display` of a `ReadError::RepeatedValue`
//! made by a reference attribute of two or more ids on a single-valued end
//! (SACM23-XMI-005; XMI 2.5.1 §7.10.1, `xsd:IDREFS` tokens; ADR 0026).
//!
//! - Each id of the attribute is one occurrence, located at its own first
//!   character in the input: line and column of the token, not of the
//!   attribute's name. Lines end at LF, CRLF or a lone CR; columns count
//!   characters.
//! - `locations()` lists every occurrence, however many; `Display` stays
//!   bounded: it names the count and the first two locations only.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the input never holds otherwise.

use sacm::diagnostic::{Code, SourceLocation};
use sacm::xmi::{self, Mode, ReadError, ReadOptions};

const ROW: &str = "SACM23-XMI-005";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks an expected location in a template.
const MARK: char = '\u{2038}';

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
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

/// The root `ArgumentPackage` `p` holding the Claims `c` and `d`, the
/// `ArgumentReasoning` `r` and the `AssertedInference` `x` with the start-tag
/// `attributes`.
fn inference(attributes: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"d\"/>\
         <argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r\"/>\
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"x\"{attributes}/>\
         </sacm:ArgumentPackage>"
    )
}

/// Reads `input` in `mode`, which the test expects to be refused.
fn refused(what: &str, mode: Mode, input: &str) -> ReadError {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(_) => panic!("{ROW}: {what}: a {mode:?} read accepted it\n{input}"),
        Err(error) => error,
    }
}

/// Asserts that `error` is `RepeatedValue { x, property }`, SACM-MULT-001,
/// at exactly `locations`, with `location()` the second.
fn assert_repeated_at(
    what: &str,
    mode: Mode,
    error: &ReadError,
    property: &str,
    locations: &[SourceLocation],
) {
    assert!(
        matches!(
            error,
            ReadError::RepeatedValue { element, property: found, .. }
                if element.as_str() == "x" && found == property
        ),
        "{ROW}: {what}: {mode:?}: not RepeatedValue {{ x, {property} }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Some(Code::MULTIPLICITY_VIOLATION),
            locations,
            locations.get(1).copied()
        ),
        "{ROW}: {what}: {mode:?}: not SACM-MULT-001 at each id's own column"
    );
}

#[test]
fn sacm23_xmi_005_each_id_of_a_repeated_reference_attribute_is_located_at_its_own_column() {
    let cases = [
        (
            "target=\"c d\"",
            "target",
            format!(" source=\"c\" target=\"{MARK}c {MARK}d\""),
        ),
        (
            "reasoning=\"  r   r \", padded",
            "reasoning",
            format!(" source=\"c\" target=\"d\" reasoning=\"  {MARK}r   {MARK}r \""),
        ),
        (
            "three ids in citedElement, tab separated",
            "citedElement",
            format!(" citedElement=\"{MARK}c\t{MARK}d\t{MARK}c\" source=\"c\" target=\"d\""),
        ),
        (
            "abstractForm with ids on three lines (LF, CRLF)",
            "abstractForm",
            format!(" abstractForm=\"{MARK}c\n  {MARK}d\r\n{MARK}c\" source=\"c\" target=\"d\""),
        ),
        (
            "abstractForm with a lone CR between the ids",
            "abstractForm",
            format!(" abstractForm=\"{MARK}c\r{MARK}d\" source=\"c\" target=\"d\""),
        ),
        (
            "citedElement with a lone CR, then a CRLF, between the ids",
            "citedElement",
            format!(" citedElement=\"{MARK}c\r{MARK}d\r\n{MARK}c\" source=\"c\" target=\"d\""),
        ),
    ];
    for (what, property, attributes) in &cases {
        let (input, marks) = marked(&inference(attributes));
        for mode in MODES {
            let error = refused(what, mode, &input);
            assert_repeated_at(what, mode, &error, property, &marks);
        }
    }
    // The ids are distinct locations even when they are the same id.
    let (input, marks) = marked(&inference(&format!(
        " source=\"c\" target=\"{MARK}c {MARK}c\""
    )));
    let error = refused("target=\"c c\"", Mode::Strict, &input);
    assert!(
        marks[0] != marks[1] && error.locations() == marks,
        "{ROW}: target=\"c c\": the two occurrences are not at two columns: {:?}",
        error.locations()
    );
}

#[test]
fn sacm23_xmi_005_an_id_after_a_character_reference_separator_is_located_at_its_character() {
    // XML 1.0 §3.3.3 appends the character a character reference names, so
    // `&#32;` is a space and `&#x9;` a tab, not a space. Both are XML white
    // space, so they separate ids (XML Schema 1.0 Part 2 §3.3.10), but the
    // location is in the source text: the id after one is at its own
    // character, past the reference as written.
    let cases = [
        (
            "target=\"c&#32;d\"",
            "target",
            format!(" source=\"c\" target=\"{MARK}c&#32;{MARK}d\""),
        ),
        (
            "reasoning=\"r&#x9;r\"",
            "reasoning",
            format!(" source=\"c\" target=\"d\" reasoning=\"{MARK}r&#x9;{MARK}r\""),
        ),
        (
            "citedElement=\"&#32;c&#x9;&#32;d\", padded by references",
            "citedElement",
            format!(" citedElement=\"&#32;{MARK}c&#x9;&#32;{MARK}d\" source=\"c\" target=\"d\""),
        ),
        (
            "abstractForm=\"c&#32;d c\", a reference and a space",
            "abstractForm",
            format!(" abstractForm=\"{MARK}c&#32;{MARK}d {MARK}c\" source=\"c\" target=\"d\""),
        ),
    ];
    for (what, property, attributes) in &cases {
        let (input, marks) = marked(&inference(attributes));
        for mode in MODES {
            let error = refused(what, mode, &input);
            assert_repeated_at(what, mode, &error, property, &marks);
        }
    }
}

#[test]
fn sacm23_xmi_005_two_ids_display_the_count_and_both_locations() {
    let (input, marks) = marked(&inference(&format!(
        " source=\"c\" target=\"{MARK}c {MARK}d\""
    )));
    let [first, second] = marks[..] else {
        panic!("{ROW}: setup: two marks expected, found {marks:?}");
    };
    for mode in MODES {
        let error = refused("target=\"c d\"", mode, &input);
        assert_eq!(
            error.to_string(),
            format!(
                "the single-valued property \"target\" of the element \"x\" is given 2 times, \
                 at {first}; {second}"
            ),
            "{ROW}: {mode:?}: the Display of two occurrences is not the pinned line"
        );
    }
}

#[test]
fn sacm23_xmi_005_a_300000_token_single_valued_end_has_a_bounded_display() {
    const COUNT: usize = 300_000;
    let mut value = String::with_capacity(COUNT * 2 + 2);
    for index in 0..COUNT {
        if index > 0 {
            value.push(' ');
        }
        if index < 2 {
            value.push(MARK);
        }
        value.push('c');
    }
    let (input, marks) = marked(&inference(&format!(" source=\"c\" target=\"{value}\"")));
    let [first, second] = marks[..] else {
        panic!("{ROW}: setup: two marks expected, found {marks:?}");
    };
    for mode in MODES {
        let error = refused("a target of 300000 ids", mode, &input);
        let locations = error.locations();
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property, .. }
                    if element.as_str() == "x" && property == "target"
            ) && error.code() == Some(Code::MULTIPLICITY_VIOLATION),
            "{ROW}: {mode:?}: not RepeatedValue {{ x, target }}: {:?}",
            error.code()
        );
        assert_eq!(
            (locations.len(), locations.first(), locations.get(1)),
            (COUNT, Some(&first), Some(&second)),
            "{ROW}: {mode:?}: locations() does not list every id from its own column"
        );
        assert!(
            locations.windows(2).all(|pair| pair[0] < pair[1]),
            "{ROW}: {mode:?}: the locations are not distinct and in document order"
        );
        let display = error.to_string();
        let expected = format!("is given {COUNT} times, first at {first}; {second}");
        assert!(
            display.len() <= 256 && display.ends_with(&expected) && !display.contains('\n'),
            "{ROW}: {mode:?}: Display is not one bounded line ending {expected:?}: {} bytes, \
             starting {:?}",
            display.len(),
            display.chars().take(200).collect::<String>()
        );
        assert_eq!(
            display.matches("line ").count(),
            2,
            "{ROW}: {mode:?}: Display names more or fewer than the first two locations"
        );
    }
}
