//! Tests for names and exact strings in XMI: the `name` containment
//! (SACM23-XMI-012) and string values that survive XML exactly
//! (SACM23-XMI-011); formal/23-05-08 §8.3, §8.6; XML 1.0 §3.3.3; ADR 0012,
//! ADR 0013, ADR 0017 amendment 1.
//!
//! Documents are built with the `test-support` feature's `Document::insert`,
//! or read from inline input, and read with the default options. They are
//! written in tolerant mode: many are nameless, which a strict write refuses
//! (LIB-COMPAT-007), and these tests check the output form, not the save mode.
//! A read reports no diagnostic, except SACM-XMI-010 for an `xsi:type` and
//! SACM-XMI-011 for an `xmi:version` (LIB-COMPAT-001), which the tests that
//! read them expect.
//!
//! The writer's output has one tag per line (ADR 0017), so a written name is
//! checked as the exact line after its element's start tag. The full form of
//! written output, with names, is checked by `xmi_write.rs`'s `parse_output`
//! and `assert_matches_document`.
//!
//! The reader reads a name only on an `AssuranceCasePackage` or
//! `ArgumentPackage` root, so round trips are on such roots. A refusal is
//! expected at the `<` of an element, at the first character of an
//! attribute's name, and at the first character of non-white-space text, as
//! in `xmi_roots.rs`. A type naming another class is expected at the `name`
//! start tag, as a root's is at the root (ADR 0016 D2).
//!
//! An `UnrepresentableCharacter` refusal names the attribute that holds the
//! character as a `WriteAttribute` (`Id`, `NameLang` or `NameContent`), and its
//! message says where (ADR 0017 amendment 1). Both are pinned.

use std::collections::HashSet;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, InsertError, LangString,
    SacmElement,
};
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

/// Line 1 of every output.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
/// The attributes the document element carries before any `xmi:id`, as written.
const DOCUMENT_ATTRIBUTES: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// The namespace declarations of a bare root that is read: `sacm`, `xmi`
/// and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);
/// The golden file of SACM23-XMI-011 item 7.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/named.golden.sacm.xmi"
);
/// The minimized third-party-form fixture of SACM23-XMI-012 item 9.
const THIRD_PARTY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/name-third-party-form-valid.sacm.xmi"
);
/// The characters outside XML 1.0 `Char` that SACM23-XMI-011 item 5 lists.
const UNREPRESENTABLE: [char; 7] = [
    '\u{0}', '\u{8}', '\u{B}', '\u{C}', '\u{1F}', '\u{FFFE}', '\u{FFFF}',
];
/// How long one `read` may take, as in `xmi_roots.rs`: a scan that stops
/// advancing fails its test instead of stalling the run.
const DEADLINE: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Inserts one element, or fails the test as a setup error.
fn add(
    row: &str,
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
    name: Option<LangString>,
) {
    let parent_id = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent_id.as_ref(), kind, ElementId::from(id), name) {
        panic!("{row}: setup could not insert the {kind} {id:?} under {parent:?}: {error}");
    }
}

/// A `LangString` with exactly the fields given.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut name = LangString::default();
    name.set_lang(lang.map(str::to_owned));
    name.set_content(content.map(str::to_owned));
    name
}

/// A document with one root of `kind`, `id` `"r"`, named `name`.
fn named_root(row: &str, kind: ElementKind, name: LangString) -> Document {
    let mut doc = Document::new();
    add(row, &mut doc, None, kind, "r", Some(name));
    doc
}

/// The expected output for one bare root `<sacm:{kind}>` with `id` `"r"`
/// whose only content is the name line `name_line`.
fn named_root_output(kind: ElementKind, name_line: &str) -> String {
    format!(
        "{DECLARATION}\n<sacm:{kind}{DOCUMENT_ATTRIBUTES} xmi:id=\"r\">\n  {name_line}\n</sacm:{kind}>\n"
    )
}

/// A nameless ArgumentPackage root `arguments` that holds a Claim `c1` named
/// `name`, and the Claim declared `declaration`.
fn named_claim(row: &str, name: LangString, declaration: AssertionDeclaration) -> Document {
    let mut doc = Document::new();
    add(
        row,
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        None,
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c1",
        Some(name),
    );
    if let Err(error) = doc.set_assertion_declaration(&ElementId::from("c1"), declaration) {
        panic!("{row}: setup could not declare c1 {declaration}: {error}");
    }
    doc
}

