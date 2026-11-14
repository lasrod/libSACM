//! Tests for the reference rules of `validation::validate`: dangling
//! references (SACM23-VAL-003, SACM-REF-001), the §8.2 citation rule
//! (SACM23-VAL-007, SACM-CITE-001) and the §8.2 abstract-form rules
//! (SACM23-VAL-008, SACM-ABS-001) (SACM 2.3 §8.2; XMI 2.5.1 rule 2j; D-SEV,
//! D-VAL-LAYER, D-PRESERVE; ADR 0022, ADR 0026).
//!
//! - SACM-REF-001: one Error per reference whose id names no element (an id
//!   absent from the document, or the id of a `name`, `content`, `key` or
//!   `value`), clause 8.2, affected `[referrer, missing id]`.
//! - SACM-CITE-001: one Error per element with a `citedElement` whose
//!   `isCitation` is not true (the §8.2 OCL), affected `[element]`.
//!   `isCitation` true alone is no finding.
//! - SACM-ABS-001: an Error for an element with an `abstractForm` whose own
//!   `isAbstract` is true, affected `[element]`; a Warning for a resolved
//!   target that is not abstract, and one for a target of another kind,
//!   affected `[element, target]`. A dangling target gives only SACM-REF-001.
//!
//! None of them has a location. A reference only comes from `xmi::read`
//! today (LIB-EDIT-008/009 will set them), so every document is read from
//! inline XMI, with every `ModelElement` named so that no other rule reports.
//! Messages are checked only for what they must name.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::model::{Document, ElementId};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const VAL_003: &str = "SACM23-VAL-003";
const VAL_007: &str = "SACM23-VAL-007";
const VAL_008: &str = "SACM23-VAL-008";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The invalid fixtures, one per row.
fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sacm23/validation/invalid")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read the fixture {}: {error}", path.display()))
}

// --- Inputs and helpers ------------------------------------------------------------

/// An ArgumentPackage root `p` named "P" holding `body`.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `id` named "C" with the start-tag `attributes`, holding `content`
/// after its name.
fn claim(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"C\"/>{content}</argumentElement>"
    )
}

/// An AssuranceCasePackage root `case` holding the ArgumentPackage `p`, which
/// holds `claims`, and beside it the ArgumentPackage `q` with the start-tag
/// `attributes`. (An ArgumentPackage holding both a package and Claims would
/// be SACM-PKG-001, SACM23-VAL-012.)
fn beside_package(claims: &str, attributes: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{claims}</argumentPackage>\
         <argumentPackage xmi:id=\"q\"{attributes}><name content=\"Q\"/></argumentPackage>\
         </sacm:AssuranceCasePackage>"
    )
}

/// One readable kind: what, the kind's name, and the input given the
/// start-tag attributes of the element `x` of that kind.
type Holder = (&'static str, &'static str, fn(&str) -> String);

/// Every readable kind, as the element `x` with the start-tag attributes
/// given, valid apart from them.
fn holders() -> [Holder; 7] {
    [
        (
            "an AssuranceCasePackage root",
            "AssuranceCasePackage",
            |attributes: &str| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}>\
                     <name content=\"X\"/></sacm:AssuranceCasePackage>"
                )
            },
        ),
        (
            "an ArgumentPackage root",
            "ArgumentPackage",
            |attributes: &str| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"x\"{attributes}>\
                     <name content=\"X\"/></sacm:ArgumentPackage>"
                )
            },
        ),
        ("a Claim", "Claim", |attributes: &str| {
            in_ap(&claim("x", attributes, ""))
        }),
        ("a Description", "Description", |attributes: &str| {
            in_ap(&format!(
                "<description xmi:id=\"x\"{attributes}><content><value content=\"d\"/></content>\
                 </description>"
            ))
        }),
        (
            "an ImplementationConstraint",
            "ImplementationConstraint",
            |attributes: &str| {
                in_ap(&format!(
                    "<implementationConstraint xmi:id=\"x\"{attributes}/>"
                ))
            },
        ),
        ("a Note", "Note", |attributes: &str| {
            in_ap(&format!("<note xmi:id=\"x\"{attributes}/>"))
        }),
        ("a TaggedValue", "TaggedValue", |attributes: &str| {
            in_ap(&format!(
                "<taggedValue xmi:id=\"x\"{attributes}><key><value content=\"k\"/></key>\
                 </taggedValue>"
            ))
        }),
    ]
}

