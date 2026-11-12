//! Tests for the validation entry point and its first rule: a `ModelElement`
//! whose name is missing is one SACM-MULT-001 error (SACM23-VAL-018;
//! formal/23-05-08 §8.6 `name:LangString[1]`; ADR 0011, ADR 0013, ADR 0018,
//! D-VAL-LAYER).
//!
//! Documents are built with the `test-support` feature's `Document::insert`,
//! where `name: None` is a missing name, or read through `xmi::read` with the
//! default options. Each expected finding is built with the `test-support`
//! `Diagnostic` constructors and compared whole, so a wrong field, an extra
//! finding or a wrong order all fail; item 6 also pins every field through
//! the accessors and the `Display` text literally.

use std::collections::BTreeSet;
use std::path::Path;

use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, ReadOptions, WriteOptions};

/// The row every test here verifies.
const ROW: &str = "SACM23-VAL-018";
/// The namespace declarations of a bare root that is read.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// The four `UtilityElement` kinds (§8.7-8.12), which have no name.
const UTILITY_KINDS: [ElementKind; 4] = [
    ElementKind::Description,
    ElementKind::ImplementationConstraint,
    ElementKind::Note,
    ElementKind::TaggedValue,
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Inserts one element, or fails the test as a setup error.
fn add(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
    name: Option<LangString>,
) {
    let parent = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent.as_ref(), kind, ElementId::from(id), name) {
        panic!("{ROW}: setup could not insert {kind} {id:?}: {error}");
    }
}

/// A non-empty name.
fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// A name with exactly the given `lang` and `content`, each absent when `None`.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut name = LangString::default();
    name.set_lang(lang.map(str::to_owned));
    name.set_content(content.map(str::to_owned));
    name
}

/// Every `ModelElement` kind, in `ElementKind::ALL` order.
fn model_element_kinds() -> Vec<ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ModelElement))
        .collect()
}

/// The message of the finding for the nameless element `id` of `kind`.
fn message(kind: ElementKind, id: &str) -> String {
    format!("{} {id:?} has no name (name:LangString[1])", kind.name())
}

/// The finding `validate` must give for the nameless element `id` of `kind`.
fn missing_name(kind: ElementKind, id: &str) -> Diagnostic {
    Diagnostic::new(
        Code::MULTIPLICITY_VIOLATION,
        Severity::Error,
        "SACM23-VAL-018",
        message(kind, id),
    )
    .with_clause("8.6")
    .with_affected([ElementId::from(id)])
}

/// `findings` sorted, as `validate` must return them.
fn sorted(mut findings: Vec<Diagnostic>) -> Vec<Diagnostic> {
    findings.sort();
    findings
}

/// Asserts that `validate(doc)` is exactly `expected`, in that order.
fn assert_findings(doc: &Document, expected: &[Diagnostic], what: &str) {
    let found = validation::validate(doc);
    assert_eq!(
        found, expected,
        "{ROW}: validate on {what} gave the wrong findings"
    );
}

/// Reads `input` with the default options, which must succeed.
fn read_ok(input: &str) -> xmi::ReadOutput {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: read refused {input:?}: {error} ({error:?})"),
    }
}

/// Writes `doc` with the default options, which must succeed.
fn write_ok(doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: write refused the document: {error} ({error:?})"),
    }
}

/// A bare root of `kind` with id `p` holding `content`.
fn bare_root(kind: ElementKind, content: &str) -> String {
    if content.is_empty() {
        format!("<sacm:{kind}{NS} xmi:id=\"p\"/>")
    } else {
        format!("<sacm:{kind}{NS} xmi:id=\"p\">{content}</sacm:{kind}>")
    }
}

/// A root of `kind` with id `p`, in an `xmi:XMI` element.
fn wrapped_root(kind: ElementKind) -> String {
    format!("<xmi:XMI{NS}><sacm:{kind} xmi:id=\"p\"/></xmi:XMI>")
}

// ---------------------------------------------------------------------------
// 1. Which kinds are ModelElements
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_exactly_34_kinds_are_model_elements_and_the_other_4_are_utility_elements() {
    assert_eq!(
        model_element_kinds().len(),
        34,
        "{ROW}: ElementKind::ALL does not hold exactly 34 ModelElement kinds"
    );
    let others: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| !kind.is_a(AbstractClass::ModelElement))
        .collect();
    assert_eq!(
        others, UTILITY_KINDS,
        "{ROW}: the kinds that are not ModelElements are not exactly the 4 UtilityElements"
    );
}

