//! Tests for the §8.6 implementation-constraint rule of `validation::validate`
//! (SACM23-VAL-021, SACM-ABS-001; formal/23-05-08 §8.6
//! "`ImplementationConstraints` should only be specified if +isAbstract is true
//! OCL: `self.implmentationConstraint->size() > 0 implies self.isAbstract =
//! true`", spec defect D15; §8.2 "isAbstract[0..1]=false"; D-SEV,
//! D-VAL-LAYER; ADR 0022 amendment 1, ADR 0029).
//!
//! `validate` reports each `ModelElement` that has one or more
//! `implementationConstraint` children and whose `isAbstract` is not true as
//! one SACM-ABS-001 Error, with requirement SACM23-VAL-021, clause `8.6`,
//! affected `[element]` and no location. A strict save then refuses the
//! document; reading and editing do not. It is independent of SACM-ABS-001's
//! clause 8.2 findings (SACM23-VAL-008).
//!
//! Every document is read from inline XMI, with every `ModelElement` named
//! and every relationship given valid ends, so that no other rule reports;
//! the edit path is built with `edit::apply`. Messages are checked only for
//! what they must name.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::{
    self, CreateArgumentPackage, CreateClaim, CreateImplementationConstraint, SetFlag,
};
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "SACM23-VAL-021";
const VAL_008: &str = "SACM23-VAL-008";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// One constraint, with no content (`content:MultiLangString[0..1]`).
const ONE: &str = "<implementationConstraint xmi:id=\"ic1\"/>";

/// One constraint with a content.
const ONE_WITH_CONTENT: &str = "<implementationConstraint xmi:id=\"ic1\">\
     <content><value content=\"{System} is named\"/></content></implementationConstraint>";

/// Two constraints.
const TWO: &str = "<implementationConstraint xmi:id=\"ic1\"/>\
     <implementationConstraint xmi:id=\"ic2\"><content><value content=\"c\"/></content>\
     </implementationConstraint>";

/// The other three `UtilityElement` kinds, one of each.
const OTHER_UTILITIES: &str = "<description xmi:id=\"d1\"><content><value content=\"d\"/></content>\
     </description><note xmi:id=\"n1\"/>\
     <taggedValue xmi:id=\"t1\"><key><value content=\"k\"/></key></taggedValue>";

// --- Inputs ------------------------------------------------------------------------

/// The input given the start-tag attributes and the content, after its
/// name, of the element `x`.
type Input = fn(&str, &str) -> String;

/// An ArgumentPackage root `p` named "P" holding the Claims `c1` and `c2`,
/// the `ArtifactReferences` `a1` and `a2`, and the element `x` of `kind` named
/// "X", with the `source` and `target` that make it valid when it is a
/// relationship, the start-tag `attributes`, and `content` after its name.
fn in_package(kind: ElementKind, attributes: &str, content: &str) -> String {
    let ends = match kind {
        ElementKind::AssertedInference | ElementKind::AssertedContext => {
            " source=\"c1\" target=\"c2\""
        }
        ElementKind::AssertedEvidence => " source=\"a1\" target=\"c1\"",
        ElementKind::AssertedArtifactSupport | ElementKind::AssertedArtifactContext => {
            " source=\"a1\" target=\"a2\""
        }
        _ => "",
    };
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"a1\"><name content=\"A1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"a2\"><name content=\"A2\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{ends}{attributes}><name content=\"X\"/>\
         {content}</argumentElement></sacm:ArgumentPackage>"
    )
}

