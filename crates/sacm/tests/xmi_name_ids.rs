//! Tests for an `xmi:id` on a `name` `LangString` (SACM23-XMI-022; XMI 2.5.1
//! §7.8.3, §7.6.1, rule 2a; ptc/22-03-13 `LangString` `uml:Class`; D-ID;
//! D-PRESERVE; ADR 0024).
//!
//! An `xmi:id` on `name` is read in both modes with no diagnostic and kept
//! with the owning element as `SacmElement::name_id()`, not on the
//! `LangString`. `xmi::write` writes it as the first attribute of `name`. It
//! shares the file's id space (a repeat is `DuplicateId`, a caller id equal
//! to it is `EditError::DuplicateId`, generated ids skip it), follows the
//! element-id rules (empty is `InvalidId`, padded is trimmed, a non-NCName is
//! reported by `validate` as SACM-ID-002 on the owner), and is compared as
//! `Difference::NameId`. An `xmi:id` on a value element stays refused.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-022";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a start tag or an attribute is expected.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// A declaration ended by CRLF, then a comment holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// The name every test name holds.
const NAME: &str = "lang=\"en\" content=\"x\"";

// --- Inputs ------------------------------------------------------------------

/// An ArgumentPackage root `p` whose content, from line 2, is `content`.
fn root_holding(content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <!-- \u{e9} --> {content}\r\n\
         </sacm:ArgumentPackage>"
    )
}

/// A root `p` named "P" holding `content` after its name.
fn in_ap(content: &str) -> String {
    root_holding(&format!("<name content=\"P\"/>\r\n  {content}"))
}

/// A Claim `id` holding `content`.
fn claim(id: &str, content: &str) -> String {
    format!("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\">{content}</argumentElement>")
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

// --- Reading and writing -------------------------------------------------------

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
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
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
                "{ROW}: {what}: {mode:?} read returned a document of {} elements",
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

fn write_in(what: &str, mode: Mode, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// The name id of the element `id` of `doc`, which must exist.
fn name_id_of(what: &str, doc: &Document, id: &str) -> Option<String> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: {what}: the document has no element {id:?}");
    };
    element.name_id().map(|found| found.as_str().to_owned())
}

/// The output's `name` lines, trimmed.
fn name_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("<name"))
        .collect()
}

// --- Item 1: read and write ----------------------------------------------------

/// The inputs of item 1: what, the input, and the owner of the name id.
fn named_with_id() -> [(&'static str, String, &'static str); 3] {
    [
        (
            "a root",
            root_holding(&format!("<name xmi:id=\"n1\" {NAME}/>")),
            "p",
        ),
        (
            "a child",
            in_ap(&claim("c1", &format!("<name xmi:id=\"n1\" {NAME}/>"))),
            "c1",
        ),
        (
            "a wrapped root",
            format!(
                "{LEAD}<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\r\n  \
                 <sacm:ArgumentPackage xmi:id=\"w\"><name {NAME} xmi:id=\"n1\"/></sacm:ArgumentPackage>\r\n\
                 </xmi:XMI>"
            ),
            "w",
        ),
    ]
}

#[test]
fn sacm23_xmi_022_a_name_id_on_a_root_or_a_child_reads_with_no_diagnostic_and_is_kept_with_the_owner()
 {
    for (what, input, owner) in named_with_id() {
        let doc = read_ok(what, &input);
        assert_eq!(
            name_id_of(what, &doc, owner),
            Some("n1".to_owned()),
            "{ROW}: {what}: name_id() is not n1"
        );
        assert_eq!(
            doc.get(owner).and_then(SacmElement::name),
            Some(&LangString::new("x").with_lang("en")),
            "{ROW}: {what}: the name is not read with its id"
        );
    }
}