/// Reads `input` in both modes, which must succeed with no read diagnostic
/// and give equal documents, and returns the strict one.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{row}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{row}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The findings of `validate` on `doc`, which must all be of the rows in
/// `rows`.
fn findings(row: &str, what: &str, doc: &Document, rows: &[&str]) -> Vec<Diagnostic> {
    let found = validation::validate(doc);
    for finding in &found {
        assert!(
            rows.contains(&finding.requirement_id()),
            "{row}: {what}: setup: another rule reports {finding:?}"
        );
    }
    found
}

/// The findings of `validate` on `doc` with `code`.
fn with_code(doc: &Document, code: Code) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == code)
        .collect()
}

/// Asserts the fields of a finding: `code`, `severity` and `row`, clause 8.2,
/// no location, exactly `affected` in that order, and a message naming each
/// of `named`.
fn assert_finding(
    row: &str,
    what: &str,
    found: &Diagnostic,
    (code, severity): (Code, Severity),
    affected: &[&str],
    named: &[&str],
) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (code, severity, row),
        "{row}: {what}: not a {severity:?} {code} of this row: {found:?}"
    );
    let affected: Vec<ElementId> = affected.iter().copied().map(ElementId::from).collect();
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        (affected.as_slice(), None, Some("8.2")),
        "{row}: {what}: the affected ids, location or clause"
    );
    let message = found.message();
    for part in named {
        assert!(
            message.contains(part),
            "{row}: {what}: the message {message:?} does not name {part:?}"
        );
    }
    assert!(
        !message.contains('\n'),
        "{row}: {what}: the message {message:?} is not one line"
    );
}

/// Reads `input`, validates it, and returns the one finding it must give.
fn one_finding(row: &str, what: &str, input: &str, rows: &[&str]) -> Diagnostic {
    let doc = read_ok(row, what, input);
    let found = findings(row, what, &doc, rows);
    let [finding] = found.as_slice() else {
        panic!("{row}: {what}: not one finding: {found:?}");
    };
    finding.clone()
}

/// Reads `input` and asserts that `validate` gives no finding.
fn no_finding(row: &str, what: &str, input: &str) {
    let found = validation::validate(&read_ok(row, what, input));
    assert!(found.is_empty(), "{row}: {what}: findings: {found:?}");
}

/// A strict write of `doc` must be `WriteError::Invalid` holding `expected`
/// among its Errors.
fn assert_strict_write_refuses(row: &str, what: &str, doc: &Document, expected: &[Diagnostic]) {
    let error = match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => panic!("{row}: {what}: a strict write returned:\n{text}"),
        Err(error) => error,
    };
    let WriteError::Invalid { diagnostics, .. } = &error else {
        panic!("{row}: {what}: the strict refusal is not Invalid: {error:?}");
    };
    for finding in expected {
        assert!(
            diagnostics.contains(finding),
            "{row}: {what}: the strict refusal does not hold {finding:?}: {diagnostics:?}"
        );
    }
}

/// A tolerant write of `doc` must succeed and read back equal; returns it.
fn tolerant_round_trip(row: &str, what: &str, doc: &Document) -> String {
    let text = match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a tolerant write refused: {error:?}"),
    };
    let back = read_ok(row, &format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the tolerant output does not read back equal\n{text}"
    );
    text
}

/// The catalog row of `code`, which must be exactly one.
fn catalog_row(row: &str, code: Code) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", code.as_str());
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&cell))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{row}: docs/sacm/diagnostics.md has {} rows for {code}, not one",
            rows.len()
        );
    };
    (*line).to_owned()
}

const REF: (Code, Severity) = (Code::REF_DANGLING, Severity::Error);
const CITE: (Code, Severity) = (Code::CITATION_INVALID, Severity::Error);
const ABS_ERROR: (Code, Severity) = (Code::ABSTRACT_FORM_INVALID, Severity::Error);
const ABS_WARNING: (Code, Severity) = (Code::ABSTRACT_FORM_INVALID, Severity::Warning);