// ---------------------------------------------------------------------------
// 2. One finding per nameless element, for every kind
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_each_nameless_model_element_kind_in_a_named_package_gives_one_diagnostic() {
    for kind in model_element_kinds() {
        let mut doc = Document::new();
        add(
            &mut doc,
            None,
            ElementKind::ArtifactPackage,
            "pkg",
            Some(named("Pkg")),
        );
        add(&mut doc, Some("pkg"), kind, "e", None);
        assert_findings(
            &doc,
            &[missing_name(kind, "e")],
            &format!("a nameless {kind} in a named ArtifactPackage"),
        );
    }
}

#[test]
fn sacm23_val_018_each_nameless_package_root_gives_one_diagnostic() {
    let packages: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_package())
        .collect();
    assert_eq!(
        packages.len(),
        12,
        "{ROW}: there are not exactly 12 package kinds"
    );
    for kind in packages {
        let mut doc = Document::new();
        add(&mut doc, None, kind, "r", None);
        assert_findings(
            &doc,
            &[missing_name(kind, "r")],
            &format!("a nameless {kind} root"),
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Many findings: no cascade, no suppression
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_all_34_nameless_kinds_give_34_diagnostics_one_per_id() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArtifactPackage,
        "pkg",
        Some(named("Pkg")),
    );
    let mut expected = Vec::new();
    for kind in model_element_kinds() {
        let id = format!("e_{}", kind.name());
        add(&mut doc, Some("pkg"), kind, &id, None);
        expected.push(missing_name(kind, &id));
    }
    let found = validation::validate(&doc);
    assert_eq!(
        found.len(),
        34,
        "{ROW}: 34 nameless elements gave {} findings",
        found.len()
    );
    let ids: BTreeSet<&str> = found
        .iter()
        .flat_map(Diagnostic::affected)
        .map(ElementId::as_str)
        .collect();
    assert_eq!(
        ids.len(),
        34,
        "{ROW}: 34 nameless elements did not give one finding per id"
    );
    assert_eq!(
        found,
        sorted(expected),
        "{ROW}: 34 nameless elements gave the wrong findings"
    );
}

#[test]
fn sacm23_val_018_a_nameless_element_in_a_nameless_parent_gives_two_diagnostics() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::ArgumentPackage, "p", None);
    add(&mut doc, Some("p"), ElementKind::Claim, "c", None);
    assert_findings(
        &doc,
        &sorted(vec![
            missing_name(ElementKind::ArgumentPackage, "p"),
            missing_name(ElementKind::Claim, "c"),
        ]),
        "a nameless Claim in a nameless ArgumentPackage",
    );
}

// ---------------------------------------------------------------------------
// 4. UtilityElements have no name to miss
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_utility_elements_under_a_named_model_element_give_no_diagnostic() {
    for kind in UTILITY_KINDS {
        // Under a named package, and under a named Claim in a named package.
        let mut in_package = Document::new();
        add(
            &mut in_package,
            None,
            ElementKind::ArtifactPackage,
            "pkg",
            Some(named("Pkg")),
        );
        add(&mut in_package, Some("pkg"), kind, "u", None);
        assert_findings(
            &in_package,
            &[],
            &format!("a {kind} in a named ArtifactPackage"),
        );

        let mut in_claim = Document::new();
        add(
            &mut in_claim,
            None,
            ElementKind::ArgumentPackage,
            "pkg",
            Some(named("Pkg")),
        );
        add(
            &mut in_claim,
            Some("pkg"),
            ElementKind::Claim,
            "c",
            Some(named("C")),
        );
        add(&mut in_claim, Some("c"), kind, "u", None);
        assert_findings(&in_claim, &[], &format!("a {kind} in a named Claim"));
    }
}