#[test]
fn sacm23_xmi_022_write_writes_the_name_id_first_on_name_and_reads_back_equal() {
    for (what, input, owner) in named_with_id() {
        let doc = read_ok(what, &input);
        let text = write_in(what, Mode::Strict, &doc);
        let expected = "<name xmi:id=\"n1\" lang=\"en\" content=\"x\" />";
        assert!(
            name_lines(&text).contains(&expected),
            "{ROW}: {what}: the output has no line {expected:?}:\n{text}"
        );
        assert_eq!(
            write_in(what, Mode::Strict, &doc),
            text,
            "{ROW}: {what}: two writes differ"
        );
        let back = read_ok(&format!("{what}, written"), &text);
        assert_eq!(back, doc, "{ROW}: {what}: the output reads back different");
        assert_eq!(
            name_id_of(what, &back, owner),
            Some("n1".to_owned()),
            "{ROW}: {what}: the name id does not survive a write"
        );
        assert_eq!(
            write_in(what, Mode::Strict, &back),
            text,
            "{ROW}: {what}: writing what was read back gives other bytes"
        );
    }
    // A name holding only its id.
    let doc = read_ok(
        "a name with only an id",
        &root_holding("<name xmi:id=\"n1\"/>"),
    );
    let text = write_in("a name with only an id", Mode::Strict, &doc);
    assert!(
        name_lines(&text).contains(&"<name xmi:id=\"n1\" />"),
        "{ROW}: a name with only an id is not written with only its id:\n{text}"
    );
}

#[test]
fn sacm23_xmi_022_name_id_is_none_without_one_and_a_name_id_is_no_element() {
    let doc = read_ok(
        "names with and without ids",
        &in_ap(&format!(
            "{}{}{}",
            claim("with", &format!("<name xmi:id=\"n1\" {NAME}/>")),
            claim("without", &format!("<name {NAME}/>")),
            claim("nameless", "")
        )),
    );
    assert_eq!(
        name_id_of("a name without an id", &doc, "without"),
        None,
        "{ROW}: a name without an id has a name id"
    );
    assert_eq!(
        name_id_of("an element without a name", &doc, "nameless"),
        None,
        "{ROW}: an element without a name has a name id"
    );
    assert_eq!(
        name_id_of("the root", &doc, "p"),
        None,
        "{ROW}: the root's name has a name id"
    );
    assert!(
        doc.get("n1").is_none(),
        "{ROW}: get(\"n1\") finds an element"
    );
    assert!(!doc.contains("n1"), "{ROW}: contains(\"n1\") is true");
    assert_eq!(doc.element_count(), 4, "{ROW}: a name id became an element");
}

// --- Item 2: one id space --------------------------------------------------------

#[test]
fn sacm23_xmi_022_a_name_id_repeating_any_id_is_duplicate_id_at_both_start_tags() {
    let cases = [
        (
            "a Claim c1 whose name is c1",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 {MARK}<name xmi:id=\"c1\" {NAME}/></argumentElement>"
            )),
            "c1",
        ),
        (
            "two names n1 on different elements",
            in_ap(&format!(
                "{}{}",
                claim("a", &format!("{MARK}<name xmi:id=\"n1\" {NAME}/>")),
                claim("b", &format!("{MARK}<name xmi:id=\"n1\" {NAME}/>"))
            )),
            "n1",
        ),
        (
            "a name n1 and a later Claim n1",
            root_holding(&format!(
                "{MARK}<name xmi:id=\"n1\" {NAME}/>\r\n  {MARK}{}",
                claim("n1", "<name content=\"C\"/>")
            )),
            "n1",
        ),
        (
            "the root's id on its name",
            format!(
                "{LEAD}{MARK}<sacm:ArgumentPackage{NS} xmi:id=\"p\">{MARK}<name xmi:id=\"p\" {NAME}/>\
                 </sacm:ArgumentPackage>"
            ),
            "p",
        ),
    ];
    for (what, template, id) in &cases {
        let (input, marks) = marked(template);
        let error = refused(what, &input);
        assert!(
            matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == *id),
            "{ROW}: {what}: not DuplicateId for {id:?}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Some(Code::ID_DUPLICATE),
            "{ROW}: {what}: the code"
        );
        assert_eq!(
            error.locations(),
            marks.as_slice(),
            "{ROW}: {what}: not both start tags"
        );
    }
}

