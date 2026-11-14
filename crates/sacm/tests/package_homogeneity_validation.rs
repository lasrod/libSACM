//! Tests for the `ArgumentPackage` homogeneity rule of `validation::validate`
//! (SACM23-VAL-012; SACM 2.3 §11.4 Constraints "If an `ArgumentPackage` has
//! nested `ArgumentPackages`, then it is only allowed to contain
//! `ArgumentPackages`."; §11.6; D-PKG-HOMOG, D-SEV, D-VAL-LAYER; ADR 0022).
//!
//! For each plain `ArgumentPackage` with a plain `ArgumentPackage` among its
//! `argumentElement` children, each other `argumentElement` child that is not
//! of an `ArgumentPackage` kind is one SACM-PKG-001 Error
//! (`Code::PACKAGE_CONTENT_INVALID`), citing clause 11.4 and affecting the
//! package, then the child. An `ArgumentPackageInterface` or
//! `ArgumentPackageBinding` child neither triggers the rule nor is reported;
//! `UtilityElement` children and `name` are not argumentation content; only a
//! plain `ArgumentPackage` is an owner; each package is checked for its own
//! children.
//!
//! Documents are read from inline XMI, built by edit, or built through the
//! `test-support` seam `Document::insert` for the kinds the reader cannot read
//! yet (interfaces, bindings and the other `ArgumentationElement` kinds). Every
//! `ModelElement` is named so that no other rule reports anything. The message
//! is checked only for what it must name, never word for word.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};

use ElementKind::{
    ArgumentPackage as Package, ArgumentPackageBinding as Binding,
    ArgumentPackageInterface as Interface, Claim,
};

const ROW: &str = "SACM23-VAL-012";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The invalid fixture of this row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/validation/invalid/argument-package-not-homogeneous.sacm.xmi"
);

// --- Inputs and helpers ------------------------------------------------------------

/// An ArgumentPackage root `p` named "P" holding `body`.
fn root_p(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}\
         </sacm:ArgumentPackage>"
    )
}

/// An AssuranceCasePackage root `case` holding an ArgumentPackage `p` named
/// "P" holding `body`.
fn p_in_case(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{body}</argumentPackage>\
         </sacm:AssuranceCasePackage>"
    )
}

/// A nested ArgumentPackage `id` holding `body`.
fn package(id: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"{id}\">\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// A Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"{id}\"/>\
         </argumentElement>"
    )
}

/// One `UtilityElement` of each kind, ids `u_<role>`.
fn every_utility() -> String {
    [
        "<description xmi:id=\"u_description\"><content><value content=\"d\"/></content></description>",
        "<implementationConstraint xmi:id=\"u_ic\"/>",
        "<note xmi:id=\"u_note\"><content><value content=\"n\"/></content></note>",
        "<taggedValue xmi:id=\"u_tv\"><key><value content=\"k\"/></key></taggedValue>",
    ]
    .concat()
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
/// another rule (such as D-IFACE-CITE's) may also report on a kind.
fn this_rows_findings(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|finding| finding.requirement_id() == ROW)
        .collect()
}

