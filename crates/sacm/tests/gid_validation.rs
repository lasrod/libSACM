//! Tests for the `gid` uniqueness rule of `validation::validate`
//! (SACM23-VAL-002; SACM 2.3 §8.2 "gid:String[0..1] - a unique identifier
//! that is unique within the scope of the model instance"; D-SEV, D-GID-EMPTY,
//! D-VAL-LAYER; ADR 0022, ADR 0025).
//!
//! Each non-empty `gid` that two or more elements hold, of any kind, is one
//! SACM-ID-003 Error (`Code::GID_DUPLICATE`), citing clause 8.2, affecting
//! every holder in document order, with no location. Values compare exactly;
//! an absent or `""` gid is not compared. Reading and editing do not refuse a
//! duplicate; a strict save does.
//!
//! A gid only comes from `xmi::read` today, so every document is read from
//! inline XMI, and every `ModelElement` in it is named so that no other rule
//! reports anything. The message is checked only for what it must name, never
//! word for word.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::edit::{self, CreateClaim};
use sacm::model::{Document, ElementId, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "SACM23-VAL-002";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The invalid fixture of this row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/validation/invalid/claims-gid-duplicate.sacm.xmi"
);

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

/// A Claim `id` with the attribute `gid="<gid>"`.
fn claim_with_gid(id: &str, gid: &str) -> String {
    claim(id, &format!(" gid=\"{gid}\""), "")
}

/// Reads `input` in both modes, which must succeed with no read diagnostic
/// and give equal documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The gid of the element `id`, which must exist.
fn gid_of<'d>(doc: &'d Document, id: &str) -> Option<&'d str> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    element.gid()
}

/// The findings of `validate` on `doc`, which must all be this row's.
fn findings(what: &str, doc: &Document) -> Vec<Diagnostic> {
    let found = validation::validate(doc);
    for finding in &found {
        assert_eq!(
            finding.requirement_id(),
            ROW,
            "{ROW}: {what}: setup: another rule reports {finding:?}"
        );
    }
    found
}

/// Asserts the fields of a SACM23-VAL-002 finding: an Error SACM-ID-003,
/// clause 8.2, no location, exactly `affected` in that order, and a message
/// naming each of `named`.
fn assert_finding(what: &str, found: &Diagnostic, affected: &[&str], named: &[&str]) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::GID_DUPLICATE, Severity::Error, ROW),
        "{ROW}: {what}: not an Error SACM-ID-003 of this row: {found:?}"
    );
    let affected: Vec<ElementId> = affected.iter().copied().map(ElementId::from).collect();
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        (affected.as_slice(), None, Some("8.2")),
        "{ROW}: {what}: the affected ids, location or clause"
    );
    let message = found.message();
    for part in named {
        assert!(
            message.contains(part),
            "{ROW}: {what}: the message {message:?} does not name {part:?}"
        );
    }
}

/// Reads `input` and validates it, which must give exactly one finding, and
/// returns it.
fn one_finding(what: &str, input: &str) -> Diagnostic {
    let found = findings(what, &read_ok(what, input));
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    finding.clone()
}

/// Reads `input` and asserts that `validate` gives no finding.
fn no_finding(what: &str, input: &str) {
    let found = validation::validate(&read_ok(what, input));
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
}

/// The catalog row of `code`, which must be exactly one.
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
            "{ROW}: docs/sacm/diagnostics.md has {} rows for {code}, not one",
            rows.len()
        );
    };
    (*line).to_owned()
}

// --- Item 1: two Claims -----------------------------------------------------------