// === SACM23-VAL-003 ================================================================

#[test]
fn sacm23_val_003_a_dangling_cited_element_is_one_error_naming_the_claim_the_end_and_the_target() {
    let row = VAL_003;
    let what = "a Claim with citedElement=\"nope\"";
    let finding = one_finding(
        row,
        what,
        &in_ap(&claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\"",
            "",
        )),
        &[VAL_003],
    );
    assert_finding(
        row,
        what,
        &finding,
        REF,
        &["c1", "nope"],
        &["Claim", "c1", "citedElement", "nope"],
    );
    assert!(
        diagnostic::has_errors(std::slice::from_ref(&finding)),
        "{row}: {what}: has_errors is false"
    );
}

#[test]
fn sacm23_val_003_a_dangling_abstract_form_is_one_error_naming_the_claim_the_end_and_the_target() {
    let row = VAL_003;
    let what = "a Claim with abstractForm=\"nope\"";
    let finding = one_finding(
        row,
        what,
        &in_ap(&claim("c1", " abstractForm=\"nope\"", "")),
        &[VAL_003],
    );
    assert_finding(
        row,
        what,
        &finding,
        REF,
        &["c1", "nope"],
        &["Claim", "c1", "abstractForm", "nope"],
    );
}

#[test]
fn sacm23_val_003_a_dangling_reference_on_every_readable_kind_names_the_kind() {
    let row = VAL_003;
    for (holder, kind, input_of) in holders() {
        for (end, attributes) in [
            ("citedElement", " isCitation=\"true\" citedElement=\"nope\""),
            ("abstractForm", " abstractForm=\"nope\""),
        ] {
            let what = format!("{end}=\"nope\" on {holder}");
            let finding = one_finding(row, &what, &input_of(attributes), &[VAL_003]);
            assert_finding(
                row,
                &what,
                &finding,
                REF,
                &["x", "nope"],
                &[kind, end, "nope"],
            );
        }
    }
}

#[test]
fn sacm23_val_003_the_fixture_gives_one_error_on_its_claim() {
    let row = VAL_003;
    let what = "the fixture";
    let finding = one_finding(
        row,
        what,
        &fixture("claim-cited-element-dangling.sacm.xmi"),
        &[VAL_003],
    );
    assert_finding(row, what, &finding, REF, &["c1", "c9"], &["c9"]);
}

#[test]
fn sacm23_val_003_the_message_names_the_end_s_declared_type() {
    let row = VAL_003;
    for (end, attributes) in [
        ("citedElement", " isCitation=\"true\" citedElement=\"nope\""),
        ("abstractForm", " abstractForm=\"nope\""),
    ] {
        let what = format!("a Claim with {end}=\"nope\"");
        let finding = one_finding(row, &what, &in_ap(&claim("c1", attributes, "")), &[VAL_003]);
        assert_finding(row, &what, &finding, REF, &["c1", "nope"], &[]);
        assert_eq!(
            finding.message(),
            format!(
                "Claim \"c1\" has the {end} \"nope\", which names no element of the document \
                 (the end's type is SACMElement)"
            ),
            "{row}: {what}: the message"
        );
    }
}

#[test]
fn sacm23_val_003_forward_cross_root_and_self_references_give_no_finding() {
    let row = VAL_003;
    let cases = [
        (
            "a forward reference",
            in_ap(&format!(
                "{}{}",
                claim("c1", " isCitation=\"true\" citedElement=\"c2\"", ""),
                claim("c2", "", "")
            )),
        ),
        (
            "a reference into another root",
            format!(
                "<xmi:XMI{NS}>\
                 <sacm:ArgumentPackage xmi:id=\"r1\"><name content=\"R1\"/>{}</sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"r2\" isAbstract=\"true\"><name content=\"R2\"/>\
                 </sacm:ArgumentPackage></xmi:XMI>",
                claim("c1", " isCitation=\"true\" citedElement=\"r2\"", "")
            ),
        ),
        (
            "a self-reference",
            in_ap(&claim("c1", " isCitation=\"true\" citedElement=\"c1\"", "")),
        ),
        (
            "a reference to the root",
            in_ap(&claim("c1", " isCitation=\"true\" citedElement=\"p\"", "")),
        ),
        (
            "a reference to a UtilityElement",
            in_ap(&claim(
                "c1",
                " isCitation=\"true\" citedElement=\"n\"",
                "<note xmi:id=\"n\"/>",
            )),
        ),
    ];
    for (what, input) in &cases {
        let doc = read_ok(row, what, input);
        assert_eq!(
            with_code(&doc, Code::REF_DANGLING),
            [],
            "{row}: {what}: a resolving reference is reported as dangling"
        );
    }
    no_finding(row, "a forward reference", &cases[0].1);
}

