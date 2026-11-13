//! Tests for an `xmi:id` with white space around it (SACM23-XMI-025; XMI
//! 2.5.1 §7.6.1 `type="xsd:ID"`; XML Schema 1.0 Part 2 §3.3.8, §4.3.6; XML
//! 1.0 §3.3.3; D-ID; ADR 0024).
//!
//! An `xmi:id`, on an element or a `name`, whose value after attribute-value
//! normalisation is not an `NCName` but whose value with XML white space (#x20,
//! #x9, #xA, #xD) trimmed at both ends is one, is read as the trimmed value:
//! the document identifies, compares, deduplicates and writes the element by
//! it. Each is reported once as Info SACM-ID-004 at the attribute's name,
//! quoting the value as read. Every other id is held as written.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading, and are counted here: lines end at LF,
//! CRLF or a lone CR, and columns count characters.

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-XMI-025";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// How long one read may take, in debug, for the largest input here.
const DEADLINE: Duration = Duration::from_secs(5);

/// Marks, in a test input, where an attribute, a start tag or a refusal is
/// expected.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// A declaration ended by CRLF, then a comment holding `é` and two spaces.
const LEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!-- \u{e9} -->  ";

/// Each padded way of writing `c1`, and its value as read after XML 1.0
/// §3.3.3 normalisation (a literal TAB, LF or CR is a space; a character
/// reference is kept).
const PADDED: [(&str, &str); 7] = [
    (" c1 ", " c1 "),
    ("c1 ", "c1 "),
    ("  c1", "  c1"),
    ("c1&#9;", "c1\t"),
    ("&#xA;c1&#xD;", "\nc1\r"),
    ("\tc1", " c1"),
    ("c1\n", "c1 "),
];

// --- Inputs ------------------------------------------------------------------

/// A named ArgumentPackage root whose `xmi:id` attribute, marked, holds `id`,
/// on line 3 after a CRLF and a namespace declaration whose prefix is `é`.
fn root_with(id: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS}\r\n  xmlns:\u{e9}=\"urn:e\" {MARK}xmi:id=\"{id}\">\r\n  \
         <name content=\"P\"/>\r\n</sacm:ArgumentPackage>"
    )
}

/// `content` in a named ArgumentPackage root `p`, after [`LEAD`].
fn in_ap(content: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n  \
         <!-- \u{e9} --> {content}\r\n</sacm:ArgumentPackage>"
    )
}

/// A named Claim in [`in_ap`] whose `xmi:id` attribute, marked, holds `id`.
fn child_with(id: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" {MARK}xmi:id=\"{id}\"><name content=\"C\"/></argumentElement>"
    ))
}

/// A root `p` whose `name`'s `xmi:id` attribute, marked, holds `id`.
fn name_with(id: &str) -> String {
    format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <!-- \u{e9} --> <name {MARK}xmi:id=\"{id}\" \
         content=\"P\"/>\r\n</sacm:ArgumentPackage>"
    )
}

