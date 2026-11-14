//! Tests for the typing of reference targets: every end's declared type
//! (SACM23-VAL-004, SACM-REF-002; ptc/22-03-13 end types; formal/23-05-08
//! §8.2, §11.2, §11.9, §11.10, §11.12, §11.13) and the `ArtifactReference` rules
//! of §11.15, §11.17 and §11.18 (SACM23-VAL-016) (D-SEV, D-VAL-LAYER as
//! amended; ADR 0028).
//!
//! - SACM23-VAL-004: one SACM-REF-002 Error for each distinct id of each end
//!   of each element that names an element whose kind does not conform to the
//!   end's declared type, with the end's clause, affected `[referrer,
//!   target]`, no location, and a message naming the referrer, the end, the
//!   target's kind and the declared type. A dangling id gives only
//!   SACM-REF-001. Every create refuses such an end as
//!   `EditError::WrongReferenceType`.
//! - SACM23-VAL-016: one SACM-REF-002 Error for each distinct id naming an
//!   `ArgumentAsset` other than an `ArtifactReference` in the `source` of an
//!   `AssertedEvidence`, or the `source` or `target` of an
//!   `AssertedArtifactSupport` or `AssertedArtifactContext`, with clause 11.15,
//!   11.17 or 11.18 and affected `[relationship, id]`. An id SACM23-VAL-004
//!   already reports is not reported again. A create does not refuse these.
//!
//! The declared type of each end is written here from ptc/22-03-13, not taken
//! from the crate, and conformance is asked of the public `is_kind_of` and
//! `is_a`.

use std::fmt::Display;
use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::{
    self, CreateArgumentGroup, CreateArgumentPackage, CreateArgumentReasoning,
    CreateArtifactReference, CreateAssertedArtifactContext, CreateAssertedArtifactSupport,
    CreateAssertedContext, CreateAssertedEvidence, CreateAssertedInference,
    CreateAssuranceCasePackage, CreateClaim, CreateNote, EditError, Operation,
};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const VAL_004: &str = "SACM23-VAL-004";
const VAL_016: &str = "SACM23-VAL-016";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

// --- Inputs ------------------------------------------------------------------------

/// A named element `id` of `kind` under `argumentElement`, with `attributes`
/// and holding `content` after its name.
fn typed(kind: &str, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"N\"/>{content}</argumentElement>"
    )
}

/// An AssuranceCasePackage root `case` holding the ArgumentPackage `p`,
/// which holds one valid element of each readable kind and then `body`:
/// - the Claims c1 and c2, c1 holding the Description d1, the
///   `ImplementationConstraint` ic1, the Note n1 and the `TaggedValue` t1;
/// - the `ArgumentReasoning` why;
/// - the `ArtifactReference`s r1 and r2;
/// - the `ArgumentGroup` g1 and the `AssertedInference` i1 (c2 to c1).
///
/// Every element is named, so that nothing else is reported.
fn in_case(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{}{}{}{}{}{}{body}</argumentPackage>\
         </sacm:AssuranceCasePackage>",
        typed(
            "Claim",
            "c1",
            "",
            "<description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
             <implementationConstraint xmi:id=\"ic1\"/><note xmi:id=\"n1\"/>\
             <taggedValue xmi:id=\"t1\"><key><value content=\"K\"/></key></taggedValue>"
        ),
        typed("Claim", "c2", "", ""),
        typed("ArgumentReasoning", "why", "", ""),
        typed("ArtifactReference", "r1", "", ""),
        typed("ArtifactReference", "r2", "", ""),
        typed("ArgumentGroup", "g1", "", ""),
        typed(
            "AssertedInference",
            "i1",
            " source=\"c2\" target=\"c1\"",
            ""
        ),
    )
}

/// The element `x` of `kind`, with `attributes`, in [`in_case`].
fn x_in_case(kind: &str, attributes: &str) -> String {
    in_case(&typed(kind, "x", attributes, ""))
}

/// The kind of each element of [`in_case`] that a reference can name.
const TARGETS: [(&str, ElementKind); 13] = [
    ("case", ElementKind::AssuranceCasePackage),
    ("p", ElementKind::ArgumentPackage),
    ("c1", ElementKind::Claim),
    ("why", ElementKind::ArgumentReasoning),
    ("r1", ElementKind::ArtifactReference),
    ("g1", ElementKind::ArgumentGroup),
    ("i1", ElementKind::AssertedInference),
    ("d1", ElementKind::Description),
    ("ic1", ElementKind::ImplementationConstraint),
    ("n1", ElementKind::Note),
    ("t1", ElementKind::TaggedValue),
    ("c2", ElementKind::Claim),
    ("r2", ElementKind::ArtifactReference),
];

/// The declared type of an end.
#[derive(Clone, Copy, Debug)]
enum Declared {
    Kind(ElementKind),
    Abstract(AbstractClass),
}

impl Declared {
    fn admits(self, kind: ElementKind) -> bool {
        match self {
            Self::Kind(declared) => kind.is_kind_of(declared),
            Self::Abstract(class) => kind.is_a(class),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Kind(kind) => kind.name(),
            Self::Abstract(class) => class.name(),
        }
    }
}

/// One end to check: what, the referrer's kind, the referrer's other
/// attributes (so that it is valid apart from the end), the end, its clause
/// and its declared type (ptc/22-03-13).
struct End {
    referrer: &'static str,
    others: &'static str,
    name: &'static str,
    clause: &'static str,
    declared: Declared,
}

