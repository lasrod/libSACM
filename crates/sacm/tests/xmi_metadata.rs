//! Tests for XMI metadata (SACM23-XMI-019; XMI 2.5.1 formal/15-06-07 §7.3.3
//! "Tools that rely on XMI are expected to store the extension information
//! and export it again to enable round trip engineering", §7.5.3 to §7.5.6,
//! §7.6.1 "The value of the label attribute is ignored on import.", §7.8.3,
//! §7.13 "Any other modifiers of the document must preserve the original
//! UUID", productions 2a, 2c and 2d; XMI.xsd; D-STRICT; ADR 0042).
//!
//! XMI metadata is read in both modes with no diagnostic, held exactly as
//! written, and written back by both saves where it stood:
//!
//! - on `xmi:XMI`: `xmi:id`, `xmi:label`, `xmi:uuid`, `href`, `xmi:idref`,
//!   `xmi:type`, `xsi:schemaLocation` and `xsi:noNamespaceSchemaLocation`;
//! - under `xmi:XMI`: `documentation`, `Documentation`, `difference`,
//!   `Difference`, `Add`, `Replace`, `Delete`, `extension` and `Extension`;
//! - on a root, a child, a `name`, a `content`, `key` or `location`, or a
//!   `value`: `xmi:uuid`, `xmi:label` and the two XSI schema locations;
//! - in any of these: `xmi:extension` and `xmi:Extension`, kept whole.
//!
//! It is never preserved content. `SacmElement::uuid` and `label` give an
//! element's own values. Any other XMI-namespace element, `href` or
//! `xmi:idref` on a definition, and identity attributes on a reference
//! element are SACM-XMI-003; an XMI-namespace document element other than
//! `xmi:XMI` is `InvalidRoot`. Metadata ids share the document's id space; a
//! strict save checks them as `NCName`s (SACM-ID-002) and the XMI.xsd IDREF
//! and IDREFS attributes of metadata (SACM-REF-001). A copy drops the
//! `xmi:uuid` of what it copies, a delete removes the metadata, a move keeps
//! it, and `semantic_compare` reports `Difference::Metadata` and
//! `Difference::DocumentMetadata`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity};
use sacm::edit::{
    self, AddReference, CopyElements, CreateArgumentPackage, CreateClaim, DeleteElement, EditError,
    MoveElement, SetContent, SetName,
};
use sacm::model::{Document, LangString, MultiLangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-019";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The declaration of `xsi`.
const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

/// Marks, in a test input, where a diagnostic or a refusal is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// Golden M1 of the design brief: its input and its output in both modes.
const M1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/xmi-metadata-valid.sacm.xmi"
);
const M1_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/xmi-metadata.golden.sacm.xmi"
);

/// Golden M2 of the design brief: `xsi:schemaLocation` on a bare root.
const M2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/xmi-schema-location-valid.sacm.xmi"
);
const M2_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/xmi-schema-location.golden.sacm.xmi"
);

// --- Inputs ----------------------------------------------------------------------

fn file(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// The root ArgumentPackage `p`, holding `body` after its name.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// An `xmi:XMI` with the extra attributes `attributes`, holding `before`,
/// the root ArgumentPackage `p` and `after`.
fn wrapper(attributes: &str, before: &str, after: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>{before}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>{after}</xmi:XMI>"
    )
}

/// `template` without its marks, and the line and column of each mark in
/// order: lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<(u32, u32)>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push((line, column));
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

/// The one mark of `template`.
fn one_mark(template: &str) -> (String, (u32, u32)) {
    let (text, marks) = marked(template);
    match marks.as_slice() {
        [mark] => (text, *mark),
        _ => panic!(
            "{ROW}: setup: {template:?} has {} marks, not one",
            marks.len()
        ),
    }
}

// --- Reading and writing ---------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which must return a document.
fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
    }
}

/// Reads `input` in `mode`, which must return a document with no
/// diagnostic.
fn read_clean(what: &str, mode: Mode, input: &str) -> Document {
    let output = read_ok(what, mode, input);
    assert_eq!(
        output.diagnostics(),
        [],
        "{ROW}: {what}: a {mode:?} read gives diagnostics"
    );
    output.into_document()
}

/// The refusal of `input` read in `mode`.
fn refusal(what: &str, mode: Mode, input: &str) -> ReadError {
    match read(mode, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a {mode:?} read returned a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

fn write(doc: &Document, mode: Mode) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(mode))
}

/// `doc` written in `mode`, which must succeed.
fn written(what: &str, doc: &Document, mode: Mode) -> String {
    match write(doc, mode) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused it: {error:?} ({error})"),
    }
}

/// The severity of SACM-XMI-003 in `mode`: an Error strict, a Warning
/// tolerant.
fn kept_severity(mode: Mode) -> Severity {
    if mode == Mode::Strict {
        Severity::Error
    } else {
        Severity::Warning
    }
}

/// The element `id` of `doc`.
fn element<'d>(what: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: {what}: the document has no element {id:?}"))
}

/// The arguments of a diagnostic, by name, each as its text or count.
fn arguments(found: &Diagnostic) -> Vec<(&'static str, String)> {
    found
        .arguments()
        .iter()
        .map(|argument| {
            let value = match argument.value() {
                ArgumentValue::Text(text) => text.clone(),
                ArgumentValue::Count(count) => count.to_string(),
                other => format!("{other:?}"),
            };
            (argument.name(), value)
        })
        .collect()
}

/// What a diagnostic is: its code, severity, template, location and
/// affected ids.
type Summary = (
    &'static str,
    Severity,
    &'static str,
    Option<(u32, u32)>,
    Vec<String>,
);

fn summary(found: &Diagnostic) -> Summary {
    (
        found.code().as_str(),
        found.severity(),
        found.template(),
        found.location().map(|at| (at.line(), at.column())),
        found
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    )
}

/// The trimmed lines of `text`.
fn trimmed(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// The `count` trimmed lines of `text` from the first that equals `first`.
fn block<'t>(what: &str, text: &'t str, first: &str, count: usize) -> Vec<&'t str> {
    let lines = trimmed(text);
    let Some(at) = lines.iter().position(|line| *line == first) else {
        panic!("{ROW}: {what}: the output has no line {first:?}:\n{text}");
    };
    lines[at..lines.len().min(at + count)].to_vec()
}

