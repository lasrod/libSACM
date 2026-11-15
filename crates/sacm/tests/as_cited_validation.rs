//! Tests for the §11.8 `asCited` rule of `validation::validate`
//! (SACM23-VAL-019, SACM-CITE-002; formal/23-05-08 §11.8 "asCited - a flag
//! indicating that because the Assertion is cited, the `AssertionDeclaration`
//! should be transitively derived from the value of the `AssertionDeclaration`
//! of the cited Assertion", §11.11, §8.2; D-SEV; ADR 0022 amendment 1).
//!
//! An element of an Assertion kind (a Claim or one of the five asserted
//! relationships) whose `assertionDeclaration` is `asCited` should be a
//! citation of an Assertion. `validate` reports, each as one SACM-CITE-002
//! Warning with requirement SACM23-VAL-019, clause `11.8` and no location:
//!
//! - its `isCitation` is not true, affecting `[element]`;
//! - its `citedElement` is absent, affecting `[element]`;
//! - its `citedElement` names an element of the document that is not of an
//!   Assertion kind, affecting `[element, target]`.
//!
//! A `citedElement` that names no element gives only SACM-REF-001. The
//! element is kept, and a strict save is not refused.
//!
//! Every document is read from inline XMI, with every `ModelElement` named
//! and every relationship given valid ends, so that no other rule reports;
//! the edit path is built with `edit::apply`. Messages are checked only for
//! what they must name.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::edit::{
    self, CreateArgumentPackage, CreateClaim, SetAssertionDeclaration, SetFlag, SetReferences,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd,
    SacmElement,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "SACM23-VAL-019";
const VAL_003: &str = "SACM23-VAL-003";
const VAL_007: &str = "SACM23-VAL-007";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The six Assertion kinds (§11.10): the Claim and the five
/// `AssertedRelationship`s (§11.13 to §11.18).
const ASSERTION_KINDS: [ElementKind; 6] = [
    ElementKind::Claim,
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

/// The elements of the document that are of an Assertion kind and may be
/// cited: the Claim `tc` and the `AssertedInference` `ti`.
const ASSERTION_TARGETS: [(&str, &str); 2] = [("a Claim", "tc"), ("an AssertedInference", "ti")];

/// The elements of the document that are not of an Assertion kind: the
/// `ArgumentReasoning` `r`, the `ArtifactReference` `a1`, the `ArgumentPackage`
/// root `p`, and the Note `n`.
const OTHER_TARGETS: [(&str, &str); 4] = [
    ("an ArgumentReasoning", "r"),
    ("an ArtifactReference", "a1"),
    ("an ArgumentPackage", "p"),
    ("a Note", "n"),
];

/// The declaration this row is about.
const AS_CITED: &str = " assertionDeclaration=\"asCited\"";

// --- Inputs ------------------------------------------------------------------------

/// The `source` and `target` that make a relationship of `kind` valid, as
/// start-tag attributes: Claims for an `AssertedInference` or `AssertedContext`,
/// an `ArtifactReference` source for an `AssertedEvidence` (§11.15), and
/// `ArtifactReferences` at both ends of an `AssertedArtifactSupport` or
/// `AssertedArtifactContext` (§11.17, §11.18). Nothing for a Claim.
fn ends(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::AssertedInference | ElementKind::AssertedContext => {
            " source=\"c1\" target=\"c2\""
        }
        ElementKind::AssertedEvidence => " source=\"a1\" target=\"c1\"",
        ElementKind::AssertedArtifactSupport | ElementKind::AssertedArtifactContext => {
            " source=\"a1\" target=\"a2\""
        }
        _ => "",
    }
}

/// An ArgumentPackage root `p` named "P" holding the Claims `c1`, `c2` and
/// `tc`, the `ArgumentReasoning` `r`, the `ArtifactReferences` `a1` and `a2`,
/// the `AssertedInference` `ti` (from `c1` to `c2`), the Note `n`, and the
/// element `x` of `kind`, named "X", valid apart from the start-tag
/// `attributes` and holding `content` after its name.
fn package(kind: ElementKind, attributes: &str, content: &str) -> String {
    let ends = ends(kind);
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <note xmi:id=\"n\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"tc\"><name content=\"TC\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r\"><name content=\"R\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"a1\"><name content=\"A1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"a2\"><name content=\"A2\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"ti\" source=\"c1\" target=\"c2\">\
         <name content=\"TI\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{ends}{attributes}><name content=\"X\"/>\
         {content}</argumentElement></sacm:ArgumentPackage>"
    )
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