#[test]
fn sacm23_val_003_a_reference_to_a_name_content_key_or_value_id_dangles() {
    let row = VAL_003;
    let targets = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\
                   <name xmi:id=\"name2\" content=\"C\"/>\
                   <description xmi:id=\"d\"><content xmi:id=\"content1\">\
                   <value xmi:id=\"value1\" content=\"d\"/></content></description>\
                   <taggedValue xmi:id=\"t\"><key xmi:id=\"key1\"><value xmi:id=\"value2\" content=\"k\"/>\
                   </key></taggedValue></argumentElement>";
    for id in ["name2", "content1", "value1", "key1", "value2"] {
        let what = format!("citedElement naming the {id} id");
        let input = in_ap(&format!(
            "{}{targets}",
            claim(
                "c1",
                &format!(" isCitation=\"true\" citedElement=\"{id}\""),
                ""
            )
        ));
        let finding = one_finding(row, &what, &input, &[VAL_003]);
        assert_finding(row, &what, &finding, REF, &["c1", id], &[id]);
    }
}

#[test]
fn sacm23_val_003_each_dangling_end_and_each_referrer_is_its_own_finding() {
    let row = VAL_003;
    let what = "both ends of c1 dangling";
    let doc = read_ok(
        row,
        what,
        &in_ap(&claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\" abstractForm=\"gone\"",
            "",
        )),
    );
    let found = findings(row, what, &doc, &[VAL_003]);
    let mut affected: Vec<Vec<&str>> = found
        .iter()
        .map(|f| f.affected().iter().map(ElementId::as_str).collect())
        .collect();
    affected.sort();
    assert_eq!(
        affected,
        [vec!["c1", "gone"], vec!["c1", "nope"]],
        "{row}: {what}: not two findings: {found:?}"
    );
    let messages: Vec<&str> = found.iter().map(Diagnostic::message).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("citedElement") && m.contains("nope"))
            && messages
                .iter()
                .any(|m| m.contains("abstractForm") && m.contains("gone")),
        "{row}: {what}: the messages do not name each end with its target: {messages:?}"
    );

    let what = "both ends of c1 naming the same missing id";
    let doc = read_ok(
        row,
        what,
        &in_ap(&claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\" abstractForm=\"nope\"",
            "",
        )),
    );
    assert_eq!(
        findings(row, what, &doc, &[VAL_003]).len(),
        2,
        "{row}: {what}: not one finding per end"
    );

    let what = "c1 and c2 citing the same missing id";
    let doc = read_ok(
        row,
        what,
        &in_ap(&format!(
            "{}{}",
            claim("c1", " isCitation=\"true\" citedElement=\"nope\"", ""),
            claim("c2", " isCitation=\"true\" citedElement=\"nope\"", "")
        )),
    );
    let found = findings(row, what, &doc, &[VAL_003]);
    let affected: Vec<Vec<&str>> = found
        .iter()
        .map(|f| f.affected().iter().map(ElementId::as_str).collect())
        .collect();
    assert_eq!(
        affected,
        [vec!["c1", "nope"], vec!["c2", "nope"]],
        "{row}: {what}: not one finding per referrer, in order: {found:?}"
    );
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(sorted, found, "{row}: {what}: the findings are not sorted");
}

