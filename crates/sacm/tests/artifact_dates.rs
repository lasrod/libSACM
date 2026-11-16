//! Tests for the date-typed attributes of slice 0055 (SACM23-ART-002;
//! formal/23-05-08 §12.7 "date: date[0..1] - the date on which the artifact
//! was created.", §12.9 "date: date[0..1] - the date on which the Event
//! occurred.", §12.11 "startTime: date[0..1] - time when the activity
//! started." and "endTime: date[0..1] - time when the activity ended.";
//! XMI 2.5.1 §8.2.1 rules 4d and 4j, §7.11.6; D-DATE; ADR 0034 amendment 1).
//!
//! SACM 2.3 gives `date` no lexical form, and the schema the XMI rules
//! produce types the four attributes `xsd:string`, so every value is held
//! exactly as written, in both modes and with no diagnostic: Artifact and
//! Event `date` (`SacmElement::date()`), Activity `startTime` and `endTime`
//! (`start_time()`, `end_time()`). Each is read from its attribute or its
//! value element (SACM23-XMI-014), `xsi:nil` included (SACM23-XMI-026), and
//! written as an attribute after the flags and references. Given twice it is
//! `RepeatedValue`; on a kind without it, it is preserved content. `SetDate`,
//! `SetStartTime` and `SetEndTime` hold any string; a character XML cannot
//! represent is refused by `xmi::write`. `semantic_compare` compares the
//! values exactly.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{
    self, EditError, Mutation, Operation, SetDate, SetEndTime, SetGid, SetName, SetStartTime,
    SetVersion,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

const ROW: &str = "SACM23-ART-002";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where an occurrence is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The worked fixture of slice 0055.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-assets-valid.sacm.xmi"
);

/// Each date-typed attribute: the class that holds it and its XMI name.
const HELD: [(&str, &str); 4] = [
    ("Artifact", "date"),
    ("Event", "date"),
    ("Activity", "startTime"),
    ("Activity", "endTime"),
];

/// The four values held verbatim, by their XMI names.
const NAMES: [&str; 4] = ["version", "date", "startTime", "endTime"];

/// Values a date-typed attribute holds as written: an `xsd:date`, two
/// `xsd:dateTime`s, a BCE year, a date no calendar has, free text, padded
/// text and the empty string.
const VALUES: [&str; 8] = [
    "2024-03-01",
    "2024-03-01T09:30:00Z",
    "2024-03-01T09:30:00.250+01:00",
    "-0044-03-15",
    "2024-02-30",
    "spring 2024",
    " 2024-03-01 ",
    "",
];

// --- Inputs ------------------------------------------------------------------------

/// An ArtifactPackage root `a` named "A" holding `body` on line 3, from
/// column 3.
fn in_a(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  {body}\n\
         </sacm:ArtifactPackage>"
    )
}

/// The element `id` of class `class`, typed in `artifactElement`, with the
/// extra start-tag `attributes`, named after its id and holding `content`
/// after its name.
fn asset(class: &str, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{content}</artifactElement>"
    )
}

/// An ArgumentPackage root `p` holding the Claim `c1`, which holds `content`
/// after its name, on line 3.
fn claim_holding(content: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/>{content}\
         </argumentElement>\n</sacm:ArgumentPackage>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

/// `template` without its one mark, and the mark's location.
fn one_mark(what: &str, template: &str) -> (String, SourceLocation) {
    let (text, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has {} marks, not one", marks.len());
    };
    (text, *at)
}

// --- Reading -----------------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert_eq!(
                    output.diagnostics(),
                    [],
                    "{ROW}: {what}: reading in {mode:?} mode reported diagnostics"
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in strict mode, which the test expects to refuse it.
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// Reads `input` in tolerant mode, which the test expects to read.
fn tolerant(what: &str, input: &str) -> ReadOutput {
    match read(Mode::Tolerant, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?} ({error})"),
    }
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, an Error in strict mode and
/// a Warning in tolerant mode, and the same document in both.
fn assert_preserved_at(what: &str, template: &str, holder: &str) -> Document {
    let (input, at) = one_mark(what, template);
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
            ),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(at),
                vec![ElementId::from(holder)]
            )],
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

/// The value `element` holds for the attribute named `name`, by its accessor.
fn held<'d>(element: SacmElement<'d>, name: &str) -> Option<&'d str> {
    match name {
        "version" => element.version(),
        "date" => element.date(),
        "startTime" => element.start_time(),
        "endTime" => element.end_time(),
        other => panic!("{ROW}: setup: no accessor for {other:?}"),
    }
}

/// Every one of the four values `element` holds, by name, in [`NAMES`] order.
fn all_held(element: SacmElement<'_>) -> Vec<(&'static str, Option<&str>)> {
    NAMES
        .iter()
        .map(|&name| (name, held(element, name)))
        .collect()
}

// --- Writing -----------------------------------------------------------------------

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

// --- Editing -----------------------------------------------------------------------