/// The document of SACM23-XMI-012 items 1, 3 and 4. Each of the twelve
/// package kinds is a root. Under each root is one child of every kind its
/// roles admit, in `ElementKind::ALL` order; under each `ModelElement` child,
/// a Description; and under each `ArtifactAsset` child, a Property that holds
/// a Property that holds a Note. When `named` is set, every `ModelElement`
/// is named: `lang` `"en"` and `content` `"Name of <id>"`.
fn every_kind_document(row: &str, named: bool) -> Document {
    let name_of =
        |id: &str| named.then(|| LangString::new(format!("Name of {id}")).with_lang("en"));
    let mut doc = Document::new();
    for root in ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_package())
    {
        let root_id = format!("{root}.root");
        add(row, &mut doc, None, root, &root_id, name_of(&root_id));
        for &kind in ElementKind::ALL {
            let child = format!("{root_id}/{kind}");
            let name = if kind.is_a(AbstractClass::ModelElement) {
                name_of(&child)
            } else {
                None
            };
            let parent = ElementId::from(root_id.as_str());
            match doc.insert(Some(&parent), kind, ElementId::from(child.as_str()), name) {
                Ok(()) => {}
                Err(InsertError::ContainmentNotAllowed { .. }) => continue,
                Err(error) => panic!("{row}: setup could not insert {child:?}: {error}"),
            }
            if kind.is_a(AbstractClass::ModelElement) {
                let description = format!("{child}/Description");
                add(
                    row,
                    &mut doc,
                    Some(&child),
                    ElementKind::Description,
                    &description,
                    None,
                );
            }
            if kind.is_a(AbstractClass::ArtifactAsset) {
                let property = format!("{child}/Property");
                let inner = format!("{property}/Property");
                let note = format!("{inner}/Note");
                add(
                    row,
                    &mut doc,
                    Some(&child),
                    ElementKind::Property,
                    &property,
                    name_of(&property),
                );
                add(
                    row,
                    &mut doc,
                    Some(&property),
                    ElementKind::Property,
                    &inner,
                    name_of(&inner),
                );
                add(row, &mut doc, Some(&inner), ElementKind::Note, &note, None);
            }
        }
    }
    doc
}

/// The number of elements above `element`: 0 for a root.
fn depth(element: SacmElement<'_>) -> usize {
    let mut depth = 0;
    let mut current = element;
    while let Some(parent) = current.parent() {
        depth += 1;
        current = parent;
    }
    depth
}

/// Writes `doc` in tolerant mode: many documents here are nameless, and a
/// strict write refuses a validation Error (LIB-COMPAT-007). The output form
/// is the same in both modes.
fn write(doc: &Document) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant))
}

/// Writes `doc`, which the test expects to be written.
fn write_ok(row: &str, doc: &Document) -> String {
    match write(doc) {
        Ok(text) => text,
        Err(error) => panic!(
            "{row}: write refused a document of {} elements: {error} ({error:?})",
            doc.element_count()
        ),
    }
}

/// Writes `doc`, which the test expects to be refused, and returns why. The
/// refusal returns no output.
fn write_err(row: &str, doc: &Document) -> WriteError {
    match write(doc) {
        Ok(text) => panic!(
            "{row}: write accepted a document of {} elements and returned:\n{text}",
            doc.element_count()
        ),
        Err(error) => error,
    }
}

/// `xmi::read` with the default options, on a worker thread. Fails the test
/// when the read panics or does not return within [`DEADLINE`].
fn read(row: &str, input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{row}: read did not return within {DEADLINE:?} for {:?}",
            String::from_utf8_lossy(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!(
                "{row}: read panicked on {:?}",
                String::from_utf8_lossy(input)
            )
        }
    }
}

/// Reads `input`, which the test expects to be read without diagnostics.
fn read_ok(row: &str, input: &[u8]) -> Document {
    read_ok_reporting(row, input, &[])
}

/// Reads `input`, which the test expects to be read with exactly the read
/// diagnostics `expected`, in order: each its code, severity and one affected
/// id (LIB-COMPAT-001).
fn read_ok_reporting(row: &str, input: &[u8], expected: &[(Code, Severity, &str)]) -> Document {
    match read(row, input) {
        Ok(output) => {
            let found: Vec<(Code, Severity, Vec<&str>)> = output
                .diagnostics()
                .iter()
                .map(|found| {
                    (
                        found.code(),
                        found.severity(),
                        found.affected().iter().map(ElementId::as_str).collect(),
                    )
                })
                .collect();
            let wanted: Vec<(Code, Severity, Vec<&str>)> = expected
                .iter()
                .map(|&(code, severity, id)| (code, severity, vec![id]))
                .collect();
            assert_eq!(
                found,
                wanted,
                "{row}: reading {:?} gave other diagnostics: {:?}",
                String::from_utf8_lossy(input),
                output.diagnostics()
            );
            output.into_document()
        }
        Err(error) => panic!(
            "{row}: read refused {:?}: {error} ({error:?})",
            String::from_utf8_lossy(input)
        ),
    }
}

/// The name of the element `id` of `doc`, which must exist.
fn name_of(row: &str, doc: &Document, id: &str) -> Option<LangString> {
    let Some(element) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    element.name().cloned()
}

/// Reads `input`, a document with one root `id`, and returns that root's name.
fn read_root_name(row: &str, input: &str, id: &str) -> Option<LangString> {
    let doc = read_ok(row, input.as_bytes());
    assert_eq!(
        doc.element_count(),
        1,
        "{row}: {input:?} does not read to one element: a name is not an element"
    );
    name_of(row, &doc, id)
}

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// A bare ArgumentPackage root `a` whose content, from line 2, is `content`.
fn root_holding(content: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\">\n{content}\n</sacm:ArgumentPackage>")
}