/// The index of the first trimmed line of `text` that `matches`.
fn line_index(what: &str, text: &str, matches: impl Fn(&str) -> bool, about: &str) -> usize {
    trimmed(text)
        .iter()
        .position(|line| matches(line))
        .unwrap_or_else(|| panic!("{ROW}: {what}: the output has no line {about}:\n{text}"))
}

/// Applies `operation` to `doc`, which must succeed.
fn apply(what: &str, doc: &mut Document, operation: impl Into<edit::Operation>) -> edit::Mutation {
    match edit::apply(doc, operation) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {what}: the edit was refused: {error:?} ({error})"),
    }
}

// --- 1. Read in both modes ---------------------------------------------------------

#[test]
fn sacm23_xmi_019_metadata_reads_in_both_modes_with_no_diagnostic() {
    for (what, path) in [("Golden M1", M1), ("Golden M2", M2)] {
        let input = file(path);
        for mode in MODES {
            let doc = read_clean(what, mode, &input);
            assert!(
                !doc.has_preserved_content(),
                "{ROW}: {what}: a {mode:?} read holds the metadata as preserved content"
            );
            for found in doc.elements() {
                assert!(
                    !found.has_preserved_content(),
                    "{ROW}: {what}: in a {mode:?} read the element {:?} holds preserved content",
                    found.id().as_str()
                );
            }
        }
    }
}

// --- 2. The accessors --------------------------------------------------------------

#[test]
fn sacm23_xmi_019_uuid_and_label_are_read_only_accessors() {
    let doc = read_clean("Golden M1", Mode::Strict, &file(M1));
    {
        let p = element("Golden M1", &doc, "p");
        assert_eq!(
            (p.uuid(), p.label()),
            (Some("urn:uuid:p"), Some("Arguments")),
            "{ROW}: Golden M1: p's uuid and label"
        );
        let c1 = element("Golden M1", &doc, "c1");
        assert_eq!(
            (c1.uuid(), c1.label()),
            (Some("urn:uuid:c1"), None),
            "{ROW}: Golden M1: c1's uuid and label"
        );
        // The metadata of the wrapper is the document's, not an element's.
        assert!(
            doc.get("doc").is_none() && doc.get("x1").is_none() && doc.get("d1").is_none(),
            "{ROW}: Golden M1: a metadata id names an element"
        );
        let mut built = Document::new();
        apply(
            "a built package",
            &mut built,
            CreateArgumentPackage::root(LangString::new("B")).with_id("b"),
        );
        apply(
            "a built Claim",
            &mut built,
            CreateClaim::new("b", LangString::new("C")).with_id("bc"),
        );
        for id in ["b", "bc"] {
            let found = element("a built document", &built, id);
            assert_eq!(
                (found.uuid(), found.label()),
                (None, None),
                "{ROW}: the built element {id:?} has a uuid or a label"
            );
        }
    }
}

// --- 3. Both saves write it where it stood -----------------------------------------

#[test]
fn sacm23_xmi_019_both_saves_write_metadata_where_it_stood() {
    for (what, path, golden_path) in [("Golden M1", M1, M1_GOLDEN), ("Golden M2", M2, M2_GOLDEN)] {
        let input = file(path);
        let golden = file(golden_path);
        for read_mode in MODES {
            let doc = read_clean(what, read_mode, &input);
            for write_mode in MODES {
                assert_eq!(
                    written(what, &doc, write_mode),
                    golden,
                    "{ROW}: {what}: read {read_mode:?}, a {write_mode:?} write is not the golden \
                     output"
                );
            }
            let back = read_clean("the golden output", Mode::Strict, &golden);
            assert_eq!(
                semantic_compare(&doc, &back),
                [],
                "{ROW}: {what}: the output read back differs"
            );
            assert!(back == doc, "{ROW}: {what}: the output read back is not ==");
            assert_eq!(
                written(what, &back, Mode::Strict),
                golden,
                "{ROW}: {what}: a second write is not byte-stable"
            );
        }
    }
    // At one place among the children, metadata comes before preserved
    // content. `<foo/>` is preserved: SACM-XMI-003 tolerant.
    let what = "<xmi:Extension/><foo/> before c1";
    let doc = read_ok(
        what,
        Mode::Tolerant,
        &package(&format!("<xmi:Extension/><foo/>{}", claim("c1", "", ""))),
    )
    .into_document();
    let text = written(what, &doc, Mode::Tolerant);
    let extension = line_index(
        what,
        &text,
        |line| line == "<xmi:Extension />",
        "<xmi:Extension />",
    );
    let foo = line_index(what, &text, |line| line == "<foo />", "<foo />");
    let c1 = line_index(what, &text, |line| line.contains("xmi:id=\"c1\""), "of c1");
    assert!(
        extension < foo && foo < c1,
        "{ROW}: {what}: not written in the order xmi:Extension, foo, c1:\n{text}"
    );
    let what = "<foo/><xmi:Extension/> before c1";
    let doc = read_ok(
        what,
        Mode::Tolerant,
        &package(&format!("<foo/><xmi:Extension/>{}", claim("c1", "", ""))),
    )
    .into_document();
    let first = written(what, &doc, Mode::Tolerant);
    let extension = line_index(
        what,
        &first,
        |line| line == "<xmi:Extension />",
        "<xmi:Extension />",
    );
    let foo = line_index(what, &first, |line| line == "<foo />", "<foo />");
    let c1 = line_index(what, &first, |line| line.contains("xmi:id=\"c1\""), "of c1");
    assert!(
        extension < foo && foo < c1,
        "{ROW}: {what}: the metadata is not written first at its place:\n{first}"
    );
    let again = read_ok("the first write", Mode::Tolerant, &first).into_document();
    assert_eq!(
        written(what, &again, Mode::Tolerant),
        first,
        "{ROW}: {what}: the second write is not byte-stable"
    );
}

// --- 4. Parts keep their metadata --------------------------------------------------