fn ends() -> [End; 9] {
    use Declared::{Abstract, Kind};
    [
        End {
            referrer: "Claim",
            others: " isCitation=\"true\"",
            name: "citedElement",
            clause: "8.2",
            declared: Abstract(AbstractClass::SACMElement),
        },
        End {
            referrer: "Claim",
            others: "",
            name: "metaClaim",
            clause: "11.10",
            declared: Kind(ElementKind::Claim),
        },
        End {
            referrer: "AssertedInference",
            others: " source=\"c2\" target=\"c1\"",
            name: "reasoning",
            clause: "11.13",
            declared: Kind(ElementKind::ArgumentReasoning),
        },
        End {
            referrer: "AssertedInference",
            others: " target=\"c1\"",
            name: "source",
            clause: "11.13",
            declared: Abstract(AbstractClass::ArgumentAsset),
        },
        End {
            referrer: "AssertedContext",
            others: " source=\"c2\"",
            name: "target",
            clause: "11.13",
            declared: Abstract(AbstractClass::ArgumentAsset),
        },
        End {
            referrer: "ArgumentReasoning",
            others: "",
            name: "structure",
            clause: "11.12",
            declared: Kind(ElementKind::ArgumentPackage),
        },
        End {
            referrer: "ArtifactReference",
            others: "",
            name: "referencedArtifactElement",
            clause: "11.9",
            declared: Abstract(AbstractClass::ArtifactElement),
        },
        End {
            referrer: "ArgumentGroup",
            others: "",
            name: "argumentElement",
            clause: "11.2",
            declared: Abstract(AbstractClass::ArgumentationElement),
        },
        End {
            referrer: "AssertedEvidence",
            others: " source=\"r1\"",
            name: "target",
            clause: "11.13",
            declared: Abstract(AbstractClass::ArgumentAsset),
        },
    ]
}

// --- Reading, writing and validating --------------------------------------------------

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

/// The findings of `validate` on `doc` of the row `row`.
fn of_row(doc: &Document, row: &str) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.requirement_id() == row)
        .collect()
}

/// The findings of `validate` on `doc` with `code`.
fn with_code(doc: &Document, code: Code) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == code)
        .collect()
}

/// The affected ids of each finding, as strings.
fn affected(found: &[Diagnostic]) -> Vec<Vec<String>> {
    found
        .iter()
        .map(|finding| {
            finding
                .affected()
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect()
        })
        .collect()
}

/// Asserts that `found` is a SACM-REF-002 Error of `row` with `clause`, no
/// location, affecting `[referrer, target]`, with a one-line message naming
/// each of `named`.
fn assert_wrong_type(
    row: &str,
    what: &str,
    found: &Diagnostic,
    clause: &str,
    (referrer, target): (&str, &str),
    named: &[&str],
) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::REF_WRONG_TYPE, Severity::Error, row),
        "{row}: {what}: not a SACM-REF-002 Error of this row: {found:?}"
    );
    assert_eq!(
        (found.clause(), found.location(), found.affected()),
        (
            Some(clause),
            None,
            [ElementId::from(referrer), ElementId::from(target)].as_slice()
        ),
        "{row}: {what}: not clause {clause}, no location, on [{referrer}, {target}]: {found:?}"
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

/// A strict write of `doc` must be `WriteError::Invalid` holding each of
/// `expected`.
fn assert_strict_write_refuses(row: &str, what: &str, doc: &Document, expected: &[Diagnostic]) {
    let error = match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => panic!("{row}: {what}: a strict write returned:\n{text}"),
        Err(error) => error,
    };
    let WriteError::Invalid { diagnostics, .. } = &error else {
        panic!("{row}: {what}: the strict refusal is not Invalid: {error:?}");
    };
    assert!(
        !expected.is_empty(),
        "{row}: {what}: setup: nothing is expected"
    );
    for finding in expected {
        assert!(
            diagnostics.contains(finding),
            "{row}: {what}: the strict refusal does not hold {finding:?}: {diagnostics:?}"
        );
    }
}

/// A tolerant write of `doc` must succeed, read back equal in both modes, and
/// write the same bytes again; returns it.
fn tolerant_round_trip(row: &str, what: &str, doc: &Document) -> String {
    let write = |doc: &Document| match xmi::write(
        doc,
        &WriteOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a tolerant write refused: {error:?}"),
    };
    let text = write(doc);
    let back = read_ok(row, &format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the tolerant output does not read back equal\n{text}"
    );
    assert_eq!(
        write(&back),
        text,
        "{row}: {what}: writing what was read back gives other bytes"
    );
    text
}

/// The catalog row of `code`, which must be exactly one.
fn catalog_row(row: &str, code: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let cell = format!("| {code} |");
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

// --- Editing -------------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies an operation the test expects to succeed, and returns the one id
/// it created.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => {
            let created = mutation.created_ids();
            let [only] = created else {
                panic!("{row}: created_ids() is {created:?}, not the one new element");
            };
            only.clone()
        }
        Err(error) => panic!("{row}: the operation was refused: {error} ({error:?})"),
    }
}

/// A document built by edits like [`in_case`]: `case` holding `p`, with the
/// Claims c1 (holding the Note n1) and c2, the `ArgumentReasoning` why, the
/// `ArtifactReference`s r1 and r2 and the `ArgumentGroup` g1.
fn built(row: &str) -> Document {
    let mut doc = Document::new();
    create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentPackage::new("case", named("P")).with_id("p"),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new("p", named("C1")).with_id("c1"),
    );
    create(
        row,
        &mut doc,
        CreateClaim::new("p", named("C2")).with_id("c2"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentReasoning::new("p", named("Why")).with_id("why"),
    );
    for id in ["r1", "r2"] {
        create(
            row,
            &mut doc,
            CreateArtifactReference::new("p", named("R")).with_id(id),
        );
    }
    create(
        row,
        &mut doc,
        CreateArgumentGroup::new("p", named("G")).with_id("g1"),
    );
    create(
        row,
        &mut doc,
        CreateNote::new("c1", named("N")).with_id("n1"),
    );
    doc
}