/// Asserts that `input` is refused as `Unsupported`, with no code, at line
/// `line`, column `column`, and that no document is returned.
fn assert_unsupported_at(row: &str, what: &str, input: &str, line: u32, column: u32) {
    let error = match read(row, input.as_bytes()) {
        Ok(output) => panic!(
            "{row}: {what}: read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    };
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{row}: {what}: expected Unsupported for {input:?}, got {error:?}"
    );
    assert!(
        error.code().is_none(),
        "{row}: {what}: the Unsupported refusal carries a code"
    );
    assert_eq!(
        error.location(),
        Some(loc(row, line, column)),
        "{row}: {what}: the refusal of {input:?} is not at line {line}, column {column}: {error}"
    );
}

/// Asserts that `error` refuses the element `id` because its `attribute`
/// (its id, or its name's `lang` or `content`) holds `character`, which XML
/// 1.0 cannot represent, with the pinned message.
fn assert_unrepresentable(
    row: &str,
    what: &str,
    error: &WriteError,
    (id, attribute): (&str, WriteAttribute),
    character: char,
) {
    match error {
        WriteError::UnrepresentableCharacter {
            element,
            character: found,
            attribute: found_attribute,
            ..
        } => {
            assert_eq!(
                *found_attribute, attribute,
                "{row}: {what}: the refusal names the wrong attribute"
            );
            assert_eq!(
                element.as_str(),
                id,
                "{row}: {what}: the refusal names the wrong element"
            );
            assert_eq!(
                *found, character,
                "{row}: {what}: the refusal names the wrong character"
            );
        }
        other => panic!("{row}: {what}: refused as {other:?}, not as an unrepresentable character"),
    }
    assert_eq!(
        error.element().map(ElementId::as_str),
        Some(id),
        "{row}: {what}: element() names the wrong element"
    );
    assert!(
        error.code().is_none(),
        "{row}: {what}: an unrepresentable character has no catalog code"
    );
    // Written out here, not taken from `WriteAttribute`'s `Display`.
    let place = match attribute {
        WriteAttribute::Id => "xmi:id",
        WriteAttribute::NameLang => "name's lang",
        WriteAttribute::NameContent => "name's content",
        other => panic!("{row}: test oracle: no pinned message for {other:?}"),
    };
    assert_eq!(
        error.to_string(),
        format!(
            "the element {id:?} cannot be written: its {place} holds U+{:04X}, which XML 1.0 cannot represent",
            u32::from(character)
        ),
        "{row}: {what}: the pinned message"
    );
}

/// The lines of written output that are a `name` element, trimmed.
fn name_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|line| line.trim_start_matches(' '))
        .filter(|line| line.starts_with("<name ") || *line == "<name/>")
        .collect()
}

/// The attribute names of a written `<name … />` line, in order. Values are
/// written escaped, so they hold no `"`.
fn attribute_names(row: &str, line: &str) -> Vec<String> {
    let Some(inner) = line
        .strip_prefix("<name")
        .and_then(|rest| rest.strip_suffix(" />"))
    else {
        panic!("{row}: {line:?} is not one self-closed `<name … />` tag");
    };
    let mut names = Vec::new();
    let mut rest = inner;
    while let Some((name, after)) = rest.split_once("=\"") {
        names.push(name.trim_start_matches(' ').to_owned());
        let Some((_, after)) = after.split_once('"') else {
            panic!("{row}: {line:?} has an unterminated attribute value");
        };
        rest = after;
    }
    assert!(
        rest.is_empty(),
        "{row}: {line:?} has something that is not an attribute: {rest:?}"
    );
    names
}