/// A case holding the ArgumentPackage `p`, whose name carries metadata and
/// an extension, with a Claim `c1` holding a Note `n1` (its `content`, `n1c`,
/// with metadata and an extension, and three values: the second with a
/// label and an extension, the third with a uuid) and a `TaggedValue` `t1`
/// (its `key` with a uuid); and the ArtifactPackage `arts` with a Resource
/// `r1` whose `location` has a label.
fn parts_input() -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <argumentPackage xmi:id=\"p\">\
         <name content=\"p\" xmi:uuid=\"urn:uuid:p-name\" xmi:label=\"P\"><xmi:Extension extender=\"T\"/></name>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
         <note xmi:id=\"n1\"><content xmi:id=\"n1c\" xmi:uuid=\"urn:uuid:n1-content\" xmi:label=\"N\">\
         <xmi:Extension extender=\"T\"/>\
         <value content=\"a\"/>\
         <value content=\"b\" xmi:label=\"second\"><xmi:Extension extender=\"T\"/></value>\
         <value content=\"c\" xmi:uuid=\"urn:uuid:v3\"/>\
         </content></note>\
         <taggedValue xmi:id=\"t1\"><key xmi:uuid=\"urn:uuid:t1-key\"><value content=\"K\"/></key></taggedValue>\
         </argumentElement></argumentPackage>\
         <artifactPackage xmi:id=\"arts\"><name content=\"arts\"/>\
         <artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"r1\"><name content=\"r1\"/>\
         <location xmi:label=\"L\"><value content=\"https://example.org/r\"/></location>\
         </artifactElement></artifactPackage>\
         </sacm:AssuranceCasePackage>"
    )
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_019_parts_keep_their_metadata() {
    let input = parts_input();
    for mode in MODES {
        let what = format!("the parts input, read {mode:?}");
        let doc = read_clean(&what, mode, &input);
        assert!(
            !doc.elements().any(SacmElement::has_preserved_content),
            "{ROW}: {what}: part metadata is held as preserved content"
        );
        let text = written(&what, &doc, Mode::Strict);
        assert_eq!(
            block(
                &what,
                &text,
                "<name content=\"p\" xmi:uuid=\"urn:uuid:p-name\" xmi:label=\"P\">",
                3
            ),
            [
                "<name content=\"p\" xmi:uuid=\"urn:uuid:p-name\" xmi:label=\"P\">",
                "<xmi:Extension extender=\"T\" />",
                "</name>",
            ],
            "{ROW}: {what}: the name's metadata is not written in place\n{text}"
        );
        assert_eq!(
            block(
                &what,
                &text,
                "<content xmi:id=\"n1c\" xmi:uuid=\"urn:uuid:n1-content\" xmi:label=\"N\">",
                8
            ),
            [
                "<content xmi:id=\"n1c\" xmi:uuid=\"urn:uuid:n1-content\" xmi:label=\"N\">",
                "<xmi:Extension extender=\"T\" />",
                "<value content=\"a\" />",
                "<value content=\"b\" xmi:label=\"second\">",
                "<xmi:Extension extender=\"T\" />",
                "</value>",
                "<value content=\"c\" xmi:uuid=\"urn:uuid:v3\" />",
                "</content>",
            ],
            "{ROW}: {what}: the content's and the values' metadata is not written in place\n{text}"
        );
        assert_eq!(
            block(&what, &text, "<key xmi:uuid=\"urn:uuid:t1-key\">", 3),
            [
                "<key xmi:uuid=\"urn:uuid:t1-key\">",
                "<value content=\"K\" />",
                "</key>",
            ],
            "{ROW}: {what}: the key's metadata is not written in place\n{text}"
        );
        assert_eq!(
            block(&what, &text, "<location xmi:label=\"L\">", 3),
            [
                "<location xmi:label=\"L\">",
                "<value content=\"https://example.org/r\" />",
                "</location>",
            ],
            "{ROW}: {what}: the location's metadata is not written in place\n{text}"
        );
        let back = read_clean("the parts output", Mode::Strict, &text);
        assert!(
            back == doc,
            "{ROW}: {what}: the output read back is not ==: {:?}",
            semantic_compare(&doc, &back)
        );
        assert_eq!(
            written(&what, &back, Mode::Strict),
            text,
            "{ROW}: {what}: a second write is not byte-stable"
        );
    }
    // `SetName` keeps the name's metadata.
    let mut doc = read_clean("the parts input", Mode::Strict, &input);
    apply(
        "SetName p",
        &mut doc,
        SetName::new("p", LangString::new("renamed")),
    );
    let text = written("SetName p", &doc, Mode::Strict);
    assert_eq!(
        block(
            "SetName p",
            &text,
            "<name content=\"renamed\" xmi:uuid=\"urn:uuid:p-name\" xmi:label=\"P\">",
            3
        ),
        [
            "<name content=\"renamed\" xmi:uuid=\"urn:uuid:p-name\" xmi:label=\"P\">",
            "<xmi:Extension extender=\"T\" />",
            "</name>",
        ],
        "{ROW}: SetName p: the name's metadata is not kept\n{text}"
    );
    // `SetContent` with two values over the held three keeps the role
    // element's metadata and that of value 2, by position; value 3's goes
    // with value 3.
    let two: MultiLangString = [LangString::new("x"), LangString::new("y")]
        .into_iter()
        .collect();
    apply("SetContent n1", &mut doc, SetContent::new("n1", two));
    let text = written("SetContent n1", &doc, Mode::Strict);
    assert_eq!(
        block(
            "SetContent n1",
            &text,
            "<content xmi:id=\"n1c\" xmi:uuid=\"urn:uuid:n1-content\" xmi:label=\"N\">",
            7
        ),
        [
            "<content xmi:id=\"n1c\" xmi:uuid=\"urn:uuid:n1-content\" xmi:label=\"N\">",
            "<xmi:Extension extender=\"T\" />",
            "<value content=\"x\" />",
            "<value content=\"y\" xmi:label=\"second\">",
            "<xmi:Extension extender=\"T\" />",
            "</value>",
            "</content>",
        ],
        "{ROW}: SetContent n1: the metadata is not kept by position\n{text}"
    );
    assert!(
        !text.contains("urn:uuid:v3"),
        "{ROW}: SetContent n1: the third value's metadata outlives the value\n{text}"
    );
    // Removing the content drops its metadata.
    apply("SetContent::remove n1", &mut doc, SetContent::remove("n1"));
    let text = written("SetContent::remove n1", &doc, Mode::Strict);
    assert!(
        !text.contains("urn:uuid:n1-content") && !text.contains("xmi:label=\"second\""),
        "{ROW}: SetContent::remove n1: the content's metadata is still written\n{text}"
    );
}