/// Every readable `ModelElement` kind as the element `x`: both package
/// roots, an ArgumentPackage nested in an AssuranceCasePackage, and each
/// kind an ArgumentPackage holds but a nested ArgumentPackage (which would be
/// SACM-PKG-001 beside the other elements).
fn holders() -> [(&'static str, Input); 12] {
    [
        (
            "an AssuranceCasePackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}><name content=\"X\"/>\
                     {content}</sacm:AssuranceCasePackage>"
                )
            },
        ),
        (
            "an ArgumentPackage root",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"x\"{attributes}><name content=\"X\"/>\
                 {content}</sacm:ArgumentPackage>"
                )
            },
        ),
        (
            "an ArgumentPackage in an AssuranceCasePackage",
            |attributes: &str, content: &str| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                     <argumentPackage xmi:id=\"x\"{attributes}><name content=\"X\"/>{content}\
                     </argumentPackage></sacm:AssuranceCasePackage>"
                )
            },
        ),
        ("a Claim", |attributes: &str, content: &str| {
            in_package(ElementKind::Claim, attributes, content)
        }),
        ("an ArgumentReasoning", |attributes: &str, content: &str| {
            in_package(ElementKind::ArgumentReasoning, attributes, content)
        }),
        ("an ArtifactReference", |attributes: &str, content: &str| {
            in_package(ElementKind::ArtifactReference, attributes, content)
        }),
        ("an ArgumentGroup", |attributes: &str, content: &str| {
            in_package(ElementKind::ArgumentGroup, attributes, content)
        }),
        ("an AssertedInference", |attributes: &str, content: &str| {
            in_package(ElementKind::AssertedInference, attributes, content)
        }),
        ("an AssertedEvidence", |attributes: &str, content: &str| {
            in_package(ElementKind::AssertedEvidence, attributes, content)
        }),
        ("an AssertedContext", |attributes: &str, content: &str| {
            in_package(ElementKind::AssertedContext, attributes, content)
        }),
        (
            "an AssertedArtifactSupport",
            |attributes: &str, content: &str| {
                in_package(ElementKind::AssertedArtifactSupport, attributes, content)
            },
        ),
        (
            "an AssertedArtifactContext",
            |attributes: &str, content: &str| {
                in_package(ElementKind::AssertedArtifactContext, attributes, content)
            },
        ),
    ]
}

// --- Reading, validating and writing -----------------------------------------------

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

/// The findings of `validate` on `doc`, which must all be of the rows in
/// `rows`.
fn findings(what: &str, doc: &Document, rows: &[&str]) -> Vec<Diagnostic> {
    let found = validation::validate(doc);
    for finding in &found {
        assert!(
            rows.contains(&finding.requirement_id()),
            "{ROW}: {what}: setup: another rule reports {finding:?}"
        );
    }
    found
}

/// Reads `input` and asserts that `validate` gives no finding.
fn no_finding(what: &str, input: &str) {
    let found = validation::validate(&read_ok(what, input));
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
}

/// Asserts that `found` is a SACM-ABS-001 Error of this row, clause `8.6`,
/// no location, affecting exactly `[id]`, with a one-line message naming
/// the `{:?}`-quoted id and `isAbstract`.
fn assert_error(what: &str, found: &Diagnostic, id: &str) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::ABSTRACT_FORM_INVALID, Severity::Error, ROW),
        "{ROW}: {what}: not a SACM-ABS-001 Error of this row: {found:?}"
    );
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        ([ElementId::from(id)].as_slice(), None, Some("8.6")),
        "{ROW}: {what}: the affected ids, location or clause of {found:?}"
    );
    let message = found.message();
    assert!(
        message.contains(&format!("{id:?}"))
            && message.contains("isAbstract")
            && !message.contains('\n'),
        "{ROW}: {what}: the message {message:?} is not one line naming {id:?} and isAbstract"
    );
}

/// Reads `input`, validates it, and asserts its one finding is this row's
/// Error on `x`. Returns the document and the finding.
fn one_error_on_x(what: &str, input: &str) -> (Document, Diagnostic) {
    let doc = read_ok(what, input);
    let found = findings(what, &doc, &[ROW]);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_error(what, finding, "x");
    let finding = finding.clone();
    (doc, finding)
}