#[test]
fn sacm23_val_003_a_strict_write_refuses_and_a_tolerant_write_keeps_the_reference() {
    let row = VAL_003;
    for (end, attributes) in [
        ("citedElement", " isCitation=\"true\" citedElement=\"nope\""),
        ("abstractForm", " abstractForm=\"nope\""),
    ] {
        let what = format!("{end}=\"nope\"");
        let doc = read_ok(row, &what, &in_ap(&claim("c1", attributes, "")));
        let expected = findings(row, &what, &doc, &[VAL_003]);
        assert_eq!(expected.len(), 1, "{row}: {what}: setup: not one finding");
        assert_strict_write_refuses(row, &what, &doc, &expected);
        let before = doc.clone();
        let text = tolerant_round_trip(row, &what, &doc);
        assert!(
            text.contains(&format!(" {end}=\"nope\"")),
            "{row}: {what}: a tolerant write drops the reference:\n{text}"
        );
        assert_eq!(
            doc, before,
            "{row}: {what}: validating or writing changed the document"
        );
    }
}

#[test]
fn sacm23_val_003_ref_dangling_is_sacm_ref_001_and_its_catalog_row_names_it_and_the_row() {
    let row = VAL_003;
    assert_eq!(
        Code::REF_DANGLING.as_str(),
        "SACM-REF-001",
        "{row}: Code::REF_DANGLING is not SACM-REF-001"
    );
    let line = catalog_row(row, Code::REF_DANGLING);
    assert!(
        line.starts_with("| SACM-REF-001 | Error |"),
        "{row}: the catalog's SACM-REF-001 row is not an Error: {line}"
    );
    for part in ["`Code::REF_DANGLING`", "SACM23-VAL-003"] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-REF-001 row does not name {part}: {line}"
        );
    }
}

// === SACM23-VAL-007 ================================================================

#[test]
fn sacm23_val_007_a_cited_element_without_is_citation_true_is_one_error_on_the_element() {
    let row = VAL_007;
    for (what, flag) in [
        ("no isCitation", ""),
        ("isCitation=\"false\"", " isCitation=\"false\""),
        ("isCitation=\"0\"", " isCitation=\"0\""),
    ] {
        let what = format!("c2 citing c1 with {what}");
        let input = in_ap(&format!(
            "{}{}",
            claim("c1", "", ""),
            claim("c2", &format!("{flag} citedElement=\"c1\""), "")
        ));
        let finding = one_finding(row, &what, &input, &[VAL_007]);
        assert_finding(row, &what, &finding, CITE, &["c2"], &["c2"]);
    }
    // The element form of both, too.
    let what = "c2 citing c1 in the element form with <isCitation>false</isCitation>";
    let input = in_ap(&format!(
        "{}{}",
        claim("c1", "", ""),
        claim(
            "c2",
            "",
            "<isCitation>false</isCitation><citedElement xmi:idref=\"c1\"/>"
        )
    ));
    let finding = one_finding(row, what, &input, &[VAL_007]);
    assert_finding(row, what, &finding, CITE, &["c2"], &["c2"]);
}

#[test]
fn sacm23_val_007_the_fixture_gives_one_error_on_its_claim() {
    let row = VAL_007;
    let what = "the fixture";
    let finding = one_finding(
        row,
        what,
        &fixture("claim-cited-without-is-citation.sacm.xmi"),
        &[VAL_007],
    );
    assert_finding(row, what, &finding, CITE, &["c2"], &["c2"]);
}

#[test]
fn sacm23_val_007_is_citation_true_with_or_without_a_cited_element_gives_no_finding() {
    let row = VAL_007;
    for (what, attributes) in [
        (
            "isCitation=\"true\" with citedElement",
            " isCitation=\"true\" citedElement=\"c1\"",
        ),
        (
            "isCitation=\"1\" with citedElement",
            " isCitation=\"1\" citedElement=\"c1\"",
        ),
        ("isCitation=\"true\" alone", " isCitation=\"true\""),
        ("neither", ""),
        ("an abstractForm without isCitation", " abstractForm=\"c1\""),
    ] {
        let input = in_ap(&format!(
            "{}{}",
            claim("c1", " isAbstract=\"true\"", ""),
            claim("c2", attributes, "")
        ));
        no_finding(row, what, &input);
    }
}