// ---------------------------------------------------------------------------
// SACM23-XMI-011: string values survive XML exactly
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_011_all_nine_absent_empty_and_non_empty_lang_and_content_combinations_round_trip() {
    let row = "SACM23-XMI-011";
    let cases: [(Option<&str>, Option<&str>, &str); 9] = [
        (None, None, "<name />"),
        (None, Some(""), "<name content=\"\" />"),
        (None, Some("Case"), "<name content=\"Case\" />"),
        (Some(""), None, "<name lang=\"\" />"),
        (Some(""), Some(""), "<name lang=\"\" content=\"\" />"),
        (
            Some(""),
            Some("Case"),
            "<name lang=\"\" content=\"Case\" />",
        ),
        (Some("en"), None, "<name lang=\"en\" />"),
        (Some("en"), Some(""), "<name lang=\"en\" content=\"\" />"),
        (
            Some("en"),
            Some("Case"),
            "<name lang=\"en\" content=\"Case\" />",
        ),
    ];
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        for (lang, content, line) in cases {
            let name = lang_string(lang, content);
            let what = format!("a {kind} named lang {lang:?}, content {content:?}");
            let text = write_ok(row, &named_root(row, kind, name.clone()));
            assert_eq!(
                text,
                named_root_output(kind, line),
                "{row}: {what} is not written with {line:?}"
            );
            let back = read_ok(row, text.as_bytes());
            assert_eq!(
                name_of(row, &back, "r"),
                Some(name),
                "{row}: {what} reads back with another name\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_xmi_011_an_empty_lang_and_an_empty_name_element_read_to_exactly_those_fields() {
    let row = "SACM23-XMI-011";
    let empty_lang = read_root_name(row, &root_holding("  <name lang=\"\" />"), "a");
    assert_eq!(
        empty_lang,
        Some(lang_string(Some(""), None)),
        "{row}: <name lang=\"\" /> is not lang Some(\"\"), content None"
    );
    let empty_content = read_root_name(row, &root_holding("  <name content=\"\" />"), "a");
    assert_eq!(
        empty_content,
        Some(lang_string(None, Some(""))),
        "{row}: <name content=\"\" /> is not lang None, content Some(\"\")"
    );
    let bare = read_root_name(row, &root_holding("  <name />"), "a");
    assert_eq!(
        bare,
        Some(LangString::default()),
        "{row}: <name /> is not LangString::default()"
    );
}

#[test]
fn sacm23_xmi_011_content_with_markup_line_breaks_non_bmp_and_edge_spaces_round_trips_exactly() {
    let row = "SACM23-XMI-011";
    let content = " & < > \" ' \t \n \r \r\n \u{85} \u{2028} \u{1F600} ";
    let line = concat!(
        "<name content=\" &amp; &lt; &gt; &quot; ' &#9; &#10; &#13; &#13;&#10;",
        " \u{85} \u{2028} \u{1F600} \" />",
    );
    let name = LangString::new(content);
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        let text = write_ok(row, &named_root(row, kind, name.clone()));
        assert_eq!(
            text,
            named_root_output(kind, line),
            "{row}: the {kind}'s name is not written with the pinned escapes"
        );
        assert!(
            !text.contains(['\t', '\r']),
            "{row}: the output holds a literal TAB or CR\n{text:?}"
        );
        assert_eq!(
            text.matches('\n').count(),
            4,
            "{row}: the output holds a literal LF inside a value\n{text:?}"
        );
        let back = read_ok(row, text.as_bytes());
        assert_eq!(
            name_of(row, &back, "r"),
            Some(name.clone()),
            "{row}: the {kind}'s name does not read back exactly\n{text:?}"
        );
    }
}

#[test]
fn sacm23_xmi_011_lang_with_markup_line_breaks_non_bmp_and_edge_spaces_round_trips_exactly() {
    let row = "SACM23-XMI-011";
    let lang = " & < > \" ' \t \n \r \r\n \u{85} \u{2028} \u{1F600} ";
    let line = concat!(
        "<name lang=\" &amp; &lt; &gt; &quot; ' &#9; &#10; &#13; &#13;&#10;",
        " \u{85} \u{2028} \u{1F600} \" />",
    );
    let name = lang_string(Some(lang), None);
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        let text = write_ok(row, &named_root(row, kind, name.clone()));
        assert_eq!(
            text,
            named_root_output(kind, line),
            "{row}: the {kind}'s lang is not written with the pinned escapes"
        );
        let back = read_ok(row, text.as_bytes());
        assert_eq!(
            name_of(row, &back, "r"),
            Some(name.clone()),
            "{row}: the {kind}'s lang does not read back exactly\n{text:?}"
        );
    }
}

#[test]
fn sacm23_xmi_011_character_references_read_as_the_characters_and_literal_white_space_as_spaces() {
    let row = "SACM23-XMI-011";
    let cases = [
        ("a&#9;b&#10;c&#13;d", "a\tb\nc\rd"),
        // XML 1.0 §3.3.3: each literal white space character becomes a space.
        ("a\tb", "a b"),
        ("a\nb", "a b"),
        // §2.11 makes a literal CR, and a CRLF, one LF before §3.3.3 applies.
        ("a\rb", "a b"),
        ("a\r\nb", "a b"),
        ("a\tb\nc\rd", "a b c d"),
    ];
    for (written, expected) in cases {
        let input = root_holding(&format!("  <name content=\"{written}\"/>"));
        assert_eq!(
            read_root_name(row, &input, "a"),
            Some(LangString::new(expected)),
            "{row}: content=\"{written:?}\" does not read as {expected:?}"
        );
    }
    // `lang` is read the same way, and never trimmed.
    let lang_cases = [
        ("a&#9;b", "a\tb"),
        ("a&#9;b&#10;c&#13;d", "a\tb\nc\rd"),
        ("a\tb", "a b"),
        (" en ", " en "),
    ];
    for (written, expected) in lang_cases {
        let input = root_holding(&format!("  <name lang=\"{written}\"/>"));
        assert_eq!(
            read_root_name(row, &input, "a"),
            Some(lang_string(Some(expected), None)),
            "{row}: lang=\"{written:?}\" does not read as {expected:?}"
        );
    }
}

#[test]
fn sacm23_xmi_011_a_character_outside_xml_char_in_lang_or_content_is_refused_naming_the_element() {
    let row = "SACM23-XMI-011";
    for character in UNREPRESENTABLE {
        let value = format!("x{character}y");
        for (field, name) in [
            (
                WriteAttribute::NameLang,
                LangString::new("ok").with_lang(value.as_str()),
            ),
            (
                WriteAttribute::NameContent,
                LangString::new(value.as_str()).with_lang("en"),
            ),
        ] {
            let what = format!("U+{:04X} in a root's {field:?}", u32::from(character));
            let error = write_err(
                row,
                &named_root(row, ElementKind::ArgumentPackage, name.clone()),
            );
            assert_unrepresentable(row, &what, &error, ("r", field), character);

            let what = format!(
                "U+{:04X} in a nested Claim's {field:?}",
                u32::from(character)
            );
            let error = write_err(row, &named_claim(row, name, AssertionDeclaration::Asserted));
            assert_unrepresentable(row, &what, &error, ("c1", field), character);
        }
    }
    // Two such characters in one value: the first is named.
    let error = write_err(
        row,
        &named_root(
            row,
            ElementKind::AssuranceCasePackage,
            LangString::new("a\u{1}b\u{FFFE}"),
        ),
    );
    assert_unrepresentable(
        row,
        "two characters in a content",
        &error,
        ("r", WriteAttribute::NameContent),
        '\u{1}',
    );
}