/// Asserts the fields of a SACM23-VAL-012 finding: an Error SACM-PKG-001,
/// clause 11.4, no location, affected exactly `[owner, child]`, and a message
/// naming each of `named`.
fn assert_finding(what: &str, found: &Diagnostic, owner: &str, child: &str, named: &[&str]) {
    assert_eq!(
        (found.code(), found.severity(), found.requirement_id()),
        (Code::PACKAGE_CONTENT_INVALID, Severity::Error, ROW),
        "{ROW}: {what}: not an Error SACM-PKG-001 of this row: {found:?}"
    );
    assert_eq!(
        (found.affected(), found.location(), found.clause()),
        (
            [ElementId::from(owner), ElementId::from(child)].as_slice(),
            None,
            Some("11.4")
        ),
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

/// Asserts that `found` is exactly one finding per `(owner, child)`, in that
/// order, each with only the fields [`assert_finding`] checks.
fn assert_findings(what: &str, found: &[Diagnostic], expected: &[(&str, &str)]) {
    assert_eq!(
        found.len(),
        expected.len(),
        "{ROW}: {what}: {} findings, not {}: {found:?}",
        found.len(),
        expected.len()
    );
    for (finding, (owner, child)) in found.iter().zip(expected) {
        assert_finding(what, finding, owner, child, &[]);
    }
}

/// Reads `input` and validates it, returning the findings.
fn read_findings(what: &str, input: &str) -> Vec<Diagnostic> {
    findings(what, &read_ok(what, input))
}

/// Inserts one element through the seam, named after its id, or fails the
/// test as a setup error.
fn insert(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    let name = kind
        .is_a(AbstractClass::ModelElement)
        .then(|| LangString::new(id));
    if let Err(error) = doc.insert(parent.as_ref(), kind, ElementId::from(id), name) {
        panic!("{ROW}: setup could not insert {kind} {id:?}: {error}");
    }
}

/// A document holding a root of `owner` kind `p` whose children, in order,
/// are the `(kind, id)` of `children`.
fn seam(owner: ElementKind, children: &[(ElementKind, &str)]) -> Document {
    let mut doc = Document::new();
    insert(&mut doc, None, owner, "p");
    for (kind, id) in children {
        insert(&mut doc, Some("p"), *kind, id);
    }
    doc
}

/// The ids of the children of `id`, in order.
fn child_ids(doc: &Document, id: &str) -> Vec<String> {
    let Some(element) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    element
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

// --- Item 1: a nested package and a Claim -------------------------------------------

#[test]
fn sacm23_val_012_a_package_nesting_a_package_and_holding_a_claim_is_one_error() {
    let layouts = [
        (
            "as a root, q first",
            root_p(&(package("q", "") + &claim("c"))),
        ),
        (
            "as a root, c first",
            root_p(&(claim("c") + &package("q", ""))),
        ),
        (
            "in an AssuranceCasePackage, q first",
            p_in_case(&(package("q", "") + &claim("c"))),
        ),
        (
            "in an AssuranceCasePackage, c first",
            p_in_case(&(claim("c") + &package("q", ""))),
        ),
    ];
    for (what, input) in layouts {
        let found = read_findings(what, &input);
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        assert_finding(what, finding, "p", "c", &["\"p\"", "\"q\"", "Claim \"c\""]);
        assert_eq!(
            finding.code().as_str(),
            "SACM-PKG-001",
            "{ROW}: {what}: the code text is not SACM-PKG-001"
        );
        assert!(
            diagnostic::has_errors(&found),
            "{ROW}: {what}: has_errors is false"
        );
    }
}

#[test]
fn sacm23_val_012_the_fixture_gives_one_error_on_the_package_and_its_claim() {
    let input = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let found = read_findings("the fixture", &input);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: the fixture: not one finding: {found:?}");
    };
    assert_finding(
        "the fixture",
        finding,
        "arguments",
        "c1",
        &["\"arguments\"", "\"braking\"", "Claim \"c1\""],
    );
}

// --- Item 2: one finding per offending child ----------------------------------------

#[test]
fn sacm23_val_012_each_other_child_is_its_own_finding_and_other_packages_are_not() {
    let what = "p{q, c1, c2}";
    let found = read_findings(
        what,
        &root_p(&[package("q", ""), claim("c1"), claim("c2")].concat()),
    );
    assert_findings(what, &found, &[("p", "c1"), ("p", "c2")]);

    let what = "p{q, r, c}";
    let found = read_findings(
        what,
        &root_p(&[package("q", ""), package("r", ""), claim("c")].concat()),
    );
    assert_findings(what, &found, &[("p", "c")]);
}

// --- Item 3: what is homogeneous, and the levels ------------------------------------

#[test]
fn sacm23_val_012_homogeneous_packages_and_lower_levels_give_no_finding() {
    let cases = [
        ("p{q, r}", root_p(&(package("q", "") + &package("r", "")))),
        ("p{c}", root_p(&claim("c"))),
        ("an empty p", root_p("")),
        ("p{q{c}}", root_p(&package("q", &claim("c")))),
    ];
    for (what, input) in cases {
        let found = read_findings(what, &input);
        assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
    }
}

#[test]
fn sacm23_val_012_each_package_is_checked_for_its_own_children() {
    let what = "p{q{r, c}}";
    let found = read_findings(
        what,
        &root_p(&package("q", &(package("r", "") + &claim("c")))),
    );
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one finding: {found:?}");
    };
    assert_finding(what, finding, "q", "c", &["\"q\"", "\"r\"", "Claim \"c\""]);
}

// --- Item 4: UtilityElements are not argumentation content ------------------------

#[test]
fn sacm23_val_012_utility_children_do_not_count_and_a_claim_beside_them_does() {
    let what = "p holding every utility kind and q";
    let found = read_findings(what, &root_p(&(every_utility() + &package("q", ""))));
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");

    let what = "p holding every utility kind, q and c";
    let found = read_findings(
        what,
        &root_p(&[every_utility(), package("q", ""), claim("c")].concat()),
    );
    assert_findings(what, &found, &[("p", "c")]);
}

// --- Item 5: interfaces and bindings ------------------------------------------------

#[test]
fn sacm23_val_012_interfaces_and_bindings_neither_trigger_nor_are_reported() {
    let quiet: [(&str, &[(ElementKind, &str)]); 3] = [
        ("p{i, c}", &[(Interface, "i"), (Claim, "c")]),
        ("p{b, c}", &[(Binding, "b"), (Claim, "c")]),
        (
            "p{q, i, b}",
            &[(Package, "q"), (Interface, "i"), (Binding, "b")],
        ),
    ];
    for (what, children) in quiet {
        let found = this_rows_findings(&seam(Package, children));
        assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
    }

    let what = "p{q, i, c}";
    let found = this_rows_findings(&seam(
        Package,
        &[(Package, "q"), (Interface, "i"), (Claim, "c")],
    ));
    assert_findings(what, &found, &[("p", "c")]);
}

#[test]
fn sacm23_val_012_an_interface_or_binding_owner_is_not_checked() {
    for owner in [Interface, Binding] {
        let what = format!("a {owner} holding a plain package and a Claim");
        let found = this_rows_findings(&seam(owner, &[(Package, "q"), (Claim, "c")]));
        assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
    }
}

// --- Item 6: every other ArgumentationElement kind ----------------------------------

#[test]
fn sacm23_val_012_each_non_package_argumentation_kind_beside_a_package_is_one_finding() {
    let kinds: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ArgumentationElement) && !kind.is_kind_of(Package))
        .collect();
    assert_eq!(
        kinds.len(),
        9,
        "{ROW}: setup: not 9 non-package ArgumentationElement kinds: {kinds:?}"
    );
    for kind in kinds {
        let what = format!("p{{q, {kind} x}}");
        let found = this_rows_findings(&seam(Package, &[(Package, "q"), (kind, "x")]));
        let [finding] = found.as_slice() else {
            panic!("{ROW}: {what}: not one finding: {found:?}");
        };
        let subject = format!("{} \"x\"", kind.name());
        assert_finding(&what, finding, "p", "x", &[subject.as_str()]);
    }
}