#[test]
fn sacm23_xmi_022_generated_ids_skip_a_name_id_on_read_and_on_edit() {
    let idless = "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>";
    for (what, input) in [
        (
            "a name id before the id-less Claim",
            in_ap(&format!(
                "{}{idless}",
                claim("c9", "<name xmi:id=\"claim_1\" content=\"C\"/>")
            )),
        ),
        (
            "a name id after the id-less Claim",
            in_ap(&format!(
                "{idless}{}",
                claim("c9", "<name xmi:id=\"claim_1\" content=\"C\"/>")
            )),
        ),
    ] {
        for mode in MODES {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what} ({mode:?}): refused: {error:?}"),
            };
            let doc = output.document();
            assert!(
                doc.get("claim_1").is_none() && doc.get("claim_2").is_some(),
                "{ROW}: {what} ({mode:?}): the id-less Claim is not claim_2: {doc:?}"
            );
        }
    }
    // An edit's generated id skips it too.
    let mut doc = read_ok(
        "a root whose name is claim_1",
        &root_holding("<name xmi:id=\"claim_1\" content=\"P\"/>"),
    );
    let created = match edit::apply(&mut doc, CreateClaim::new("p", LangString::new("New"))) {
        Ok(mutation) => mutation.created_ids()[0].clone(),
        Err(error) => panic!("{ROW}: creating a Claim was refused: {error}"),
    };
    assert_eq!(
        created.as_str(),
        "claim_2",
        "{ROW}: an edit generated the name id claim_1"
    );
}

#[test]
fn sacm23_xmi_022_a_caller_id_equal_to_a_name_id_is_duplicate_id() {
    let mut doc = read_ok(
        "a root whose name is n1",
        &root_holding(&format!("<name xmi:id=\"n1\" {NAME}/>")),
    );
    let before = doc.clone();
    let refusals = [
        edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("C")).with_id("n1"),
        ),
        edit::apply(
            &mut doc,
            CreateArgumentPackage::root(LangString::new("A")).with_id("n1"),
        ),
    ];
    for result in refusals {
        let error = match result {
            Ok(mutation) => panic!("{ROW}: creating n1 succeeded: {mutation:?}"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "n1"),
            "{ROW}: creating n1 is not DuplicateId: {error:?}"
        );
    }
    assert_eq!(doc, before, "{ROW}: a refused edit changed the document");
}

// --- Item 3 and 4: the element-id rules --------------------------------------------

#[test]
fn sacm23_xmi_022_an_empty_name_id_is_invalid_id_at_the_name_and_a_padded_one_is_trimmed() {
    let (input, marks) = marked(&in_ap(&claim(
        "c1",
        &format!("{MARK}<name xmi:id=\"\" {NAME}/>"),
    )));
    let error = refused("an empty name id", &input);
    assert!(
        matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
        "{ROW}: an empty name id is not InvalidId: {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(marks[0]),
        "{ROW}: an empty name id is not refused at the name start tag"
    );

    let (input, marks) = marked(&in_ap(&claim(
        "c1",
        &format!("<name {MARK}xmi:id=\" n1 \" {NAME}/>"),
    )));
    for mode in MODES {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!("{ROW}: a padded name id ({mode:?}) was refused: {error:?}"),
        };
        assert_eq!(
            name_id_of("a padded name id", output.document(), "c1"),
            Some("n1".to_owned()),
            "{ROW}: a padded name id ({mode:?}) is not read trimmed"
        );
        let [info] = output.diagnostics() else {
            panic!(
                "{ROW}: a padded name id ({mode:?}): not one diagnostic: {:?}",
                output.diagnostics()
            );
        };
        assert_eq!(
            (info.code(), info.severity(), info.requirement_id()),
            (Code::ID_TRIMMED, Severity::Info, "SACM23-XMI-025"),
            "{ROW}: a padded name id ({mode:?}): not the SACM-ID-004 Info"
        );
        assert_eq!(
            (info.affected(), info.location()),
            ([ElementId::from("c1")].as_slice(), Some(marks[0])),
            "{ROW}: a padded name id ({mode:?}): the owner and the attribute's name"
        );
    }
}