/// Asserts that `found` is a SACM-CITE-002 Warning of this row, clause
/// `11.8`, no location, affecting exactly `affected` in that order, with a
/// one-line message naming `asCited` and each id of `affected`
/// `{:?}`-quoted.
fn assert_warning(what: &str, found: &Diagnostic, affected: &[&str]) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::AS_CITED_INVALID, Severity::Warning, ROW),
        "{ROW}: {what}: not a SACM-CITE-002 Warning of this row: {found:?}"
    );
    let ids: Vec<ElementId> = affected.iter().copied().map(ElementId::from).collect();
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        (ids.as_slice(), None, Some("11.8")),
        "{ROW}: {what}: the affected ids, location or clause of {found:?}"
    );
    let message = found.message();
    assert!(
        message.contains("asCited") && !message.contains('\n'),
        "{ROW}: {what}: the message {message:?} is not one line naming asCited"
    );
    for id in affected {
        assert!(
            message.contains(&format!("{id:?}")),
            "{ROW}: {what}: the message {message:?} does not name {id:?}"
        );
    }
}

/// Asserts that `found` is exactly the two Warnings of an element `x` that
/// is not a citation and has no `citedElement`: both affecting `[x]`, one
/// naming `isCitation` and the other `citedElement`.
fn assert_two_warnings_on_x(what: &str, found: &[Diagnostic]) {
    let [first, second] = found else {
        panic!("{ROW}: {what}: not two findings: {found:?}");
    };
    assert_warning(what, first, &["x"]);
    assert_warning(what, second, &["x"]);
    assert_ne!(
        first.message(),
        second.message(),
        "{ROW}: {what}: the two Warnings have one message"
    );
    let names = |needle: &str| {
        [first, second]
            .iter()
            .filter(|f| f.message().contains(needle))
            .count()
    };
    assert!(
        names("isCitation") >= 1 && names("citedElement") >= 1,
        "{ROW}: {what}: the Warnings do not name isCitation and citedElement: {found:?}"
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

// --- Acceptance 1: an asCited citation of an Assertion -----------------------------

#[test]
fn sacm23_val_019_the_setup_gives_no_finding_without_as_cited() {
    // Without the declaration, every document these tests build is valid,
    // so each finding below is this row's.
    for kind in ASSERTION_KINDS {
        no_finding(
            &format!("an {kind} with no declaration"),
            &package(kind, "", ""),
        );
    }
}

#[test]
fn sacm23_val_019_an_as_cited_citation_of_a_claim_or_an_asserted_inference_gives_no_finding() {
    for kind in ASSERTION_KINDS {
        for (target, id) in ASSERTION_TARGETS {
            for (form, attributes, content) in [
                (
                    "attributes",
                    format!("{AS_CITED} isCitation=\"true\" citedElement=\"{id}\""),
                    String::new(),
                ),
                (
                    "value elements",
                    String::new(),
                    format!(
                        "<assertionDeclaration>asCited</assertionDeclaration>\
                         <isCitation>true</isCitation><citedElement xmi:idref=\"{id}\"/>"
                    ),
                ),
            ] {
                let what = format!("an asCited {kind} citing {target} {id:?}, as {form}");
                let doc = read_ok(&what, &package(kind, &attributes, &content));
                assert_eq!(
                    doc.get("x").and_then(SacmElement::assertion_declaration),
                    Some(AssertionDeclaration::AsCited),
                    "{ROW}: {what}: setup: x is not read as asCited"
                );
                let found = validation::validate(&doc);
                assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
            }
        }
    }
}

// --- Acceptance 2: not a citation, or no citedElement -------------------------------

#[test]
fn sacm23_val_019_an_as_cited_non_citation_without_cited_element_gives_two_warnings_on_the_element()
{
    for kind in ASSERTION_KINDS {
        for (form, attributes, content) in [
            ("no isCitation", AS_CITED, ""),
            (
                "isCitation=\"false\"",
                " assertionDeclaration=\"asCited\" isCitation=\"false\"",
                "",
            ),
            (
                "the declaration as a value element",
                "",
                "<assertionDeclaration>asCited</assertionDeclaration>",
            ),
        ] {
            let what = format!("an asCited {kind} with {form} and no citedElement");
            let doc = read_ok(&what, &package(kind, attributes, content));
            let found = findings(&what, &doc, &[ROW]);
            assert_two_warnings_on_x(&what, &found);
            assert!(
                !diagnostic::has_errors(&found),
                "{ROW}: {what}: an Error among {found:?}"
            );
        }
    }
}

#[test]
fn sacm23_val_019_an_as_cited_citation_without_cited_element_gives_one_warning_on_the_element() {
    for kind in ASSERTION_KINDS {
        for flag in ["true", "1"] {
            let what = format!("an asCited {kind} with isCitation=\"{flag}\" and no citedElement");
            let doc = read_ok(
                &what,
                &package(kind, &format!("{AS_CITED} isCitation=\"{flag}\""), ""),
            );
            let found = findings(&what, &doc, &[ROW]);
            let [finding] = found.as_slice() else {
                panic!("{ROW}: {what}: not one finding: {found:?}");
            };
            assert_warning(&what, finding, &["x"]);
            assert!(
                finding.message().contains("citedElement"),
                "{ROW}: {what}: the message {:?} does not name citedElement",
                finding.message()
            );
        }
    }
}

// --- Acceptance 3: a citation of a non-Assertion, or a dangling one -----------------

#[test]
fn sacm23_val_019_an_as_cited_citation_of_a_non_assertion_gives_one_warning_on_the_element_and_target()
 {
    for kind in ASSERTION_KINDS {
        for (target, id) in OTHER_TARGETS {
            let what = format!("an asCited {kind} citing {target} {id:?}");
            let doc = read_ok(
                &what,
                &package(
                    kind,
                    &format!("{AS_CITED} isCitation=\"true\" citedElement=\"{id}\""),
                    "",
                ),
            );
            let found = findings(&what, &doc, &[ROW]);
            let [finding] = found.as_slice() else {
                panic!("{ROW}: {what}: not one finding: {found:?}");
            };
            assert_warning(&what, finding, &["x", id]);
        }
    }
}

#[test]
fn sacm23_val_019_an_as_cited_citation_of_itself_gives_no_finding() {
    // A reference to the element itself resolves, and the element is of an
    // Assertion kind.
    for kind in ASSERTION_KINDS {
        let what = format!("an asCited {kind} citing itself");
        no_finding(
            &what,
            &package(
                kind,
                &format!("{AS_CITED} isCitation=\"true\" citedElement=\"x\""),
                "",
            ),
        );
    }
}

#[test]
fn sacm23_val_019_a_dangling_cited_element_gives_only_ref_001() {
    for kind in ASSERTION_KINDS {
        let what = format!("an asCited {kind} with isCitation and citedElement=\"nowhere\"");
        let doc = read_ok(
            &what,
            &package(
                kind,
                &format!("{AS_CITED} isCitation=\"true\" citedElement=\"nowhere\""),
                "",
            ),
        );
        let found = findings(&what, &doc, &[ROW, VAL_003]);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not only SACM-REF-001: {found:?}");
        };
        assert_eq!(
            (
                finding.code(),
                finding.severity(),
                finding.requirement_id(),
                finding.affected()
            ),
            (
                Code::REF_DANGLING,
                Severity::Error,
                VAL_003,
                [ElementId::from("x"), ElementId::from("nowhere")].as_slice()
            ),
            "{ROW}: {what}: not the SACM-REF-001 of x and \"nowhere\": {finding:?}"
        );

        // Without isCitation, the isCitation Warning (and SACM-CITE-001) are
        // still given, but no Warning names the dangling target.
        let what = format!("an asCited {kind} with no isCitation and citedElement=\"nowhere\"");
        let doc = read_ok(
            &what,
            &package(kind, &format!("{AS_CITED} citedElement=\"nowhere\""), ""),
        );
        let found = findings(&what, &doc, &[ROW, VAL_003, VAL_007]);
        let codes: Vec<(Code, &str)> = found
            .iter()
            .map(|f| (f.code(), f.requirement_id()))
            .collect();
        assert_eq!(
            codes,
            [
                (Code::CITATION_INVALID, VAL_007),
                (Code::AS_CITED_INVALID, ROW),
                (Code::REF_DANGLING, VAL_003),
            ],
            "{ROW}: {what}: not SACM-CITE-001, one SACM-CITE-002 and SACM-REF-001: {found:?}"
        );
        assert_warning(&what, &found[1], &["x"]);
    }
}

// --- Acceptance 4: coexistence with SACM-CITE-001 -----------------------------------

#[test]
fn sacm23_val_019_a_cited_claim_without_is_citation_gives_cite_001_and_this_row_s_warning() {
    for kind in ASSERTION_KINDS {
        let what = format!("an asCited {kind} citing the Claim \"tc\" without isCitation");
        let doc = read_ok(
            &what,
            &package(kind, &format!("{AS_CITED} citedElement=\"tc\""), ""),
        );
        let found = findings(&what, &doc, &[ROW, VAL_007]);
        let [cite, warning] = found.as_slice() else {
            panic!("{ROW}: {what}: not two findings: {found:?}");
        };
        assert_eq!(
            (
                cite.code(),
                cite.severity(),
                cite.requirement_id(),
                cite.affected(),
                cite.clause()
            ),
            (
                Code::CITATION_INVALID,
                Severity::Error,
                VAL_007,
                [ElementId::from("x")].as_slice(),
                Some("8.2")
            ),
            "{ROW}: {what}: the first finding is not SACM-CITE-001 on x: {cite:?}"
        );
        assert_warning(&what, warning, &["x"]);
        assert!(
            warning.message().contains("isCitation"),
            "{ROW}: {what}: the Warning {:?} does not name isCitation",
            warning.message()
        );
    }
}

#[test]
fn sacm23_val_019_findings_come_in_validate_s_order_code_then_affected_ids() {
    // Not a citation, and citing an ArgumentReasoning: SACM-CITE-001 on x,
    // then the two Warnings, `[x]` before `[x, r]`.
    for kind in ASSERTION_KINDS {
        let what =
            format!("an asCited {kind} citing the ArgumentReasoning \"r\" without isCitation");
        let doc = read_ok(
            &what,
            &package(kind, &format!("{AS_CITED} citedElement=\"r\""), ""),
        );
        let found = findings(&what, &doc, &[ROW, VAL_007]);
        let [cite, on_x, on_target] = found.as_slice() else {
            panic!("{ROW}: {what}: not three findings: {found:?}");
        };
        assert_eq!(
            (cite.code(), cite.requirement_id()),
            (Code::CITATION_INVALID, VAL_007),
            "{ROW}: {what}: the first finding is not SACM-CITE-001: {cite:?}"
        );
        assert_warning(&what, on_x, &["x"]);
        assert_warning(&what, on_target, &["x", "r"]);
        let mut sorted = found.clone();
        sorted.sort();
        assert_eq!(
            sorted, found,
            "{ROW}: {what}: validate's result is not sorted"
        );
    }
}

// --- Acceptance 5: other declarations, and strict writes ----------------------------

#[test]
fn sacm23_val_019_every_other_declaration_gives_no_finding() {
    for kind in ASSERTION_KINDS {
        for &declaration in AssertionDeclaration::ALL {
            if declaration == AssertionDeclaration::AsCited {
                continue;
            }
            let declared = format!(" assertionDeclaration=\"{declaration}\"");
            for (form, attributes) in [
                ("not a citation", declared.clone()),
                (
                    "a citation of the Claim \"tc\"",
                    format!("{declared} isCitation=\"true\" citedElement=\"tc\""),
                ),
                (
                    "a citation of the ArgumentReasoning \"r\"",
                    format!("{declared} isCitation=\"true\" citedElement=\"r\""),
                ),
                (
                    "a citation with no citedElement",
                    format!("{declared} isCitation=\"true\""),
                ),
            ] {
                no_finding(
                    &format!("an {kind} declared {declaration}, {form}"),
                    &package(kind, &attributes, ""),
                );
            }
        }
        // The default, written or not, is not asCited.
        no_finding(
            &format!("an {kind} with no declaration citing the Claim \"tc\""),
            &package(kind, " isCitation=\"true\" citedElement=\"tc\"", ""),
        );
    }
}

#[test]
fn sacm23_val_019_the_warnings_do_not_refuse_a_strict_write() {
    for kind in ASSERTION_KINDS {
        for (form, attributes) in [
            ("not a citation, no citedElement", AS_CITED.to_owned()),
            (
                "citing the ArtifactReference \"a1\"",
                format!("{AS_CITED} isCitation=\"true\" citedElement=\"a1\""),
            ),
        ] {
            let what = format!("an asCited {kind}, {form}");
            let doc = read_ok(&what, &package(kind, &attributes, ""));
            let found = findings(&what, &doc, &[ROW]);
            assert!(
                !found.is_empty() && !diagnostic::has_errors(&found),
                "{ROW}: {what}: setup: not Warnings only: {found:?}"
            );
            for mode in MODES {
                let text = write_round_trip(&what, &doc, mode);
                assert!(
                    text.contains(" assertionDeclaration=\"asCited\""),
                    "{ROW}: {what}: the {mode:?} write drops the declaration:\n{text}"
                );
            }
        }
    }
}

// --- Acceptance 6: the edit path ---------------------------------------------------

#[test]
fn sacm23_val_019_set_assertion_declaration_as_cited_on_a_non_citation_succeeds_and_validate_warns()
{
    let mut doc = Document::new();
    let setup = edit::apply(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    )
    .and_then(|_| {
        edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("X")).with_id("x"),
        )
    })
    .and_then(|_| {
        edit::apply(
            &mut doc,
            CreateClaim::new("p", LangString::new("TC")).with_id("tc"),
        )
    });
    if let Err(error) = setup {
        panic!("{ROW}: setup: an edit was refused: {error:?}");
    }
    let what = "after the setup edits";
    let found = validation::validate(&doc);
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");

    if let Err(error) = edit::apply(
        &mut doc,
        SetAssertionDeclaration::new("x", AssertionDeclaration::AsCited),
    ) {
        panic!(
            "{ROW}: SetAssertionDeclaration to asCited on a non-citation was refused: {error:?}"
        );
    }
    assert_eq!(
        doc.get("x")
            .map(|x| (x.assertion_declaration(), x.is_citation())),
        Some((Some(AssertionDeclaration::AsCited), false)),
        "{ROW}: the edit did not set asCited, or changed isCitation"
    );
    let what = "after SetAssertionDeclaration(x, AsCited)";
    assert_two_warnings_on_x(what, &findings(what, &doc, &[ROW]));

    if let Err(error) = edit::apply(&mut doc, SetFlag::new("x", Flag::IsCitation, true)) {
        panic!("{ROW}: SetFlag(x, IsCitation, true) was refused: {error:?}");
    }
    let what = "after SetFlag(x, IsCitation, true)";
    let found = findings(what, &doc, &[ROW]);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_warning(what, finding, &["x"]);

    if let Err(error) = edit::apply(
        &mut doc,
        SetReferences::new("x", ReferenceEnd::CitedElement, ["tc"]),
    ) {
        panic!("{ROW}: SetReferences(x, citedElement, [tc]) was refused: {error:?}");
    }
    let what = "after SetReferences(x, citedElement, [tc])";
    let found = validation::validate(&doc);
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
}

// --- Acceptance 7: the code and its catalog row -------------------------------------

#[test]
fn sacm23_val_019_as_cited_invalid_is_sacm_cite_002_and_its_catalog_row_names_it_and_the_row() {
    assert_eq!(
        Code::AS_CITED_INVALID.as_str(),
        "SACM-CITE-002",
        "{ROW}: Code::AS_CITED_INVALID is not SACM-CITE-002"
    );
    let line = catalog_row(Code::AS_CITED_INVALID);
    assert!(
        line.starts_with("| SACM-CITE-002 | Warning |"),
        "{ROW}: the catalog's SACM-CITE-002 row is not a Warning: {line}"
    );
    for part in [
        "`Code::AS_CITED_INVALID`",
        "SACM23-VAL-019",
        "asCited",
        "isCitation",
        "citedElement",
        "11.8",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-CITE-002 row does not name {part}: {line}"
        );
    }
}
