//! Tests for the multiplicities of an asserted relationship's ends
//! (SACM23-VAL-005; formal/23-05-08 §11.13 `source:ArgumentAsset[1..*]`,
//! `target:ArgumentAsset[1]`; ptc/22-03-13 `Argumentation::AssertedRelationship`;
//! XMI 2.5.1 §7.8.2; D-SEV, D-VAL-LAYER; ADR 0022, ADR 0027).
//!
//! `validation::validate` reports each asserted relationship with no
//! `source`, and each with no `target`, as one SACM-MULT-001 Error per
//! missing end: requirement SACM23-VAL-005, clause `11.13`, affected
//! `[relationship]`, no location, the message naming the end. A strict write
//! refuses such a relationship; a tolerant write keeps it as read.
//!
//! Documents are read from inline XMI with every `ModelElement` named, so
//! that no other rule reports, or built through the `test-support` seam for
//! the kinds this release does not read.

use std::path::Path;

use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::model::{AssertionDeclaration, Document, ElementId, ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "SACM23-VAL-005";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The five `AssertedRelationship` kinds (§11.13 to §11.18).
const RELATIONSHIP_KINDS: [ElementKind; 5] = [
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

/// The invalid fixture of this row.
fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "tests/data/sacm23/validation/invalid/asserted-inference-no-source-no-target.sacm.xmi",
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("{ROW}: cannot read the fixture {}: {error}", path.display())
    })
}

// --- Inputs and helpers ------------------------------------------------------------

/// An ArgumentPackage root `p` named "P", holding the named Claims c1 and
/// c2, then the relationship `x` of `kind` named "X" with `attributes` and
/// holding `content` after its name.
fn input(kind: ElementKind, attributes: &str, content: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}><name content=\"X\"/>\
         {content}</argumentElement></sacm:ArgumentPackage>"
    )
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

/// Asserts that `found` is a SACM-MULT-001 Error of this row with clause
/// `11.13`, affecting exactly `[id]`, with no location and a one-line
/// message naming `end` and `id`.
fn assert_finding(what: &str, found: &Diagnostic, id: &str, end: &str) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::MULTIPLICITY_VIOLATION, Severity::Error, ROW),
        "{ROW}: {what}: not a SACM-MULT-001 Error of this row: {found:?}"
    );
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        ([ElementId::from(id)].as_slice(), None, Some("11.13")),
        "{ROW}: {what}: the affected ids, location or clause"
    );
    let message = found.message();
    assert!(
        message.contains(end) && message.contains(&format!("{id:?}")) && !message.contains('\n'),
        "{ROW}: {what}: the message {message:?} is not one line naming {end} and {id:?}"
    );
}

/// The two findings of a relationship `id` missing both ends: one naming
/// `source` and one naming `target`, in either order.
fn assert_both_missing(what: &str, found: &[Diagnostic], id: &str) {
    let [first, second] = found else {
        panic!("{ROW}: {what}: not two findings: {found:?}");
    };
    let (source, target) =
        if first.message().contains("source") && !first.message().contains("target") {
            (first, second)
        } else {
            (second, first)
        };
    assert_finding(what, source, id, "source");
    assert_finding(what, target, id, "target");
    assert_ne!(
        source.message(),
        target.message(),
        "{ROW}: {what}: the two findings have one message"
    );
}

/// The tolerant write of `doc`, which must succeed.
fn tolerant_write(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a tolerant write refused: {error:?}"),
    }
}

// --- Items 1 and 2: one Error per missing end ------------------------------------------------

fn one_error_per_missing_end(kind: ElementKind) {
    for (what, attributes, content, end) in [
        ("no source", " target=\"c1\"", "", "source"),
        (
            "no source, a target element",
            "",
            "<target xmi:idref=\"c1\"/>",
            "source",
        ),
        ("no target", " source=\"c2\"", "", "target"),
        ("no target, two sources", " source=\"c1 c2\"", "", "target"),
        (
            "no target, a source element",
            "",
            "<source href=\"#c2\"/>",
            "target",
        ),
    ] {
        let what = format!("an {kind} with {what}");
        let doc = read_ok(&what, &input(kind, attributes, content));
        let found = findings(&what, &doc);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        assert_finding(&what, finding, "x", end);
    }
    let what = format!("an {kind} with neither");
    let doc = read_ok(&what, &input(kind, "", ""));
    assert_both_missing(&what, &findings(&what, &doc), "x");
}

#[test]
fn sacm23_val_005_an_asserted_inference_missing_its_source_or_target_gives_one_error_per_missing_end()
 {
    one_error_per_missing_end(ElementKind::AssertedInference);
}