/// A strict write of `doc` must be `WriteError::Invalid` holding `expected`.
fn assert_strict_write_refuses(what: &str, doc: &Document, expected: &Diagnostic) {
    let error = match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: {what}: a strict write returned:\n{text}"),
        Err(error) => error,
    };
    let WriteError::Invalid { diagnostics, .. } = &error else {
        panic!("{ROW}: {what}: the strict refusal is not Invalid: {error:?}");
    };
    assert!(
        diagnostics.contains(expected),
        "{ROW}: {what}: the strict refusal does not hold {expected:?}: {diagnostics:?}"
    );
}

/// Writes `doc` in `mode`, which must succeed; the output must read back
/// equal. Returns the output.
fn write_round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused: {error:?}"),
    };
    let back = read_ok(&format!("{what}, written in {mode:?} mode"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the {mode:?} output does not read back equal\n{text}"
    );
    text
}

/// The ids of the implementation constraints of `id` in `doc`.
fn constraints_of(doc: &Document, id: &str) -> Vec<String> {
    doc.get(id)
        .map(|element| {
            element
                .implementation_constraints()
                .map(|c| c.id().as_str().to_owned())
                .collect()
        })
        .unwrap_or_default()
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

// --- Acceptance 1: one Error per concrete element with constraints ------------------

#[test]
fn sacm23_val_021_the_setup_gives_no_finding_without_a_constraint() {
    // Without a constraint, every document these tests build is valid, so
    // each finding below is this row's.
    for (holder, input_of) in holders() {
        no_finding(&format!("{holder} with no constraint"), &input_of("", ""));
    }
}

#[test]
fn sacm23_val_021_a_concrete_element_of_each_readable_kind_with_a_constraint_gives_one_error() {
    for (holder, input_of) in holders() {
        for (flag, attributes) in [
            ("no isAbstract", ""),
            ("isAbstract=\"false\"", " isAbstract=\"false\""),
            ("isAbstract=\"0\"", " isAbstract=\"0\""),
        ] {
            for (constraints, content) in [
                ("one constraint", ONE),
                ("one constraint with a content", ONE_WITH_CONTENT),
                ("two constraints", TWO),
                (
                    "a constraint among the other utility elements",
                    &format!("{OTHER_UTILITIES}{ONE}") as &str,
                ),
            ] {
                let what = format!("{holder} with {flag} and {constraints}");
                one_error_on_x(&what, &input_of(attributes, content));
            }
        }
    }
}

#[test]
fn sacm23_val_021_the_one_error_names_how_many_constraints_the_element_holds() {
    // One finding however many constraints, so the message is where the count
    // shows: "an implementationConstraint" for one, "<n>
    // implementationConstraints" for more.
    for (holder, input_of) in holders() {
        for (constraints, content, says, never) in [
            (
                "one constraint",
                ONE,
                "an implementationConstraint",
                "1 implementationConstraints",
            ),
            (
                "two constraints",
                TWO,
                "2 implementationConstraints",
                "an implementationConstraint",
            ),
        ] {
            let what = format!("{holder} with {constraints}");
            let (_, finding) = one_error_on_x(&what, &input_of("", content));
            let message = finding.message();
            assert!(
                message.contains(says) && !message.contains(never),
                "{ROW}: {what}: the message {message:?} does not say {says:?}"
            );
        }
    }
}

// --- Acceptance 2: abstract elements, and the other utility elements ----------------

#[test]
fn sacm23_val_021_an_abstract_element_with_constraints_gives_no_finding() {
    for (holder, input_of) in holders() {
        for (flag, attributes) in [
            ("isAbstract=\"true\"", " isAbstract=\"true\""),
            ("isAbstract=\"1\"", " isAbstract=\"1\""),
        ] {
            for (constraints, content) in [("one constraint", ONE), ("two constraints", TWO)] {
                no_finding(
                    &format!("{holder} with {flag} and {constraints}"),
                    &input_of(attributes, content),
                );
            }
        }
        no_finding(
            &format!("{holder} with <isAbstract>true</isAbstract> and one constraint"),
            &input_of("", &format!("<isAbstract>true</isAbstract>{ONE}")),
        );
    }
}

#[test]
fn sacm23_val_021_descriptions_notes_and_tagged_values_alone_give_no_finding() {
    for (holder, input_of) in holders() {
        for (flag, attributes) in [("concrete", ""), ("abstract", " isAbstract=\"true\"")] {
            no_finding(
                &format!("{holder}, {flag}, with a Description, a Note and a TaggedValue"),
                &input_of(attributes, OTHER_UTILITIES),
            );
        }
    }
}

// --- Acceptance 3: strict and tolerant writes ---------------------------------------

#[test]
fn sacm23_val_021_a_strict_write_refuses_and_a_tolerant_write_keeps_the_constraint() {
    for (holder, input_of) in holders() {
        let what = format!("{holder}, concrete, with two constraints");
        let (doc, finding) = one_error_on_x(&what, &input_of("", TWO));
        assert_eq!(
            constraints_of(&doc, "x"),
            ["ic1", "ic2"],
            "{ROW}: {what}: reading did not keep both constraints"
        );
        assert_strict_write_refuses(&what, &doc, &finding);
        let text = write_round_trip(&what, &doc, Mode::Tolerant);
        for id in ["ic1", "ic2"] {
            assert!(
                text.contains(&format!("<implementationConstraint xmi:id=\"{id}\"")),
                "{ROW}: {what}: the tolerant write drops the constraint {id:?}:\n{text}"
            );
        }
        // Made abstract, the same document is written strictly.
        let what = format!("{holder}, abstract, with two constraints");
        let doc = read_ok(&what, &input_of(" isAbstract=\"true\"", TWO));
        let text = write_round_trip(&what, &doc, Mode::Strict);
        assert!(
            text.contains("<implementationConstraint xmi:id=\"ic2\""),
            "{ROW}: {what}: the strict write drops a constraint:\n{text}"
        );
    }
}

// --- Acceptance 4: the edit path ---------------------------------------------------

#[test]
fn sacm23_val_021_create_implementation_constraint_then_set_is_abstract_clears_the_error() {
    let mut doc = Document::new();
    let setup = edit::apply(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    )
    .and_then(|_| {
        edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("{System} is safe")).with_id("c"),
        )
    });
    if let Err(error) = setup {
        panic!("{ROW}: setup: an edit was refused: {error:?}");
    }
    let found = validation::validate(&doc);
    assert!(
        found.is_empty(),
        "{ROW}: after the setup edits: findings: {found:?}"
    );

    if let Err(error) = edit::apply(
        &mut doc,
        CreateImplementationConstraint::new("c", LangString::new("{System} is named"))
            .with_id("ic"),
    ) {
        panic!(
            "{ROW}: CreateImplementationConstraint under a concrete Claim was refused: {error:?}"
        );
    }
    assert_eq!(
        constraints_of(&doc, "c"),
        ["ic"],
        "{ROW}: the constraint is not a child of the Claim"
    );
    let what = "after CreateImplementationConstraint(c)";
    let found = findings(what, &doc, &[ROW]);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_error(what, finding, "c");
    assert_strict_write_refuses(what, &doc, finding);

    if let Err(error) = edit::apply(&mut doc, SetFlag::new("c", Flag::IsAbstract, true)) {
        panic!("{ROW}: SetFlag(c, IsAbstract, true) was refused: {error:?}");
    }
    let what = "after SetFlag(c, IsAbstract, true)";
    let found = validation::validate(&doc);
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
    let text = write_round_trip(what, &doc, Mode::Strict);
    assert!(
        text.contains("<implementationConstraint xmi:id=\"ic\""),
        "{ROW}: {what}: the strict write drops the constraint:\n{text}"
    );

    // Making it concrete again brings the Error back: editing never refuses.
    if let Err(error) = edit::apply(&mut doc, SetFlag::new("c", Flag::IsAbstract, false)) {
        panic!("{ROW}: SetFlag(c, IsAbstract, false) was refused: {error:?}");
    }
    let what = "after SetFlag(c, IsAbstract, false)";
    let found = findings(what, &doc, &[ROW]);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_error(what, finding, "c");
}