/// The name of a reference end in an `EditError`, by its `Display`.
fn end_name(end: &dyn Display) -> String {
    end.to_string()
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, and returns why.
fn refuse(row: &str, what: &str, doc: &mut Document, operation: Operation) -> EditError {
    let before = doc.clone();
    let error = match edit::apply(doc, operation) {
        Ok(mutation) => panic!(
            "{row}: {what}: accepted, creating {:?}",
            mutation.created_ids()
        ),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{row}: {what}: the refusal changed the document"
    );
    error
}

/// One refused create: what it is, the operation, and the expected
/// `(end, id, kind, expected)` of its `WrongReferenceType` refusal.
type RefusedCase<'a> = (&'a str, Operation, (&'a str, &'a str, ElementKind, &'a str));

/// Asserts that `error` is `WrongReferenceType { end, id, kind, expected }`
/// with SACM-REF-002.
fn assert_refused_type(
    row: &str,
    what: &str,
    error: &EditError,
    (end, id, kind, expected): (&str, &str, ElementKind, &str),
) {
    let EditError::WrongReferenceType {
        end: found_end,
        id: found_id,
        kind: found_kind,
        expected: found_expected,
        ..
    } = error
    else {
        panic!("{row}: {what}: not WrongReferenceType: {error:?}");
    };
    assert_eq!(
        (
            end_name(found_end),
            found_id.as_str(),
            *found_kind,
            *found_expected,
            error.code()
        ),
        (end.to_owned(), id, kind, expected, Code::REF_WRONG_TYPE),
        "{row}: {what}: not WrongReferenceType {{ {end}, {id}, {kind}, {expected} }} (SACM-REF-002)"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{row}: {what}: the message {message:?} is not one line starting in lower case"
    );
}

// =====================================================================================
// SACM23-VAL-004
// =====================================================================================

// --- Item 1: a meta-claim that is no Claim -------------------------------------------

#[test]
fn sacm23_val_004_a_meta_claim_naming_a_reasoning_is_one_error_with_clause_11_10() {
    let row = VAL_004;
    let what = "a Claim with metaClaim=\"why\"";
    let doc = read_ok(row, what, &x_in_case("Claim", " metaClaim=\"why\""));
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{row}: {what}: not one finding: {found:?}");
    };
    assert_wrong_type(
        row,
        what,
        finding,
        "11.10",
        ("x", "why"),
        &["\"x\"", "metaClaim", "ArgumentReasoning", "Claim"],
    );
}

// --- Item 2: the ends of a relationship -------------------------------------------------

#[test]
fn sacm23_val_004_each_wrongly_typed_end_of_an_asserted_inference_is_one_error_with_clause_11_13() {
    let row = VAL_004;
    for (what, attributes, expected) in [
        (
            "reasoning naming a Claim",
            " source=\"c2\" target=\"c1\" reasoning=\"c2\"",
            vec![("c2", "reasoning", "Claim", "ArgumentReasoning")],
        ),
        (
            "a package source and a group target",
            " source=\"p\" target=\"g1\"",
            vec![
                ("p", "source", "ArgumentPackage", "ArgumentAsset"),
                ("g1", "target", "ArgumentGroup", "ArgumentAsset"),
            ],
        ),
        (
            "a Note source",
            " source=\"c2 n1\" target=\"c1\"",
            vec![("n1", "source", "Note", "ArgumentAsset")],
        ),
    ] {
        let what = format!("an AssertedInference with {what}");
        let doc = read_ok(row, &what, &x_in_case("AssertedInference", attributes));
        let found = validation::validate(&doc);
        assert_eq!(
            found.len(),
            expected.len(),
            "{row}: {what}: not {} findings: {found:?}",
            expected.len()
        );
        for (target, end, kind, declared) in expected {
            let Some(finding) = found
                .iter()
                .find(|finding| finding.affected().get(1).map(ElementId::as_str) == Some(target))
            else {
                panic!("{row}: {what}: no finding on {target}: {found:?}");
            };
            assert_wrong_type(
                row,
                &what,
                finding,
                "11.13",
                ("x", target),
                &[end, kind, declared],
            );
        }
    }
}

// --- Item 3: structure, referencedArtifactElement and a group's members ------------------

#[test]
fn sacm23_val_004_a_wrong_structure_reference_or_member_is_one_error_with_its_clause() {
    let row = VAL_004;
    for (what, kind, attributes, clause, target, named) in [
        (
            "structure=\"c1\"",
            "ArgumentReasoning",
            " structure=\"c1\"",
            "11.12",
            "c1",
            ["structure", "Claim", "ArgumentPackage"],
        ),
        (
            "referencedArtifactElement=\"t1\"",
            "ArtifactReference",
            " referencedArtifactElement=\"t1\"",
            "11.9",
            "t1",
            [
                "referencedArtifactElement",
                "TaggedValue",
                "ArtifactElement",
            ],
        ),
        (
            "a group's argumentElement=\"case\"",
            "ArgumentGroup",
            " argumentElement=\"case\"",
            "11.2",
            "case",
            [
                "argumentElement",
                "AssuranceCasePackage",
                "ArgumentationElement",
            ],
        ),
    ] {
        let doc = read_ok(row, what, &x_in_case(kind, attributes));
        let found = validation::validate(&doc);
        let [finding] = found.as_slice() else {
            panic!("{row}: {what}: not one finding: {found:?}");
        };
        assert_wrong_type(row, what, finding, clause, ("x", target), &named);
    }
}

#[test]
fn sacm23_val_004_every_end_reports_exactly_the_targets_its_declared_type_does_not_admit() {
    let row = VAL_004;
    let mut failures = Vec::new();
    for end in ends() {
        for (target, kind) in TARGETS {
            let what = format!("{} {}=\"{target}\" naming a {kind}", end.referrer, end.name);
            let input = x_in_case(
                end.referrer,
                &format!("{} {}=\"{target}\"", end.others, end.name),
            );
            let doc = read_ok(row, &what, &input);
            let found = of_row(&doc, row);
            let expected = if end.declared.admits(kind) {
                Vec::new()
            } else {
                vec![vec!["x".to_owned(), target.to_owned()]]
            };
            if affected(&found) != expected {
                failures.push(format!(
                    "{what} ({} admits it: {}): {found:?}",
                    end.declared.name(),
                    end.declared.admits(kind)
                ));
                continue;
            }
            let misdescribed = found.iter().find(|finding| {
                finding.clause() != Some(end.clause)
                    || finding.code() != Code::REF_WRONG_TYPE
                    || !finding.message().contains(end.declared.name())
            });
            if let Some(finding) = misdescribed {
                failures.push(format!(
                    "{what}: not clause {} naming {}: {finding:?}",
                    end.clause,
                    end.declared.name()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{row}: {} references are not typed as declared:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// --- Item 4: what is not wrongly typed -------------------------------------------------

#[test]
fn sacm23_val_004_cited_element_or_abstract_form_naming_a_note_gives_no_ref_002() {
    let row = VAL_004;
    for attributes in [
        " isCitation=\"true\" citedElement=\"n1\"",
        " abstractForm=\"n1\"",
        " isCitation=\"true\" citedElement=\"t1\" abstractForm=\"d1\"",
    ] {
        let what = format!("a Claim with{attributes}");
        let doc = read_ok(row, &what, &x_in_case("Claim", attributes));
        assert_eq!(
            with_code(&doc, Code::REF_WRONG_TYPE),
            [],
            "{row}: {what}: a SACMElement end naming a UtilityElement is reported as wrongly typed"
        );
    }
}

#[test]
fn sacm23_val_004_a_dangling_id_or_a_name_id_gives_only_ref_001() {
    let row = VAL_004;
    let name_id = in_case(&format!(
        "{}{}",
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"y\"><name xmi:id=\"y_name\" content=\"Y\"/>\
         </argumentElement>",
        typed("ArgumentGroup", "x", " argumentElement=\"y_name nope\"", "")
    ));
    for (what, input, missing) in [
        (
            "metaClaim=\"nope\"".to_owned(),
            x_in_case("Claim", " metaClaim=\"nope\""),
            vec!["nope"],
        ),
        (
            "structure=\"nope\"".to_owned(),
            x_in_case("ArgumentReasoning", " structure=\"nope\""),
            vec!["nope"],
        ),
        (
            "a member naming a name's id and a missing id".to_owned(),
            name_id,
            // In Diagnostic order: by the affected ids.
            vec!["nope", "y_name"],
        ),
    ] {
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            with_code(&doc, Code::REF_WRONG_TYPE),
            [],
            "{row}: {what}: a dangling id is reported as wrongly typed"
        );
        let dangling = with_code(&doc, Code::REF_DANGLING);
        assert_eq!(
            affected(&dangling),
            missing
                .iter()
                .map(|id| vec!["x".to_owned(), (*id).to_owned()])
                .collect::<Vec<_>>(),
            "{row}: {what}: not one SACM-REF-001 per missing id"
        );
    }
}

#[test]
fn sacm23_val_004_a_repeated_wrongly_typed_source_is_one_error_and_one_repeat_warning() {
    let row = VAL_004;
    let what = "source=\"n1 n1\"";
    let doc = read_ok(
        row,
        what,
        &x_in_case("AssertedInference", " source=\"n1 n1\" target=\"c1\""),
    );
    let wrong = of_row(&doc, row);
    assert_eq!(
        affected(&wrong),
        [["x", "n1"]],
        "{row}: {what}: not one SACM-REF-002 for the repeated id: {wrong:?}"
    );
    let repeats = of_row(&doc, "SACM23-VAL-022");
    assert_eq!(
        (
            affected(&repeats),
            repeats.first().map(Diagnostic::severity)
        ),
        (
            vec![vec!["x".to_owned(), "n1".to_owned()]],
            Some(Severity::Warning)
        ),
        "{row}: {what}: not one SACM23-VAL-022 Warning: {repeats:?}"
    );
    assert_eq!(
        validation::validate(&doc).len(),
        2,
        "{row}: {what}: other findings than the two"
    );
}

// --- Item 5: loading and saving -----------------------------------------------------------

#[test]
fn sacm23_val_004_a_strict_write_refuses_a_tolerant_write_keeps_the_bytes_and_both_loads_succeed() {
    let row = VAL_004;
    for (what, kind, attributes) in [
        (
            "a meta-claim naming a reasoning",
            "Claim",
            " metaClaim=\"why\"",
        ),
        (
            "a Note source",
            "AssertedInference",
            " source=\"n1\" target=\"c1\"",
        ),
        (
            "a Claim structure",
            "ArgumentReasoning",
            " structure=\"c1\"",
        ),
        (
            "a TaggedValue artifact element",
            "ArtifactReference",
            " referencedArtifactElement=\"t1\"",
        ),
        (
            "a case member",
            "ArgumentGroup",
            " argumentElement=\"case\"",
        ),
    ] {
        // Both loads succeed: read_ok reads in both modes.
        let doc = read_ok(row, what, &x_in_case(kind, attributes));
        let expected = of_row(&doc, row);
        assert_eq!(expected.len(), 1, "{row}: {what}: setup: {expected:?}");
        assert_strict_write_refuses(row, what, &doc, &expected);
        let text = tolerant_round_trip(row, what, &doc);
        let Some(line) = text.lines().find(|line| line.contains("xmi:id=\"x\"")) else {
            panic!("{row}: {what}: x is not written\n{text}");
        };
        assert!(
            line.contains(attributes.trim_start()),
            "{row}: {what}: the reference is not written back\n{text}"
        );
    }
}

#[test]
fn sacm23_val_004_the_invalid_fixture_gives_one_error_on_its_claim() {
    let row = VAL_004;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sacm23/validation/invalid/meta-claim-wrong-type.sacm.xmi");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let doc = read_ok(row, "the fixture", &text);
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{row}: the fixture: not one finding: {found:?}");
    };
    assert_wrong_type(
        row,
        "the fixture",
        finding,
        "11.10",
        ("c2", "r1"),
        &["metaClaim", "ArgumentReasoning", "Claim"],
    );
}

// --- Item 6: every create refuses a wrongly typed end ------------------------------------

#[test]
fn sacm23_val_004_the_reasoning_inference_and_context_creates_refuse_a_wrongly_typed_end_unchanged()
{
    let row = VAL_004;
    let asset = AbstractClass::ArgumentAsset.name();
    let reasoning = ElementKind::ArgumentReasoning.name();
    let mut doc = built(row);
    let name = || named("New");
    let cases: Vec<RefusedCase<'_>> = vec![
        (
            "CreateArgumentReasoning with a Claim structure",
            CreateArgumentReasoning::new("p", name())
                .with_structure("c1")
                .into(),
            (
                "structure",
                "c1",
                ElementKind::Claim,
                ElementKind::ArgumentPackage.name(),
            ),
        ),
        (
            "CreateArgumentReasoning with the case as structure",
            CreateArgumentReasoning::new("p", name())
                .with_structure("case")
                .into(),
            (
                "structure",
                "case",
                ElementKind::AssuranceCasePackage,
                ElementKind::ArgumentPackage.name(),
            ),
        ),
        (
            "CreateAssertedInference with a package source",
            CreateAssertedInference::new("p", name(), ["c2", "p"], "c1").into(),
            ("source", "p", ElementKind::ArgumentPackage, asset),
        ),
        (
            "CreateAssertedInference with a group target",
            CreateAssertedInference::new("p", name(), ["c2"], "g1").into(),
            ("target", "g1", ElementKind::ArgumentGroup, asset),
        ),
        (
            "CreateAssertedInference with a Claim reasoning",
            CreateAssertedInference::new("p", name(), ["c2"], "c1")
                .with_reasoning("c2")
                .into(),
            ("reasoning", "c2", ElementKind::Claim, reasoning),
        ),
        (
            "CreateAssertedContext with a Note source",
            CreateAssertedContext::new("p", name(), ["n1"], "c1").into(),
            ("source", "n1", ElementKind::Note, asset),
        ),
        (
            "CreateAssertedContext with a package target",
            CreateAssertedContext::new("p", name(), ["c2"], "p").into(),
            ("target", "p", ElementKind::ArgumentPackage, asset),
        ),
        (
            "CreateAssertedContext with a reasoning naming an ArtifactReference",
            CreateAssertedContext::new("p", name(), ["c2"], "c1")
                .with_reasoning("r1")
                .into(),
            ("reasoning", "r1", ElementKind::ArtifactReference, reasoning),
        ),
    ];
    for (what, operation, expected) in cases {
        let error = refuse(row, what, &mut doc, operation);
        assert_refused_type(row, what, &error, expected);
    }
}

#[test]
fn sacm23_val_004_this_slices_five_creates_refuse_a_wrongly_typed_end_unchanged() {
    let row = VAL_004;
    let asset = AbstractClass::ArgumentAsset.name();
    let reasoning = ElementKind::ArgumentReasoning.name();
    let mut doc = built(row);
    let name = || named("New");
    let cases: Vec<RefusedCase<'_>> = vec![
        (
            "CreateArtifactReference with a Note",
            CreateArtifactReference::new("p", name())
                .with_referenced_artifact_elements(["c1", "n1"])
                .into(),
            (
                "referencedArtifactElement",
                "n1",
                ElementKind::Note,
                AbstractClass::ArtifactElement.name(),
            ),
        ),
        (
            "CreateArgumentGroup with the case",
            CreateArgumentGroup::new("p", name())
                .with_group_members(["case"])
                .into(),
            (
                "argumentElement",
                "case",
                ElementKind::AssuranceCasePackage,
                AbstractClass::ArgumentationElement.name(),
            ),
        ),
        (
            "CreateAssertedEvidence with a group source",
            CreateAssertedEvidence::new("p", name(), ["r1", "g1"], "c1").into(),
            ("source", "g1", ElementKind::ArgumentGroup, asset),
        ),
        (
            "CreateAssertedArtifactSupport with a Note target",
            CreateAssertedArtifactSupport::new("p", name(), ["r1"], "n1").into(),
            ("target", "n1", ElementKind::Note, asset),
        ),
        (
            "CreateAssertedArtifactContext with a Claim reasoning",
            CreateAssertedArtifactContext::new("p", name(), ["r1"], "r2")
                .with_reasoning("c1")
                .into(),
            ("reasoning", "c1", ElementKind::Claim, reasoning),
        ),
    ];
    for (what, operation, expected) in cases {
        let error = refuse(row, what, &mut doc, operation);
        assert_refused_type(row, what, &error, expected);
    }
}

#[test]
fn sacm23_val_004_a_wrong_reference_type_refusal_displays_its_end_id_and_both_types_with_a_or_an() {
    let row = VAL_004;
    let mut doc = built(row);
    let name = || named("New");
    let cases: Vec<(&str, Operation, &str)> = vec![
        (
            "a Claim as structure",
            CreateArgumentReasoning::new("p", name())
                .with_structure("c1")
                .into(),
            "the structure \"c1\" names a Claim, and the structure must be an ArgumentPackage",
        ),
        (
            "the case as structure",
            CreateArgumentReasoning::new("p", name())
                .with_structure("case")
                .into(),
            "the structure \"case\" names an AssuranceCasePackage, \
             and the structure must be an ArgumentPackage",
        ),
        (
            "a Note as referenced artifact element",
            CreateArtifactReference::new("p", name())
                .with_referenced_artifact_elements(["n1"])
                .into(),
            "the referencedArtifactElement \"n1\" names a Note, \
             and the referencedArtifactElement must be an ArtifactElement",
        ),
        (
            "an ArtifactReference as reasoning",
            CreateAssertedContext::new("p", name(), ["c2"], "c1")
                .with_reasoning("r1")
                .into(),
            "the reasoning \"r1\" names an ArtifactReference, \
             and the reasoning must be an ArgumentReasoning",
        ),
    ];
    for (what, operation, expected) in cases {
        let error = refuse(row, what, &mut doc, operation);
        assert_eq!(
            error.to_string(),
            expected,
            "{row}: {what}: the WrongReferenceType message is not the pinned line"
        );
    }
}

#[test]
fn sacm23_val_004_a_create_checks_every_id_exists_before_it_checks_any_type() {
    let row = VAL_004;
    let mut doc = built(row);
    let name = || named("New");
    for (what, operation, end, id) in [
        (
            "a Note source and a missing target",
            Operation::from(CreateAssertedInference::new("p", name(), ["n1"], "nope")),
            "target",
            "nope",
        ),
        (
            "a package target and a missing reasoning",
            CreateAssertedContext::new("p", name(), ["c2"], "p")
                .with_reasoning("nope")
                .into(),
            "reasoning",
            "nope",
        ),
        (
            "a Note member and a missing one",
            CreateArgumentGroup::new("p", name())
                .with_group_members(["n1", "nope"])
                .into(),
            "argumentElement",
            "nope",
        ),
    ] {
        let error = refuse(row, what, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::ReferenceNotFound { end: found_end, id: found_id, .. }
                    if *found_end == end && found_id.as_str() == id
            ),
            "{row}: {what}: not ReferenceNotFound {{ {end}, {id} }} first: {error:?}"
        );
    }
    // The ends are typed in order: the sources, the target, the reasoning.
    let what = "a Note source, a package target and a Claim reasoning";
    let error = refuse(
        row,
        what,
        &mut doc,
        CreateAssertedInference::new("p", name(), ["c2", "n1"], "p")
            .with_reasoning("c1")
            .into(),
    );
    assert_refused_type(
        row,
        what,
        &error,
        (
            "source",
            "n1",
            ElementKind::Note,
            AbstractClass::ArgumentAsset.name(),
        ),
    );
    let what = "a package target and a Claim reasoning";
    let error = refuse(
        row,
        what,
        &mut doc,
        CreateAssertedInference::new("p", name(), ["c2"], "p")
            .with_reasoning("c1")
            .into(),
    );
    assert_refused_type(
        row,
        what,
        &error,
        (
            "target",
            "p",
            ElementKind::ArgumentPackage,
            AbstractClass::ArgumentAsset.name(),
        ),
    );
}

#[test]
fn sacm23_val_004_a_well_typed_create_of_each_kind_succeeds_and_validates_clean() {
    let row = VAL_004;
    let mut doc = built(row);
    let name = || named("New");
    for operation in [
        Operation::from(CreateArgumentReasoning::new("p", name()).with_structure("p")),
        CreateAssertedInference::new("p", name(), ["c2", "r1", "why"], "c1")
            .with_reasoning("why")
            .into(),
        CreateAssertedContext::new("p", name(), ["r1"], "c1").into(),
        CreateArtifactReference::new("p", name())
            .with_referenced_artifact_elements(["case", "p", "c1", "g1"])
            .into(),
        CreateArgumentGroup::new("p", name())
            .with_group_members(["p", "c1", "why", "r1", "g1"])
            .into(),
        CreateAssertedEvidence::new("p", name(), ["r1"], "c1").into(),
        CreateAssertedArtifactSupport::new("p", name(), ["r1"], "r2").into(),
        CreateAssertedArtifactContext::new("p", name(), ["r2"], "r1").into(),
    ] {
        create(row, &mut doc, operation);
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the well-typed creates give a finding"
    );
}

// --- Item 7: the code and the catalog ---------------------------------------------------

#[test]
fn sacm23_val_004_ref_wrong_type_is_sacm_ref_002_and_its_catalog_row_names_it_and_the_row() {
    let row = VAL_004;
    assert_eq!(
        Code::REF_WRONG_TYPE.as_str(),
        "SACM-REF-002",
        "{row}: Code::REF_WRONG_TYPE is not SACM-REF-002"
    );
    let line = catalog_row(row, "SACM-REF-002");
    assert!(
        line.starts_with("| SACM-REF-002 | Error |"),
        "{row}: the catalog's SACM-REF-002 row is not an Error: {line}"
    );
    for part in ["`Code::REF_WRONG_TYPE`", "SACM23-VAL-004"] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-REF-002 row does not name {part}: {line}"
        );
    }
    // SACM-REF-003 is LIB-COMPAT-003's: no constant names it.
    let reserved = catalog_row(row, "SACM-REF-003");
    assert!(
        !reserved.contains("`Code::"),
        "{row}: the catalog's SACM-REF-003 row names a constant: {reserved}"
    );
}