// --- 5. XMI elements XMI does not define -------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_019_xmi_elements_xmi_does_not_define_are_sacm_xmi_003() {
    // In a Claim: each kept as preserved content of the Claim, at its `<`.
    for child in [
        "<xmi:content/>",
        "<xmi:documentation/>",
        "<xmi:Difference/>",
        "<xmi:XMI/>",
    ] {
        let written_name = child.trim_start_matches('<').trim_end_matches("/>");
        let (input, at) = one_mark(&package(&claim("c1", "", &format!("{MARK}{child}"))));
        for mode in MODES {
            let what = format!("{child} in a Claim, {mode:?}");
            let output = read_ok(&what, mode, &input);
            let found: Vec<Summary> = output.diagnostics().iter().map(summary).collect();
            assert_eq!(
                found,
                [(
                    "SACM-XMI-003",
                    kept_severity(mode),
                    "SACM-XMI-003/element-kept",
                    Some(at),
                    vec!["c1".to_owned()]
                )],
                "{ROW}: {what}: not one SACM-XMI-003/element-kept at its start tag"
            );
            assert_eq!(
                arguments(&output.diagnostics()[0]),
                [
                    ("element", written_name.to_owned()),
                    ("holder_class", "Claim".to_owned()),
                    ("holder_id", "c1".to_owned()),
                ],
                "{ROW}: {what}: the arguments"
            );
            assert!(
                element(&what, output.document(), "c1").has_preserved_content(),
                "{ROW}: {what}: the Claim does not hold it"
            );
        }
    }
    // In a name: the whole name is kept, and the name is absent.
    let (input, at) = one_mark(&package(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">{MARK}<name content=\"C\">\
         <xmi:foo/></name></argumentElement>"
    )));
    for mode in MODES {
        let what = format!("<xmi:foo/> in a name, {mode:?}");
        let output = read_ok(&what, mode, &input);
        let found: Vec<Summary> = output.diagnostics().iter().map(summary).collect();
        assert_eq!(
            found,
            [(
                "SACM-XMI-003",
                kept_severity(mode),
                "SACM-XMI-003/element-kept",
                Some(at),
                vec!["c1".to_owned()]
            )],
            "{ROW}: {what}: not one SACM-XMI-003/element-kept at the name"
        );
        assert_eq!(
            arguments(&output.diagnostics()[0]),
            [
                ("element", "name".to_owned()),
                ("holder_class", "Claim".to_owned()),
                ("holder_id", "c1".to_owned()),
            ],
            "{ROW}: {what}: the arguments"
        );
        assert_eq!(
            element(&what, output.document(), "c1").name(),
            None,
            "{ROW}: {what}: the name is not absent"
        );
    }
    // Under `xmi:XMI`: a defect a strict read refuses, which a tolerant read
    // keeps as the document's content.
    let (input, at) = one_mark(&wrapper("", &format!("{MARK}<xmi:foo/>"), ""));
    let error = refusal("<xmi:foo/> under xmi:XMI", Mode::Strict, &input);
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: <xmi:foo/> under xmi:XMI: the strict refusal is not Unsupported: {error:?}"
    );
    assert_eq!(
        (
            error.code().as_str(),
            error.template(),
            error.location().map(|loc| (loc.line(), loc.column()))
        ),
        ("SACM-XMI-003", "SACM-XMI-003/wrapper-element", Some(at)),
        "{ROW}: <xmi:foo/> under xmi:XMI: the strict refusal's code, template and location"
    );
    let strict_arguments: Vec<(&str, Option<&str>)> = error
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.value().as_text()))
        .collect();
    assert_eq!(
        strict_arguments,
        [("element", Some("xmi:foo")), ("container", Some("xmi:XMI"))],
        "{ROW}: <xmi:foo/> under xmi:XMI: the strict refusal's arguments"
    );
    assert!(
        error.to_string().ends_with("(a defect in the input)"),
        "{ROW}: <xmi:foo/> under xmi:XMI: the Display is not labelled a defect: {error}"
    );
    let output = read_ok("<xmi:foo/> under xmi:XMI", Mode::Tolerant, &input);
    let found: Vec<Summary> = output.diagnostics().iter().map(summary).collect();
    assert_eq!(
        found,
        [(
            "SACM-XMI-003",
            Severity::Warning,
            "SACM-XMI-003/wrapper-element-kept",
            Some(at),
            Vec::new()
        )],
        "{ROW}: <xmi:foo/> under xmi:XMI: a tolerant read does not keep it with one Warning \
         affecting no element"
    );
    assert_eq!(
        arguments(&output.diagnostics()[0]),
        [
            ("element", "xmi:foo".to_owned()),
            ("container", "xmi:XMI".to_owned()),
        ],
        "{ROW}: <xmi:foo/> under xmi:XMI: the tolerant arguments"
    );
    assert!(
        output.document().has_preserved_content()
            && !output
                .document()
                .elements()
                .any(SacmElement::has_preserved_content),
        "{ROW}: <xmi:foo/> under xmi:XMI: it is not the document's own content"
    );
    // An XMI-namespace document element other than `xmi:XMI` is InvalidRoot.
    let input = format!("<xmi:Extension xmlns:xmi=\"{XMI_NS}\"/>");
    for mode in MODES {
        let what = format!("a document element xmi:Extension, {mode:?}");
        let error = refusal(&what, mode, &input);
        assert!(
            matches!(error, ReadError::InvalidRoot { .. }),
            "{ROW}: {what}: not InvalidRoot: {error:?}"
        );
        assert_eq!(
            (
                error.code(),
                error.template(),
                error.location().map(|loc| (loc.line(), loc.column()))
            ),
            (
                Code::XMI_INVALID_ROOT,
                "SACM-XMI-001/xmi-namespace-root",
                Some((1, 1))
            ),
            "{ROW}: {what}: code, template and location"
        );
        assert_eq!(
            error.to_string(),
            "the document element \"xmi:Extension\" is in the XMI namespace but is not xmi:XMI, \
             so the document holds no SACM package at line 1, column 1",
            "{ROW}: {what}: Display"
        );
    }
}