fn fixture() -> Document {
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    read_ok("the fixture", &text)
}

fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    mutation
}

fn refused_edit(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        checked,
        Err(error.clone()),
        "{ROW}: check answers {operation:?} otherwise than apply"
    );
    error
}

/// The set operation of the attribute `name`: `Some` sets, `None` removes.
fn set(name: &str, element: &str, value: Option<&str>) -> Operation {
    match (name, value) {
        ("date", Some(value)) => SetDate::new(element, value).into(),
        ("date", None) => SetDate::remove(element).into(),
        ("startTime", Some(value)) => SetStartTime::new(element, value).into(),
        ("startTime", None) => SetStartTime::remove(element).into(),
        ("endTime", Some(value)) => SetEndTime::new(element, value).into(),
        ("endTime", None) => SetEndTime::remove(element).into(),
        ("version", Some(value)) => SetVersion::new(element, value).into(),
        ("version", None) => SetVersion::remove(element).into(),
        (other, _) => panic!("{ROW}: setup: no set operation for {other:?}"),
    }
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

/// The variant of `value`, from its `Debug`.
fn variant<T: std::fmt::Debug>(value: &T) -> String {
    let shown = format!("{value:?}");
    shown
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned()
}

// =====================================================================================
// Item 1: every value is read verbatim, in both modes, with no diagnostic
// =====================================================================================

#[test]
fn sacm23_art_002_each_date_attribute_reads_verbatim_in_both_modes_with_no_diagnostic() {
    for (class, name) in HELD {
        for value in VALUES {
            let what = format!("{name}={value:?} on an {class}");
            let doc = read_ok(
                &what,
                &in_a(&asset(class, "x", &format!(" {name}=\"{value}\""), "")),
            );
            let x = element(&doc, "x");
            let expected: Vec<(&str, Option<&str>)> = NAMES
                .iter()
                .map(|&other| (other, (other == name).then_some(value)))
                .collect();
            assert_eq!(
                (x.kind().name(), all_held(x)),
                (class, expected),
                "{ROW}: {what}: not held exactly as written, and alone"
            );
            assert_eq!(
                validation::validate(&doc),
                [],
                "{ROW}: {what}: validate reports the value"
            );
        }
        // An absent attribute is None.
        let what = format!("an {class} with no {name}");
        let doc = read_ok(&what, &in_a(&asset(class, "x", "", "")));
        assert_eq!(
            held(element(&doc, "x"), name),
            None,
            "{ROW}: {what}: not None"
        );
    }
    // The fixture: an xsd:dateTime with an offset and free text, held as
    // written, and validate finds nothing.
    let doc = fixture();
    assert_eq!(
        (
            element(&doc, "released").date(),
            element(&doc, "reviewed").date(),
            element(&doc, "report").date()
        ),
        (
            Some("2024-03-05T14:00:00+01:00"),
            Some("spring 2024"),
            Some("2024-03-01")
        ),
        "{ROW}: the fixture's dates"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture has findings"
    );
}

// =====================================================================================
// Item 2: the element form and xsi:nil; the attribute form is written
// =====================================================================================

#[test]
fn sacm23_art_002_the_element_form_and_xsi_nil_read_equal_to_the_attribute_form() {
    for (class, name) in HELD {
        let attribute_form = read_ok(
            "the attribute form",
            &in_a(&asset(class, "x", &format!(" {name}=\"2024-03-01\""), "")),
        );
        let attribute_text = write_in("the attribute form", &attribute_form, Mode::Strict);
        for (form, value_element) in [
            ("plain", format!("<{name}>2024-03-01</{name}>")),
            ("CDATA", format!("<{name}><![CDATA[2024-03-01]]></{name}>")),
            ("a comment", format!("<{name}>2024-<!--c-->03-01</{name}>")),
            (
                "a character reference",
                format!("<{name}>2024&#x2D;03-01</{name}>"),
            ),
            (
                "split CDATA and text",
                format!("<{name}>2024<![CDATA[-03]]>-01</{name}>"),
            ),
        ] {
            let what = format!("<{name}> ({form}) on an {class}");
            let doc = read_ok(&what, &in_a(&asset(class, "x", "", &value_element)));
            assert_eq!(
                held(element(&doc, "x"), name),
                Some("2024-03-01"),
                "{ROW}: {what}: not the value"
            );
            assert!(
                doc == attribute_form,
                "{ROW}: {what}: does not read equal to the attribute form: {:?}",
                semantic_compare(&doc, &attribute_form)
            );
            for mode in MODES {
                assert_eq!(
                    write_in(&what, &doc, mode),
                    attribute_text,
                    "{ROW}: {what}: not written byte-identical to the attribute form in {mode:?} \
                     mode"
                );
            }
        }
        // xsi:nil reads as absent.
        let what = format!("<{name} xsi:nil=\"true\"/> on an {class}");
        let doc = read_ok(
            &what,
            &in_a(&asset(
                class,
                "x",
                "",
                &format!("<{name} xsi:nil=\"true\"/>"),
            )),
        );
        let bare = read_ok("no value", &in_a(&asset(class, "x", "", "")));
        assert!(
            held(element(&doc, "x"), name).is_none() && doc == bare,
            "{ROW}: {what}: not read as absent"
        );
    }
    // The output order: the flags and references, then `version`, `date`,
    // `startTime` and `endTime`, however the input ordered them.
    for (class, attributes, written) in [
        (
            "Artifact",
            " date=\"2024-03-01\" version=\"1.2\" abstractForm=\"x2\" citedElement=\"x2\" \
             isCitation=\"true\" gid=\"G\"",
            " gid=\"G\" isCitation=\"true\" citedElement=\"x2\" abstractForm=\"x2\" \
             version=\"1.2\" date=\"2024-03-01\"",
        ),
        (
            "Event",
            " date=\"2024-03-05\" abstractForm=\"x2\" gid=\"G\"",
            " gid=\"G\" abstractForm=\"x2\" date=\"2024-03-05\"",
        ),
        (
            "Activity",
            " endTime=\"2024-02-29\" startTime=\"2024-02-12\" citedElement=\"x2\" \
             isCitation=\"true\"",
            " isCitation=\"true\" citedElement=\"x2\" startTime=\"2024-02-12\" \
             endTime=\"2024-02-29\"",
        ),
    ] {
        let what = format!("an {class} with every attribute");
        let doc = read_ok(
            &what,
            &in_a(&format!(
                "{}{}",
                asset(class, "x1", attributes, ""),
                asset(class, "x2", "", "")
            )),
        );
        for mode in MODES {
            let text = write_in(&what, &doc, mode);
            assert_eq!(
                line_with(&text, "x1"),
                format!("<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x1\"{written}>"),
                "{ROW}: {what}: the start tag in {mode:?} mode\n{text}"
            );
        }
    }
}

// =====================================================================================
// Item 3: escaping, and a round trip with the same bytes
// =====================================================================================

#[test]
fn sacm23_art_002_values_with_markup_and_line_breaks_are_escaped_and_round_trip_exactly() {
    let escaped = "a&amp;b&#9;c&#10;d&#13;e&lt;f&gt;g&quot;h";
    let value = "a&b\tc\nd\re<f>g\"h";
    for (class, name) in HELD {
        let what = format!("{name}=\"{escaped}\" on an {class}");
        let doc = read_ok(
            &what,
            &in_a(&asset(class, "x", &format!(" {name}=\"{escaped}\""), "")),
        );
        assert_eq!(
            held(element(&doc, "x"), name),
            Some(value),
            "{ROW}: {what}: not read to the characters"
        );
        let text = write_in(&what, &doc, Mode::Tolerant);
        assert!(
            line_with(&text, "x").contains(&format!(" {name}=\"{escaped}\"")),
            "{ROW}: {what}: not written escaped\n{text}"
        );
        let back = read_ok(&format!("{what}, written"), &text);
        assert!(
            back == doc,
            "{ROW}: {what}: does not read back equal: {:?}",
            semantic_compare(&doc, &back)
        );
        assert_eq!(
            write_in(&what, &back, Mode::Tolerant),
            text,
            "{ROW}: {what}: a second write gives other bytes"
        );
    }
}

// =====================================================================================
// Item 4: a value given twice
// =====================================================================================

#[test]
fn sacm23_art_002_a_date_given_twice_is_a_repeated_value_and_tolerant_keeps_the_first() {
    for (class, name) in HELD {
        for (form, attributes, content, first) in [
            (
                "an attribute and a value element",
                format!(" {MARK}{name}=\"a\""),
                format!("{MARK}<{name}>b</{name}>"),
                Some("a"),
            ),
            (
                "two value elements",
                String::new(),
                format!("{MARK}<{name}>a</{name}>\n{MARK}<{name}>b</{name}>"),
                Some("a"),
            ),
            (
                "xsi:nil, then a value element",
                String::new(),
                format!("{MARK}<{name} xsi:nil=\"true\"/>{MARK}<{name}>b</{name}>"),
                None,
            ),
        ] {
            let what = format!("{name}: {form} on an {class}");
            let (input, marks) = marked(&in_a(&asset(class, "x", &attributes, &content)));
            let error = refused(&what, &input);
            let ReadError::RepeatedValue {
                element: found_element,
                property,
                locations,
                ..
            } = &error
            else {
                panic!("{ROW}: {what}: not RepeatedValue: {error:?} ({error})");
            };
            assert_eq!(
                (found_element.as_str(), property.as_str(), locations),
                ("x", name, &marks),
                "{ROW}: {what}: the element, property or locations"
            );
            assert_eq!(
                (error.code(), error.location()),
                (Some(Code::MULTIPLICITY_VIOLATION), Some(marks[1])),
                "{ROW}: {what}: not SACM-MULT-001 with location() the second"
            );
            // Tolerant: the first is kept, the later one preserved, with one
            // SACM-MULT-001 Warning at it.
            let output = tolerant(&what, &input);
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|found| (found.code(), found.severity(), found.location()))
                .collect();
            assert_eq!(
                found,
                [(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Warning,
                    Some(marks[1])
                )],
                "{ROW}: {what}: not one SACM-MULT-001 Warning at the later occurrence"
            );
            let x = element(output.document(), "x");
            assert_eq!(
                (held(x, name), x.has_preserved_content()),
                (first, true),
                "{ROW}: {what}: the first is not kept, or the later one not preserved"
            );
        }
    }
}