#[test]
fn sacm23_xmi_011_within_one_element_the_id_is_checked_before_the_name_and_lang_before_content() {
    let row = "SACM23-XMI-011";
    // The id, then the name.
    let mut doc = Document::new();
    add(
        row,
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        None,
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c\u{1}",
        Some(LangString::new("\u{3}").with_lang("\u{2}")),
    );
    let error = write_err(row, &doc);
    assert_unrepresentable(
        row,
        "an id and a name",
        &error,
        ("c\u{1}", WriteAttribute::Id),
        '\u{1}',
    );

    // `lang`, then `content`.
    let both = LangString::new("\u{3}").with_lang("\u{2}");
    let error = write_err(row, &named_claim(row, both, AssertionDeclaration::Asserted));
    assert_unrepresentable(
        row,
        "a lang and a content",
        &error,
        ("c1", WriteAttribute::NameLang),
        '\u{2}',
    );

    // The name, then the `assertionDeclaration` (ADR 0017 amendment 1).
    let content = LangString::new("\u{3}").with_lang("en");
    let error = write_err(
        row,
        &named_claim(row, content, AssertionDeclaration::Defeated),
    );
    assert_unrepresentable(
        row,
        "a content and a declaration",
        &error,
        ("c1", WriteAttribute::NameContent),
        '\u{3}',
    );

    // Across elements, the first in document order: the root's name before
    // the Claim's id.
    let mut doc = Document::new();
    add(
        row,
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        Some(LangString::new("\u{4}")),
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c\u{1}",
        None,
    );
    let error = write_err(row, &doc);
    assert_unrepresentable(
        row,
        "a root's name and a Claim's id",
        &error,
        ("arguments", WriteAttribute::NameContent),
        '\u{4}',
    );
}

/// The document of `named.golden.sacm.xmi`: a named AssuranceCasePackage
/// holding a named ArgumentPackage that holds a Claim whose `lang` is empty.
fn golden_document(row: &str) -> Document {
    let mut doc = Document::new();
    add(
        row,
        &mut doc,
        None,
        ElementKind::AssuranceCasePackage,
        "case",
        Some(LangString::new("Case").with_lang("en")),
    );
    add(
        row,
        &mut doc,
        Some("case"),
        ElementKind::ArgumentPackage,
        "arguments",
        Some(LangString::new("Arguments").with_lang("en")),
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "claim_1",
        Some(LangString::new("Top claim").with_lang("")),
    );
    doc
}

#[test]
fn sacm23_xmi_011_a_named_document_writes_identical_bytes_twice_and_matches_the_golden_file() {
    let row = "SACM23-XMI-011";
    let doc = golden_document(row);
    let first = write_ok(row, &doc);
    assert_eq!(
        write_ok(row, &doc),
        first,
        "{row}: a second write gives other bytes"
    );
    assert_eq!(
        write_ok(row, &golden_document(row)),
        first,
        "{row}: the same inserts give other bytes"
    );
    let golden = std::fs::read(GOLDEN)
        .unwrap_or_else(|error| panic!("{row}: cannot read {GOLDEN}: {error}"));
    assert_eq!(
        first.as_bytes(),
        golden.as_slice(),
        "{row}: the output differs from named.golden.sacm.xmi\n--- got ---\n{first}"
    );
}

// ---------------------------------------------------------------------------
// SACM23-XMI-012: the name containment in XMI — writing
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_012_every_model_element_kind_at_every_depth_is_written_with_its_name_first() {
    let row = "SACM23-XMI-012";
    let doc = every_kind_document(row, true);
    let text = write_ok(row, &doc);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut named_kinds = HashSet::new();
    let mut named_depths = HashSet::new();
    let mut named = 0;
    for element in doc.elements() {
        let id = element.id().as_str();
        let open = format!(" xmi:id=\"{id}\">");
        let empty = format!(" xmi:id=\"{id}\" />");
        let at: Vec<usize> = (0..lines.len())
            .filter(|&index| lines[index].ends_with(&open) || lines[index].ends_with(&empty))
            .collect();
        assert_eq!(
            at.len(),
            1,
            "{row}: {id:?} has {} start tags in the output",
            at.len()
        );
        let line = lines[at[0]];
        let next = lines.get(at[0] + 1).copied().unwrap_or("");
        let indent = line.len() - line.trim_start_matches(' ').len();
        match element.name() {
            Some(_) => {
                assert!(
                    line.ends_with(&open),
                    "{row}: the named {id:?} is self-closed: {line:?}"
                );
                let expected = format!(
                    "{}<name lang=\"en\" content=\"Name of {id}\" />",
                    " ".repeat(indent + 2)
                );
                assert_eq!(
                    next, expected,
                    "{row}: the first child of {id:?} is not its name"
                );
                named += 1;
                named_kinds.insert(element.kind());
                named_depths.insert(depth(element));
            }
            None => assert!(
                !next.trim_start_matches(' ').starts_with("<name"),
                "{row}: the nameless {id:?} is written with a name: {next:?}"
            ),
        }
    }
    assert_eq!(
        name_lines(&text).len(),
        named,
        "{row}: the output has `name` elements that no named element owns"
    );
    let model_kinds: HashSet<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ModelElement))
        .collect();
    assert_eq!(
        named_kinds, model_kinds,
        "{row}: setup: not every ModelElement kind is named"
    );
    assert_eq!(
        named_depths,
        HashSet::from([0, 1, 2, 3]),
        "{row}: setup: named elements are not at depths 0 to 3"
    );
}