#[test]
fn sacm23_val_007_a_dangling_cited_element_without_is_citation_gives_both_findings() {
    let row = VAL_007;
    let what = "citedElement=\"nope\" with no isCitation";
    let doc = read_ok(
        row,
        what,
        &in_ap(&claim("c1", " citedElement=\"nope\"", "")),
    );
    let found = findings(row, what, &doc, &[VAL_003, VAL_007]);
    let cite: Vec<&Diagnostic> = found
        .iter()
        .filter(|f| f.code() == Code::CITATION_INVALID)
        .collect();
    let dangling: Vec<&Diagnostic> = found
        .iter()
        .filter(|f| f.code() == Code::REF_DANGLING)
        .collect();
    assert_eq!(
        (found.len(), cite.len(), dangling.len()),
        (2, 1, 1),
        "{row}: {what}: not one SACM-CITE-001 and one SACM-REF-001: {found:?}"
    );
    assert_finding(row, what, cite[0], CITE, &["c1"], &["c1"]);
    assert_finding(VAL_003, what, dangling[0], REF, &["c1", "nope"], &["nope"]);
}

#[test]
fn sacm23_val_007_the_rule_holds_on_every_readable_kind_and_a_strict_write_refuses() {
    let row = VAL_007;
    for (holder, _, input_of) in holders() {
        // A self-reference resolves, so only the citation rule reports.
        let what = format!("citedElement=\"x\" without isCitation on {holder}");
        let doc = read_ok(row, &what, &input_of(" citedElement=\"x\""));
        let found = findings(row, &what, &doc, &[VAL_007]);
        let [finding] = found.as_slice() else {
            panic!("{row}: {what}: not one finding: {found:?}");
        };
        assert_finding(row, &what, finding, CITE, &["x"], &["x"]);
        assert_strict_write_refuses(row, &what, &doc, &found);
        let text = tolerant_round_trip(row, &what, &doc);
        assert!(
            text.contains(" citedElement=\"x\""),
            "{row}: {what}: a tolerant write drops the reference:\n{text}"
        );
        no_finding(
            row,
            &format!("{what}, with isCitation=\"true\""),
            &input_of(" isCitation=\"true\" citedElement=\"x\""),
        );
    }
}

#[test]
fn sacm23_val_007_citation_invalid_is_sacm_cite_001_and_its_catalog_row_states_only_the_ocl_direction()
 {
    let row = VAL_007;
    assert_eq!(
        Code::CITATION_INVALID.as_str(),
        "SACM-CITE-001",
        "{row}: Code::CITATION_INVALID is not SACM-CITE-001"
    );
    let line = catalog_row(row, Code::CITATION_INVALID);
    assert!(
        line.starts_with("| SACM-CITE-001 | Error |"),
        "{row}: the catalog's SACM-CITE-001 row is not an Error: {line}"
    );
    for part in [
        "`Code::CITATION_INVALID`",
        "SACM23-VAL-007",
        "citedElement",
        "isCitation",
        "8.2",
    ] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-CITE-001 row does not name {part}: {line}"
        );
    }
    // §8.2 states only `citedElement <> null implies isCitation = true`.
    let lower = line.to_lowercase();
    assert!(
        !lower.contains("vice") && !line.contains("but `citedElement` is absent"),
        "{row}: the catalog's SACM-CITE-001 row still states the converse direction: {line}"
    );
}

// === SACM23-VAL-008 ================================================================

#[test]
fn sacm23_val_008_a_concrete_claim_conforming_to_an_abstract_claim_gives_no_finding() {
    let row = VAL_008;
    for (what, attributes) in [
        ("isAbstract absent", " abstractForm=\"a\""),
        (
            "isAbstract=\"false\"",
            " isAbstract=\"false\" abstractForm=\"a\"",
        ),
        ("the element form", ""),
    ] {
        let content = if attributes.is_empty() {
            "<abstractForm href=\"#a\"/>"
        } else {
            ""
        };
        let input = in_ap(&format!(
            "{}{}",
            claim("a", " isAbstract=\"true\"", ""),
            claim("c1", attributes, content)
        ));
        no_finding(row, &format!("a concrete Claim, {what}"), &input);
    }
    // The fixture of SACM23-BASE-007/008 does too.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sacm23/xmi/citations-and-abstract-forms-valid.sacm.xmi");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    no_finding(row, "the citations and abstract forms fixture", &text);
}

