//! Tests for the description multiplicity rule of `validation::validate`
//! (SACM23-VAL-011; SACM 2.3 §8.6 "description: Description[0..1]
//! (composition)"; the model says `0..*`, specification defect D6;
//! D-DESC-MULT, D-SEV).
//!
//! A `ModelElement` holding two or more `description` children is one
//! SACM-MULT-001 Warning, citing clause 8.6 and affecting the element, with no
//! location. Every description is kept. Notes, implementation constraints and
//! tagged values never count. Being a Warning, it does not stop a strict save.
//!
//! Documents are read from inline XMI, built by edit, or built through the
//! `test-support` seam `Document::insert` for kinds the reader cannot read
//! yet. Every `ModelElement` is named so that no other rule reports anything.
//! The message is checked only for what it must name, never word for word.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::edit::{self, CreateArgumentPackage, CreateDescription};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "SACM23-VAL-011";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The multiplicity the message must quote.
const MULTIPLICITY: &str = "description:Description[0..1]";

/// The fixture of this row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/validation/invalid/claim-descriptions-repeated.sacm.xmi"
);

// --- Inputs and helpers ------------------------------------------------------------

/// An ArgumentPackage root `p` named "P" holding `body` after its name.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `claim` named "C" holding `body` after its name.
fn in_claim(body: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"claim\"><name content=\"C\"/>{body}\
         </argumentElement>"
    ))
}

/// A Description `id` with one value.
fn description(id: &str) -> String {
    format!(
        "<description xmi:id=\"{id}\"><content><value content=\"{id}\"/></content></description>"
    )
}

/// A `UtilityElement` `id` of `role` with one value; a `TaggedValue` also gets
/// a key.
fn utility(role: &str, id: &str) -> String {
    let key = if role == "taggedValue" {
        "<key><value content=\"k\"/></key>"
    } else {
        ""
    };
    format!("<{role} xmi:id=\"{id}\">{key}<content><value content=\"{id}\"/></content></{role}>")
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

/// The findings of this row only, for documents built through the seam, where
/// another rule may also report on a kind.
fn this_rows_findings(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|finding| finding.requirement_id() == ROW)
        .collect()
}

/// Asserts the fields of a SACM23-VAL-011 finding: a Warning SACM-MULT-001,
/// clause 8.6, no location, `affected` only, and a message naming each of
/// `named`.
fn assert_finding(what: &str, found: &Diagnostic, affected: &str, named: &[&str]) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::MULTIPLICITY_VIOLATION, Severity::Warning, ROW),
        "{ROW}: {what}: not a Warning SACM-MULT-001 of this row: {found:?}"
    );
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        ([ElementId::from(affected)].as_slice(), None, Some("8.6")),
        "{ROW}: {what}: the affected id, location or clause"
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

/// The ids of the element `id`'s descriptions, in order.
fn description_ids(doc: &Document, id: &str) -> Vec<String> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    element
        .descriptions()
        .map(|d| d.id().as_str().to_owned())
        .collect()
}

/// Inserts one element through the seam, or fails the test as a setup error.
fn insert(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let name = kind
        .is_a(AbstractClass::ModelElement)
        .then(|| LangString::new(id));
    if let Err(error) = doc.insert(parent.as_ref(), kind, ElementId::from(id), name) {
        panic!("{ROW}: setup could not insert {kind} {id:?}: {error}");
    }
}

/// The text of `docs/sacm/diagnostics.md`, or of a file under the crate.
fn text_of(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

// --- Item 1: two descriptions on a Claim --------------------------------------------

#[test]
fn sacm23_val_011_a_claim_with_two_descriptions_reads_without_diagnostic_and_gives_one_warning() {
    let what = "a Claim with two Descriptions";
    let doc = read_ok(what, &in_claim(&(description("d1") + &description("d2"))));
    assert_eq!(
        description_ids(&doc, "claim"),
        ["d1", "d2"],
        "{ROW}: {what}: setup: both descriptions are not read"
    );
    let found = findings(what, &doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_finding(
        what,
        finding,
        "claim",
        &["Claim", "\"claim\"", "2", MULTIPLICITY],
    );
    assert_eq!(
        finding.code().as_str(),
        "SACM-MULT-001",
        "{ROW}: {what}: the code text is not SACM-MULT-001"
    );
}

#[test]
fn sacm23_val_011_the_fixture_gives_one_warning_on_the_claim() {
    let input = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let finding = one_finding("the fixture", &input);
    assert_finding("the fixture", &finding, "c1", &["\"c1\"", MULTIPLICITY]);
}

// --- Item 2: how many -----------------------------------------------------------------

#[test]
fn sacm23_val_011_three_descriptions_give_one_finding_saying_3_and_none_or_one_give_none() {
    let what = "a Claim with three Descriptions";
    let three = [description("d1"), description("d2"), description("d3")].concat();
    let finding = one_finding(what, &in_claim(&three));
    assert_finding(what, &finding, "claim", &["\"claim\"", "3", MULTIPLICITY]);

    no_finding("a Claim with no Description", &in_claim(""));
    no_finding(
        "a Claim with one Description",
        &in_claim(&description("d1")),
    );
    no_finding("a package with one Description", &in_ap(&description("d1")));
}

// --- Item 3: every ModelElement kind -----------------------------------------------

#[test]
fn sacm23_val_011_every_model_element_kind_with_two_descriptions_gives_one_finding() {
    let kinds: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ModelElement))
        .collect();
    assert_eq!(kinds.len(), 34, "{ROW}: setup: not 34 ModelElement kinds");
    for kind in kinds {
        let mut doc = Document::new();
        if kind.is_package() {
            insert(&mut doc, None, kind, "x");
        } else {
            // An ArtifactPackage admits every ArtifactElement (ADR 0014),
            // which is every ModelElement.
            insert(&mut doc, None, ElementKind::ArtifactPackage, "pkg");
            insert(&mut doc, Some("pkg"), kind, "x");
        }
        insert(&mut doc, Some("x"), ElementKind::Description, "d1");
        insert(&mut doc, Some("x"), ElementKind::Description, "d2");
        let what = format!("a {kind} with two Descriptions");
        let found = this_rows_findings(&doc);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        assert_finding(&what, finding, "x", &[kind.name(), "\"x\""]);
    }
}