// --- 6. Links on a definition --------------------------------------------------------

/// A case of a link on a definition: what it is, the input, the template,
/// the argument other than the holder's, the holder's class and its id.
type LinkCase<'a> = (
    &'a str,
    String,
    &'a str,
    (&'a str, &'a str),
    &'a str,
    &'a str,
);

#[test]
fn sacm23_xmi_019_link_attributes_on_a_definition_are_sacm_xmi_003() {
    // A Note `n1` of `p`, whose `content` holds `value`.
    let note = |value: &str| {
        package(&format!(
            "<note xmi:id=\"n1\">{MARK}<content>{value}</content></note>"
        ))
    };
    // (what, input, template, arguments other than the holder, holder class,
    // holder id).
    let cases: Vec<LinkCase<'_>> = vec![
        (
            "href on a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}href=\"o.xmi#p\"><name content=\"p\"/>\
                 </sacm:ArgumentPackage>"
            ),
            "SACM-XMI-003/attribute-kept",
            ("attribute", "href"),
            "ArgumentPackage",
            "p",
        ),
        (
            "xmi:idref on a root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}xmi:idref=\"p\"><name content=\"p\"/>\
                 </sacm:ArgumentPackage>"
            ),
            "SACM-XMI-003/attribute-kept",
            ("attribute", "xmi:idref"),
            "ArgumentPackage",
            "p",
        ),
        (
            "href on a name",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 {MARK}<name content=\"C\" href=\"#c1\"/></argumentElement>"
            )),
            "SACM-XMI-003/element-kept",
            ("element", "name"),
            "Claim",
            "c1",
        ),
        (
            "xmi:idref on a value",
            note("<value content=\"x\" xmi:idref=\"c1\"/>"),
            "SACM-XMI-003/element-kept",
            ("element", "content"),
            "Note",
            "n1",
        ),
        (
            "xmi:id on the C++-shape value inside a name",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 {MARK}<name><value xmi:id=\"nv\" content=\"C\"/></name></argumentElement>"
            )),
            "SACM-XMI-003/element-kept",
            ("element", "name"),
            "Claim",
            "c1",
        ),
    ];
    for (what, template, kept, (name, value), holder_class, holder_id) in cases {
        let (input, at) = one_mark(&template);
        for mode in MODES {
            let what = format!("{what}, {mode:?}");
            let output = read_ok(&what, mode, &input);
            let found: Vec<Summary> = output.diagnostics().iter().map(summary).collect();
            assert_eq!(
                found,
                [(
                    "SACM-XMI-003",
                    kept_severity(mode),
                    kept,
                    Some(at),
                    vec![holder_id.to_owned()]
                )],
                "{ROW}: {what}: not one {kept}"
            );
            assert_eq!(
                arguments(&output.diagnostics()[0]),
                [
                    (name, value.to_owned()),
                    ("holder_class", holder_class.to_owned()),
                    ("holder_id", holder_id.to_owned()),
                ],
                "{ROW}: {what}: the arguments"
            );
            assert!(
                element(&what, output.document(), holder_id).has_preserved_content(),
                "{ROW}: {what}: the holder does not keep it"
            );
        }
    }
}

// --- 7. The wrapper's attributes -----------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_019_wrapper_attributes_are_metadata() {
    let schema_location = format!("{XSI} xsi:schemaLocation=\"{SACM_NS} SACM.xsd\"");
    let no_namespace = format!("{XSI} xsi:noNamespaceSchemaLocation=\"SACM.xsd\"");
    let cases: [(&str, String, &str); 8] = [
        ("xmi:id", " xmi:id=\"w\"".to_owned(), "xmi:id=\"w\""),
        (
            "xmi:label",
            " xmi:label=\"l\"".to_owned(),
            "xmi:label=\"l\"",
        ),
        ("xmi:uuid", " xmi:uuid=\"u\"".to_owned(), "xmi:uuid=\"u\""),
        ("href", " href=\"#p\"".to_owned(), "href=\"#p\""),
        (
            "xmi:idref",
            " xmi:idref=\"p\"".to_owned(),
            "xmi:idref=\"p\"",
        ),
        (
            "xmi:type",
            " xmi:type=\"xmi:XMI\"".to_owned(),
            "xmi:type=\"xmi:XMI\"",
        ),
        (
            "xsi:schemaLocation",
            schema_location,
            "xsi:schemaLocation=\"http://www.omg.org/spec/SACM/20220301 SACM.xsd\"",
        ),
        (
            "xsi:noNamespaceSchemaLocation",
            no_namespace,
            "xsi:noNamespaceSchemaLocation=\"SACM.xsd\"",
        ),
    ];
    for (what, attributes, written_as) in cases {
        let input = wrapper(&attributes, "", "");
        for mode in MODES {
            let doc = read_clean(what, mode, &input);
            assert!(
                !doc.has_preserved_content(),
                "{ROW}: {what}: a {mode:?} read keeps it as preserved content"
            );
            for write_mode in MODES {
                let text = written(what, &doc, write_mode);
                let wrapper_line = trimmed(&text)
                    .into_iter()
                    .find(|line| line.starts_with("<xmi:XMI "))
                    .unwrap_or_else(|| {
                        panic!("{ROW}: {what}: a {write_mode:?} write is not wrapped:\n{text}")
                    })
                    .to_owned();
                assert!(
                    wrapper_line.contains(written_as),
                    "{ROW}: {what}: a {write_mode:?} write does not write {written_as:?} on \
                     xmi:XMI:\n{text}"
                );
                let back = read_clean("the output", Mode::Strict, &text);
                assert!(
                    back == doc,
                    "{ROW}: {what}: the {write_mode:?} output read back is not =="
                );
            }
        }
    }
    // `xmi:version` stays SACM-XMI-011 and is not written; beside metadata,
    // the document is still wrapped.
    let input = wrapper(" xmi:version=\"2.5.1\" xmi:uuid=\"u\"", "", "");
    for mode in MODES {
        let what = format!("xmi:version beside xmi:uuid, {mode:?}");
        let output = read_ok(&what, mode, &input);
        let found: Vec<(&str, Severity, &str)> = output
            .diagnostics()
            .iter()
            .map(|found| (found.code().as_str(), found.severity(), found.template()))
            .collect();
        assert_eq!(
            found,
            [(
                "SACM-XMI-011",
                Severity::Info,
                "SACM-XMI-011/version-on-xmi-element"
            )],
            "{ROW}: {what}: not one SACM-XMI-011 Info"
        );
        let text = written(&what, output.document(), Mode::Strict);
        assert!(
            text.contains("xmi:uuid=\"u\"") && !text.contains("xmi:version"),
            "{ROW}: {what}: the uuid is not written, or the version is:\n{text}"
        );
    }
    // Another attribute is still a defect.
    let input = wrapper(" note=\"x\"", "", "");
    let error = refusal("note=\"x\" on xmi:XMI", Mode::Strict, &input);
    assert_eq!(
        (error.code().as_str(), error.template()),
        ("SACM-XMI-003", "SACM-XMI-003/unread-attribute"),
        "{ROW}: note=\"x\" on xmi:XMI: the strict refusal"
    );
    let output = read_ok("note=\"x\" on xmi:XMI", Mode::Tolerant, &input);
    let found: Vec<&str> = output
        .diagnostics()
        .iter()
        .map(Diagnostic::template)
        .collect();
    assert_eq!(
        found,
        ["SACM-XMI-003/wrapper-attribute-kept"],
        "{ROW}: note=\"x\" on xmi:XMI: the tolerant read"
    );
}

