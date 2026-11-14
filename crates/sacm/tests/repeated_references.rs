//! Tests for a repeated target in a multi-valued reference end
//! (SACM23-VAL-022; ptc/22-03-13 ends with no `isUnique`, so unique by the
//! UML default; XMI 2.5.1 §7.8.5; D-SEV, D-STRICT; ADR 0027, ADR 0028; the
//! maintainer's choice of a Warning, 2026-11-14).
//!
//! `validation::validate` reports each id that one multi-valued end of one
//! element holds two or more times as one SACM-MULT-001 Warning: requirement
//! SACM23-VAL-022, the end's clause (`metaClaim` 11.10, `source` 11.13,
//! `referencedArtifactElement` 11.9, a group's `argumentElement` 11.2),
//! affected `[element, id]`, no location, and a message naming the kind, the
//! element's id, the end, the repeated id and the count. It is reported
//! whatever else applies to the id. The file is kept as read, and a strict
//! save is not refused.

use std::path::Path;

use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, Operation};
use sacm::model::{Document, ElementId, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "SACM23-VAL-022";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

// --- Inputs and helpers ------------------------------------------------------------

/// A named element `id` of `kind` under `argumentElement`, with `attributes`.
fn typed(kind: &str, id: &str, attributes: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"N\"/></argumentElement>"
    )
}

/// An ArgumentPackage root `p` holding the named Claims a, b and m, then
/// `body`.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}{}{}{body}\
         </sacm:ArgumentPackage>",
        typed("Claim", "a", ""),
        typed("Claim", "b", ""),
        typed("Claim", "m", ""),
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

/// The findings of this row.
fn repeats(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.requirement_id() == ROW)
        .collect()
}

/// Asserts that `found` is a SACM-MULT-001 Warning of this row with `clause`,
/// on `[element, id]`, with no location and a one-line message naming the
/// kind, the element, the end, the id and the count.
fn assert_repeat(
    what: &str,
    found: &Diagnostic,
    (kind, element, end): (&str, &str, &str),
    (id, count): (&str, usize),
    clause: &str,
) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::MULTIPLICITY_VIOLATION, Severity::Warning, ROW),
        "{ROW}: {what}: not a SACM-MULT-001 Warning of this row: {found:?}"
    );
    assert_eq!(
        (found.clause(), found.location(), found.affected()),
        (
            Some(clause),
            None,
            [ElementId::from(element), ElementId::from(id)].as_slice()
        ),
        "{ROW}: {what}: not clause {clause}, no location, on [{element}, {id}]: {found:?}"
    );
    let message = found.message();
    for part in [
        kind.to_owned(),
        format!("{element:?}"),
        end.to_owned(),
        format!("{id:?}"),
        count.to_string(),
    ] {
        assert!(
            message.contains(&part),
            "{ROW}: {what}: the message {message:?} does not name {part}"
        );
    }
    assert!(
        !message.contains('\n'),
        "{ROW}: {what}: the message {message:?} is not one line"
    );
}

fn strict_write(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a strict write refused: {error:?}"),
    }
}

// --- Item 1: metaClaim --------------------------------------------------------------

#[test]
fn sacm23_val_022_a_repeated_meta_claim_is_one_warning_with_clause_11_10() {
    let what = "metaClaim=\"m m\"";
    let doc = read_ok(what, &in_ap(&typed("Claim", "c", " metaClaim=\"m m\"")));
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_repeat(
        what,
        finding,
        ("Claim", "c", "metaClaim"),
        ("m", 2),
        "11.10",
    );
}

// --- Item 2: source ---------------------------------------------------------------------

#[test]
fn sacm23_val_022_each_repeated_source_is_one_warning_with_clause_11_13() {
    let what = "source=\"a b a a\"";
    let doc = read_ok(
        what,
        &in_ap(&typed(
            "AssertedInference",
            "x",
            " source=\"a b a a\" target=\"m\"",
        )),
    );
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_repeat(
        what,
        finding,
        ("AssertedInference", "x", "source"),
        ("a", 3),
        "11.13",
    );

    let what = "source=\"a b a b\"";
    let doc = read_ok(
        what,
        &in_ap(&typed(
            "AssertedContext",
            "x",
            " source=\"a b a b\" target=\"m\"",
        )),
    );
    let found = validation::validate(&doc);
    let [first, second] = found.as_slice() else {
        panic!("{ROW}: {what}: not two findings: {found:?}");
    };
    assert_repeat(
        what,
        first,
        ("AssertedContext", "x", "source"),
        ("a", 2),
        "11.13",
    );
    assert_repeat(
        what,
        second,
        ("AssertedContext", "x", "source"),
        ("b", 2),
        "11.13",
    );
}

// --- Item 3: referencedArtifactElement and a group's members ----------------------------

#[test]
fn sacm23_val_022_repeats_in_referenced_artifact_element_and_group_members_have_their_clauses() {
    for (what, kind, attributes, end, clause) in [
        (
            "referencedArtifactElement=\"a m a\"",
            "ArtifactReference",
            " referencedArtifactElement=\"a m a\"",
            "referencedArtifactElement",
            "11.9",
        ),
        (
            "a group's argumentElement=\"a m a\"",
            "ArgumentGroup",
            " argumentElement=\"a m a\"",
            "argumentElement",
            "11.2",
        ),
    ] {
        let doc = read_ok(what, &in_ap(&typed(kind, "x", attributes)));
        let found = validation::validate(&doc);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        assert_repeat(what, finding, (kind, "x", end), ("a", 2), clause);
    }
}

// --- Item 4: whatever else applies; single-valued ends -----------------------------------