// --- Item 7: other package owners ---------------------------------------------------

#[test]
fn sacm23_val_012_artifact_and_assurance_case_package_owners_are_out_of_scope() {
    // An ArtifactPackage admits every ArtifactElement (ADR 0014), an
    // ArgumentPackage and a Claim among them.
    let what = "an ArtifactPackage holding an ArgumentPackage and a Claim";
    let found = this_rows_findings(&seam(
        ElementKind::ArtifactPackage,
        &[(Package, "q"), (Claim, "c")],
    ));
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");

    let what = "an AssuranceCasePackage holding an ArgumentPackage, another AssuranceCasePackage and a Description";
    let input = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"q\"><name content=\"Q\"/></argumentPackage>\
         <assuranceCasePackage xmi:id=\"sub\"><name content=\"Sub\"/></assuranceCasePackage>\
         <description xmi:id=\"d\"><content><value content=\"d\"/></content></description>\
         </sacm:AssuranceCasePackage>"
    );
    let found = read_findings(what, &input);
    assert!(found.is_empty(), "{ROW}: {what}: findings: {found:?}");
}

// --- Item 8: by edit ----------------------------------------------------------------

#[test]
fn sacm23_val_012_edits_that_mix_the_content_succeed_and_validate_reports() {
    let name = |text: &str| LangString::new(text).with_lang("en");
    // A Claim into a package that nests one, and a package into a package
    // holding a Claim.
    let orders: [(&str, bool); 2] = [
        ("CreateClaim after CreateArgumentPackage", true),
        ("CreateArgumentPackage after CreateClaim", false),
    ];
    for (what, package_first) in orders {
        let mut doc = Document::new();
        let mut results = vec![edit::apply(
            &mut doc,
            CreateArgumentPackage::root(name("P")).with_id("p"),
        )];
        let nested = CreateArgumentPackage::new("p", name("Q")).with_id("q");
        let create_claim = CreateClaim::new("p", name("C")).with_id("c");
        if package_first {
            results.push(edit::apply(&mut doc, nested));
            results.push(edit::apply(&mut doc, create_claim));
        } else {
            results.push(edit::apply(&mut doc, create_claim));
            results.push(edit::apply(&mut doc, nested));
        }
        for result in results {
            if let Err(error) = result {
                panic!("{ROW}: {what}: an edit was refused: {error:?} ({error})");
            }
        }
        let found = findings(what, &doc);
        assert_findings(what, &found, &[("p", "c")]);
    }
}