// =====================================================================================
// Item 5: on a kind without it, or malformed: preserved content
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the kinds without the attribute and the malformed forms read best as one list"
)]
fn sacm23_art_002_a_date_attribute_on_a_kind_without_it_or_a_malformed_value_element_is_preserved()
{
    let mut cases: Vec<(String, String, &str)> = Vec::new();
    for (name, class) in [
        ("date", "Technique"),
        ("date", "Activity"),
        ("date", "Resource"),
        ("date", "Property"),
        ("date", "Participant"),
        ("startTime", "Artifact"),
        ("startTime", "Event"),
        ("endTime", "Artifact"),
        ("endTime", "Resource"),
    ] {
        cases.push((
            format!("{name}=\"2024-03-01\" on a {class}"),
            in_a(&asset(
                class,
                "h",
                &format!(" {MARK}{name}=\"2024-03-01\""),
                "",
            )),
            "h",
        ));
        cases.push((
            format!("<{name}> on a {class}"),
            in_a(&asset(
                class,
                "h",
                "",
                &format!("{MARK}<{name}>2024-03-01</{name}>"),
            )),
            "h",
        ));
    }
    cases.push((
        "<endTime> on a Claim".to_owned(),
        claim_holding(&format!("{MARK}<endTime>2024-03-01</endTime>")),
        "c1",
    ));
    cases.push((
        "date=\"2024-03-01\" on a Claim".to_owned(),
        claim_holding("").replace(
            "xmi:id=\"c1\">",
            &format!("xmi:id=\"c1\" {MARK}date=\"2024-03-01\">"),
        ),
        "c1",
    ));
    // A malformed value element, kept whole at its start tag; the namespaced
    // forms are not the attribute.
    for (class, name) in HELD {
        for (form, value_element) in [
            (
                "a foo attribute",
                format!("<{name} foo=\"1\">2024</{name}>"),
            ),
            (
                "a lang attribute",
                format!("<{name} lang=\"en\">2024</{name}>"),
            ),
            ("an xmi:id", format!("<{name} xmi:id=\"v\">2024</{name}>")),
            ("an element child", format!("<{name}><b/></{name}>")),
            (
                "an element after text",
                format!("<{name}>2024<b/></{name}>"),
            ),
            (
                "the sacm prefix",
                format!("<sacm:{name}>2024</sacm:{name}>"),
            ),
            (
                "a foreign namespace",
                format!("<o:{name} xmlns:o=\"urn:o\">2024</o:{name}>"),
            ),
        ] {
            cases.push((
                format!("<{name}> with {form} on an {class}"),
                in_a(&asset(class, "h", "", &format!("{MARK}{value_element}"))),
                "h",
            ));
        }
    }
    for (what, template, holder) in &cases {
        let doc = assert_preserved_at(what, template, holder);
        let holder_element = element(&doc, holder);
        assert!(
            all_held(holder_element)
                .iter()
                .all(|(_, value)| value.is_none()),
            "{ROW}: {what}: a value was read: {:?}",
            all_held(holder_element)
        );
    }
    // An XMI-namespace value element is XMI metadata this version does not
    // read yet (SACM23-XMI-019): refused at its start tag.
    for (class, name) in HELD {
        let what = format!("<xmi:{name}> on an {class}");
        let (input, at) = one_mark(
            &what,
            &in_a(&asset(
                class,
                "h",
                "",
                &format!("{MARK}<xmi:{name}>2024</xmi:{name}>"),
            )),
        );
        let error = refused(&what, &input);
        let ReadError::Unsupported { reason, .. } = &error else {
            panic!("{ROW}: {what}: not Unsupported: {error:?}");
        };
        assert!(
            error.location() == Some(at)
                && reason.ends_with("(a limit of this version; SACM23-XMI-019 reads it)"),
            "{ROW}: {what}: not Unsupported at its start tag naming SACM23-XMI-019: {error:?}"
        );
    }
}