/// A named Claim with the `xmi:id` value `id`, its start tag marked.
fn marked_claim(id: &str) -> String {
    format!(
        "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/></argumentElement>"
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

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: {mode:?} read refused {input:?}: {error:?} ({error})"),
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

/// Asserts that `found` is the SACM-ID-004 Info for the element `affected`,
/// located at `at`, quoting `as_read`.
fn assert_trimmed_info(
    what: &str,
    found: &Diagnostic,
    affected: &str,
    as_read: &str,
    at: SourceLocation,
) {
    assert_eq!(found.code(), Code::ID_TRIMMED, "{ROW}: {what}: the code");
    assert_eq!(
        found.code().as_str(),
        "SACM-ID-004",
        "{ROW}: {what}: the code is not SACM-ID-004"
    );
    assert_eq!(found.severity(), Severity::Info, "{ROW}: {what}: not Info");
    assert_eq!(
        found.requirement_id(),
        ROW,
        "{ROW}: {what}: the requirement"
    );
    assert_eq!(found.clause(), Some("7.6.1"), "{ROW}: {what}: the clause");
    assert_eq!(
        found.affected(),
        [ElementId::from(affected)],
        "{ROW}: {what}: the affected id"
    );
    assert_eq!(
        found.location(),
        Some(at),
        "{ROW}: {what}: not at the xmi:id attribute's name"
    );
    let quoted = format!("{as_read:?}");
    assert!(
        found.message().contains(&quoted),
        "{ROW}: {what}: the message {:?} does not quote the value as read, {quoted}",
        found.message()
    );
}

/// The SACM-ID-002 findings of `validate`.
fn id_findings(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == Code::ID_INVALID)
        .collect()
}

// --- Item 1: read trimmed, with one Info --------------------------------------

#[test]
fn sacm23_xmi_025_a_padded_id_reads_trimmed_with_one_info_on_a_root_a_child_and_a_name() {
    for (written, as_read) in PADDED {
        let shown = written.escape_debug().to_string();
        let places = [
            ("a root", root_with(written), "c1"),
            ("a child", child_with(written), "c1"),
            ("a name", name_with(written), "p"),
        ];
        for (place, template, affected) in &places {
            let what = format!("xmi:id=\"{shown}\" on {place}");
            let (input, marks) = marked(template);
            let mut documents = Vec::new();
            for mode in MODES {
                let output = read_ok(&what, mode, &input);
                let [info] = output.diagnostics() else {
                    panic!(
                        "{ROW}: {what} ({mode:?}): not exactly one diagnostic: {:?}",
                        output.diagnostics()
                    );
                };
                assert_trimmed_info(&what, info, affected, as_read, marks[0]);
                let doc = output.document();
                if *place == "a name" {
                    assert_eq!(
                        doc.get("p")
                            .and_then(SacmElement::name_id)
                            .map(ElementId::as_str),
                        Some("c1"),
                        "{ROW}: {what} ({mode:?}): the name id is not trimmed"
                    );
                    assert!(
                        doc.get("c1").is_none(),
                        "{ROW}: {what}: a name id became an element"
                    );
                } else {
                    assert!(
                        doc.get("c1").is_some(),
                        "{ROW}: {what} ({mode:?}): the element is not held as c1"
                    );
                }
                assert!(
                    doc.get(as_read).is_none() && doc.get(written).is_none(),
                    "{ROW}: {what} ({mode:?}): the untrimmed id is held"
                );
                assert!(
                    id_findings(doc).is_empty(),
                    "{ROW}: {what} ({mode:?}): validate reports the trimmed id"
                );
                documents.push(output.into_document());
            }
            assert_eq!(
                documents[0], documents[1],
                "{ROW}: {what}: the modes read different documents"
            );
        }
    }
}

// --- Item 2: duplicates after trimming ----------------------------------------

#[test]
fn sacm23_xmi_025_ids_equal_after_trimming_are_duplicate_id() {
    for (first, second) in [(" c1 ", "c1"), ("c1", " c1 "), (" c1", "c1 ")] {
        let what = format!("{first:?} beside {second:?}");
        let (input, marks) = marked(&in_ap(&format!(
            "{}\r\n  {}",
            marked_claim(first),
            marked_claim(second)
        )));
        for mode in MODES {
            let error = refused(&what, mode, &input);
            assert!(
                matches!(&error, ReadError::DuplicateId { id, .. } if id.as_str() == "c1"),
                "{ROW}: {what} ({mode:?}): not DuplicateId for \"c1\": {error:?}"
            );
            assert_eq!(
                error.locations(),
                marks.as_slice(),
                "{ROW}: {what} ({mode:?}): not both start tags"
            );
        }
    }
}

// --- Item 3 and 4: what is not trimmed ----------------------------------------

#[test]
fn sacm23_xmi_025_characters_that_are_not_xml_white_space_are_not_trimmed() {
    // U+00A0, U+2003 and U+200B cannot start or continue an NCName, so
    // validate reports the id. U+FEFF can start one: it is in the
    // NameStartChar range [#xFDF0-#xFFFD] of XML 1.0 (Fifth Edition) §2.3, so
    // "\u{FEFF}c1" is an NCName and validate reports nothing for it.
    for (written, held, not_an_ncname) in [
        ("&#xA0;c1", "\u{A0}c1", true),
        ("c1&#x2003;", "c1\u{2003}", true),
        ("&#x200B;c1", "\u{200B}c1", true),
        ("&#xFEFF;c1", "\u{FEFF}c1", false),
    ] {
        let (input, _) = marked(&child_with(written));
        for mode in MODES {
            let output = read_ok(written, mode, &input);
            assert!(
                output.diagnostics().is_empty(),
                "{ROW}: {written} ({mode:?}): reported {:?}",
                output.diagnostics()
            );
            let doc = output.document();
            assert!(
                doc.get(held).is_some() && doc.get("c1").is_none(),
                "{ROW}: {written} ({mode:?}): not held as written"
            );
            let affected: Vec<Vec<ElementId>> = id_findings(doc)
                .iter()
                .map(|found| found.affected().to_vec())
                .collect();
            let expected = if not_an_ncname {
                vec![vec![ElementId::from(held)]]
            } else {
                Vec::new()
            };
            assert_eq!(
                affected, expected,
                "{ROW}: {written} ({mode:?}): the SACM-ID-002 findings"
            );
        }
    }
}

#[test]
fn sacm23_xmi_025_an_id_still_not_an_ncname_after_trimming_is_held_as_written() {
    for written in [" 1c ", " a b ", "   "] {
        let (input, _) = marked(&child_with(written));
        for mode in MODES {
            let output = read_ok(written, mode, &input);
            assert!(
                output
                    .diagnostics()
                    .iter()
                    .all(|found| found.code() != Code::ID_TRIMMED),
                "{ROW}: {written:?} ({mode:?}): reported as trimmed: {:?}",
                output.diagnostics()
            );
            let doc = output.document();
            assert!(
                doc.get(written).is_some(),
                "{ROW}: {written:?} ({mode:?}): not held as written"
            );
            let affected: Vec<Vec<ElementId>> = id_findings(doc)
                .iter()
                .map(|found| found.affected().to_vec())
                .collect();
            assert_eq!(
                affected,
                [vec![ElementId::from(written)]],
                "{ROW}: {written:?} ({mode:?}): not one SACM-ID-002"
            );
        }
    }
    let template = in_ap(&format!(
        "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>"
    ));
    let (input, marks) = marked(&template);
    for mode in MODES {
        let error = refused("an empty id", mode, &input);
        assert!(
            matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
            "{ROW}: an empty id ({mode:?}) is not InvalidId: {error:?}"
        );
        assert_eq!(
            error.location(),
            Some(marks[0]),
            "{ROW}: an empty id: the location"
        );
    }
}

// --- Item 5: write back --------------------------------------------------------

#[test]
fn sacm23_xmi_025_write_writes_the_trimmed_id_and_it_reads_back_without_the_info() {
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\" p \">\n  <name xmi:id=\"&#9;n1\" content=\"P\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1&#xA;\"><name content=\"C\"/></argumentElement>\n\
         </sacm:ArgumentPackage>"
    );
    for mode in MODES {
        let doc = read_ok("three padded ids", mode, &input).into_document();
        let text = match xmi::write(&doc, &WriteOptions::default()) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: a strict save of the read document failed: {error:?}"),
        };
        for id in ["p", "n1", "c1"] {
            assert!(
                text.contains(&format!(" xmi:id=\"{id}\"")),
                "{ROW}: the output does not write {id:?} trimmed:\n{text}"
            );
        }
        assert!(
            !text.contains("&#9;") && !text.contains("&#10;") && !text.contains("\" p \""),
            "{ROW}: the output keeps the white space:\n{text}"
        );
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

// --- Item 6: edits -------------------------------------------------------------

#[test]
fn sacm23_xmi_025_after_reading_a_padded_id_a_caller_id_equal_to_the_trimmed_one_is_duplicate_id() {
    let (input, _) = marked(&child_with(" c1 "));
    let mut doc = read_ok("a padded Claim id", Mode::Tolerant, &input).into_document();
    let before = doc.clone();
    let refusals = [
        edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("Again")).with_id("c1"),
        ),
        edit::apply(
            &mut doc,
            CreateArgumentPackage::root(LangString::new("Again")).with_id("c1"),
        ),
    ];
    for result in refusals {
        let error = match result {
            Ok(mutation) => panic!("{ROW}: creating c1 again succeeded: {mutation:?}"),
            Err(error) => error,
        };
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
            "{ROW}: creating c1 again is not DuplicateId: {error:?}"
        );
    }
    assert_eq!(doc, before, "{ROW}: a refused edit changed the document");
}