#[test]
fn sacm23_val_002_two_claims_sharing_a_gid_read_without_diagnostic_and_give_one_error() {
    let what = "two Claims with gid=\"g\"";
    let input = in_ap(&(claim_with_gid("c1", "g") + &claim_with_gid("c2", "g")));
    let doc = read_ok(what, &input);
    assert_eq!(
        (gid_of(&doc, "c1"), gid_of(&doc, "c2")),
        (Some("g"), Some("g")),
        "{ROW}: {what}: setup: the gids are not read, so this tests nothing"
    );
    let found = findings(what, &doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_finding(what, finding, &["c1", "c2"], &["\"g\"", "c1", "c2"]);
    assert_eq!(
        finding.code().as_str(),
        "SACM-ID-003",
        "{ROW}: {what}: the code text is not SACM-ID-003"
    );
    assert!(
        diagnostic::has_errors(&found),
        "{ROW}: {what}: has_errors is false for a duplicate gid"
    );
}

#[test]
fn sacm23_val_002_the_fixture_gives_one_error_on_both_claims() {
    let input = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let finding = one_finding("the fixture", &input);
    assert_finding(
        "the fixture",
        &finding,
        &["c1", "c2"],
        &["\"urn:example:g\""],
    );
}

// --- Item 2: three holders of several kinds, in document order ---------------------

#[test]
fn sacm23_val_002_three_holders_are_one_finding_affecting_them_in_document_order() {
    // Document order is z_case, arguments, m_claim, a_note: not sorted.
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"z_case\" gid=\"g\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"arguments\"><name content=\"A\"/>{}</argumentPackage>\
         </sacm:AssuranceCasePackage>",
        claim(
            "m_claim",
            " gid=\"g\"",
            "<note xmi:id=\"a_note\" gid=\"g\"/>"
        )
    );
    let what = "an AssuranceCasePackage, a Claim and a Note sharing \"g\"";
    let doc = read_ok(what, &input);
    let order: Vec<&str> = doc.elements().map(|e| e.id().as_str()).collect();
    assert_eq!(
        order,
        ["z_case", "arguments", "m_claim", "a_note"],
        "{ROW}: {what}: setup: elements() order is not the one this test needs"
    );
    let found = findings(what, &doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_finding(
        what,
        finding,
        &["z_case", "m_claim", "a_note"],
        &["\"g\"", "3"],
    );
}

#[test]
fn sacm23_val_002_two_holders_out_of_id_order_are_affected_in_document_order() {
    let what = "c2 before c1, sharing \"g\"";
    let finding = one_finding(
        what,
        &in_ap(&(claim_with_gid("c2", "g") + &claim_with_gid("c1", "g"))),
    );
    assert_finding(what, &finding, &["c2", "c1"], &["\"g\""]);
}

// --- Item 3: two duplicated values ------------------------------------------------

#[test]
fn sacm23_val_002_two_duplicated_values_give_two_findings_in_sorted_order() {
    // "g" is held by c3 and c4, "h" by c1 and c2, interleaved. Sorted by
    // affected ids, the "h" finding comes first.
    let body = [
        claim_with_gid("c3", "g"),
        claim_with_gid("c1", "h"),
        claim_with_gid("c4", "g"),
        claim_with_gid("c2", "h"),
    ]
    .concat();
    let what = "\"g\" on c3, c4 and \"h\" on c1, c2";
    let doc = read_ok(what, &in_ap(&body));
    let found = findings(what, &doc);
    let [first, second] = found.as_slice() else {
        panic!("{ROW}: {what}: not two findings: {found:?}");
    };
    assert_finding(what, first, &["c1", "c2"], &["\"h\""]);
    assert_finding(what, second, &["c3", "c4"], &["\"g\""]);
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(sorted, found, "{ROW}: {what}: the findings are not sorted");
}

// --- Item 4: every readable kind is in scope ----------------------------------------

#[test]
fn sacm23_val_002_each_utility_kind_sharing_a_gid_with_a_claim_is_one_finding() {
    for (role, extra) in [
        ("description", ""),
        ("implementationConstraint", ""),
        ("note", ""),
        ("taggedValue", "<key><value content=\"k\"/></key>"),
    ] {
        let what = format!("a {role} sharing \"g\" with its Claim");
        let utility = format!("<{role} xmi:id=\"u\" gid=\"g\">{extra}</{role}>");
        let finding = one_finding(&what, &in_ap(&claim("c1", " gid=\"g\"", &utility)));
        assert_finding(&what, &finding, &["c1", "u"], &["\"g\""]);
    }
}

#[test]
fn sacm23_val_002_two_roots_of_one_xmi_document_sharing_a_gid_are_one_finding() {
    let input = format!(
        "<xmi:XMI{NS}>\
         <sacm:AssuranceCasePackage xmi:id=\"r1\" gid=\"g\"><name content=\"R1\"/></sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:id=\"r2\" gid=\"g\"><name content=\"R2\"/></sacm:ArgumentPackage>\
         </xmi:XMI>"
    );
    let what = "two roots of one xmi:XMI sharing \"g\"";
    let finding = one_finding(what, &input);
    assert_finding(what, &finding, &["r1", "r2"], &["\"g\""]);
}

// --- Item 5: what is not compared, and what is ------------------------------------

#[test]
fn sacm23_val_002_empty_absent_case_and_space_differences_give_no_finding() {
    let cases = [
        (
            "two gid=\"\"",
            claim_with_gid("c1", "") + &claim_with_gid("c2", ""),
        ),
        (
            "gid=\"\" and an absent gid",
            claim_with_gid("c1", "") + &claim("c2", "", ""),
        ),
        (
            "two absent gids",
            claim("c1", "", "") + &claim("c2", "", ""),
        ),
        (
            "\"g\" and \"G\"",
            claim_with_gid("c1", "g") + &claim_with_gid("c2", "G"),
        ),
        (
            "\"g\" and \" g \"",
            claim_with_gid("c1", "g") + &claim_with_gid("c2", " g "),
        ),
        (
            "a gid equal to another element's xmi:id",
            claim_with_gid("c1", "c2") + &claim("c2", "", ""),
        ),
        (
            "a gid equal to its own xmi:id",
            claim_with_gid("c1", "c1") + &claim("c2", "", ""),
        ),
        (
            "unique gids",
            claim_with_gid("c1", "g") + &claim_with_gid("c2", "h"),
        ),
    ];
    for (what, body) in cases {
        no_finding(what, &in_ap(&body));
    }
    // Also an empty gid on the package and both Claims, and `<gid/>`.
    no_finding(
        "gid=\"\" on a root and <gid/> on a Claim",
        &in_ap(&claim("c1", "", "<gid/>")).replace("xmi:id=\"p\"", "xmi:id=\"p\" gid=\"\""),
    );
}

#[test]
fn sacm23_val_002_equal_values_of_only_white_space_or_with_spaces_give_one_finding() {
    for value in [" g ", "  "] {
        let what = format!("two gid={value:?}");
        let doc = read_ok(
            &what,
            &in_ap(&(claim_with_gid("c1", value) + &claim_with_gid("c2", value))),
        );
        assert_eq!(
            gid_of(&doc, "c1"),
            Some(value),
            "{ROW}: {what}: setup: the gid is not read as written"
        );
        let found = findings(&what, &doc);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        let quoted = format!("{value:?}");
        assert_finding(&what, finding, &["c1", "c2"], &[quoted.as_str()]);
    }
}

// --- Item 6: the two forms compare equal --------------------------------------------

#[test]
fn sacm23_val_002_an_attribute_gid_and_an_element_gid_of_one_value_are_one_finding() {
    let what = "gid=\"g\" and <gid>g</gid>";
    let body = claim_with_gid("c1", "g") + &claim("c2", "", "<gid>g</gid>");
    let finding = one_finding(what, &in_ap(&body));
    assert_finding(what, &finding, &["c1", "c2"], &["\"g\""]);
}

// --- Item 7: saving -----------------------------------------------------------------

#[test]
fn sacm23_val_002_a_strict_write_refuses_and_a_tolerant_write_keeps_both_gids() {
    let what = "two Claims with gid=\"g\"";
    let doc = read_ok(
        what,
        &in_ap(&(claim_with_gid("c1", "g") + &claim_with_gid("c2", "g"))),
    );
    let expected = validation::validate(&doc);
    assert_eq!(
        expected.len(),
        1,
        "{ROW}: {what}: setup: not one finding: {expected:?}"
    );

    let error = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: {what}: a strict write returned:\n{text}"),
        Err(error) => error,
    };
    let WriteError::Invalid { diagnostics, .. } = &error else {
        panic!("{ROW}: {what}: the strict refusal is not Invalid: {error:?}");
    };
    assert_eq!(
        diagnostics, &expected,
        "{ROW}: {what}: the strict refusal does not hold the finding"
    );

    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a tolerant write refused: {error:?}"),
    };
    let back = read_ok(&format!("{what}, written"), &text);
    assert_eq!(
        (gid_of(&back, "c1"), gid_of(&back, "c2")),
        (Some("g"), Some("g")),
        "{ROW}: {what}: the gids do not read back\n{text}"
    );
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: {what}: the tolerant output does not read back equal\n{text}"
    );
}