// =====================================================================================
// Item 6: SetDate, SetStartTime and SetEndTime
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the sets, their refusals and the write refusals of one row read best together"
)]
fn sacm23_art_002_set_date_start_time_and_end_time_set_remove_and_refuse_in_order() {
    // Each holder: report (Artifact) and released (Event) hold `date`,
    // testing (Activity) the two times.
    for (target, name) in [
        ("report", "date"),
        ("released", "date"),
        ("testing", "startTime"),
        ("testing", "endTime"),
    ] {
        let mut doc = fixture();
        for value in VALUES.into_iter().chain(["\u{2013} \u{1F4C5}"]) {
            let before = held(element(&doc, target), name).map(str::to_owned);
            let mutation = applied(&mut doc, set(name, target, Some(value)));
            let expected: Vec<ElementId> = if before.as_deref() == Some(value) {
                Vec::new()
            } else {
                vec![id(target)]
            };
            assert_eq!(
                (
                    held(element(&doc, target), name),
                    mutation.changed_ids(),
                    mutation.created_ids(),
                    mutation.deleted_ids()
                ),
                (
                    Some(value),
                    expected.as_slice(),
                    [].as_slice(),
                    [].as_slice()
                ),
                "{ROW}: setting {name} of {target} to {value:?}"
            );
            // The same value again changes nothing.
            let again = applied(&mut doc, set(name, target, Some(value)));
            assert_eq!(
                again.changed_ids(),
                [],
                "{ROW}: setting {name} of {target} to {value:?} again"
            );
        }
        // It writes as held and reads back.
        applied(&mut doc, set(name, target, Some(" 2024-03-01 ")));
        let text = write_in("the edited fixture", &doc, Mode::Strict);
        let back = read_ok("the edited fixture, written", &text);
        assert_eq!(
            held(element(&back, target), name),
            Some(" 2024-03-01 "),
            "{ROW}: the set {name} of {target} does not read back as held"
        );
        // Removing: changed once, then nothing to change.
        let mutation = applied(&mut doc, set(name, target, None));
        assert_eq!(
            (held(element(&doc, target), name), mutation.changed_ids()),
            (None, [id(target)].as_slice()),
            "{ROW}: removing {name} of {target}"
        );
        let mutation = applied(&mut doc, set(name, target, None));
        assert_eq!(
            mutation.changed_ids(),
            [],
            "{ROW}: removing the absent {name} of {target}"
        );
    }
    // An unknown element, then a kind without the attribute.
    let mut doc = fixture();
    for name in ["date", "startTime", "endTime"] {
        for value in [Some("2024-03-01"), None] {
            let error = refused_edit(&mut doc, set(name, "nowhere", value));
            assert!(
                matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
                "{ROW}: {name} on an unknown element: not ElementNotFound: {error:?}"
            );
            assert_eq!(
                error.code(),
                Code::CMD_TARGET_NOT_FOUND,
                "{ROW}: {name} on an unknown element: not SACM-CMD-001"
            );
        }
    }
    for (name, target, kind) in [
        ("date", "testing", ElementKind::Activity),
        ("date", "report_pdf", ElementKind::Resource),
        ("date", "report_passed", ElementKind::Property),
        ("date", "evidence", ElementKind::ArtifactPackage),
        ("startTime", "report", ElementKind::Artifact),
        ("startTime", "released", ElementKind::Event),
        ("endTime", "report_pdf", ElementKind::Resource),
        ("endTime", "report", ElementKind::Artifact),
    ] {
        for value in [Some("2024-03-01"), None] {
            let error = refused_edit(&mut doc, set(name, target, value));
            assert!(
                matches!(
                    &error,
                    EditError::FeatureNotApplicable { element, kind: found, feature, .. }
                        if element.as_str() == target && *found == kind && *feature == name
                ),
                "{ROW}: {name} on the {kind} {target}: not FeatureNotApplicable naming {name}: \
                 {error:?}"
            );
            assert_eq!(
                (error.code(), error.to_string()),
                (
                    Code::CMD_FEATURE_NOT_APPLICABLE,
                    format!("the {kind} \"{target}\" has no {name}")
                ),
                "{ROW}: {name} on the {kind} {target}: the code and message"
            );
        }
    }
    // U+0001 is held, and the write refuses it: after Gid and Version, before
    // NameLang.
    let mut doc = fixture();
    applied(&mut doc, SetGid::new("report", "g\u{1}"));
    applied(&mut doc, SetVersion::new("report", "v\u{1}"));
    applied(&mut doc, SetDate::new("report", "d\u{1}"));
    applied(
        &mut doc,
        SetName::new("report", LangString::new("n\u{1}").with_lang("e\u{1}")),
    );
    applied(&mut doc, SetStartTime::new("testing", "s\u{1}"));
    applied(&mut doc, SetEndTime::new("testing", "e\u{1}"));
    applied(
        &mut doc,
        SetName::new("testing", LangString::new("n\u{1}").with_lang("e\u{1}")),
    );
    applied(&mut doc, SetDate::new("released", "d\u{1}"));
    assert_eq!(
        (
            element(&doc, "report").date(),
            element(&doc, "testing").start_time(),
            element(&doc, "testing").end_time()
        ),
        (Some("d\u{1}"), Some("s\u{1}"), Some("e\u{1}")),
        "{ROW}: a value holding U+0001 is not held exactly"
    );
    let refusal = |doc: &Document| -> (String, WriteAttribute) {
        let mut found = Vec::new();
        for mode in MODES {
            match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
                Ok(text) => panic!("{ROW}: a value holding U+0001 was written in {mode:?}: {text}"),
                Err(WriteError::UnrepresentableCharacter {
                    element,
                    character: '\u{1}',
                    attribute,
                    ..
                }) => found.push((element.as_str().to_owned(), attribute)),
                Err(other) => panic!("{ROW}: not UnrepresentableCharacter in {mode:?}: {other:?}"),
            }
        }
        assert_eq!(found[0], found[1], "{ROW}: the modes refuse otherwise");
        found.swap_remove(0)
    };
    // report holds four bad attributes, before testing and released.
    assert_eq!(
        refusal(&doc),
        ("report".to_owned(), WriteAttribute::Gid),
        "{ROW}: the gid first"
    );
    applied(&mut doc, SetGid::remove("report"));
    assert_eq!(
        refusal(&doc),
        ("report".to_owned(), WriteAttribute::Version),
        "{ROW}: the version after the gid"
    );
    applied(&mut doc, SetVersion::remove("report"));
    assert_eq!(
        refusal(&doc),
        ("report".to_owned(), WriteAttribute::Date),
        "{ROW}: the date after the version"
    );
    let error = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: written: {text}"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "the element \"report\" cannot be written: its date holds U+0001, which XML 1.0 cannot \
         represent",
        "{ROW}: the message of a date refusal"
    );
    applied(&mut doc, SetDate::remove("report"));
    assert_eq!(
        refusal(&doc),
        ("report".to_owned(), WriteAttribute::NameLang),
        "{ROW}: the name's lang after the date"
    );
    applied(&mut doc, SetName::new("report", LangString::new("Report")));
    assert_eq!(
        refusal(&doc),
        ("released".to_owned(), WriteAttribute::Date),
        "{ROW}: an Event's date"
    );
    applied(&mut doc, SetDate::remove("released"));
    assert_eq!(
        refusal(&doc),
        ("testing".to_owned(), WriteAttribute::StartTime),
        "{ROW}: the startTime first on an Activity"
    );
    applied(&mut doc, SetStartTime::remove("testing"));
    assert_eq!(
        refusal(&doc),
        ("testing".to_owned(), WriteAttribute::EndTime),
        "{ROW}: the endTime after the startTime"
    );
    applied(&mut doc, SetEndTime::remove("testing"));
    assert_eq!(
        refusal(&doc),
        ("testing".to_owned(), WriteAttribute::NameLang),
        "{ROW}: the name's lang after the endTime"
    );
    // The attributes display by their XMI names.
    assert_eq!(
        [
            WriteAttribute::Version,
            WriteAttribute::Date,
            WriteAttribute::StartTime,
            WriteAttribute::EndTime
        ]
        .map(|attribute| attribute.to_string()),
        ["version", "date", "startTime", "endTime"],
        "{ROW}: the Display of the WriteAttributes"
    );
    // The operations hold what they are given.
    let date = SetDate::new("released", "2024-03-06");
    let start = SetStartTime::new("testing", "2024-02-12");
    let end = SetEndTime::remove("testing");
    assert_eq!(
        (
            (date.element().as_str(), date.date()),
            (start.element().as_str(), start.start_time()),
            (end.element().as_str(), end.end_time()),
            SetDate::remove("x").date(),
            SetEndTime::new("x", "").end_time()
        ),
        (
            ("released", Some("2024-03-06")),
            ("testing", Some("2024-02-12")),
            ("testing", None),
            None,
            Some("")
        ),
        "{ROW}: the accessors of the operations"
    );
    assert_eq!(
        [
            Operation::from(date).operation_name(),
            Operation::from(start).operation_name(),
            Operation::from(end).operation_name()
        ],
        ["SetDate", "SetStartTime", "SetEndTime"],
        "{ROW}: operation_name()"
    );
}

