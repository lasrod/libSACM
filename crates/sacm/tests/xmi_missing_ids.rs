//! Tests for elements read without an `xmi:id` (SACM23-XMI-020; XMI 2.5.1
//! §7.6.1 `use="optional"`, rules 2e and 2f; D-ID; D-NO-ID; ADR 0019;
//! ADR 0024).
//!
//! Both modes read an element with no `xmi:id`, a root or a child. It gets a
//! generated id in the edit scheme `<snake_case kind>_<n>`: the smallest `n`
//! above the kind's counter whose id is not the value of any `xmi:id` in the
//! input (later ones, trimmed padded ones and `name` ids included) and was not
//! generated earlier in the read, in document order. Each such element is
//! reported once as Info SACM-XMI-004 at its start tag. The generated id is an
//! ordinary id: written on save, compared and edited like any other, and the
//! counters continue above it.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters. The inputs end their
//! lines with CRLF.

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-020";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a start tag or a refusal is expected.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The start tag of an XMI wrapper that declares `xmi` and `sacm`.
const WRAPPER_OPEN: &str = concat!(
    "<xmi:XMI",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
);

/// A declaration ended by CRLF, then a comment holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// How long one read may take, in debug, for the largest input here.
const DEADLINE: Duration = Duration::from_secs(5);

// --- Inputs ------------------------------------------------------------------

/// `content` in a named ArgumentPackage root `p`, after [`LEAD`].
fn in_ap(content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n  \
         <!-- \u{e9} --> {content}\r\n</sacm:ArgumentPackage>"
    )
}

/// `content` in a named AssuranceCasePackage root `case`, after [`LEAD`].
fn in_acp(content: &str) -> String {
    format!(
        "{LEAD}<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\r\n  <name content=\"Case\"/>\r\n  \
         <!-- \u{e9} --> {content}\r\n</sacm:AssuranceCasePackage>"
    )
}

/// `content` in an XMI wrapper, after [`LEAD`].
fn wrapped(content: &str) -> String {
    format!("{LEAD}{WRAPPER_OPEN}\r\n{content}\r\n</xmi:XMI>")
}

/// A named Claim with no `xmi:id`, marked at its start tag.
fn idless_claim(name: &str) -> String {
    format!(
        "{MARK}<argumentElement xmi:type=\"sacm:Claim\"><name content=\"{name}\"/></argumentElement>"
    )
}

/// A named Claim with the `xmi:id` attribute value `id`, unmarked.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/></argumentElement>"
    )
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

/// `xmi::read` in `mode`, on a worker thread. Fails the test when the read
/// panics or does not return within [`DEADLINE`].
fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    let owned = input.as_bytes().to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default().with_mode(mode)));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: a {mode:?} read of {} bytes did not return within {DEADLINE:?}",
            input.len()
        ),
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: read panicked on {input:?}"),
    }
}

fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: {mode:?} read refused the input: {error:?} ({error})"),
    }
}