#[test]
fn sacm23_val_008_an_abstract_claim_with_an_abstract_form_is_one_error_on_the_claim() {
    let row = VAL_008;
    let what = "an abstract Claim c1 conforming to the abstract Claim a";
    let input = in_ap(&format!(
        "{}{}",
        claim("a", " isAbstract=\"true\"", ""),
        claim("c1", " isAbstract=\"true\" abstractForm=\"a\"", "")
    ));
    let finding = one_finding(row, what, &input, &[VAL_008]);
    assert_finding(
        row,
        what,
        &finding,
        ABS_ERROR,
        &["c1"],
        &["c1", "isAbstract"],
    );
}

#[test]
fn sacm23_val_008_the_fixture_gives_one_error_on_its_claim() {
    let row = VAL_008;
    let what = "the fixture";
    let finding = one_finding(
        row,
        what,
        &fixture("claim-abstract-with-abstract-form.sacm.xmi"),
        &[VAL_008],
    );
    assert_finding(row, what, &finding, ABS_ERROR, &["c1"], &["c1"]);
}

#[test]
fn sacm23_val_008_a_target_not_abstract_or_of_another_kind_is_a_warning_on_the_element_and_target()
{
    let row = VAL_008;
    let what = "a concrete Claim conforming to the concrete Claim c2";
    let finding = one_finding(
        row,
        what,
        &in_ap(&format!(
            "{}{}",
            claim("c1", " abstractForm=\"c2\"", ""),
            claim("c2", "", "")
        )),
        &[VAL_008],
    );
    assert_finding(
        row,
        what,
        &finding,
        ABS_WARNING,
        &["c1", "c2"],
        &["c1", "c2"],
    );
    let not_abstract = finding.message().to_owned();

    let what = "a concrete Claim conforming to the abstract ArgumentPackage q";
    let finding = one_finding(
        row,
        what,
        &beside_package(
            &claim("c1", " abstractForm=\"q\"", ""),
            " isAbstract=\"true\"",
        ),
        &[VAL_008],
    );
    assert_finding(
        row,
        what,
        &finding,
        ABS_WARNING,
        &["c1", "q"],
        &["c1", "q", "Claim", "ArgumentPackage"],
    );
    let other_kind = finding.message().to_owned();
    assert_ne!(
        not_abstract, other_kind,
        "{row}: the two Warnings cannot be told apart by their messages"
    );

    let what = "a concrete Claim conforming to the concrete ArgumentPackage q";
    let doc = read_ok(
        row,
        what,
        &beside_package(&claim("c1", " abstractForm=\"q\"", ""), ""),
    );
    let found = findings(row, what, &doc, &[VAL_008]);
    assert_eq!(found.len(), 2, "{row}: {what}: not two Warnings: {found:?}");
    for finding in &found {
        assert_finding(row, what, finding, ABS_WARNING, &["c1", "q"], &["c1", "q"]);
    }
    assert_ne!(
        found[0].message(),
        found[1].message(),
        "{row}: {what}: the two Warnings are the same finding twice"
    );
    assert!(
        !diagnostic::has_errors(&found),
        "{row}: {what}: a Warning counts as an Error"
    );
}

#[test]
fn sacm23_val_008_a_self_reference_is_the_error_when_abstract_and_the_warning_when_concrete() {
    let row = VAL_008;
    let what = "an abstract Claim conforming to itself";
    let finding = one_finding(
        row,
        what,
        &in_ap(&claim("c1", " isAbstract=\"true\" abstractForm=\"c1\"", "")),
        &[VAL_008],
    );
    assert_finding(row, what, &finding, ABS_ERROR, &["c1"], &["c1"]);

    let what = "a concrete Claim conforming to itself";
    let finding = one_finding(
        row,
        what,
        &in_ap(&claim("c1", " abstractForm=\"c1\"", "")),
        &[VAL_008],
    );
    assert_finding(row, what, &finding, ABS_WARNING, &["c1", "c1"], &["c1"]);
}