// =====================================================================================
// Item 7: semantic_compare
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the exact comparison, the Display and the walk order of one row read best together"
)]
fn sacm23_art_002_semantic_compare_reports_date_start_time_and_end_time_exactly() {
    let with = |class: &str, name: &str, value: Option<&str>| -> Document {
        let attributes = value.map_or_else(String::new, |value| format!(" {name}=\"{value}\""));
        read_ok("a pair", &in_a(&asset(class, "x", &attributes, "")))
    };
    for (class, name) in HELD {
        for (left, right) in [
            (Some("2024-03-01"), Some("2024-03-01 ")),
            (None, Some("")),
            (
                Some("2024-03-01T09:30:00Z"),
                Some("2024-03-01T09:30:00+00:00"),
            ),
            (Some("spring 2024"), None),
        ] {
            let (left_doc, right_doc) = (with(class, name, left), with(class, name, right));
            let found = semantic_compare(&left_doc, &right_doc);
            let (x, l, r) = (id("x"), left.map(str::to_owned), right.map(str::to_owned));
            let expected = match name {
                "date" => Difference::Date {
                    id: x,
                    left: l,
                    right: r,
                },
                "startTime" => Difference::StartTime {
                    id: x,
                    left: l,
                    right: r,
                },
                _ => Difference::EndTime {
                    id: x,
                    left: l,
                    right: r,
                },
            };
            assert_eq!(
                found,
                [expected],
                "{ROW}: {name} {left:?} against {right:?} on an {class}: not one difference"
            );
            assert_eq!(
                found[0].element_id(),
                Some(&id("x")),
                "{ROW}: element_id() of the difference"
            );
            assert!(left_doc != right_doc, "{ROW}: the pair is ==");
        }
    }
    // Display.
    for (class, element_id, name, left, right, shown) in [
        (
            "Event",
            "released",
            "date",
            Some("2024-03-05"),
            None,
            "element \"released\" differs in date: \"2024-03-05\" on the left, no date on the right",
        ),
        (
            "Artifact",
            "report",
            "date",
            None,
            Some(""),
            "element \"report\" differs in date: no date on the left, \"\" on the right",
        ),
        (
            "Activity",
            "testing",
            "startTime",
            Some("2024-02-12"),
            Some("2024-02-13"),
            "element \"testing\" differs in startTime: \"2024-02-12\" on the left, \"2024-02-13\" \
             on the right",
        ),
        (
            "Activity",
            "testing",
            "endTime",
            None,
            Some("a\"b"),
            "element \"testing\" differs in endTime: no endTime on the left, \"a\\\"b\" on the right",
        ),
    ] {
        let doc = |value: Option<&str>| {
            let attributes = value.map_or_else(String::new, |value| {
                format!(" {name}=\"{}\"", value.replace('"', "&quot;"))
            });
            read_ok("a pair", &in_a(&asset(class, element_id, &attributes, "")))
        };
        let found = semantic_compare(&doc(left), &doc(right));
        let [difference] = found.as_slice() else {
            panic!("{ROW}: {shown}: not one difference: {found:?}");
        };
        assert_eq!(difference.to_string(), shown, "{ROW}: the Display");
    }
    // The walk order: an element's name, then `version` and `date`, then its
    // child order; an Activity's `startTime` before its `endTime`.
    let left = read_ok(
        "the left",
        &in_a(&format!(
            "{}{}",
            asset(
                "Artifact",
                "x",
                " gid=\"g1\" version=\"1\" date=\"d1\"",
                "<property xmi:id=\"p1\"><name content=\"P1\"/></property>\
                 <property xmi:id=\"p2\"><name content=\"P2\"/></property>"
            ),
            asset("Activity", "y", " startTime=\"s1\" endTime=\"e1\"", "")
        )),
    );
    let right = read_ok(
        "the right",
        &in_a(&format!(
            "{}{}",
            asset(
                "Artifact",
                "x",
                " gid=\"g2\" version=\"2\" date=\"d2\"",
                "<property xmi:id=\"p2\"><name content=\"P2\"/></property>\
                 <property xmi:id=\"p1\"><name content=\"P1\"/></property>"
            )
            .replace("<name content=\"x\"/>", "<name content=\"X\"/>"),
            asset("Activity", "y", " startTime=\"s2\" endTime=\"e2\"", "")
                .replace("<name content=\"y\"/>", "<name content=\"Y\"/>")
        )),
    );
    let found: Vec<(String, Option<String>)> = semantic_compare(&left, &right)
        .iter()
        .map(|difference| {
            (
                variant(difference),
                difference
                    .element_id()
                    .map(|found| found.as_str().to_owned()),
            )
        })
        .collect();
    let expected: Vec<(String, Option<String>)> = [
        ("Gid", "x"),
        ("Name", "x"),
        ("Version", "x"),
        ("Date", "x"),
        ("ChildOrder", "x"),
        ("Name", "y"),
        ("StartTime", "y"),
        ("EndTime", "y"),
    ]
    .iter()
    .map(|(variant, element)| ((*variant).to_owned(), Some((*element).to_owned())))
    .collect();
    assert_eq!(found, expected, "{ROW}: the walk order of the differences");
}