#[test]
fn sacm23_xmi_022_a_name_id_that_is_not_an_ncname_reads_and_is_one_id_002_on_the_owner() {
    let doc = read_ok(
        "a name id 1n",
        &root_holding("<name xmi:id=\"1n\" content=\"P\"/>"),
    );
    assert_eq!(
        name_id_of("a name id 1n", &doc, "p"),
        Some("1n".to_owned()),
        "{ROW}: the name id 1n is not held as written"
    );
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!(
            "{ROW}: validate gave {} findings, not one: {found:?}",
            found.len()
        );
    };
    assert_eq!(
        (finding.code(), finding.severity()),
        (Code::ID_INVALID, Severity::Error),
        "{ROW}: the finding is not the SACM-ID-002 Error"
    );
    assert_eq!(
        finding.affected(),
        [ElementId::from("p")],
        "{ROW}: the finding does not affect the owner"
    );
    let message = finding.message();
    assert!(
        message.contains("name") && message.contains("\"1n\""),
        "{ROW}: the message {message:?} does not name `name` and the id \"1n\""
    );
    let error = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: a strict write of the name id 1n succeeded:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. })
            && error
                .diagnostics()
                .iter()
                .any(|d| d.code() == Code::ID_INVALID && d.affected() == [ElementId::from("p")]),
        "{ROW}: the strict refusal does not hold the SACM-ID-002: {error:?}"
    );
    let text = write_in("a name id 1n", Mode::Tolerant, &doc);
    assert!(
        name_lines(&text).contains(&"<name xmi:id=\"1n\" content=\"P\" />"),
        "{ROW}: a tolerant write does not keep the name id 1n:\n{text}"
    );
}

// --- Item 5: comparison --------------------------------------------------------------

#[test]
fn sacm23_xmi_022_documents_differing_only_in_a_name_id_give_one_name_id_difference() {
    let with = |name: &str| read_ok(name, &root_holding(name));
    let n1 = with("<name xmi:id=\"n1\" content=\"P\"/>");
    let n2 = with("<name xmi:id=\"n2\" content=\"P\"/>");
    let none = with("<name content=\"P\"/>");
    let id = |value: &str| Some(ElementId::from(value));
    let cases = [
        ("present and absent", &n1, &none, id("n1"), None),
        ("absent and present", &none, &n1, None, id("n1")),
        ("different values", &n1, &n2, id("n1"), id("n2")),
    ];
    for (what, left, right, left_id, right_id) in cases {
        assert_ne!(left, right, "{ROW}: {what}: the documents are ==");
        let differences = semantic_compare(left, right);
        let expected = Difference::NameId {
            id: ElementId::from("p"),
            left: left_id,
            right: right_id,
        };
        assert_eq!(
            differences,
            std::slice::from_ref(&expected),
            "{ROW}: {what}: not one NameId difference"
        );
        assert_eq!(
            expected.element_id(),
            Some(&ElementId::from("p")),
            "{ROW}: {what}: element_id() is not the owner"
        );
        let shown = expected.to_string();
        assert!(
            !shown.contains('\n') && shown.contains("\"p\""),
            "{ROW}: {what}: Display {shown:?} is not one line naming \"p\""
        );
    }
    // With the name too, the NameId comes after the Name.
    let other = with("<name content=\"Q\"/>");
    let differences = semantic_compare(&n1, &other);
    assert!(
        matches!(
            differences.as_slice(),
            [Difference::Name { .. }, Difference::NameId { .. }]
        ),
        "{ROW}: a name and a name id that both differ are not Name then NameId: {differences:?}"
    );
}

// --- Item 6: a value element's id ----------------------------------------------------

#[test]
fn sacm23_xmi_022_an_xmi_id_on_a_value_element_stays_unsupported_at_the_attribute() {
    for template in [
        root_holding(&format!(
            "<name><content {MARK}xmi:id=\"v1\">x</content></name>"
        )),
        root_holding(&format!(
            "<name xmi:id=\"n1\"><lang {MARK}xmi:id=\"v1\">en</lang></name>"
        )),
    ] {
        let (input, marks) = marked(&template);
        let error = refused("an id on a value element", &input);
        assert!(
            matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
            "{ROW}: an id on a value element is not Unsupported: {error:?}"
        );
        assert_eq!(
            error.location(),
            Some(marks[0]),
            "{ROW}: an id on a value element is not refused at the attribute: {error}"
        );
    }
}