// --- Item 4: what counts ------------------------------------------------------------

#[test]
fn sacm23_val_011_interleaved_descriptions_count_and_other_utility_roles_do_not() {
    let what = "d1, n1, d2 on a Claim";
    let body = [description("d1"), utility("note", "n1"), description("d2")].concat();
    let finding = one_finding(what, &in_claim(&body));
    assert_finding(what, &finding, "claim", &["\"claim\"", "2"]);

    for role in ["note", "implementationConstraint", "taggedValue"] {
        let body = utility(role, "u1") + &utility(role, "u2");
        no_finding(&format!("two {role}s on a Claim"), &in_claim(&body));
    }
}

#[test]
fn sacm23_val_011_only_the_element_holding_two_is_reported_not_its_package() {
    let what = "a package with one Description whose Claim has two";
    let input = in_ap(&format!(
        "{}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"claim\"><name content=\"C\"/>{}{}\
         </argumentElement>",
        description("pd"),
        description("d1"),
        description("d2")
    ));
    let finding = one_finding(what, &input);
    assert_finding(what, &finding, "claim", &["\"claim\""]);
}

// --- Item 5: by edit ----------------------------------------------------------------

#[test]
fn sacm23_val_011_two_create_description_edits_succeed_and_validate_reports_the_warning() {
    let mut doc = Document::new();
    let text = |content: &str| LangString::new(content).with_lang("en");
    let operations = [
        edit::apply(
            &mut doc,
            CreateArgumentPackage::root(text("Arguments")).with_id("arguments"),
        ),
        edit::apply(
            &mut doc,
            CreateDescription::new("arguments", text("Why")).with_id("d1"),
        ),
        edit::apply(
            &mut doc,
            CreateDescription::new("arguments", text("How")).with_id("d2"),
        ),
    ];
    for result in operations {
        if let Err(error) = result {
            panic!("{ROW}: an edit was refused: {error:?} ({error})");
        }
    }
    let what = "an ArgumentPackage given two Descriptions by edit";
    let found = findings(what, &doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_finding(
        what,
        finding,
        "arguments",
        &["ArgumentPackage", "\"arguments\"", "2"],
    );
}

// --- Item 6: a Warning does not stop a strict save ---------------------------------

#[test]
fn sacm23_val_011_the_warning_alone_is_no_error_and_a_strict_write_round_trips_both() {
    let what = "a Claim with two Descriptions";
    let doc = read_ok(what, &in_claim(&(description("d1") + &description("d2"))));
    let found = findings(what, &doc);
    assert_eq!(found.len(), 1, "{ROW}: {what}: not one finding: {found:?}");
    assert!(
        !diagnostic::has_errors(&found),
        "{ROW}: {what}: has_errors is true for the Warning alone"
    );
    let text = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a strict write refused: {error:?} ({error})"),
    };
    let back = read_ok(&format!("{what}, written"), &text);
    assert_eq!(
        description_ids(&back, "claim"),
        ["d1", "d2"],
        "{ROW}: {what}: the descriptions do not read back in order\n{text}"
    );
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
}

// --- Item 7: the catalog and the code's documentation ------------------------------

#[test]
fn sacm23_val_011_the_catalog_row_is_error_or_warning_and_names_this_row_as_the_warning() {
    let catalog = text_of("../../docs/sacm/diagnostics.md");
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("| SACM-MULT-001 |"))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-MULT-001, not one",
            rows.len()
        );
    };
    let Some(meaning) = line.strip_prefix("| SACM-MULT-001 | Error / Warning |") else {
        panic!("{ROW}: the catalog's SACM-MULT-001 severity is not \"Error / Warning\": {line}");
    };
    // The row names this row where it speaks of the Warning.
    let Some((_, after_warning)) = meaning.split_once("Warning") else {
        panic!("{ROW}: the catalog's SACM-MULT-001 meaning does not say Warning: {line}");
    };
    for part in ["SACM23-VAL-011", "`description:Description[0..1]`"] {
        assert!(
            after_warning.contains(part),
            "{ROW}: the catalog's SACM-MULT-001 row does not name {part} as the Warning: {line}"
        );
    }
}

#[test]
fn sacm23_val_011_the_multiplicity_violation_doc_names_the_warning_and_this_row() {
    let source = text_of("src/diagnostic.rs");
    let Some(at) = source.find("pub const MULTIPLICITY_VIOLATION: Code") else {
        panic!("{ROW}: src/diagnostic.rs has no MULTIPLICITY_VIOLATION constant");
    };
    // The doc comment is the run of `///` lines just above the constant.
    let mut doc: Vec<&str> = source[..at]
        .lines()
        .rev()
        .skip(1)
        .map(str::trim_start)
        .take_while(|line| line.starts_with("///"))
        .collect();
    doc.reverse();
    let doc = doc.join(" ");
    for part in ["Warning", "SACM23-VAL-011", "description"] {
        assert!(
            doc.contains(part),
            "{ROW}: the Code::MULTIPLICITY_VIOLATION doc does not say {part:?}: {doc}"
        );
    }
}