#[test]
fn sacm23_xmi_012_an_element_holding_only_its_name_has_a_start_tag_a_name_line_and_an_end_tag() {
    let row = "SACM23-XMI-012";
    let mut doc = Document::new();
    add(
        row,
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        Some(LangString::new("Arguments").with_lang("en")),
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c1",
        Some(LangString::new("Named")),
    );
    add(
        row,
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "c2",
        None,
    );
    let expected = format!(
        "{DECLARATION}\n<sacm:ArgumentPackage{DOCUMENT_ATTRIBUTES} xmi:id=\"arguments\">\n\
         \x20 <name lang=\"en\" content=\"Arguments\" />\n\
         \x20 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n\
         \x20   <name content=\"Named\" />\n\
         \x20 </argumentElement>\n\
         \x20 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" />\n\
         </sacm:ArgumentPackage>\n"
    );
    assert_eq!(
        write_ok(row, &doc),
        expected,
        "{row}: a name-only element is not a start tag, a name line and an end tag"
    );
    // A root that holds only its name.
    let root = named_root(
        row,
        ElementKind::AssuranceCasePackage,
        LangString::new("Case"),
    );
    assert_eq!(
        write_ok(row, &root),
        named_root_output(
            ElementKind::AssuranceCasePackage,
            "<name content=\"Case\" />"
        ),
        "{row}: a root that holds only its name"
    );
}

#[test]
fn sacm23_xmi_012_an_element_without_a_name_and_every_utility_element_have_no_name_child() {
    let row = "SACM23-XMI-012";
    let nameless = write_ok(row, &every_kind_document(row, false));
    assert_eq!(
        name_lines(&nameless),
        Vec::<&str>::new(),
        "{row}: a nameless document is written with `name` elements"
    );
    // In the named document, every UtilityElement is still childless and
    // self-closed, so it has no `name` child.
    let doc = every_kind_document(row, true);
    let text = write_ok(row, &doc);
    let mut utilities = 0;
    for element in doc.elements() {
        if element.kind().is_a(AbstractClass::ModelElement) {
            continue;
        }
        utilities += 1;
        let empty = format!(" xmi:id=\"{}\" />", element.id().as_str());
        assert!(
            text.split('\n').any(|line| line.ends_with(&empty)),
            "{row}: the {} {:?} is not written self-closed, with no name",
            element.kind(),
            element.id().as_str()
        );
    }
    assert!(
        utilities > 20,
        "{row}: setup: only {utilities} UtilityElements"
    );
}

#[test]
fn sacm23_xmi_012_no_xmi_id_or_type_attribute_is_ever_written_on_name() {
    let row = "SACM23-XMI-012";
    let mut outputs = vec![
        write_ok(row, &every_kind_document(row, true)),
        write_ok(row, &golden_document(row)),
    ];
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        outputs.push(write_ok(row, &named_root(row, kind, LangString::default())));
        outputs.push(write_ok(
            row,
            &named_root(row, kind, LangString::new("x").with_lang("")),
        ));
    }
    let mut checked = 0;
    for text in &outputs {
        for line in name_lines(text) {
            let names = attribute_names(row, line);
            let allowed = [
                Vec::<String>::new(),
                vec!["lang".to_owned()],
                vec!["content".to_owned()],
                vec!["lang".to_owned(), "content".to_owned()],
            ];
            assert!(
                allowed.contains(&names),
                "{row}: a `name` is written with the attributes {names:?}: {line:?}"
            );
            checked += 1;
        }
    }
    assert!(checked > 50, "{row}: setup: only {checked} `name` lines");
}

// ---------------------------------------------------------------------------
// SACM23-XMI-012: the name containment in XMI — reading
// ---------------------------------------------------------------------------