// --- 8. One id space ---------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_019_metadata_ids_share_the_id_space() {
    // An Extension id that repeats an element's id.
    let input = package(&claim("c1", "", "<xmi:Extension xmi:id=\"c1\"/>"));
    for mode in MODES {
        let what = format!("an Extension with the id of c1, {mode:?}");
        let error = refusal(&what, mode, &input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id, .. } if id.as_str() == "c1"),
            "{ROW}: {what}: not DuplicateId c1: {error:?}"
        );
        assert_eq!(error.code(), Code::ID_DUPLICATE, "{ROW}: {what}: code()");
    }
    // A generated id skips a metadata id.
    let input = package(
        "<xmi:Extension xmi:id=\"claim_1\"/><argumentElement xmi:type=\"sacm:Claim\">\
         <name content=\"C\"/></argumentElement>",
    );
    for mode in MODES {
        let what = format!("an id-less Claim beside an Extension claim_1, {mode:?}");
        let output = read_ok(&what, mode, &input);
        let codes: Vec<&str> = output
            .diagnostics()
            .iter()
            .map(|found| found.code().as_str())
            .collect();
        assert_eq!(
            codes,
            ["SACM-XMI-004"],
            "{ROW}: {what}: not one SACM-XMI-004 for the missing id"
        );
        let claims: Vec<String> = output
            .document()
            .elements()
            .filter(|found| found.parent().is_some())
            .map(|found| found.id().as_str().to_owned())
            .collect();
        assert!(
            claims.len() == 1 && claims[0] != "claim_1",
            "{ROW}: {what}: the generated id is a metadata id: {claims:?}"
        );
    }
    let mut doc = read_clean(
        "Golden M1",
        Mode::Strict,
        &file(M1).replace("xmi:id=\"x1\"", "xmi:id=\"claim_1\""),
    );
    let created = apply(
        "CreateClaim without an id",
        &mut doc,
        CreateClaim::new("p", LangString::new("G")),
    );
    assert!(
        created
            .created_ids()
            .iter()
            .all(|id| !["claim_1", "d1", "doc"].contains(&id.as_str())),
        "{ROW}: CreateClaim generates a metadata id: {:?}",
        created.created_ids()
    );
    // An edit cannot take one, nor name one.
    let mut doc = read_clean("Golden M1", Mode::Strict, &file(M1));
    for taken in ["x1", "d1", "doc"] {
        let answer = edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("C")).with_id(taken),
        );
        assert!(
            matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == taken),
            "{ROW}: CreateClaim::with_id({taken:?}) is not DuplicateId: {answer:?}"
        );
    }
    let answer = edit::apply(
        &mut doc,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "x1"),
    );
    assert!(
        matches!(answer, Err(EditError::ReferenceNotFound { .. })),
        "{ROW}: AddReference naming x1 is not ReferenceNotFound: {answer:?}"
    );
    // A typed reference to a metadata id is SACM-REF-003.
    let input = file(M1).replace(
        "xmi:id=\"c1\" xmi:uuid=\"urn:uuid:c1\"",
        "xmi:id=\"c1\" xmi:uuid=\"urn:uuid:c1\" isCitation=\"true\" citedElement=\"x1\"",
    );
    let doc = read_clean("c1 citing x1", Mode::Strict, &input);
    let found: Vec<(&str, Vec<String>)> = validation::validate(&doc)
        .iter()
        .map(|found| {
            (
                found.code().as_str(),
                found
                    .affected()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
            )
        })
        .collect();
    assert!(
        found
            .iter()
            .any(|(code, affected)| *code == "SACM-REF-003" && affected.contains(&"c1".to_owned()))
            && found
                .iter()
                .all(|(code, _)| *code != "SACM-REF-001" && *code != "SACM-REF-002"),
        "{ROW}: citedElement=\"x1\" is not SACM-REF-003 alone: {found:?}"
    );
}

// --- 9. A strict save checks metadata ids and IDREFs ---------------------------------