#[test]
fn sacm23_val_004_no_document_yields_sacm_ref_003() {
    let row = VAL_004;
    let mut documents: Vec<(String, Document)> = Vec::new();
    for end in ends() {
        for (target, _) in TARGETS {
            let input = x_in_case(
                end.referrer,
                &format!("{} {}=\"{target}\"", end.others, end.name),
            );
            documents.push((
                format!("{} {}={target}", end.referrer, end.name),
                read_ok(row, "a reference", &input),
            ));
        }
    }
    // Every fixture, valid or not, that reads.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sacm23");
    let mut pending = vec![root];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let read = std::fs::read(&path).ok().and_then(|bytes| {
                    xmi::read(&bytes, &ReadOptions::default().with_mode(Mode::Tolerant)).ok()
                });
                if let Some(output) = read {
                    documents.push((path.display().to_string(), output.into_document()));
                }
            }
        }
    }
    for (what, doc) in &documents {
        let found: Vec<Diagnostic> = validation::validate(doc)
            .into_iter()
            .filter(|finding| finding.code().as_str() == "SACM-REF-003")
            .collect();
        assert_eq!(found, [], "{row}: {what} yields SACM-REF-003");
    }
}

// =====================================================================================
// SACM23-VAL-016
// =====================================================================================

// --- Item 1: an AssertedEvidence's source --------------------------------------------------