#[test]
fn sacm23_val_022_a_repeated_dangling_id_is_one_ref_001_and_one_warning() {
    let what = "metaClaim=\"z z\"";
    let doc = read_ok(what, &in_ap(&typed("Claim", "c", " metaClaim=\"z z\"")));
    let found = validation::validate(&doc);
    let dangling: Vec<&Diagnostic> = found
        .iter()
        .filter(|finding| finding.code() == Code::REF_DANGLING)
        .collect();
    assert_eq!(
        dangling
            .iter()
            .map(|finding| finding.affected().to_vec())
            .collect::<Vec<_>>(),
        [vec![ElementId::from("c"), ElementId::from("z")]],
        "{ROW}: {what}: not one SACM-REF-001 on [c, z]: {found:?}"
    );
    let warned = repeats(&doc);
    let [finding] = warned.as_slice() else {
        panic!("{ROW}: {what}: not one Warning: {found:?}");
    };
    assert_repeat(
        what,
        finding,
        ("Claim", "c", "metaClaim"),
        ("z", 2),
        "11.10",
    );
    assert_eq!(found.len(), 2, "{ROW}: {what}: other findings: {found:?}");
}

#[test]
fn sacm23_val_022_one_id_in_several_ends_or_in_single_valued_ends_gives_no_warning() {
    for (what, body) in [
        (
            "the same id as source, target and meta-claim",
            typed(
                "AssertedInference",
                "x",
                " metaClaim=\"a\" source=\"a\" target=\"a\" reasoning=\"why\"",
            ),
        ),
        (
            "the same id as citedElement and source",
            typed(
                "AssertedInference",
                "x",
                " isCitation=\"true\" citedElement=\"a\" source=\"a b\" target=\"b\"",
            ),
        ),
        (
            "one id in each of two elements' sources",
            format!(
                "{}{}",
                typed("AssertedInference", "x", " source=\"a\" target=\"b\""),
                typed("AssertedInference", "y", " source=\"a\" target=\"b\"")
            ),
        ),
    ] {
        let doc = read_ok(
            what,
            &in_ap(&format!("{}{body}", typed("ArgumentReasoning", "why", ""))),
        );
        assert_eq!(
            repeats(&doc),
            [],
            "{ROW}: {what}: a repeat is reported where no end holds an id twice"
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
    }
}

// --- Item 5: saving and creating --------------------------------------------------------

#[test]
fn sacm23_val_022_a_strict_write_succeeds_and_a_create_with_a_repeated_source_then_warns() {
    let what = "a repeated meta-claim and a repeated source";
    let doc = read_ok(
        what,
        &in_ap(&format!(
            "{}{}",
            typed("Claim", "c", " metaClaim=\"m m\""),
            typed("AssertedInference", "x", " source=\"a a\" target=\"m\"")
        )),
    );
    let found = validation::validate(&doc);
    assert!(
        found.len() == 2 && found.iter().all(|finding| finding.requirement_id() == ROW),
        "{ROW}: {what}: setup: not only this row's two Warnings: {found:?}"
    );
    let text = strict_write(what, &doc);
    assert!(
        text.contains("metaClaim=\"m m\"") && text.contains("source=\"a a\""),
        "{ROW}: {what}: the repeats are not written as read\n{text}"
    );
    let back = read_ok(&format!("{what}, written"), &text);
    assert!(
        back == doc,
        "{ROW}: {what}: does not read back equal\n{text}"
    );

    // An edit keeps the repeat, and validate then warns.
    let mut doc = Document::new();
    for operation in [
        Operation::from(CreateArgumentPackage::root(LangString::new("P")).with_id("p")),
        CreateClaim::new("p", LangString::new("A"))
            .with_id("a")
            .into(),
        CreateClaim::new("p", LangString::new("B"))
            .with_id("b")
            .into(),
    ] {
        if let Err(error) = edit::apply(&mut doc, operation) {
            panic!("{ROW}: setup: {error}");
        }
    }
    let created = match edit::apply(
        &mut doc,
        CreateAssertedInference::new("p", LangString::new("I"), ["a", "a"], "b"),
    ) {
        Ok(mutation) => mutation.created_ids().to_vec(),
        Err(error) => panic!("{ROW}: CreateAssertedInference with [a, a] was refused: {error}"),
    };
    let [id] = created.as_slice() else {
        panic!("{ROW}: not one created id: {created:?}");
    };
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: the created relationship: not one finding: {found:?}");
    };
    assert_repeat(
        "the created relationship",
        finding,
        ("AssertedInference", id.as_str(), "source"),
        ("a", 2),
        "11.13",
    );
    strict_write("the edited document", &doc);
}

#[test]
fn sacm23_val_022_the_invalid_fixture_gives_one_warning_and_writes_strictly() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/sacm23/validation/invalid/meta-claim-repeated.sacm.xmi");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let doc = read_ok("the fixture", &text);
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: the fixture: not one finding: {found:?}");
    };
    assert_repeat(
        "the fixture",
        finding,
        ("Claim", "c2", "metaClaim"),
        ("m1", 2),
        "11.10",
    );
    strict_write("the fixture", &doc);
}

// --- Item 6: the catalog --------------------------------------------------------------------

#[test]
fn sacm23_val_022_the_catalog_mult_001_row_covers_a_repeat_as_a_warning_naming_the_row() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
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
    assert!(
        line.contains("Warning") && line.contains(ROW),
        "{ROW}: the catalog's SACM-MULT-001 row does not name a Warning and {ROW}: {line}"
    );
    // The Warning half of the row names this row.
    let Some(warning) = line.find("Warning:") else {
        panic!("{ROW}: the catalog's SACM-MULT-001 row has no \"Warning:\" part: {line}");
    };
    assert!(
        line[warning..].contains(ROW),
        "{ROW}: the catalog's SACM-MULT-001 Warning part does not name {ROW}: {line}"
    );
}