/// The diagnostics of the strict write refusal of `doc`, which must be
/// `WriteError::Invalid`.
fn invalid_diagnostics(what: &str, doc: &Document) -> Vec<Diagnostic> {
    match write(doc, Mode::Strict) {
        Err(error @ WriteError::Invalid { .. }) => error.diagnostics().to_vec(),
        Err(error) => panic!("{ROW}: {what}: the strict write is not Invalid: {error:?}"),
        Ok(text) => panic!("{ROW}: {what}: the strict write writes it:\n{text}"),
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_xmi_019_a_strict_save_checks_metadata_ids_and_idrefs() {
    // A dangling IDREFS token under xmi:XMI.
    let what = "<xmi:Difference target=\"p nope\"/>";
    let doc = read_clean(
        what,
        Mode::Strict,
        &wrapper("", "<xmi:Difference target=\"p nope\"/>", ""),
    );
    let found = invalid_diagnostics(what, &doc);
    let summaries: Vec<(Summary, &str, Option<&str>)> = found
        .iter()
        .map(|found| (summary(found), found.requirement_id(), found.clause()))
        .collect();
    assert_eq!(
        summaries,
        [(
            (
                "SACM-REF-001",
                Severity::Error,
                "SACM-REF-001/metadata-dangling",
                None,
                vec!["nope".to_owned()]
            ),
            ROW,
            None
        )],
        "{ROW}: {what}: not one SACM-REF-001/metadata-dangling"
    );
    assert_eq!(
        arguments(&found[0]),
        [
            ("attribute", "target".to_owned()),
            ("element", "xmi:Difference".to_owned()),
            ("target", "nope".to_owned()),
        ],
        "{ROW}: {what}: the arguments"
    );
    let text = written(what, &doc, Mode::Tolerant);
    assert!(
        text.contains("<xmi:Difference target=\"p nope\" />"),
        "{ROW}: {what}: a tolerant write does not write it:\n{text}"
    );
    // A metadata id that is not an NCName.
    let what = "<xmi:Extension xmi:id=\"1x\"/> in p";
    let doc = read_clean(
        what,
        Mode::Strict,
        &package("<xmi:Extension xmi:id=\"1x\"/>"),
    );
    let found = invalid_diagnostics(what, &doc);
    let summaries: Vec<(Summary, &str)> = found
        .iter()
        .map(|found| (summary(found), found.requirement_id()))
        .collect();
    assert_eq!(
        summaries,
        [(
            (
                "SACM-ID-002",
                Severity::Error,
                "SACM-ID-002/metadata-id-not-an-ncname",
                None,
                vec!["p".to_owned()]
            ),
            ROW
        )],
        "{ROW}: {what}: not one SACM-ID-002/metadata-id-not-an-ncname"
    );
    assert_eq!(
        arguments(&found[0]),
        [
            ("element", "xmi:Extension".to_owned()),
            ("xmi_id", "1x".to_owned()),
        ],
        "{ROW}: {what}: the arguments"
    );
    let text = written(what, &doc, Mode::Tolerant);
    assert!(
        text.contains("<xmi:Extension xmi:id=\"1x\" />"),
        "{ROW}: {what}: a tolerant write does not write it:\n{text}"
    );
    // The wrapper's own xmi:idref is checked: naming p it passes, naming
    // nothing it is SACM-REF-001.
    let what = "xmi:idref=\"p\" on xmi:XMI";
    let doc = read_clean(what, Mode::Strict, &wrapper(" xmi:idref=\"p\"", "", ""));
    let text = written(what, &doc, Mode::Strict);
    assert!(
        text.contains("xmi:idref=\"p\""),
        "{ROW}: {what}: a strict write does not write it:\n{text}"
    );
    let what = "xmi:idref=\"nope\" on xmi:XMI";
    let doc = read_clean(what, Mode::Strict, &wrapper(" xmi:idref=\"nope\"", "", ""));
    let found = invalid_diagnostics(what, &doc);
    assert_eq!(
        found.iter().map(Diagnostic::template).collect::<Vec<_>>(),
        ["SACM-REF-001/metadata-dangling"],
        "{ROW}: {what}: not one metadata-dangling"
    );
    assert_eq!(
        arguments(&found[0]),
        [
            ("attribute", "xmi:idref".to_owned()),
            ("element", "xmi:XMI".to_owned()),
            ("target", "nope".to_owned()),
        ],
        "{ROW}: {what}: the arguments"
    );
}

// --- 10. Copy, delete and move ------------------------------------------------------

/// The root ArgumentPackage `p` holding the Claim `c1` with a uuid and a
/// label, a name with a uuid, and the extension `extension`.
fn with_marked_claim(extension: &str) -> String {
    package(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" xmi:uuid=\"urn:uuid:c1\" \
         xmi:label=\"C one\"><name content=\"C1\" xmi:uuid=\"urn:uuid:c1-name\"/>{extension}\
         </argumentElement>"
    ))
}

/// The trimmed lines of the element `id` in `text`, from its start tag to
/// the first `</argumentElement>` after it.
fn element_lines<'t>(what: &str, text: &'t str, id: &str) -> Vec<&'t str> {
    let lines = trimmed(text);
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(start) = lines.iter().position(|line| line.contains(&attribute)) else {
        panic!("{ROW}: {what}: the output has no line with {attribute}:\n{text}");
    };
    let end = lines[start..]
        .iter()
        .position(|line| *line == "</argumentElement>")
        .map_or(lines.len(), |offset| start + offset + 1);
    lines[start..end].to_vec()
}