fn refused(what: &str, mode: Mode, input: &str) -> ReadError {
    match read(mode, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: {mode:?} read returned a document of {} elements",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// The SACM-XMI-004 diagnostics of `output`.
fn missing_id_infos(output: &ReadOutput) -> Vec<&Diagnostic> {
    output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == Code::XMI_MISSING_ID)
        .collect()
}

/// Asserts that `found` is the SACM-XMI-004 Info of the element of `kind`
/// given the id `id`, located at `at`.
fn assert_missing_id_info(
    what: &str,
    found: &Diagnostic,
    (id, kind): (&str, ElementKind),
    at: SourceLocation,
) {
    assert_eq!(
        found.code(),
        Code::XMI_MISSING_ID,
        "{ROW}: {what}: the code"
    );
    assert_eq!(
        found.code().as_str(),
        "SACM-XMI-004",
        "{ROW}: {what}: the code is not SACM-XMI-004"
    );
    assert_eq!(
        found.severity(),
        Severity::Info,
        "{ROW}: {what}: not an Info"
    );
    assert_eq!(
        found.requirement_id(),
        ROW,
        "{ROW}: {what}: the requirement"
    );
    assert_eq!(found.clause(), Some("7.6.1"), "{ROW}: {what}: the clause");
    assert_eq!(
        found.affected(),
        [ElementId::from(id)],
        "{ROW}: {what}: affected is not [the generated id]"
    );
    assert_eq!(
        found.location(),
        Some(at),
        "{ROW}: {what}: not at the element's start tag"
    );
    let message = found.message();
    assert!(
        message.contains(kind.name()) && message.contains(&format!("{id:?}")),
        "{ROW}: {what}: the message {message:?} does not name the kind {kind} and the id {id:?}"
    );
}

/// Reads `template` in both modes and asserts that each marked element is
/// read with the generated id given for it, in `expected` order, with exactly
/// one SACM-XMI-004 each, in document order, and no other diagnostic.
/// Returns the strict document.
fn assert_generated(what: &str, template: &str, expected: &[(&str, ElementKind)]) -> Document {
    let (input, marks) = marked(template);
    assert_eq!(
        marks.len(),
        expected.len(),
        "{ROW}: setup: {what}: a mark per id-less element"
    );
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read_ok(what, mode, &input);
        assert_eq!(
            output.diagnostics().len(),
            expected.len(),
            "{ROW}: {what} ({mode:?}): not one diagnostic per id-less element: {:?}",
            output.diagnostics()
        );
        for ((found, &id_and_kind), &at) in output.diagnostics().iter().zip(expected).zip(&marks) {
            assert_missing_id_info(&format!("{what} ({mode:?})"), found, id_and_kind, at);
        }
        for &(id, kind) in expected {
            let element = output.document().get(id);
            assert_eq!(
                element.map(SacmElement::kind),
                Some(kind),
                "{ROW}: {what} ({mode:?}): no {kind} with the generated id {id:?}"
            );
        }
        let findings = validation::validate(output.document());
        assert!(
            findings.is_empty(),
            "{ROW}: {what} ({mode:?}): validate reports a generated id: {findings:?}"
        );
        documents.push(output.into_document());
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn ids_of(doc: &Document) -> Vec<String> {
    doc.elements()
        .map(|element| element.id().as_str().to_owned())
        .collect()
}

// --- Item 1 and 2: roots and children, with their Info ------------------------

#[test]
fn sacm23_xmi_020_an_id_less_bare_root_reads_with_a_generated_id_and_one_info() {
    for (kind, id) in [
        (ElementKind::ArgumentPackage, "argument_package_1"),
        (
            ElementKind::AssuranceCasePackage,
            "assurance_case_package_1",
        ),
    ] {
        let template =
            format!("{LEAD}{MARK}<sacm:{kind}{NS}>\r\n  <name content=\"R\"/>\r\n</sacm:{kind}>");
        let doc = assert_generated(&format!("a bare {kind}"), &template, &[(id, kind)]);
        assert_eq!(
            doc.roots()
                .map(|root| root.id().as_str().to_owned())
                .collect::<Vec<_>>(),
            [id],
            "{ROW}: a bare {kind}: the roots"
        );
    }
}

#[test]
fn sacm23_xmi_020_an_id_less_root_in_a_wrapper_reads_with_a_generated_id() {
    let template = wrapped(&format!(
        "  <!-- \u{e9} --> {MARK}<sacm:ArgumentPackage><name content=\"A\"/></sacm:ArgumentPackage>"
    ));
    assert_generated(
        "a wrapped root",
        &template,
        &[("argument_package_1", ElementKind::ArgumentPackage)],
    );
}

#[test]
fn sacm23_xmi_020_an_id_less_child_reads_with_a_generated_id_under_its_parent() {
    let cases = [
        (
            "a Claim in argumentElement",
            in_ap(&idless_claim("C")),
            ("claim_1", ElementKind::Claim),
            "p",
        ),
        (
            "an ArgumentPackage in argumentElement",
            in_ap(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:ArgumentPackage\"><name content=\"A\"/></argumentElement>"
            )),
            ("argument_package_1", ElementKind::ArgumentPackage),
            "p",
        ),
        (
            "an ArgumentPackage in argumentPackage",
            in_acp(&format!(
                "{MARK}<argumentPackage><name content=\"A\"/></argumentPackage>"
            )),
            ("argument_package_1", ElementKind::ArgumentPackage),
            "case",
        ),
        (
            "an AssuranceCasePackage in assuranceCasePackage",
            in_acp(&format!(
                "{MARK}<assuranceCasePackage><name content=\"S\"/></assuranceCasePackage>"
            )),
            (
                "assurance_case_package_1",
                ElementKind::AssuranceCasePackage,
            ),
            "case",
        ),
    ];
    for (what, template, generated, parent) in &cases {
        let doc = assert_generated(what, template, &[*generated]);
        assert_eq!(
            doc.get(generated.0)
                .and_then(SacmElement::parent)
                .map(|found| found.id().as_str().to_owned()),
            Some((*parent).to_owned()),
            "{ROW}: {what}: the parent"
        );
    }
}

/// Id-less roots and children at once, with one explicit id among them.
fn everything_at_once() -> String {
    wrapped(&format!(
        "  {MARK}<sacm:ArgumentPackage><name content=\"A\"/>\r\n    {}\r\n    {}\r\n    {}\r\n  \
         </sacm:ArgumentPackage>\r\n  \
         {MARK}<sacm:AssuranceCasePackage><name content=\"Case\"/>\r\n    \
         {MARK}<argumentPackage><name content=\"Inner\"/>\r\n      {}\r\n    </argumentPackage>\r\n  \
         </sacm:AssuranceCasePackage>\r\n  \
         {MARK}<sacm:ArgumentPackage><name content=\"B\"/></sacm:ArgumentPackage>",
        idless_claim("C1"),
        claim("explicit"),
        idless_claim("C2"),
        idless_claim("C3"),
    ))
}

#[test]
fn sacm23_xmi_020_id_less_elements_everywhere_at_once_each_get_one_info_in_document_order() {
    let expected = [
        ("argument_package_1", ElementKind::ArgumentPackage),
        ("claim_1", ElementKind::Claim),
        ("claim_2", ElementKind::Claim),
        (
            "assurance_case_package_1",
            ElementKind::AssuranceCasePackage,
        ),
        ("argument_package_2", ElementKind::ArgumentPackage),
        ("claim_3", ElementKind::Claim),
        ("argument_package_3", ElementKind::ArgumentPackage),
    ];
    let doc = assert_generated("everything at once", &everything_at_once(), &expected);
    assert_eq!(
        ids_of(&doc),
        [
            "argument_package_1",
            "claim_1",
            "explicit",
            "claim_2",
            "assurance_case_package_1",
            "argument_package_2",
            "claim_3",
            "argument_package_3",
        ],
        "{ROW}: everything at once: the elements in document order"
    );
    for mode in MODES {
        let (input, _) = marked(&everything_at_once());
        let output = read_ok("everything at once", mode, &input);
        let mut sorted = output.diagnostics().to_vec();
        sorted.sort();
        assert_eq!(
            output.diagnostics(),
            sorted.as_slice(),
            "{ROW}: everything at once ({mode:?}): the diagnostics are not in order"
        );
    }
}

// --- Item 3: determinism -----------------------------------------------------

#[test]
fn sacm23_xmi_020_two_reads_of_the_same_bytes_give_equal_documents_and_identical_output() {
    let (input, _) = marked(&everything_at_once());
    for mode in MODES {
        let first = read_ok("the first read", mode, &input);
        let second = read_ok("the second read", mode, &input);
        assert_eq!(
            first.document(),
            second.document(),
            "{ROW}: two {mode:?} reads differ"
        );
        assert_eq!(
            ids_of(first.document()),
            ids_of(second.document()),
            "{ROW}: two {mode:?} reads give other ids"
        );
        assert_eq!(
            first.diagnostics(),
            second.diagnostics(),
            "{ROW}: two {mode:?} reads report differently"
        );
        let write = |doc: &Document| match xmi::write(doc, &WriteOptions::default()) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: a strict write refused the document: {error:?}"),
        };
        assert_eq!(
            write(first.document()),
            write(second.document()),
            "{ROW}: the two {mode:?} reads write different bytes"
        );
    }
}