// --- Item 7: the catalog -------------------------------------------------------

#[test]
fn sacm23_xmi_025_the_catalog_id_004_row_names_the_code_and_the_row() {
    assert_eq!(
        Code::ID_TRIMMED.as_str(),
        "SACM-ID-004",
        "{ROW}: Code::ID_TRIMMED is not SACM-ID-004"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("| SACM-ID-004 |"))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: the catalog has {} SACM-ID-004 rows, not one",
            rows.len()
        );
    };
    assert!(
        line.starts_with("| SACM-ID-004 | Info |"),
        "{ROW}: the SACM-ID-004 row is not Info: {line}"
    );
    for part in ["`Code::ID_TRIMMED`", "SACM23-XMI-025"] {
        assert!(
            line.contains(part),
            "{ROW}: the SACM-ID-004 row does not name {part}: {line}"
        );
    }
}

// --- Linear time -----------------------------------------------------------------

/// `xmi::read` in `mode`, on a worker thread. Fails the test when the read
/// panics or does not return within [`DEADLINE`].
fn read_within_deadline(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
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
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: the read panicked"),
    }
}

/// The diagnostic with `code` that affects exactly `id`.
fn found_for<'o>(output: &'o ReadOutput, code: Code, id: &str) -> Option<&'o Diagnostic> {
    output
        .diagnostics()
        .iter()
        .find(|found| found.code() == code && found.affected() == [ElementId::from(id)])
}