// =====================================================================================
// Added in step 2: the place of the location differences in the walk
// =====================================================================================

/// SACM23-ART-005 acceptance 9: a Resource's `Location`, `LocationId` and
/// `LocationValueIds` come after an Artifact's `Version` and `Date`, and
/// before an Activity's `StartTime` and `EndTime`. Only an element whose kind
/// differs can differ in both, so each pair holds one id `x` of two kinds.
#[test]
fn sacm23_art_005_location_differences_come_after_date_and_before_start_time() {
    const ART_005: &str = "SACM23-ART-005";
    let strict = |what: &str, body: &str| match read(Mode::Strict, &in_a(body)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ART_005}: setup: {what} does not read strictly: {error}"),
    };
    let location = "<location xmi:id=\"l\"><value xmi:id=\"v\" content=\"u\"/></location>";
    let cases = [
        (
            "an Artifact against a Resource",
            asset("Artifact", "x", " version=\"1\" date=\"d\"", ""),
            asset("Resource", "x", "", location),
            [
                "Kind",
                "Version",
                "Date",
                "Location",
                "LocationId",
                "LocationValueIds",
            ],
        ),
        (
            "a Resource against an Activity",
            asset("Resource", "x", "", location),
            asset("Activity", "x", " startTime=\"s\" endTime=\"e\"", ""),
            [
                "Kind",
                "Location",
                "LocationId",
                "LocationValueIds",
                "StartTime",
                "EndTime",
            ],
        ),
    ];
    for (what, left, right, expected) in cases {
        let found: Vec<String> = semantic_compare(&strict(what, &left), &strict(what, &right))
            .iter()
            .map(variant)
            .collect();
        assert_eq!(
            found, expected,
            "{ART_005}: {what}: the differences are not in the documented order"
        );
    }
}