// --- Item 4 and 5: no clash, and no duplicate --------------------------------

#[test]
fn sacm23_xmi_020_a_later_explicit_id_is_never_generated() {
    let cases = [
        (
            "a later Claim claim_1",
            in_ap(&format!("{}{}", idless_claim("C"), claim("claim_1"))),
            "claim_2",
        ),
        (
            "a later padded claim_1",
            in_ap(&format!("{}{}", idless_claim("C"), claim(" claim_1 "))),
            "claim_2",
        ),
        (
            "a later name id claim_1",
            in_ap(&format!(
                "{}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\">\
                 <name xmi:id=\"claim_1\" content=\"C\"/></argumentElement>",
                idless_claim("C")
            )),
            "claim_2",
        ),
        (
            "later Claims claim_2 and claim_1",
            in_ap(&format!(
                "{}{}{}",
                idless_claim("C"),
                claim("claim_2"),
                claim("claim_1")
            )),
            "claim_3",
        ),
    ];
    for (what, template, generated) in &cases {
        let (input, marks) = marked(template);
        for mode in MODES {
            let output = read_ok(what, mode, &input);
            let infos = missing_id_infos(&output);
            let [info] = infos.as_slice() else {
                panic!("{ROW}: {what} ({mode:?}): not one SACM-XMI-004: {infos:?}");
            };
            assert_missing_id_info(what, info, (*generated, ElementKind::Claim), marks[0]);
            assert_eq!(
                output.document().get(*generated).map(SacmElement::kind),
                Some(ElementKind::Claim),
                "{ROW}: {what} ({mode:?}): the id-less Claim is not {generated:?}"
            );
        }
    }
    // A later root, too.
    let template = wrapped(&format!(
        "  {MARK}<sacm:ArgumentPackage><name content=\"A\"/></sacm:ArgumentPackage>\r\n  \
         <sacm:ArgumentPackage xmi:id=\"argument_package_1\"><name content=\"B\"/></sacm:ArgumentPackage>"
    ));
    assert_generated(
        "a later root argument_package_1",
        &template,
        &[("argument_package_2", ElementKind::ArgumentPackage)],
    );
}