#[test]
fn sacm23_xmi_012_a_bare_or_wrapped_package_root_reads_its_name_child() {
    let row = "SACM23-XMI-012";
    let case = LangString::new("Case").with_lang("en");
    for kind in ["AssuranceCasePackage", "ArgumentPackage"] {
        let named = format!(
            "<sacm:{kind}{NS} xmi:id=\"p\">\n  <name lang=\"en\" content=\"Case\"/>\n</sacm:{kind}>"
        );
        assert_eq!(
            read_root_name(row, &named, "p"),
            Some(case.clone()),
            "{row}: a bare {kind}'s name"
        );
        let nameless = format!("<sacm:{kind}{NS} xmi:id=\"p\"/>");
        assert_eq!(
            read_root_name(row, &nameless, "p"),
            None,
            "{row}: a bare {kind} without `name` reads to a name"
        );
        let empty = format!("<sacm:{kind}{NS} xmi:id=\"p\"><name/></sacm:{kind}>");
        assert_eq!(
            read_root_name(row, &empty, "p"),
            Some(LangString::default()),
            "{row}: a bare {kind}'s <name/> is not LangString::default()"
        );
    }
    let wrapped = concat!(
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
        " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
        "  <sacm:AssuranceCasePackage xmi:id=\"c\"><name lang=\"en\" content=\"Case\"/></sacm:AssuranceCasePackage>\n",
        "  <sacm:ArgumentPackage xmi:id=\"a\">\n    <name/>\n  </sacm:ArgumentPackage>\n",
        "  <sacm:ArgumentPackage xmi:id=\"b\"/>\n",
        "</xmi:XMI>\n",
    );
    let doc = read_ok(row, wrapped.as_bytes());
    assert_eq!(
        doc.element_count(),
        3,
        "{row}: the wrapped roots do not read to three elements"
    );
    assert_eq!(
        name_of(row, &doc, "c"),
        Some(case),
        "{row}: the wrapped AssuranceCasePackage's name"
    );
    assert_eq!(
        name_of(row, &doc, "a"),
        Some(LangString::default()),
        "{row}: the wrapped ArgumentPackage's <name/>"
    );
    assert_eq!(
        name_of(row, &doc, "b"),
        None,
        "{row}: the wrapped ArgumentPackage without `name`"
    );
}

#[test]
fn sacm23_xmi_012_white_space_comments_and_processing_instructions_in_name_are_accepted() {
    let row = "SACM23-XMI-012";
    for inside in [
        " ",
        "\n    \t\r\n  ",
        "<!-- a comment -->",
        "<?pi data?>",
        "\n    <!-- c --> <?pi?>\n  ",
    ] {
        let input = root_holding(&format!(
            "  <name lang=\"en\" content=\"A\">{inside}</name>"
        ));
        assert_eq!(
            read_root_name(row, &input, "a"),
            Some(LangString::new("A").with_lang("en")),
            "{row}: a name holding {inside:?} is not read"
        );
    }
}

#[test]
fn sacm23_xmi_012_a_second_name_is_unsupported_at_the_second_name() {
    let row = "SACM23-XMI-012";
    for (first, second) in [
        ("<name content=\"A\"/>", "<name content=\"B\"/>"),
        ("<name/>", "<name/>"),
    ] {
        let input = root_holding(&format!("  {first}\n  {second}"));
        assert_unsupported_at(row, "a second name", &input, 3, 3);
    }
}

#[test]
fn sacm23_xmi_012_an_attribute_other_than_lang_and_content_on_name_is_unsupported_at_it() {
    let row = "SACM23-XMI-012";
    for attribute in [
        "xml:lang=\"en\"",
        "xmi:id=\"n1\"",
        "xmi:uuid=\"u1\"",
        "foo=\"1\"",
        "sacm:lang=\"en\"",
    ] {
        // The attribute starts line 3, after four spaces.
        let input = root_holding(&format!("  <name content=\"A\"\n    {attribute}/>"));
        assert_unsupported_at(row, attribute, &input, 3, 5);
    }
}

#[test]
fn sacm23_xmi_012_a_type_naming_another_class_on_name_is_unsupported_at_the_name() {
    let row = "SACM23-XMI-012";
    for attribute in [
        "xmi:type=\"sacm:ExpressionLangString\"",
        "xsi:type=\"sacm:ExpressionLangString\"",
        "xmi:type=\"sacm:MultiLangString\"",
    ] {
        let input = root_holding(&format!("  <name content=\"A\"\n    {attribute}/>"));
        assert_unsupported_at(row, attribute, &input, 2, 3);
    }
}

#[test]
fn sacm23_xmi_012_an_element_child_or_text_in_name_is_unsupported_at_that_content() {
    let row = "SACM23-XMI-012";
    // `  <name>` is 8 characters, so what it holds starts at column 9.
    for (what, inside) in [
        ("a content element", "<content>x</content>"),
        ("a value element", "<value content=\"x\"/>"),
        ("text", "text"),
    ] {
        let input = root_holding(&format!("  <name>{inside}</name>"));
        assert_unsupported_at(row, what, &input, 2, 9);
    }
    // Text after a comment, at its first character.
    let after_comment = root_holding("  <name><!--c-->x</name>");
    assert_unsupported_at(row, "text after a comment", &after_comment, 2, 17);
}

#[test]
fn sacm23_xmi_012_a_name_element_in_a_namespace_is_unsupported_at_the_element() {
    let row = "SACM23-XMI-012";
    for element in [
        "<sacm:name content=\"A\"/>",
        "<name xmlns=\"http://www.omg.org/spec/SACM/20220301\" content=\"A\"/>",
    ] {
        let input = root_holding(&format!("  {element}"));
        assert_unsupported_at(row, element, &input, 2, 3);
    }
}