#[test]
fn sacm23_val_005_an_asserted_context_missing_its_source_or_target_gives_one_error_per_missing_end()
{
    one_error_per_missing_end(ElementKind::AssertedContext);
}

#[test]
fn sacm23_val_005_each_relationship_kind_inserted_without_ends_gives_two_errors() {
    for kind in RELATIONSHIP_KINDS {
        let mut doc = Document::new();
        for (parent, element_kind, id, name) in [
            (None, ElementKind::ArgumentPackage, "p", "P"),
            (Some("p"), kind, "x", "X"),
        ] {
            let parent = parent.map(ElementId::from);
            if let Err(error) = doc.insert(
                parent.as_ref(),
                element_kind,
                ElementId::from(id),
                Some(LangString::new(name)),
            ) {
                panic!("{ROW}: setup could not insert the {element_kind} {id:?}: {error}");
            }
        }
        let what = format!("an {kind} inserted with no ends");
        let found: Vec<Diagnostic> = validation::validate(&doc)
            .into_iter()
            .filter(|finding| finding.requirement_id() == ROW)
            .collect();
        assert_both_missing(&what, &found, "x");
    }
}

#[test]
fn sacm23_val_005_the_fixture_gives_two_errors_on_its_inference() {
    let what = "the fixture";
    let doc = read_ok(what, &fixture());
    assert_both_missing(what, &findings(what, &doc), "i1");
}

// --- Item 3: no finding with both ends ---------------------------------------------------------

#[test]
fn sacm23_val_005_a_relationship_with_a_source_and_a_target_gives_none_whatever_its_declaration_or_is_counter()
 {
    for kind in [ElementKind::AssertedInference, ElementKind::AssertedContext] {
        for &declaration in AssertionDeclaration::ALL {
            for counter in ["", " isCounter=\"true\"", " isCounter=\"false\""] {
                let attributes = format!(
                    " assertionDeclaration=\"{}\"{counter} source=\"c2\" target=\"c1\"",
                    declaration.as_str()
                );
                let what = format!("an {kind} with{attributes}");
                let doc = read_ok(&what, &input(kind, &attributes, ""));
                assert_eq!(
                    validation::validate(&doc),
                    [],
                    "{ROW}: {what}: validate reports a finding"
                );
            }
        }
        // The element forms count as well.
        let what = format!("an {kind} with its ends as elements");
        let doc = read_ok(
            &what,
            &input(
                kind,
                "",
                "<source xmi:idref=\"c2\"/><source xmi:idref=\"c1\"/><target href=\"#c1\"/>",
            ),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
    }
}

// --- Item 4: saving ---------------------------------------------------------------------------

#[test]
fn sacm23_val_005_a_strict_write_refuses_and_a_tolerant_write_keeps_the_relationship_as_read() {
    for kind in [ElementKind::AssertedInference, ElementKind::AssertedContext] {
        for (what, attributes) in [
            ("no source", " target=\"c1\""),
            ("no target", " source=\"c2\""),
            ("neither", ""),
        ] {
            let what = format!("an {kind} with {what}");
            let doc = read_ok(&what, &input(kind, attributes, ""));
            let expected = findings(&what, &doc);
            assert!(
                !expected.is_empty(),
                "{ROW}: {what}: setup: validate reports nothing"
            );
            let before = doc.clone();
            let error = match xmi::write(&doc, &WriteOptions::default()) {
                Ok(text) => panic!("{ROW}: {what}: a strict write returned:\n{text}"),
                Err(error) => error,
            };
            let WriteError::Invalid { diagnostics, .. } = &error else {
                panic!("{ROW}: {what}: the strict refusal is not Invalid: {error:?}");
            };
            assert_eq!(
                diagnostics, &expected,
                "{ROW}: {what}: the strict refusal does not hold exactly the findings"
            );
            assert!(
                doc == before,
                "{ROW}: {what}: the refused write changed the document"
            );

            let text = tolerant_write(&what, &doc);
            let Some(line) = text.lines().find(|line| line.contains("xmi:id=\"x\"")) else {
                panic!(
                    "{ROW}: {what}: x is not written
{text}"
                );
            };
            assert_eq!(
                line.trim(),
                format!("<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}>"),
                "{ROW}: {what}: the tolerant write does not keep the relationship as read\n{text}"
            );
            let back = read_ok(&format!("{what}, written"), &text);
            assert!(
                back == doc,
                "{ROW}: {what}: the tolerant output does not read back equal\n{text}"
            );
        }
    }
}