#[test]
fn sacm23_xmi_020_a_generated_id_never_takes_part_in_a_duplicate_id() {
    let cases = [
        (
            "two explicit c1 among id-less Claims",
            in_ap(&format!(
                "{}{MARK}{}{}{MARK}{}",
                idless_claim("A").replace(MARK, ""),
                claim("c1"),
                idless_claim("B").replace(MARK, ""),
                claim("c1")
            )),
            "c1",
        ),
        (
            "two explicit claim_1 after an id-less Claim",
            in_ap(&format!(
                "{}{MARK}{}{MARK}{}",
                idless_claim("A").replace(MARK, ""),
                claim("claim_1"),
                claim("claim_1")
            )),
            "claim_1",
        ),
    ];
    for (what, template, id) in &cases {
        let (input, marks) = marked(template);
        for mode in MODES {
            let error = refused(what, mode, &input);
            assert!(
                matches!(&error, ReadError::DuplicateId { id: found, .. } if found.as_str() == *id),
                "{ROW}: {what} ({mode:?}): not DuplicateId for {id:?}: {error:?}"
            );
            assert_eq!(
                error.locations(),
                marks.as_slice(),
                "{ROW}: {what} ({mode:?}): the locations are not only the explicit ones"
            );
        }
    }
}

// --- Item 6: write back ------------------------------------------------------

#[test]
fn sacm23_xmi_020_write_writes_the_generated_id_and_it_reads_back_equal_without_the_info() {
    let input = format!(
        "{LEAD}<sacm:ArgumentPackage{NS}>\r\n  <name content=\"P\"/>\r\n  \
         <argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>\r\n\
         </sacm:ArgumentPackage>"
    );
    for mode in MODES {
        let doc = read_ok("an id-less root and Claim", mode, &input).into_document();
        let text = match xmi::write(&doc, &WriteOptions::default()) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: a strict save of the read document failed: {error:?}"),
        };
        for id in ["argument_package_1", "claim_1"] {
            assert!(
                text.contains(&format!(" xmi:id=\"{id}\"")),
                "{ROW}: the output does not write the generated id {id:?}:\n{text}"
            );
        }
        let back = read_ok("the output", mode, &text);
        assert!(
            back.diagnostics().is_empty(),
            "{ROW}: reading the output reports {:?}",
            back.diagnostics()
        );
        assert_eq!(
            back.document(),
            &doc,
            "{ROW}: the output reads back different"
        );
    }
}