// ---------------------------------------------------------------------------
// 5. An absent or empty lang or content is not a missing name
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_absent_or_empty_lang_or_content_is_not_a_missing_name() {
    let names = [
        ("LangString::default()", LangString::default()),
        ("lang=\"\" content=\"\"", lang_string(Some(""), Some(""))),
        ("lang absent, content=\"\"", lang_string(None, Some(""))),
        ("lang=\"en\", content absent", lang_string(Some("en"), None)),
        ("lang=\"\", content absent", lang_string(Some(""), None)),
    ];
    for (what, name) in names {
        let mut doc = Document::new();
        add(
            &mut doc,
            None,
            ElementKind::ArgumentPackage,
            "pkg",
            Some(name.clone()),
        );
        add(&mut doc, Some("pkg"), ElementKind::Claim, "c1", Some(name));
        assert_findings(&doc, &[], &format!("elements named with {what}"));
    }
}

#[test]
fn sacm23_val_018_an_empty_name_read_through_xmi_is_not_a_missing_name() {
    let names = [
        ("<name/>", LangString::default()),
        (
            "<name lang=\"\" content=\"\"/>",
            lang_string(Some(""), Some("")),
        ),
    ];
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        for (child, name) in &names {
            let input = bare_root(kind, child);
            let output = read_ok(&input);
            let read_name = output.document().get("p").and_then(|p| p.name().cloned());
            assert_eq!(
                read_name.as_ref(),
                Some(name),
                "{ROW}: {input:?} did not read to the expected name, so it tests nothing"
            );
            assert_findings(output.document(), &[], &format!("{input:?}"));
        }
    }
}

// ---------------------------------------------------------------------------
// 6. The fields, the message and Display
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_the_diagnostic_fields_message_and_display_are_pinned() {
    assert_eq!(
        Code::MULTIPLICITY_VIOLATION.as_str(),
        "SACM-MULT-001",
        "{ROW}: Code::MULTIPLICITY_VIOLATION is not SACM-MULT-001"
    );
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "pkg",
        Some(named("Pkg")),
    );
    add(&mut doc, Some("pkg"), ElementKind::Claim, "c1", None);
    let found = validation::validate(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: one nameless Claim gave {found:?}, not one finding");
    };
    assert_eq!(
        finding.code(),
        Code::MULTIPLICITY_VIOLATION,
        "{ROW}: the code is wrong"
    );
    assert_eq!(
        finding.code().as_str(),
        "SACM-MULT-001",
        "{ROW}: the code text is wrong"
    );
    assert_eq!(
        finding.severity(),
        Severity::Error,
        "{ROW}: the severity is not Error"
    );
    assert_eq!(
        finding.requirement_id(),
        "SACM23-VAL-018",
        "{ROW}: the requirement id is wrong"
    );
    assert_eq!(finding.clause(), Some("8.6"), "{ROW}: the clause is wrong");
    assert_eq!(
        finding.affected(),
        [ElementId::from("c1")],
        "{ROW}: the affected ids are not exactly the Claim's"
    );
    assert_eq!(
        finding.location(),
        None,
        "{ROW}: the finding has a location"
    );
    assert_eq!(
        finding.message(),
        "Claim \"c1\" has no name (name:LangString[1])",
        "{ROW}: the message is wrong"
    );
    assert_eq!(
        finding.to_string(),
        "error SACM-MULT-001 [SACM23-VAL-018, clause 8.6]: \
         Claim \"c1\" has no name (name:LangString[1])",
        "{ROW}: Display is wrong"
    );
}

#[test]
fn sacm23_val_018_an_id_with_a_quote_line_break_backslash_and_combining_mark_is_debug_quoted() {
    // The id is `a"b`, a line feed, `c\d` and U+0301 COMBINING ACUTE ACCENT.
    let input = format!("<sacm:ArgumentPackage{NS} xmi:id=\"a&quot;b&#10;c&#92;d&#x301;\"/>");
    let output = read_ok(&input);
    let ids: Vec<&str> = output
        .document()
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        ids,
        ["a\"b\nc\\d\u{301}"],
        "{ROW}: {input:?} did not read to the expected id, so it tests nothing"
    );
    let found = validation::validate(output.document());
    let [finding] = found.as_slice() else {
        panic!("{ROW}: one nameless ArgumentPackage gave {found:?}, not one finding");
    };
    // Written out, not built with `format!`: the id as `{:?}` writes it, with
    // the quote, line feed, backslash and combining mark escaped.
    assert_eq!(
        finding.message(),
        r#"ArgumentPackage "a\"b\nc\\d\u{301}" has no name (name:LangString[1])"#,
        "{ROW}: the id in the message is not quoted with {{:?}}"
    );
    let text = finding.to_string();
    assert!(
        !text.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']),
        "{ROW}: Display {text:?} is not one line"
    );
}