// --- Item 9: saving -----------------------------------------------------------------

#[test]
fn sacm23_val_012_a_strict_write_refuses_and_a_tolerant_write_keeps_every_child() {
    let what = "p{c1, q, every utility kind, c2}";
    let input = root_p(&[claim("c1"), package("q", ""), every_utility(), claim("c2")].concat());
    let doc = read_ok(what, &input);
    let expected = validation::validate(&doc);
    assert_findings(what, &expected, &[("p", "c1"), ("p", "c2")]);

    let error = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: {what}: a strict write returned:\n{text}"),
        Err(error) => error,
    };
    let WriteError::Invalid { diagnostics, .. } = &error else {
        panic!("{ROW}: {what}: the strict refusal is not Invalid: {error:?}");
    };
    assert_eq!(
        diagnostics, &expected,
        "{ROW}: {what}: the strict refusal does not hold the findings"
    );

    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a tolerant write refused: {error:?}"),
    };
    let back = read_ok(&format!("{what}, written"), &text);
    assert_eq!(
        child_ids(&back, "p"),
        ["c1", "q", "u_description", "u_ic", "u_note", "u_tv", "c2"],
        "{ROW}: {what}: the children do not read back in order\n{text}"
    );
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: {what}: the tolerant output does not read back equal\n{text}"
    );
}

// --- Item 10: the code and the catalog ----------------------------------------------

#[test]
fn sacm23_val_012_package_content_invalid_is_sacm_pkg_001_and_its_catalog_row_names_it() {
    assert_eq!(
        Code::PACKAGE_CONTENT_INVALID.as_str(),
        "SACM-PKG-001",
        "{ROW}: Code::PACKAGE_CONTENT_INVALID is not SACM-PKG-001"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("| SACM-PKG-001 |"))
        .collect();
    let [line] = rows.as_slice() else {
        panic!(
            "{ROW}: docs/sacm/diagnostics.md has {} rows for SACM-PKG-001, not one",
            rows.len()
        );
    };
    for part in [
        "| SACM-PKG-001 | Error",
        "`Code::PACKAGE_CONTENT_INVALID`",
        "SACM23-VAL-012",
        "11.4",
    ] {
        assert!(
            line.contains(part),
            "{ROW}: the catalog's SACM-PKG-001 row does not say {part:?}: {line}"
        );
    }
}