#[test]
fn sacm23_xmi_020_a_generated_id_is_unmarked_and_compares_like_a_written_one() {
    let idless = in_ap(&idless_claim("C").replace(MARK, ""));
    let written = in_ap(&claim("claim_1"));
    let other = in_ap(&claim("c1"));
    let read_doc = |input: &str| read_ok("a document", Mode::Tolerant, input).into_document();
    let (idless, written, other) = (read_doc(&idless), read_doc(&written), read_doc(&other));
    assert_eq!(
        semantic_compare(&idless, &written),
        [],
        "{ROW}: a generated claim_1 differs from a written claim_1"
    );
    assert_eq!(idless, written, "{ROW}: a generated id is marked");
    assert_eq!(
        semantic_compare(&idless, &other),
        [
            Difference::LeftOnly {
                id: ElementId::from("claim_1"),
                kind: ElementKind::Claim
            },
            Difference::RightOnly {
                id: ElementId::from("c1"),
                kind: ElementKind::Claim
            },
        ],
        "{ROW}: a generated id is not compared by its value"
    );
}

// --- Item 7: edits after the read --------------------------------------------

fn create(doc: &mut Document, operation: impl Into<edit::Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => mutation.created_ids()[0].clone(),
        Err(error) => panic!("{ROW}: an edit after the read was refused: {error}"),
    }
}

#[test]
fn sacm23_xmi_020_edits_after_the_read_never_reuse_a_generated_id() {
    let cases = [
        ("one id-less Claim", in_ap(&idless_claim("C")), "claim_2"),
        (
            "an id-less Claim before claim_1",
            in_ap(&format!("{}{}", idless_claim("C"), claim("claim_1"))),
            "claim_3",
        ),
    ];
    for (what, template, next) in &cases {
        let (input, _) = marked(template);
        let mut doc = read_ok(what, Mode::Tolerant, &input).into_document();
        let created = create(&mut doc, CreateClaim::new("p", LangString::new("New")));
        assert_eq!(
            created.as_str(),
            *next,
            "{ROW}: {what}: the next generated Claim id"
        );
    }
    let mut doc = read_ok(
        "an id-less root",
        Mode::Tolerant,
        &format!("<sacm:ArgumentPackage{NS}><name content=\"A\"/></sacm:ArgumentPackage>"),
    )
    .into_document();
    let created = create(&mut doc, CreateArgumentPackage::root(LangString::new("B")));
    assert_eq!(
        created.as_str(),
        "argument_package_2",
        "{ROW}: the next generated ArgumentPackage id"
    );
}

#[test]
fn sacm23_xmi_020_a_caller_id_equal_to_a_generated_one_is_duplicate_id() {
    let (input, _) = marked(&in_ap(&idless_claim("C")));
    let mut doc = read_ok("one id-less Claim", Mode::Tolerant, &input).into_document();
    let before = doc.clone();
    let error = match edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("Again")).with_id("claim_1"),
    ) {
        Ok(mutation) => panic!("{ROW}: creating claim_1 again succeeded: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "claim_1"),
        "{ROW}: creating claim_1 again is not DuplicateId: {error:?}"
    );
    assert_eq!(doc, before, "{ROW}: the refused edit changed the document");
}

// --- Item 8: names, unqualified ids, empty ids -------------------------------

#[test]
fn sacm23_xmi_020_no_id_is_generated_for_a_name() {
    let template = format!(
        "{LEAD}{MARK}<sacm:ArgumentPackage{NS}>\r\n  <name content=\"P\"/>\r\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/></argumentElement>\r\n\
         </sacm:ArgumentPackage>"
    );
    let doc = assert_generated(
        "a root and a Claim with names",
        &template,
        &[("argument_package_1", ElementKind::ArgumentPackage)],
    );
    for id in ["argument_package_1", "c1"] {
        assert_eq!(
            doc.get(id).and_then(SacmElement::name_id),
            None,
            "{ROW}: the name of {id:?} was given an id"
        );
    }
    assert_eq!(doc.element_count(), 2, "{ROW}: a name became an element");
}