/// SACM23-ART-002 acceptance 5: only the attribute in no namespace is the
/// value, as for `gid` (SACM23-XMI-014). A date-typed attribute in the SACM
/// namespace or another one, on a kind that holds the attribute, is
/// preserved content of the element with one SACM-XMI-003, and the value is
/// absent.
#[test]
fn sacm23_art_002_a_qualified_date_attribute_is_preserved_and_not_read() {
    for (class, attribute, name) in [
        ("Artifact", "sacm:date", "date"),
        ("Event", "o:date", "date"),
        ("Activity", "sacm:startTime", "startTime"),
        ("Activity", "o:endTime", "endTime"),
    ] {
        let what = format!("{attribute} on an {class}");
        let doc = assert_preserved_at(
            &what,
            &in_a(&asset(
                class,
                "h",
                &format!(" xmlns:o=\"urn:o\" {MARK}{attribute}=\"2024-03-01\""),
                "",
            )),
            "h",
        );
        let held = element(&doc, "h");
        let value = match name {
            "date" => held.date(),
            "startTime" => held.start_time(),
            _ => held.end_time(),
        };
        assert_eq!(value, None, "{ROW}: {what}: it is taken for the {name}");
    }
}

/// SACM23-ART-004 acceptance 2: `version` is the attribute in no namespace
/// only. On an Artifact, `xmi:version` (the XMI attribute, which a child does
/// not carry) and `sacm:version` are each preserved content of the Artifact
/// with one SACM-XMI-003, and `version()` is `None`.
#[test]
fn sacm23_art_004_a_qualified_version_attribute_is_not_the_version() {
    const ART_004: &str = "SACM23-ART-004";
    for attribute in ["xmi:version", "sacm:version"] {
        let template = in_a(&asset(
            "Artifact",
            "h",
            &format!(" {MARK}{attribute}=\"1.2\""),
            "",
        ));
        let (input, at) = one_mark(attribute, &template);
        for (mode, severity) in [
            (Mode::Strict, Severity::Error),
            (Mode::Tolerant, Severity::Warning),
        ] {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => {
                    panic!("{ART_004}: {attribute}: a {mode:?} read refused it: {error:?}")
                }
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|found| (found.code(), found.severity(), found.location()))
                .collect();
            assert_eq!(
                found,
                [(Code::XMI_UNKNOWN_CONTENT, severity, Some(at))],
                "{ART_004}: {attribute}: not one SACM-XMI-003 at it in {mode:?} mode"
            );
            let doc = output.into_document();
            let artifact = doc.get("h");
            assert_eq!(
                (
                    artifact.and_then(SacmElement::version),
                    artifact.is_some_and(SacmElement::has_preserved_content)
                ),
                (None, true),
                "{ART_004}: {attribute}: taken for the version, or not preserved, in {mode:?} \
                 mode"
            );
        }
    }
}

/// SACM23-ART-002 acceptance 4 and 5: a malformed value element is preserved
/// whole and is no occurrence of its property (ADR 0033), so a later value
/// element of the same property is read, not a repeat. One SACM-XMI-003 is
/// reported, at the malformed one.
#[test]
fn sacm23_art_002_a_preserved_value_element_is_no_occurrence_so_a_later_one_is_read() {
    for (class, name) in HELD {
        let what = format!("<{name} foo> then <{name}> on an {class}");
        let doc = assert_preserved_at(
            &what,
            &in_a(&asset(
                class,
                "h",
                "",
                &format!("{MARK}<{name} foo=\"1\">bad</{name}><{name}>2024-03-01</{name}>"),
            )),
            "h",
        );
        let held = element(&doc, "h");
        let value = match name {
            "date" => held.date(),
            "startTime" => held.start_time(),
            _ => held.end_time(),
        };
        assert_eq!(
            value,
            Some("2024-03-01"),
            "{ROW}: {what}: the later value element is not the {name}"
        );
    }
}