#[test]
fn sacm23_xmi_025_many_padded_ids_after_xsi_type_are_located_in_linear_time() {
    // Each Claim's xsi:type (SACM-XMI-010) comes before its padded xmi:id
    // (SACM-ID-004) on the same start tag, so a locator that is asked for the
    // id first and then rescans the input from the start for the earlier
    // xsi:type is quadratic in the number of Claims.
    const COUNT: usize = 20_000;
    let claims = (1..=COUNT)
        .map(|n| {
            format!(
                "  <argumentElement {MARK}xsi:type=\"sacm:Claim\" {MARK}xmi:id=\" c{n} \"/>\r\n"
            )
        })
        .collect::<Vec<String>>()
        .concat();
    let template = format!(
        "{LEAD}<sacm:ArgumentPackage{NS} xmi:id=\"p\">\r\n  <name content=\"P\"/>\r\n{claims}\
         </sacm:ArgumentPackage>"
    );
    let (input, marks) = marked(&template);
    assert_eq!(marks.len(), 2 * COUNT, "{ROW}: the test input's marks");
    let started = Instant::now();
    let output = match read_within_deadline(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: a tolerant read refused {COUNT} Claims: {error:?} ({error})"),
    };
    let elapsed = started.elapsed();
    let count_of = |code: Code| {
        output
            .diagnostics()
            .iter()
            .filter(|found| found.code() == code)
            .count()
    };
    assert_eq!(
        count_of(Code::ID_TRIMMED),
        COUNT,
        "{ROW}: not one SACM-ID-004 per Claim (read in {elapsed:?})"
    );
    assert_eq!(
        count_of(Code::XMI_XSI_TYPE),
        COUNT,
        "{ROW}: not one SACM-XMI-010 per Claim (read in {elapsed:?})"
    );
    assert_eq!(
        output.diagnostics().len(),
        2 * COUNT,
        "{ROW}: diagnostics other than SACM-ID-004 and SACM-XMI-010"
    );
    for n in [1, 2, COUNT / 2, COUNT - 1, COUNT] {
        let id = format!("c{n}");
        let (type_at, id_at) = (marks[2 * (n - 1)], marks[2 * (n - 1) + 1]);
        let Some(info) = found_for(&output, Code::ID_TRIMMED, &id) else {
            panic!("{ROW}: no SACM-ID-004 for {id}");
        };
        assert_trimmed_info(&format!("Claim {id}"), info, &id, &format!(" {id} "), id_at);
        let Some(typed) = found_for(&output, Code::XMI_XSI_TYPE, &id) else {
            panic!("{ROW}: no SACM-XMI-010 for {id}");
        };
        assert_eq!(
            typed.severity(),
            Severity::Warning,
            "{ROW}: Claim {id}: a tolerant xsi:type is not a Warning"
        );
        assert_eq!(
            typed.location(),
            Some(type_at),
            "{ROW}: Claim {id}: not at the xsi:type attribute's name"
        );
    }
    assert_eq!(
        output.document().get("p").map(|p| p.children().count()),
        Some(COUNT),
        "{ROW}: not every Claim is read"
    );
}