#[test]
fn sacm23_val_008_a_dangling_abstract_form_gives_only_ref_001() {
    let row = VAL_008;
    let what = "a concrete Claim with abstractForm=\"nope\"";
    let doc = read_ok(
        row,
        what,
        &in_ap(&claim("c1", " abstractForm=\"nope\"", "")),
    );
    let found = findings(row, what, &doc, &[VAL_003, VAL_008]);
    let codes: Vec<Code> = found.iter().map(Diagnostic::code).collect();
    assert_eq!(
        codes,
        [Code::REF_DANGLING],
        "{row}: {what}: not only SACM-REF-001: {found:?}"
    );
}

#[test]
fn sacm23_val_008_an_abstract_claim_with_a_dangling_abstract_form_gives_the_error_and_ref_001_only()
{
    let row = VAL_008;
    let what = "an abstract Claim with abstractForm=\"nope\"";
    let doc = read_ok(
        row,
        what,
        &in_ap(&claim(
            "c1",
            " isAbstract=\"true\" abstractForm=\"nope\"",
            "",
        )),
    );
    let found = findings(row, what, &doc, &[VAL_003, VAL_008]);
    let abs: Vec<&Diagnostic> = found
        .iter()
        .filter(|f| f.code() == Code::ABSTRACT_FORM_INVALID)
        .collect();
    let dangling: Vec<&Diagnostic> = found
        .iter()
        .filter(|f| f.code() == Code::REF_DANGLING)
        .collect();
    assert_eq!(
        (found.len(), abs.len(), dangling.len()),
        (2, 1, 1),
        "{row}: {what}: not one SACM-ABS-001 and one SACM-REF-001: {found:?}"
    );
    // The own-`isAbstract` Error does not depend on the target; no Warning,
    // since the target does not resolve.
    assert_finding(row, what, abs[0], ABS_ERROR, &["c1"], &["c1", "isAbstract"]);
    assert_finding(VAL_003, what, dangling[0], REF, &["c1", "nope"], &["nope"]);
    assert!(
        found.iter().all(|f| f.severity() != Severity::Warning),
        "{row}: {what}: a Warning is reported for a dangling target: {found:?}"
    );
}

#[test]
fn sacm23_val_008_errors_make_a_strict_write_refuse_and_warnings_alone_do_not() {
    let row = VAL_008;
    let what = "an abstract Claim with an abstractForm";
    let doc = read_ok(
        row,
        what,
        &in_ap(&format!(
            "{}{}",
            claim("a", " isAbstract=\"true\"", ""),
            claim("c1", " isAbstract=\"true\" abstractForm=\"a\"", "")
        )),
    );
    let expected = findings(row, what, &doc, &[VAL_008]);
    assert_strict_write_refuses(row, what, &doc, &expected);
    tolerant_round_trip(row, what, &doc);

    let what = "a concrete Claim conforming to the concrete ArgumentPackage q";
    let doc = read_ok(
        row,
        what,
        &beside_package(&claim("c1", " abstractForm=\"q\"", ""), ""),
    );
    assert_eq!(
        findings(row, what, &doc, &[VAL_008]).len(),
        2,
        "{row}: {what}: setup: not two Warnings"
    );
    let text = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a strict write refused Warnings only: {error:?}"),
    };
    assert!(
        text.contains(" abstractForm=\"q\""),
        "{row}: {what}: the strict write drops the abstractForm:\n{text}"
    );
}

#[test]
fn sacm23_val_008_abstract_form_invalid_is_sacm_abs_001_and_its_catalog_row_names_it_and_the_row() {
    let row = VAL_008;
    assert_eq!(
        Code::ABSTRACT_FORM_INVALID.as_str(),
        "SACM-ABS-001",
        "{row}: Code::ABSTRACT_FORM_INVALID is not SACM-ABS-001"
    );
    let line = catalog_row(row, Code::ABSTRACT_FORM_INVALID);
    assert!(
        line.starts_with("| SACM-ABS-001 | Error / Warning |"),
        "{row}: the catalog's SACM-ABS-001 row is not Error / Warning: {line}"
    );
    for part in ["`Code::ABSTRACT_FORM_INVALID`", "SACM23-VAL-008"] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-ABS-001 row does not name {part}: {line}"
        );
    }
}