#[test]
fn sacm23_xmi_012_a_type_naming_lang_string_on_name_is_accepted() {
    let row = "SACM23-XMI-012";
    let expected = Some(LangString::new("A").with_lang("en"));
    // An `xsi:type` is read, and reported as SACM-XMI-010 on the owner, a
    // Warning in the default tolerant mode (LIB-COMPAT-001).
    let xsi_type: &[(Code, Severity, &str)] = &[(Code::XMI_XSI_TYPE, Severity::Warning, "a")];
    for (attribute, reported) in [
        ("xmi:type=\"sacm:LangString\"", &[][..]),
        ("xsi:type=\"sacm:LangString\"", xsi_type),
        ("xmi:type=\" sacm:LangString \"", &[][..]),
        ("xmi:type=\"s:LangString\"", &[][..]),
    ] {
        let input = format!(
            "<sacm:ArgumentPackage{NS} xmlns:s=\"http://www.omg.org/spec/SACM/20220301\" \
             xmi:id=\"a\">\n  <name {attribute} lang=\"en\" content=\"A\"/>\n</sacm:ArgumentPackage>"
        );
        let doc = read_ok_reporting(row, input.as_bytes(), reported);
        assert_eq!(
            name_of(row, &doc, "a"),
            expected,
            "{row}: a name with {attribute} is not read"
        );
    }
}

#[test]
fn sacm23_xmi_012_the_type_on_name_is_resolved_in_the_scope_of_the_name_element() {
    let row = "SACM23-XMI-012";
    // A prefix declared on `name` itself, unknown on the root: accepted.
    let declared_on_name = root_holding(
        "  <name xmlns:t=\"http://www.omg.org/spec/SACM/20220301\" xmi:type=\"t:LangString\" \
         lang=\"en\" content=\"A\"/>",
    );
    assert_eq!(
        read_root_name(row, &declared_on_name, "a"),
        Some(LangString::new("A").with_lang("en")),
        "{row}: a type whose prefix is declared on `name` is not resolved there"
    );
    // `sacm` rebound on `name`: the type names no SACM class there, though it
    // would on the root.
    let rebound_on_name = root_holding(
        "  <name xmlns:sacm=\"urn:other\" xmi:type=\"sacm:LangString\" content=\"A\"/>",
    );
    assert_unsupported_at(row, "sacm rebound on name", &rebound_on_name, 2, 3);
}

#[test]
fn sacm23_xmi_012_a_name_undeclaring_the_default_namespace_reads_as_the_plain_form() {
    let row = "SACM23-XMI-012";
    let plain = read_root_name(
        row,
        &root_holding("  <name lang=\"en\" content=\"A\"/>"),
        "a",
    );
    assert_eq!(
        plain,
        Some(LangString::new("A").with_lang("en")),
        "{row}: setup: the plain form"
    );
    let under_default = concat!(
        "<ArgumentPackage xmlns=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\">",
        "<name xmlns=\"\" lang=\"en\" content=\"A\"/></ArgumentPackage>",
    );
    assert_eq!(
        read_root_name(row, under_default, "a"),
        plain,
        "{row}: `name xmlns=\"\"` under a default SACM namespace is not the plain form"
    );
    let under_prefixed = root_holding("  <name xmlns=\"\" lang=\"en\" content=\"A\"/>");
    assert_eq!(
        read_root_name(row, &under_prefixed, "a"),
        plain,
        "{row}: `name xmlns=\"\"` under a sacm:-prefixed root is not the plain form"
    );
}

#[test]
fn sacm23_xmi_012_a_minimized_third_party_name_reads_and_round_trips() {
    let row = "SACM23-XMI-012";
    let input = std::fs::read(THIRD_PARTY)
        .unwrap_or_else(|error| panic!("{row}: cannot read {THIRD_PARTY}: {error}"));
    let id = "_hX3kQK1sEe6xAbCdEfGhIj";
    // The fixture's `xmi:version="2.0"` is reported as SACM-XMI-011, Info
    // (LIB-COMPAT-001).
    let doc = read_ok_reporting(
        row,
        &input,
        &[(Code::XMI_VERSION_IGNORED, Severity::Info, id)],
    );
    let expected = LangString::new("Argument for the braking function").with_lang("English");
    assert_eq!(doc.element_count(), 1, "{row}: the fixture's elements");
    assert_eq!(
        doc.roots().map(SacmElement::kind).collect::<Vec<_>>(),
        [ElementKind::ArgumentPackage],
        "{row}: the fixture's root"
    );
    assert_eq!(
        name_of(row, &doc, id),
        Some(expected.clone()),
        "{row}: the fixture's name"
    );
    let text = write_ok(row, &doc);
    assert!(
        text.contains(
            "\n  <name lang=\"English\" content=\"Argument for the braking function\" />\n"
        ),
        "{row}: the fixture's name is not written in the attribute form\n{text}"
    );
    let back = read_ok(row, text.as_bytes());
    assert_eq!(
        name_of(row, &back, id),
        Some(expected),
        "{row}: the fixture's name does not survive a write and a read\n{text}"
    );
    assert_eq!(
        write_ok(row, &back),
        text,
        "{row}: writing what was read from the output gives other bytes"
    );
}