// ---------------------------------------------------------------------------
// 7. The element is kept
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_validate_keeps_the_nameless_element_and_the_written_output() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "pkg",
        Some(named("Pkg")),
    );
    add(&mut doc, Some("pkg"), ElementKind::Claim, "c1", None);
    add(
        &mut doc,
        Some("pkg"),
        ElementKind::Claim,
        "c2",
        Some(named("C2")),
    );
    let count = doc.element_count();
    let before = write_ok(&doc);

    let found = validation::validate(&doc);
    assert_eq!(
        found,
        [missing_name(ElementKind::Claim, "c1")],
        "{ROW}: the nameless Claim was not reported"
    );

    assert_eq!(
        doc.element_count(),
        count,
        "{ROW}: validate changed the element count"
    );
    let Some(c1) = doc.get("c1") else {
        panic!("{ROW}: the nameless Claim is gone after validate");
    };
    assert_eq!(
        c1.name(),
        None,
        "{ROW}: the nameless Claim has a name after validate"
    );
    assert_eq!(
        write_ok(&doc),
        before,
        "{ROW}: the written output changed after validate"
    );
}

// ---------------------------------------------------------------------------
// 8. Reading does not validate (D-VAL-LAYER)
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_reading_a_nameless_root_gives_no_multiplicity_diagnostic_but_validate_does() {
    for kind in [
        ElementKind::AssuranceCasePackage,
        ElementKind::ArgumentPackage,
    ] {
        for input in [bare_root(kind, ""), wrapped_root(kind)] {
            let output = read_ok(&input);
            assert!(
                output
                    .diagnostics()
                    .iter()
                    .all(|found| found.code() != Code::MULTIPLICITY_VIOLATION),
                "{ROW}: reading {input:?} gave a SACM-MULT-001: {:?}",
                output.diagnostics()
            );
            assert_findings(
                output.document(),
                &[missing_name(kind, "p")],
                &format!("the document read from {input:?}"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 9. Order and determinism
// ---------------------------------------------------------------------------

/// A document with named packages `p1` and `p2` and nameless Claims `a10`,
/// `a2` and `b`, inserted in the order given: each entry is a Claim's parent
/// and id. `packages` gives the order of the two roots.
fn three_nameless_claims(packages: [&str; 2], claims: [(&str, &str); 3]) -> Document {
    let mut doc = Document::new();
    for package in packages {
        add(
            &mut doc,
            None,
            ElementKind::ArgumentPackage,
            package,
            Some(named(package)),
        );
    }
    for (parent, id) in claims {
        add(&mut doc, Some(parent), ElementKind::Claim, id, None);
    }
    doc
}

#[test]
fn sacm23_val_018_findings_are_sorted_by_affected_id_whatever_the_insertion_order() {
    let expected = [
        missing_name(ElementKind::Claim, "a10"),
        missing_name(ElementKind::Claim, "a2"),
        missing_name(ElementKind::Claim, "b"),
    ];
    // Every insertion order of the three Claims, all in `p1`.
    let permutations: [[&str; 3]; 6] = [
        ["a10", "a2", "b"],
        ["a10", "b", "a2"],
        ["a2", "a10", "b"],
        ["a2", "b", "a10"],
        ["b", "a10", "a2"],
        ["b", "a2", "a10"],
    ];
    let mut documents = Vec::new();
    for [x, y, z] in permutations {
        documents.push(three_nameless_claims(
            ["p1", "p2"],
            [("p1", x), ("p1", y), ("p1", z)],
        ));
    }
    // `b` in the first root, so that document order puts it first.
    for packages in [["p1", "p2"], ["p2", "p1"]] {
        documents.push(three_nameless_claims(
            packages,
            [("p1", "b"), ("p2", "a2"), ("p2", "a10")],
        ));
    }
    for doc in &documents {
        let first = validation::validate(doc);
        assert_eq!(
            first, expected,
            "{ROW}: the findings are not sorted by affected id (a10, a2, b) for {doc:?}"
        );
        assert_eq!(
            validation::validate(doc),
            first,
            "{ROW}: two calls on {doc:?} gave different results"
        );
    }
}

// ---------------------------------------------------------------------------
// 10. No findings, and has_errors
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_an_empty_or_fully_named_document_gives_no_diagnostic_and_no_errors() {
    let empty = Document::new();
    let found = validation::validate(&empty);
    assert!(found.is_empty(), "{ROW}: an empty document gave {found:?}");
    assert!(
        !diagnostic::has_errors(&found),
        "{ROW}: has_errors is true for an empty document's findings"
    );

    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArtifactPackage,
        "pkg",
        Some(named("Pkg")),
    );
    for kind in model_element_kinds() {
        let id = format!("e_{}", kind.name());
        add(&mut doc, Some("pkg"), kind, &id, Some(named(&id)));
    }
    for (index, kind) in UTILITY_KINDS.into_iter().enumerate() {
        add(&mut doc, Some("pkg"), kind, &format!("u{index}"), None);
    }
    let found = validation::validate(&doc);
    assert!(
        found.is_empty(),
        "{ROW}: a document where every ModelElement has a name gave {found:?}"
    );
    assert!(
        !diagnostic::has_errors(&found),
        "{ROW}: has_errors is true for a fully named document's findings"
    );

    add(&mut doc, Some("pkg"), ElementKind::Claim, "nameless", None);
    let found = validation::validate(&doc);
    assert!(
        diagnostic::has_errors(&found),
        "{ROW}: has_errors is false when a name is missing: {found:?}"
    );
}

#[test]
fn sacm23_val_018_has_errors_is_true_exactly_when_a_list_holds_an_error() {
    let of = |severity| Diagnostic::new(Code::new("SACM-TEST-001"), severity, ROW, "m");
    assert!(
        !diagnostic::has_errors(&[]),
        "{ROW}: has_errors is true for no diagnostics"
    );
    let below = [of(Severity::Info), of(Severity::Warning)];
    assert!(
        !diagnostic::has_errors(&below),
        "{ROW}: has_errors is true for an Info and a Warning"
    );
    let with_error = [
        of(Severity::Info),
        of(Severity::Warning),
        of(Severity::Error),
    ];
    assert!(
        diagnostic::has_errors(&with_error),
        "{ROW}: has_errors is false for a list whose last diagnostic is an Error"
    );
    assert!(
        diagnostic::has_errors(&[of(Severity::Error)]),
        "{ROW}: has_errors is false for one Error"
    );
}

// ---------------------------------------------------------------------------
// 11. The catalog
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_multiplicity_violation_code_is_sacm_mult_001_in_the_catalog() {
    assert_eq!(
        Code::MULTIPLICITY_VIOLATION.as_str(),
        "SACM-MULT-001",
        "{ROW}: Code::MULTIPLICITY_VIOLATION is not SACM-MULT-001"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", Code::MULTIPLICITY_VIOLATION.as_str());
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&cell))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-MULT-001, not one",
            rows.len()
        );
    };
    assert!(
        line.starts_with("| SACM-MULT-001 | Error |"),
        "{ROW}: the catalog's SACM-MULT-001 row is not an Error: {line}"
    );
    // The widened text: any multiplicity violation, with the missing name as
    // its first use.
    for part in [
        "a property held more or fewer times than its multiplicity allows",
        "`name:LangString[1]`",
        "SACM23-VAL-018",
        "`Code::MULTIPLICITY_VIOLATION`",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-MULT-001 row does not say {part:?}: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// 12. A deep document
// ---------------------------------------------------------------------------

#[test]
fn sacm23_val_018_a_nameless_element_at_the_end_of_a_deep_chain_is_reported() {
    // As deep as `document.rs`'s deep traversal test: a recursive rule would
    // overflow the test thread's stack.
    const DEPTH: usize = 100_000;
    let id_at = |level: usize| format!("acp_{level}");
    let mut doc = Document::new();
    for level in 0..DEPTH {
        let parent = level.checked_sub(1).map(id_at);
        let id = id_at(level);
        let name = (level + 1 < DEPTH).then(|| LangString::new(id.clone()));
        add(
            &mut doc,
            parent.as_deref(),
            ElementKind::AssuranceCasePackage,
            &id,
            name,
        );
    }
    assert_findings(
        &doc,
        &[missing_name(
            ElementKind::AssuranceCasePackage,
            &id_at(DEPTH - 1),
        )],
        "a 100,000-deep chain whose last package is nameless",
    );
}
