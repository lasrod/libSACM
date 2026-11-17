//! Tests that a concrete Expression references only concrete
//! `ExpressionElements` (SACM23-VAL-017; formal/23-05-08 §10.10 OCL
//! "self.isAbstract = false implies self.element->forall(expr|expr.isAbstract
//! = false)."; §8.2 "isAbstract[0..1]=false"; D-SEV).
//!
//! `validate` reports, for each Expression whose `isAbstract` is not true,
//! each distinct id of its `element` end that names an abstract Term or
//! Expression of the document, as one SACM-ABS-001 Error with clause 10.10,
//! affecting `[expression, element]`. A dangling id, one of another kind and
//! one held in preserved content are the SACM-REF rules' alone. Reading,
//! editing and both saves never refuse it.
//!
//! The fixture is `validation/invalid/expression-abstract-element.sacm.xmi`.

use std::path::Path;

use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::{self, CreateExpression, Operation, SetFlag};
use sacm::model::{Document, ElementId, Flag, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "SACM23-VAL-017";

const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/validation/invalid/expression-abstract-element.sacm.xmi"
);

const CATALOG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/sacm/diagnostics.md"
);
const CODES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/diagnostic.rs");

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn text_of(path: &str) -> String {
    std::fs::read_to_string(Path::new(path))
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// Reads `input` in both modes, each with no diagnostic, to equal
/// documents; returns the tolerant one.
fn read_clean(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
            Ok(output) => {
                assert_eq!(
                    output.diagnostics(),
                    [],
                    "{ROW}: {what}: a {mode:?} read reports diagnostics"
                );
                documents.push(output.into_document());
            }
            Err(error) => panic!("{ROW}: {what}: a {mode:?} read refuses it: {error}"),
        }
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the strict and the tolerant read differ"
    );
    documents.remove(1)
}

/// Reads `input` tolerantly, whatever it reports.
fn read_tolerant(what: &str, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: a tolerant read refuses it: {error}"),
    }
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().map(|&value| ElementId::from(value)).collect()
}

fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) {
    let operation = operation.into();
    if let Err(error) = edit::apply(doc, operation.clone()) {
        panic!("{ROW}: {what}: {operation:?} was refused: {error}");
    }
}

/// The SACM-ABS-001 findings of `doc` with requirement SACM23-VAL-017, as
/// `(severity, clause, location, affected)`.
fn concrete_findings(doc: &Document) -> Vec<(Severity, Option<String>, bool, Vec<ElementId>)> {
    validation::validate(doc)
        .iter()
        .filter(|one| one.code() == Code::ABSTRACT_FORM_INVALID && one.requirement_id() == ROW)
        .map(|one| {
            (
                one.severity(),
                one.clause().map(str::to_owned),
                one.location().is_some(),
                one.affected().to_vec(),
            )
        })
        .collect()
}

fn finding(expression: &str, target: &str) -> (Severity, Option<String>, bool, Vec<ElementId>) {
    (
        Severity::Error,
        Some("10.10".to_owned()),
        false,
        ids(&[expression, target]),
    )
}

/// Every SACM-ABS-001 of `doc`, of any row.
fn abs_001(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|one| one.code() == Code::ABSTRACT_FORM_INVALID)
        .collect()
}

/// A root AssuranceCasePackage holding a TerminologyPackage `terms` with
/// `terms_body` and an ArgumentPackage `p` with `arguments_body`.
fn case(terms_body: &str, arguments_body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n\
         <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/>\n{terms_body}\n</terminologyPackage>\n\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>\n{arguments_body}\n</argumentPackage>\n\
         </sacm:AssuranceCasePackage>\n"
    )
}

/// A `TerminologyElement` of `kind` with `id` and the extra `attributes`.
fn terminology(kind: &str, id: &str, attributes: &str) -> String {
    format!(
        "<terminologyElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\" value=\"{id}\"{attributes}><name content=\"{id}\"/></terminologyElement>"
    )
}