#[test]
fn sacm23_xmi_019_a_copy_drops_the_uuid_a_delete_removes_and_a_move_keeps() {
    // A copy drops the uuid of the element and of its parts, and keeps its
    // label and extensions.
    let source = read_clean(
        "c1 with metadata",
        Mode::Strict,
        &with_marked_claim("<xmi:Extension extender=\"T\"/>"),
    );
    let mut doc = source.clone();
    let copy = match CopyElements::new(&source, ["c1"], "p") {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: CopyElements::new refused c1: {error}"),
    };
    let mutation = apply("CopyElements c1 into p", &mut doc, copy);
    let Some(copied) = mutation.created_ids().first().cloned() else {
        panic!("{ROW}: CopyElements c1 created nothing");
    };
    let text = written("the copy", &doc, Mode::Strict);
    let lines = element_lines("the copy", &text, copied.as_str());
    assert!(
        lines.iter().all(|line| !line.contains("xmi:uuid")),
        "{ROW}: the copy, or a part of it, keeps a uuid:\n{text}"
    );
    assert!(
        lines.contains(&"<xmi:Extension extender=\"T\" />")
            && lines
                .iter()
                .any(|line| line.contains("xmi:label=\"C one\"")),
        "{ROW}: the copy does not keep its label and its extension:\n{text}"
    );
    assert!(
        element_lines("the original", &text, "c1")
            .iter()
            .filter(|line| line.contains("xmi:uuid"))
            .count()
            == 2,
        "{ROW}: the original lost its uuids:\n{text}"
    );
    {
        let found = element("the copy", &doc, copied.as_str());
        assert_eq!(
            (found.uuid(), found.label()),
            (None, Some("C one")),
            "{ROW}: the copy's uuid and label"
        );
        assert_eq!(
            element("the original", &doc, "c1").uuid(),
            Some("urn:uuid:c1"),
            "{ROW}: the original's uuid"
        );
    }
    // A delete removes the metadata.
    let mut doc = read_clean("Golden M1", Mode::Strict, &file(M1));
    apply("DeleteElement c1", &mut doc, DeleteElement::new("c1"));
    let text = written("DeleteElement c1", &doc, Mode::Strict);
    assert!(
        !text.contains("urn:uuid:c1"),
        "{ROW}: DeleteElement c1: its uuid is still written:\n{text}"
    );
    // A move keeps it.
    let mut doc = read_clean("Golden M1", Mode::Strict, &file(M1));
    apply(
        "CreateArgumentPackage q",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("Q")).with_id("q"),
    );
    apply("MoveElement c1 to q", &mut doc, MoveElement::new("c1", "q"));
    let text = written("MoveElement c1 to q", &doc, Mode::Strict);
    assert!(
        element_lines("MoveElement c1 to q", &text, "c1")[0].contains("xmi:uuid=\"urn:uuid:c1\""),
        "{ROW}: MoveElement c1 to q: its uuid is not kept:\n{text}"
    );
    assert_eq!(
        element("MoveElement c1 to q", &doc, "c1").uuid(),
        Some("urn:uuid:c1"),
        "{ROW}: MoveElement c1 to q: uuid()"
    );
    // A copy whose extension holds an id taken in the target.
    let source = read_clean(
        "c1 with an extension x9",
        Mode::Strict,
        &with_marked_claim("<xmi:Extension xmi:id=\"x9\"/>"),
    );
    let mut doc = source.clone();
    let copy = match CopyElements::new(&source, ["c1"], "p") {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: CopyElements::new refused c1: {error}"),
    };
    let answer = edit::apply(&mut doc, copy);
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "x9"),
        "{ROW}: a copy whose extension's id is taken is not DuplicateId x9: {answer:?}"
    );
    assert!(
        doc == source,
        "{ROW}: the refused copy changed the document"
    );
}

// --- 11. Compared --------------------------------------------------------------------

#[test]
fn sacm23_xmi_019_metadata_differences_are_compared() {
    let m1 = read_clean("Golden M1", Mode::Strict, &file(M1));
    let other = read_clean(
        "Golden M1 with another uuid of c1",
        Mode::Strict,
        &file(M1).replace("urn:uuid:c1", "urn:uuid:c1-other"),
    );
    assert!(m1 != other, "{ROW}: a different uuid is ==");
    let differences = semantic_compare(&m1, &other);
    // Element metadata, and its place before the element's preserved
    // content.
    let preserved = |uuid: &str, kept: &str| {
        package(&claim(
            "c1",
            &format!(" xmi:uuid=\"{uuid}\""),
            &format!("<{kept}/>"),
        ))
    };
    let left = read_ok("uuid a and <foo/>", Mode::Tolerant, &preserved("a", "foo")).into_document();
    let right =
        read_ok("uuid b and <bar/>", Mode::Tolerant, &preserved("b", "bar")).into_document();
    let ordered = semantic_compare(&left, &right);
    // Document metadata, and its place before the document's preserved
    // content.
    let outside = |uuid: &str, kept: &str| wrapper(&format!(" xmi:uuid=\"{uuid}\""), kept, "");
    let wrapper_left = read_ok(
        "wrapper uuid a and <xmi:foo/>",
        Mode::Tolerant,
        &outside("a", "<xmi:foo/>"),
    )
    .into_document();
    let wrapper_right = read_ok(
        "wrapper uuid b and <xmi:bar/>",
        Mode::Tolerant,
        &outside("b", "<xmi:bar/>"),
    )
    .into_document();
    let document_ordered = semantic_compare(&wrapper_left, &wrapper_right);
    let metadata_only = semantic_compare(
        &read_clean("wrapper uuid a", Mode::Strict, &outside("a", "")),
        &read_clean("wrapper uuid b", Mode::Strict, &outside("b", "")),
    );
    {
        use sacm::compare::Difference;
        use sacm::model::ElementId;
        assert_eq!(
            differences,
            [Difference::Metadata {
                id: ElementId::from("c1")
            }],
            "{ROW}: Golden M1 against another uuid of c1"
        );
        assert_eq!(
            differences[0].element_id().map(ElementId::as_str),
            Some("c1"),
            "{ROW}: the Metadata difference's element_id()"
        );
        assert_eq!(
            ordered,
            [
                Difference::Metadata {
                    id: ElementId::from("c1")
                },
                Difference::PreservedContent {
                    id: ElementId::from("c1")
                },
            ],
            "{ROW}: Metadata is not reported just before PreservedContent"
        );
        assert_eq!(
            metadata_only,
            [Difference::DocumentMetadata],
            "{ROW}: differing wrapper metadata"
        );
        assert_eq!(
            metadata_only[0].element_id(),
            None,
            "{ROW}: DocumentMetadata names an element"
        );
        assert_eq!(
            document_ordered,
            [
                Difference::DocumentMetadata,
                Difference::DocumentPreservedContent
            ],
            "{ROW}: DocumentMetadata is not reported before DocumentPreservedContent"
        );
        for difference in differences.iter().chain(metadata_only.iter()) {
            let shown = difference.to_string();
            assert!(
                !shown.is_empty() && !shown.contains('\n'),
                "{ROW}: the Display of {difference:?} is not one line: {shown:?}"
            );
        }
    }
}

// --- 13. The ptc/22-03-13 header -------------------------------------------------------

#[test]
fn sacm23_xmi_019_the_ptc_22_03_13_header_reads() {
    let header = "<xmi:documentation xmi:type=\"xmi:Documentation\">\
                  <xmi:exporter>MagicDraw Clean XMI Exporter</xmi:exporter>\
                  <xmi:exporterVersion>19.0 v9</xmi:exporterVersion></xmi:documentation>";
    let input = wrapper("", header, "");
    let doc = read_clean("the ptc/22-03-13 header", Mode::Strict, &input);
    for mode in MODES {
        let text = written("the ptc/22-03-13 header", &doc, mode);
        assert!(
            trimmed(&text).contains(&header),
            "{ROW}: the ptc/22-03-13 header: a {mode:?} write does not write it back exactly:\n{text}"
        );
    }
}