#[test]
fn sacm23_val_016_an_asserted_evidence_from_a_claim_is_one_error_with_clause_11_15() {
    let row = VAL_016;
    let what = "an AssertedEvidence with source c1";
    let doc = read_ok(
        row,
        what,
        &x_in_case("AssertedEvidence", " source=\"c1\" target=\"c2\""),
    );
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{row}: {what}: not one finding: {found:?}");
    };
    assert_wrong_type(
        row,
        what,
        finding,
        "11.15",
        ("x", "c1"),
        &["source", "ArtifactReference"],
    );
    // Its target is never checked here: any ArgumentAsset gives nothing.
    for target in ["c1", "why", "r1", "i1"] {
        let what = format!("an AssertedEvidence from r1 to {target}");
        let doc = read_ok(
            row,
            &what,
            &x_in_case(
                "AssertedEvidence",
                &format!(" source=\"r1\" target=\"{target}\""),
            ),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
}

#[test]
fn sacm23_val_016_a_repeated_claim_source_is_one_finding() {
    let row = VAL_016;
    let what = "sources \"r1 c1 c1\"";
    let doc = read_ok(
        row,
        what,
        &x_in_case("AssertedEvidence", " source=\"r1 c1 c1\" target=\"c2\""),
    );
    let found = of_row(&doc, row);
    assert_eq!(
        affected(&found),
        [["x", "c1"]],
        "{row}: {what}: not one finding, for c1: {found:?}"
    );
    // Also an ArgumentReasoning and a relationship source, each once, in
    // Diagnostic order (by the affected ids).
    let what = "sources \"why r1 i1 c1\"";
    let doc = read_ok(
        row,
        what,
        &x_in_case("AssertedEvidence", " source=\"why r1 i1 c1\" target=\"c2\""),
    );
    assert_eq!(
        affected(&of_row(&doc, row)),
        [["x", "c1"], ["x", "i1"], ["x", "why"]],
        "{row}: {what}: not one finding per non-ArtifactReference source"
    );
}

// --- Item 3: AssertedArtifactSupport and AssertedArtifactContext --------------------------

fn artifact_kind_ends_must_be_artifact_references(kind: &str, clause: &str) {
    let row = VAL_016;
    for (attributes, expected) in [
        (" source=\"r1\" target=\"c1\"", vec![["x", "c1"]]),
        (" source=\"c1\" target=\"r2\"", vec![["x", "c1"]]),
        (
            " source=\"c1\" target=\"c2\"",
            vec![["x", "c1"], ["x", "c2"]],
        ),
        (" source=\"r1 r2\" target=\"r2\"", Vec::new()),
    ] {
        let what = format!("an {kind} with{attributes}");
        let doc = read_ok(row, &what, &x_in_case(kind, attributes));
        let found = of_row(&doc, row);
        assert_eq!(
            affected(&found),
            expected
                .iter()
                .map(|pair| pair.map(str::to_owned).to_vec())
                .collect::<Vec<_>>(),
            "{row}: {what}: not the findings expected: {found:?}"
        );
        for finding in &found {
            let target = finding.affected()[1].as_str().to_owned();
            assert_wrong_type(
                row,
                &what,
                finding,
                clause,
                ("x", &target),
                &["ArtifactReference"],
            );
        }
        assert_eq!(
            validation::validate(&doc).len(),
            found.len(),
            "{row}: {what}: another rule reports"
        );
    }
}

#[test]
fn sacm23_val_016_an_asserted_artifact_support_needs_artifact_references_at_both_ends_clause_11_17()
{
    artifact_kind_ends_must_be_artifact_references("AssertedArtifactSupport", "11.17");
}

#[test]
fn sacm23_val_016_an_asserted_artifact_context_needs_artifact_references_at_both_ends_clause_11_18()
{
    artifact_kind_ends_must_be_artifact_references("AssertedArtifactContext", "11.18");
}

// --- Item 4: AssertedInference and AssertedContext --------------------------------------

#[test]
fn sacm23_val_016_inference_and_context_with_artifact_reference_or_claim_ends_give_no_finding() {
    let row = VAL_016;
    for kind in ["AssertedInference", "AssertedContext"] {
        for attributes in [
            " source=\"r1\" target=\"r2\"",
            " source=\"c1\" target=\"c2\"",
            " source=\"r1 c1\" target=\"c2\"",
            " source=\"c1\" target=\"r1\"",
        ] {
            let what = format!("an {kind} with{attributes}");
            let doc = read_ok(row, &what, &x_in_case(kind, attributes));
            assert_eq!(
                validation::validate(&doc),
                [],
                "{row}: {what}: validate reports a finding"
            );
        }
    }
}

// --- Item 5: what the generic rules already report ---------------------------------------

#[test]
fn sacm23_val_016_a_dangling_note_or_missing_end_gives_only_the_generic_finding() {
    let row = VAL_016;
    for kind in [
        "AssertedEvidence",
        "AssertedArtifactSupport",
        "AssertedArtifactContext",
    ] {
        let target = if kind == "AssertedEvidence" {
            "c1"
        } else {
            "r2"
        };
        for (what, attributes, only) in [
            (
                "a dangling source",
                format!(" source=\"nope\" target=\"{target}\""),
                "SACM23-VAL-003",
            ),
            (
                "a Note source",
                format!(" source=\"n1\" target=\"{target}\""),
                VAL_004,
            ),
            (
                "a missing target",
                " source=\"r1\"".to_owned(),
                "SACM23-VAL-005",
            ),
        ] {
            let what = format!("an {kind} with {what}");
            let doc = read_ok(row, &what, &x_in_case(kind, &attributes));
            let found = validation::validate(&doc);
            let [finding] = found.as_slice() else {
                panic!("{row}: {what}: not one finding: {found:?}");
            };
            assert_eq!(
                finding.requirement_id(),
                only,
                "{row}: {what}: not only {only}: {finding:?}"
            );
        }
        // A Note target of the artifact kinds: only SACM23-VAL-004.
        if kind != "AssertedEvidence" {
            let what = format!("an {kind} with a Note target");
            let doc = read_ok(row, &what, &x_in_case(kind, " source=\"r1\" target=\"n1\""));
            let found = validation::validate(&doc);
            assert_eq!(
                found
                    .iter()
                    .map(Diagnostic::requirement_id)
                    .collect::<Vec<_>>(),
                [VAL_004],
                "{row}: {what}: not only SACM23-VAL-004: {found:?}"
            );
        }
    }
}

// --- Item 6: saving and creating ----------------------------------------------------------

#[test]
fn sacm23_val_016_a_strict_write_refuses_and_create_asserted_evidence_from_a_claim_succeeds() {
    let row = VAL_016;
    for (kind, attributes) in [
        ("AssertedEvidence", " source=\"c1\" target=\"c2\""),
        ("AssertedArtifactSupport", " source=\"r1\" target=\"c1\""),
        ("AssertedArtifactContext", " source=\"c1\" target=\"r1\""),
    ] {
        let what = format!("an {kind} with{attributes}");
        let doc = read_ok(row, &what, &x_in_case(kind, attributes));
        let expected = of_row(&doc, row);
        assert_strict_write_refuses(row, &what, &doc, &expected);
        tolerant_round_trip(row, &what, &doc);
    }
    // A create does not refuse it: it is content (D-VAL-LAYER).
    let mut doc = built(row);
    let id = create(
        row,
        &mut doc,
        CreateAssertedEvidence::new("p", named("From a Claim"), ["c1"], "c2"),
    );
    let found = of_row(&doc, row);
    assert_eq!(
        affected(&found),
        [[id.as_str().to_owned(), "c1".to_owned()]],
        "{row}: a created AssertedEvidence from a Claim is not reported"
    );
    for (what, operation) in [
        (
            "CreateAssertedArtifactSupport from a Claim",
            Operation::from(CreateAssertedArtifactSupport::new(
                "p",
                named("S"),
                ["c1"],
                "c2",
            )),
        ),
        (
            "CreateAssertedArtifactContext to a Claim",
            CreateAssertedArtifactContext::new("p", named("X"), ["r1"], "c2").into(),
        ),
    ] {
        let answer = edit::apply(&mut doc, operation);
        assert!(answer.is_ok(), "{row}: {what} is refused: {answer:?}");
    }
}

#[test]
fn sacm23_val_016_the_invalid_fixture_gives_one_error_on_its_evidence() {
    let row = VAL_016;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sacm23/validation/invalid/asserted-evidence-claim-source.sacm.xmi");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let doc = read_ok(row, "the fixture", &text);
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{row}: the fixture: not one finding: {found:?}");
    };
    assert_wrong_type(
        row,
        "the fixture",
        finding,
        "11.15",
        ("e1", "c2"),
        &["source", "ArtifactReference"],
    );
}