// --- Item 8: editing and validating change nothing ---------------------------------

#[test]
fn sacm23_val_002_an_edit_is_not_refused_and_validate_changes_nothing() {
    let what = "two Claims with gid=\"g\"";
    let mut doc = read_ok(
        what,
        &in_ap(&(claim_with_gid("c1", "g") + &claim_with_gid("c2", "g"))),
    );
    let create = CreateClaim::new("p", LangString::new("C3")).with_id("c3");
    if let Err(error) = edit::apply(&mut doc, create) {
        panic!("{ROW}: {what}: CreateClaim was refused: {error:?} ({error})");
    }
    assert!(
        doc.contains("c3"),
        "{ROW}: {what}: the new Claim is not in the document"
    );

    let before = doc.clone();
    let first = findings(what, &doc);
    let [finding] = first.as_slice() else {
        panic!("{ROW}: {what}: after the edit, not one finding: {first:?}");
    };
    assert_finding(what, finding, &["c1", "c2"], &["\"g\""]);
    assert_eq!(doc, before, "{ROW}: {what}: validate changed the document");
    assert_eq!(
        validation::validate(&doc),
        first,
        "{ROW}: {what}: a second validate gave another result"
    );
}

// --- Item 9: the code and the catalog -----------------------------------------------

#[test]
fn sacm23_val_002_gid_duplicate_is_sacm_id_003_and_its_catalog_row_names_it_and_the_row() {
    assert_eq!(
        Code::GID_DUPLICATE.as_str(),
        "SACM-ID-003",
        "{ROW}: Code::GID_DUPLICATE is not SACM-ID-003"
    );
    let line = catalog_row(Code::GID_DUPLICATE);
    assert!(
        line.starts_with("| SACM-ID-003 | Error |"),
        "{ROW}: the catalog's SACM-ID-003 row is not an Error: {line}"
    );
    for part in ["`Code::GID_DUPLICATE`", "SACM23-VAL-002"] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-ID-003 row does not name {part}: {line}"
        );
    }
}