// ---------------------------------------------------------------------------
// Acceptance 1: the fixture
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_017_the_fixture_gives_its_three_lines() {
    let doc = read_clean("the fixture", &text_of(FIXTURE));
    let found = validation::validate(&doc);
    let shown: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        [
            "error SACM-ABS-001 [SACM23-VAL-017, clause 10.10]: Expression \"guarded\" is concrete \
             but has the element \"guard\", an abstract Term: a concrete Expression references \
             only concrete ExpressionElements (§10.10)",
            "error SACM-ABS-001 [SACM23-VAL-017, clause 10.10]: Expression \"guarded\" is concrete \
             but has the element \"pattern\", an abstract Expression: a concrete Expression \
             references only concrete ExpressionElements (§10.10)",
            "warning SACM-MULT-001 [SACM23-VAL-022, clause 10.10]: Expression \"guarded\" holds \
             \"guard\" 2 times in its element, whose ids are unique in SACM 2.3 (the repeat is kept)",
        ],
        "{ROW}: the fixture's three lines"
    );
    // guard, held twice, gives one; pattern gives one; the abstract pattern
    // gives none for its own abstract guard.
    assert_eq!(
        concrete_findings(&doc),
        [finding("guarded", "guard"), finding("guarded", "pattern")],
        "{ROW}: the findings by severity, clause, location and affected ids"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 2: what is not this rule's
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_017_dangling_wrong_kind_preserved_and_concrete_elements_give_no_finding() {
    // A concrete Expression whose elements are concrete: no finding at all.
    let concrete = case(
        &format!(
            "{}{}{}",
            terminology("Term", "blade", ""),
            terminology("Expression", "inner", " element=\"blade\""),
            terminology("Expression", "outer", " element=\"blade inner\"")
        ),
        "",
    );
    let doc = read_clean("concrete elements", &concrete);
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: a concrete Expression of concrete elements has findings"
    );
    // Each other case: no SACM-ABS-001, and the one SACM-REF code.
    let cases = [
        (
            "a dangling element",
            case(&terminology("Expression", "e", " element=\"x\""), ""),
            Code::REF_DANGLING,
        ),
        (
            "an element naming an abstract Claim",
            case(
                &terminology("Expression", "e", " element=\"c\""),
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" isAbstract=\"true\"><name content=\"c\"/></argumentElement>",
            ),
            Code::REF_WRONG_TYPE,
        ),
        (
            "an element naming an id held in preserved content",
            case(
                &terminology("Expression", "e", " element=\"x1\""),
                "<argumentElement xmi:type=\"sacm:Term\" xmi:id=\"x1\" isAbstract=\"true\" value=\"x\"><name content=\"x1\"/></argumentElement>",
            ),
            Code::REF_PRESERVED_TARGET,
        ),
    ];
    for (what, input, code) in cases {
        let doc = read_tolerant(what, &input);
        assert_eq!(abs_001(&doc), [], "{ROW}: {what} gives SACM-ABS-001");
        let codes: Vec<Code> = validation::validate(&doc)
            .iter()
            .filter(|one| {
                one.affected()
                    .first()
                    .is_some_and(|first| first.as_str() == "e")
            })
            .map(Diagnostic::code)
            .collect();
        assert_eq!(codes, [code], "{ROW}: {what}: the findings of e");
    }
}

// ---------------------------------------------------------------------------
// Acceptance 3: edits change the findings, and never refuse
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_017_set_flag_removes_and_adds_findings() {
    let doc = read_clean("the fixture", &text_of(FIXTURE));
    assert_eq!(
        concrete_findings(&doc).len(),
        2,
        "{ROW}: setup: the fixture's two findings"
    );
    let mut abstract_guarded = doc.clone();
    apply(
        "make guarded abstract",
        &mut abstract_guarded,
        SetFlag::new("guarded", Flag::IsAbstract, true),
    );
    assert_eq!(
        concrete_findings(&abstract_guarded),
        [],
        "{ROW}: an abstract guarded still gives findings"
    );
    let mut abstract_blade = doc.clone();
    apply(
        "make blade abstract",
        &mut abstract_blade,
        SetFlag::new("blade", Flag::IsAbstract, true),
    );
    assert_eq!(
        concrete_findings(&abstract_blade),
        [
            finding("guarded", "blade"),
            finding("guarded", "guard"),
            finding("guarded", "pattern"),
        ],
        "{ROW}: an abstract blade gives one finding more, on guarded (pattern is abstract)"
    );
}

#[test]
fn sacm23_val_017_edits_and_strict_write_never_refuse_it() {
    let doc = read_clean("the fixture", &text_of(FIXTURE));
    let mut created = doc.clone();
    apply(
        "a concrete Expression of the abstract guard",
        &mut created,
        CreateExpression::new("terms", LangString::new("Guarded again"), "$Guard$")
            .with_id("again")
            .with_expression_elements(["guard"]),
    );
    if let Err(error) = xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        panic!("{ROW}: a strict write of the fixture is refused: {error}");
    }
    if let Err(error) = xmi::write(&created, &WriteOptions::default().with_mode(Mode::Strict)) {
        panic!("{ROW}: a strict write after the create is refused: {error}");
    }
}

// ---------------------------------------------------------------------------
// Acceptance 4: the catalog and the code's doc
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_017_the_catalog_and_the_code_doc_state_the_rule() {
    let catalog = text_of(CATALOG);
    let Some(row) = catalog
        .lines()
        .find(|line| line.starts_with("| SACM-ABS-001 |"))
    else {
        panic!("{ROW}: the catalog has no SACM-ABS-001 row");
    };
    assert!(
        row.contains(ROW)
            && row.contains("10.10")
            && !row.contains("The clause 10.10 part is reserved"),
        "{ROW}: the catalog's SACM-ABS-001 row does not state the clause 10.10 rule as emitted: \
         {row}"
    );
    let source = text_of(CODES);
    let Some(end) = source.find("pub const ABSTRACT_FORM_INVALID") else {
        panic!("{ROW}: setup: no ABSTRACT_FORM_INVALID in {CODES}");
    };
    let doc: Vec<&str> = source[..end]
        .lines()
        .rev()
        .skip(1)
        .take_while(|line| line.trim_start().starts_with("///"))
        .collect();
    let doc = doc.join(" ");
    assert!(
        doc.contains(ROW) && doc.contains("10.10"),
        "{ROW}: the doc of Code::ABSTRACT_FORM_INVALID does not state the clause 10.10 rule: {doc}"
    );
}