// --- Acceptance 5: independent of the clause 8.2 findings ---------------------------

/// An ArgumentPackage root `p` holding the abstract Claim `pattern`, the
/// concrete Claim `other`, and the Claim `x` with `attributes` and the one
/// constraint `ic1`.
fn with_abstract_form(attributes: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"pattern\" isAbstract=\"true\">\
         <name content=\"Pattern\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"other\"><name content=\"Other\"/>\
         </argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}><name content=\"X\"/>\
         {ONE}</argumentElement></sacm:ArgumentPackage>"
    )
}

#[test]
fn sacm23_val_021_is_independent_of_the_clause_8_2_abstract_form_findings() {
    // Abstract, with an abstractForm: the clause 8.2 Error, and nothing of
    // this row.
    let what = "an abstract Claim with a constraint and an abstractForm";
    let doc = read_ok(
        what,
        &with_abstract_form(" isAbstract=\"true\" abstractForm=\"pattern\""),
    );
    let found = findings(what, &doc, &[ROW, VAL_008]);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_eq!(
        (
            finding.code(),
            finding.severity(),
            finding.requirement_id(),
            finding.affected(),
            finding.clause()
        ),
        (
            Code::ABSTRACT_FORM_INVALID,
            Severity::Error,
            VAL_008,
            [ElementId::from("x")].as_slice(),
            Some("8.2")
        ),
        "{ROW}: {what}: not the clause 8.2 Error on x: {finding:?}"
    );

    // Concrete, conforming to an abstract Claim: this row's Error only.
    let what = "a concrete Claim with a constraint, conforming to an abstract Claim";
    one_error_on_x(what, &with_abstract_form(" abstractForm=\"pattern\""));

    // Concrete, conforming to a concrete Claim: this row's Error, then the
    // clause 8.2 Warning on x and its target.
    let what = "a concrete Claim with a constraint, conforming to a concrete Claim";
    let doc = read_ok(what, &with_abstract_form(" abstractForm=\"other\""));
    let found = findings(what, &doc, &[ROW, VAL_008]);
    let [error, warning] = found.as_slice() else {
        panic!("{ROW}: {what}: not two findings: {found:?}");
    };
    assert_error(what, error, "x");
    assert_eq!(
        (
            warning.code(),
            warning.severity(),
            warning.requirement_id(),
            warning.affected(),
            warning.clause()
        ),
        (
            Code::ABSTRACT_FORM_INVALID,
            Severity::Warning,
            VAL_008,
            [ElementId::from("x"), ElementId::from("other")].as_slice(),
            Some("8.2")
        ),
        "{ROW}: {what}: the second finding is not the clause 8.2 Warning: {warning:?}"
    );
}

// --- Acceptance 6: the catalog row --------------------------------------------------

#[test]
fn sacm23_val_021_abstract_form_invalid_covers_it_and_its_catalog_row_names_the_row() {
    assert_eq!(
        Code::ABSTRACT_FORM_INVALID.as_str(),
        "SACM-ABS-001",
        "{ROW}: Code::ABSTRACT_FORM_INVALID is not SACM-ABS-001"
    );
    let line = catalog_row(Code::ABSTRACT_FORM_INVALID);
    assert!(
        line.starts_with("| SACM-ABS-001 | Error / Warning |"),
        "{ROW}: the catalog's SACM-ABS-001 row is not Error / Warning: {line}"
    );
    for part in [
        "`Code::ABSTRACT_FORM_INVALID`",
        "SACM23-VAL-021",
        "SACM23-VAL-008",
        "implementationConstraint",
        "8.6",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-ABS-001 row does not name {part}: {line}"
        );
    }
}