#[test]
fn sacm23_xmi_020_an_unqualified_id_is_not_an_xmi_id_and_stays_refused_at_its_name() {
    let cases = [
        (
            "an id on a root",
            format!(
                "{LEAD}<sacm:ArgumentPackage{NS} {MARK}id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>"
            ),
        ),
        (
            "an id on a Claim",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" {MARK}id=\"c1\"><name content=\"C\"/></argumentElement>"
            )),
        ),
        (
            "an id in a foreign namespace on a Claim",
            in_ap(&format!(
                "<argumentElement xmlns:o=\"urn:o\" xmi:type=\"sacm:Claim\" {MARK}o:id=\"c1\">\
                 <name content=\"C\"/></argumentElement>"
            )),
        ),
    ];
    for (what, template) in &cases {
        let (input, marks) = marked(template);
        for mode in MODES {
            let error = refused(what, mode, &input);
            assert!(
                matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
                "{ROW}: {what} ({mode:?}): expected Unsupported, got {error:?}"
            );
            assert_eq!(
                error.location(),
                Some(marks[0]),
                "{ROW}: {what} ({mode:?}): not refused at the attribute's name: {error}"
            );
        }
    }
}

#[test]
fn sacm23_xmi_020_an_empty_xmi_id_stays_invalid_id() {
    let (input, marks) = marked(&in_ap(&format!(
        "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"><name content=\"C\"/></argumentElement>"
    )));
    for mode in MODES {
        let error = refused("an empty xmi:id", mode, &input);
        assert!(
            matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
            "{ROW}: an empty xmi:id ({mode:?}) is not InvalidId: {error:?}"
        );
        assert_eq!(
            error.location(),
            Some(marks[0]),
            "{ROW}: an empty xmi:id ({mode:?}): the location"
        );
    }
}

// --- Item 9: linear time -----------------------------------------------------

#[test]
fn sacm23_xmi_020_ten_thousand_id_less_claims_read_within_the_deadline() {
    const COUNT: usize = 10_000;
    let idless =
        "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>\n";
    // Alone, and followed by explicit ids claim_1 to claim_10000, so that
    // generation must skip them all without rescanning.
    let explicit: String = (1..=COUNT)
        .map(|n| claim(&format!("claim_{n}")) + "\n")
        .collect();
    for (what, later, first, last) in [
        ("alone", String::new(), "claim_1", "claim_10000"),
        (
            "before claim_1 to claim_10000",
            explicit,
            "claim_10001",
            "claim_20000",
        ),
    ] {
        let input = format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\n{}{later}</sacm:ArgumentPackage>",
            idless.repeat(COUNT)
        );
        let output = read_ok(what, Mode::Tolerant, &input);
        let doc = output.document();
        let children: Vec<&str> = doc
            .get("p")
            .map(|p| p.children().map(|child| child.id().as_str()).collect())
            .unwrap_or_default();
        assert_eq!(
            (children.first().copied(), children.get(COUNT - 1).copied()),
            (Some(first), Some(last)),
            "{ROW}: {what}: the first and last generated ids"
        );
        assert_eq!(
            missing_id_infos(&output).len(),
            COUNT,
            "{ROW}: {what}: not one SACM-XMI-004 per id-less Claim"
        );
    }
}

// --- Item 10: the catalog and the decisions ----------------------------------

fn doc_text(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

#[test]
fn sacm23_xmi_020_the_catalog_xmi_004_row_is_info_and_names_the_code_and_the_row() {
    assert_eq!(
        Code::XMI_MISSING_ID.as_str(),
        "SACM-XMI-004",
        "{ROW}: Code::XMI_MISSING_ID is not SACM-XMI-004"
    );
    let catalog = doc_text("../../docs/sacm/diagnostics.md");
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("| SACM-XMI-004 |"))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: the catalog has {} SACM-XMI-004 rows, not one",
            rows.len()
        );
    };
    assert!(
        line.starts_with("| SACM-XMI-004 | Info |"),
        "{ROW}: the SACM-XMI-004 row is not Info: {line}"
    );
    for part in ["`Code::XMI_MISSING_ID`", "SACM23-XMI-020"] {
        assert!(
            line.contains(part),
            "{ROW}: the SACM-XMI-004 row does not name {part}: {line}"
        );
    }
}

#[test]
fn sacm23_xmi_020_d_no_id_is_a_decision_and_q_no_id_is_no_longer_open() {
    let decisions = doc_text("../../docs/sacm/decisions.md");
    let starts = |prefix: &str| {
        decisions
            .lines()
            .any(|line| line.trim_start().starts_with(prefix))
    };
    assert!(
        starts("| D-NO-ID |"),
        "{ROW}: decisions.md has no D-NO-ID row"
    );
    assert!(
        !starts("| Q-NO-ID |"),
        "{ROW}: decisions.md still lists Q-NO-ID as open"
    );
}
