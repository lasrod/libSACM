//! Tests for the package interface and binding rules of `validation::validate`
//! (slice 0068): interface and binding content (SACM23-VAL-013; formal/23-05-08
//! §10.5, §10.6, §11.5, §11.6, §12.4, §12.5; D-IFACE-CITE), `implements` and
//! distinct participants (SACM23-VAL-014; §9.3, §9.4, §10.5, §10.6, §11.5,
//! §11.6, §12.4, §12.5; D-BIND-PART), `AssuranceCasePackageInterface` content
//! (SACM23-VAL-015; the §9.3 OCL) and interface placement (SACM23-VAL-023;
//! §10.5, §11.6, §12.5 Semantics; D-SEV, SACM-PKG-002).
//!
//! A citation is an element with `isCitation` true and a `citedElement`, an
//! id or a cross-document `href` (the §11.5 OCL `e.isCitation = true and
//! e.citedElement <> null`; ADR 0040). Every finding is checked by code,
//! severity, row, clause, location and affected ids, and by its template; the
//! message and the arguments where the design gives them.
//!
//! Documents are read from inline XMI in both modes, which must read them
//! with no diagnostic unless the test keeps preserved content, or built by
//! edit. Every element is named, and every Term has a value, so that no other
//! rule reports anything unless the test says so.

use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateClaim, MoveElement, Mutation, Operation, SetFlag, SetReferences};
use sacm::model::{Document, ElementId, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const VAL_013: &str = "SACM23-VAL-013";
const VAL_014: &str = "SACM23-VAL-014";
const VAL_015: &str = "SACM23-VAL-015";
const VAL_023: &str = "SACM23-VAL-023";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a root: `sacm`, `xmi` and a vendor `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:v=\"urn:vendor\"",
);

const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";

/// The findings fixture of the slice.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/invalid/modularity-findings.sacm.xmi"
);

const ERROR: Severity = Severity::Error;
const WARNING: Severity = Severity::Warning;

const MULT_001: &str = "SACM-MULT-001";
const PKG_001: &str = "SACM-PKG-001";
const PKG_002: &str = "SACM-PKG-002";

// --- The package families --------------------------------------------------------

/// One package family: its package, interface and binding classes, the role
/// that holds their content, the role an AssuranceCasePackage holds them in,
/// the clauses, the kind an interface cites, and a plain element of the
/// family that is no citation.
#[derive(Clone, Copy, Debug)]
struct Family {
    package: &'static str,
    interface: &'static str,
    binding: &'static str,
    /// The role of the content: `argumentElement`, `artifactElement` or
    /// `terminologyElement`.
    role: &'static str,
    /// The role of an AssuranceCasePackage that holds the family's packages.
    case_role: &'static str,
    interface_clause: &'static str,
    binding_clause: &'static str,
    /// What an interface's citation cites (§11.6, §12.5, §10.5).
    asset: &'static str,
    /// The class of a plain element of the family.
    child: &'static str,
    /// The attributes that element needs to give no finding of its own.
    child_attributes: &'static str,
}

const ARGUMENT: Family = Family {
    package: "ArgumentPackage",
    interface: "ArgumentPackageInterface",
    binding: "ArgumentPackageBinding",
    role: "argumentElement",
    case_role: "argumentPackage",
    interface_clause: "11.6",
    binding_clause: "11.5",
    asset: "ArgumentAsset",
    child: "Claim",
    child_attributes: "",
};

const ARTIFACT: Family = Family {
    package: "ArtifactPackage",
    interface: "ArtifactPackageInterface",
    binding: "ArtifactPackageBinding",
    role: "artifactElement",
    case_role: "artifactPackage",
    interface_clause: "12.5",
    binding_clause: "12.4",
    asset: "ArtifactAsset",
    child: "Artifact",
    child_attributes: "",
};

const TERMINOLOGY: Family = Family {
    package: "TerminologyPackage",
    interface: "TerminologyPackageInterface",
    binding: "TerminologyPackageBinding",
    role: "terminologyElement",
    case_role: "terminologyPackage",
    interface_clause: "10.5",
    binding_clause: "10.6",
    asset: "TerminologyElement",
    child: "Term",
    child_attributes: " value=\"v\"",
};

/// The three families whose interfaces and bindings hold citations.
const FAMILIES: [Family; 3] = [ARGUMENT, ARTIFACT, TERMINOLOGY];

/// The `AssuranceCase` family: its interface (§9.3) and binding (§9.4).
const CASE_INTERFACE: &str = "AssuranceCasePackageInterface";
const CASE_BINDING: &str = "AssuranceCasePackageBinding";

// --- Inputs ----------------------------------------------------------------------

/// An element of `class` in the role `role`, `id`, with `attributes`, named
/// after its id, holding `body`.
fn el(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// A root of `class`, `id`, with `attributes`, named after its id, holding
/// `body`.
fn root(class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<sacm:{class}{NS} xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</sacm:{class}>"
    )
}

/// The root AssuranceCasePackage `case` holding `body`.
fn case(body: &str) -> String {
    root("AssuranceCasePackage", "case", "", body)
}

/// Several roots in one `xmi:XMI` wrapper.
fn wrapped(roots: &[String]) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:v=\"urn:vendor\">{}</xmi:XMI>",
        roots
            .iter()
            .map(|one| one.replacen(NS, "", 1))
            .collect::<String>()
    )
}

impl Family {
    /// A plain package of the family, `id`, in the case's role.
    fn package_in_case(self, id: &str, body: &str) -> String {
        el(self.case_role, self.package, id, "", body)
    }

    /// A plain package of the family, `id`, nested in a package of the
    /// family.
    fn nested_package(self, id: &str, body: &str) -> String {
        el(self.role, self.package, id, "", body)
    }

    /// An interface `id` of the family with `attributes`, in the content role.
    fn interface_el(self, id: &str, attributes: &str, body: &str) -> String {
        el(self.role, self.interface, id, attributes, body)
    }

    /// A binding `id` of the family with `attributes`, in the case's role.
    fn binding_in_case(self, id: &str, attributes: &str, body: &str) -> String {
        el(self.case_role, self.binding, id, attributes, body)
    }

    /// A plain element `id` of the family, with `attributes`.
    fn child_el(self, id: &str, attributes: &str) -> String {
        el(
            self.role,
            self.child,
            id,
            &format!("{}{attributes}", self.child_attributes),
            "",
        )
    }

    /// A citation `id` of the family citing `cited`.
    fn citation(self, id: &str, cited: &str) -> String {
        self.child_el(
            id,
            &format!(" isCitation=\"true\" citedElement=\"{cited}\""),
        )
    }

    /// The root package `p` of the family holding the element `c0` and the
    /// interface `i`, which implements `p` and holds `body`.
    fn i_in_p(self, body: &str) -> String {
        root(
            self.package,
            "p",
            "",
            &format!(
                "{}{}",
                self.child_el("c0", ""),
                self.interface_el("i", " implements=\"p\"", body)
            ),
        )
    }
}

// --- Reading, writing and validating ---------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{row}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{row}: {what}: a {mode:?} read refused it: {error} ({error:?})\n{input}")
            }
        }
    }
    assert!(
        documents[0] == documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The document a tolerant read of `input` returns, which must read.
fn tolerant(row: &str, what: &str, input: &str) -> Document {
    match read(Mode::Tolerant, input) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error} ({error:?})"),
    }
}

fn write_in(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in both modes and reads each output back `==`.
fn assert_both_saves_write(row: &str, what: &str, doc: &Document) {
    for mode in MODES {
        let text = write_in(row, what, doc, mode);
        let back = match read(mode, &text) {
            Ok(output) => output.into_document(),
            Err(error) => {
                panic!("{row}: {what}: the {mode:?} output does not read back: {error}\n{text}")
            }
        };
        assert_eq!(
            semantic_compare(doc, &back),
            [],
            "{row}: {what}: the {mode:?} output does not read back equal\n{text}"
        );
    }
}

/// One finding: code, severity, row, clause, location, affected ids and
/// template.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Found {
    code: String,
    severity: Severity,
    row: String,
    clause: Option<String>,
    location: Option<SourceLocation>,
    affected: Vec<String>,
    template: String,
}

fn found(diagnostic: &Diagnostic) -> Found {
    Found {
        code: diagnostic.code().as_str().to_owned(),
        severity: diagnostic.severity(),
        row: diagnostic.requirement_id().to_owned(),
        clause: diagnostic.clause().map(str::to_owned),
        location: diagnostic.location(),
        affected: diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        template: diagnostic.template().to_owned(),
    }
}

/// The expected finding: no location.
fn f(template: &str, severity: Severity, row: &str, clause: &str, affected: &[&str]) -> Found {
    let code = template.split_once('/').map_or(template, |(code, _)| code);
    Found {
        code: code.to_owned(),
        severity,
        row: row.to_owned(),
        clause: Some(clause.to_owned()),
        location: None,
        affected: affected.iter().map(|&id| id.to_owned()).collect(),
        template: template.to_owned(),
    }
}

/// Every finding of `validate` on `doc`.
fn findings(doc: &Document) -> Vec<Found> {
    validation::validate(doc).iter().map(found).collect()
}

/// The findings of `validate` on `doc` with the requirement `row`.
fn of_row(doc: &Document, row: &str) -> Vec<Found> {
    findings(doc)
        .into_iter()
        .filter(|one| one.row == row)
        .collect()
}

/// The findings of `validate` on `doc` with the code `code`.
fn of_code(doc: &Document, code: &str) -> Vec<Found> {
    findings(doc)
        .into_iter()
        .filter(|one| one.code == code)
        .collect()
}

/// The one diagnostic of `validate` on `doc` with `template`, which must be
/// exactly one.
fn the_one(row: &str, what: &str, doc: &Document, template: &str) -> Diagnostic {
    let all = validation::validate(doc);
    let matching: Vec<&Diagnostic> = all
        .iter()
        .filter(|one| one.template() == template)
        .collect();
    let [one] = matching.as_slice() else {
        panic!("{row}: {what}: not one {template} finding: {all:#?}");
    };
    (*one).clone()
}

/// The arguments of `diagnostic` by name, values as text.
fn arguments(diagnostic: &Diagnostic) -> Vec<(String, String)> {
    diagnostic
        .arguments()
        .iter()
        .map(|argument| (argument.name().to_owned(), argument.value().to_string()))
        .collect()
}

/// `pairs` as owned strings.
fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|&(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

/// The catalog row of `code`, which must be exactly one.
fn catalog_row(row: &str, code: &str) -> String {
    let catalog = catalog(row);
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

fn catalog(row: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()))
}

// --- Edits -----------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies `operation`, which the test expects to succeed.
fn applied(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: {operation:?} was refused: {error} ({error:?})"),
    }
}

// =================================================================================
// SACM23-VAL-013: interface and binding content
// =================================================================================

#[test]
fn sacm23_val_013_a_non_citation_in_an_interface_is_one_error_with_its_clause() {
    let row = VAL_013;
    for family in FAMILIES {
        let what = format!("a non-citation {} in an {}", family.child, family.interface);
        let doc = read_ok(row, &what, &family.i_in_p(&family.child_el("c", "")));
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/not-a-citation",
                ERROR,
                row,
                family.interface_clause,
                &["i", "c"]
            )],
            "{row}: {what}: not exactly one SACM-PKG-001 Error with clause {}",
            family.interface_clause
        );
        let one = the_one(row, &what, &doc, "SACM-PKG-001/not-a-citation");
        assert_eq!(
            arguments(&one),
            pairs(&[
                ("class", family.interface),
                ("id", "i"),
                ("child_class", family.child),
                ("child_id", "c"),
                ("clause", family.interface_clause),
            ]),
            "{row}: {what}: the arguments"
        );
        assert_eq!(
            one.message(),
            format!(
                "{} \"i\" contains {} \"c\", which is no citation (isCitation true and a \
                 citedElement), but §{} allows only citations in it",
                family.interface, family.child, family.interface_clause
            ),
            "{row}: {what}: the message"
        );
    }
}

#[test]
fn sacm23_val_013_is_citation_without_a_cited_element_is_no_citation() {
    let row = VAL_013;
    for family in FAMILIES {
        let what = format!(
            "an {} child with isCitation and no citedElement",
            family.interface
        );
        let doc = read_ok(
            row,
            &what,
            &family.i_in_p(&family.child_el("ci", " isCitation=\"true\"")),
        );
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/not-a-citation",
                ERROR,
                row,
                family.interface_clause,
                &["i", "ci"]
            )],
            "{row}: {what}: isCitation=\"true\" alone is no citation (§11.5 OCL)"
        );
        // A citedElement without isCitation: this Error and SACM-CITE-001.
        let what = format!(
            "an {} child with a citedElement and no isCitation",
            family.interface
        );
        let doc = read_ok(
            row,
            &what,
            &family.i_in_p(&family.child_el("ci", " citedElement=\"c0\"")),
        );
        assert_eq!(
            findings(&doc),
            [
                f(
                    "SACM-CITE-001/not-a-citation",
                    ERROR,
                    "SACM23-VAL-007",
                    "8.2",
                    &["ci"]
                ),
                f(
                    "SACM-PKG-001/not-a-citation",
                    ERROR,
                    row,
                    family.interface_clause,
                    &["i", "ci"]
                ),
            ],
            "{row}: {what}: not SACM-CITE-001 and this row's Error, and no Warning"
        );
    }
}

#[test]
fn sacm23_val_013_a_citation_by_id_href_or_dangling_id_gives_no_error() {
    let row = VAL_013;
    for family in FAMILIES {
        let what = format!("an {} citing c0 by id", family.interface);
        let doc = read_ok(row, &what, &family.i_in_p(&family.citation("ci", "c0")));
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
        let what = format!("an {} citing a dangling id", family.interface);
        let doc = read_ok(row, &what, &family.i_in_p(&family.citation("ci", "gone")));
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-REF-001/dangling",
                ERROR,
                "SACM23-VAL-003",
                "8.2",
                &["ci", "gone"]
            )],
            "{row}: {what}: not only SACM-REF-001"
        );
        let what = format!("an {} citing by a cross-document href", family.interface);
        let citation = el(
            family.role,
            family.child,
            "ci",
            &format!("{} isCitation=\"true\"", family.child_attributes),
            "<citedElement href=\"o.xmi#c\"/>",
        );
        let doc = read_ok(row, &what, &family.i_in_p(&citation));
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
}

#[test]
fn sacm23_val_013_a_terminology_binding_allows_only_citations() {
    let row = VAL_013;
    let family = TERMINOLOGY;
    let input = |body: &str| {
        case(&format!(
            "{}{}{}",
            family.package_in_case("t1", &family.child_el("c0", "")),
            family.package_in_case("t2", ""),
            family.binding_in_case("b", " participantPackage=\"t1 t2\"", body)
        ))
    };
    let what = "a non-citation Term in a TerminologyPackageBinding";
    let doc = read_ok(row, what, &input(&family.child_el("c", "")));
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/not-a-citation",
            ERROR,
            row,
            family.binding_clause,
            &["b", "c"]
        )],
        "{row}: {what}: not one Error with clause 10.6"
    );
    assert_eq!(
        the_one(row, what, &doc, "SACM-PKG-001/not-a-citation").message(),
        "TerminologyPackageBinding \"b\" contains Term \"c\", which is no citation (isCitation \
         true and a citedElement), but §10.6 allows only citations in it",
        "{row}: {what}: the message"
    );
    let what = "a citation Term in a TerminologyPackageBinding";
    let doc = read_ok(row, what, &input(&family.citation("ci", "c0")));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
}

/// The AssuranceCasePackage `case` holding the `ArgumentPackage`s `p` (with the
/// Claim `c0`) and `q`, and the `ArgumentPackageBinding` `b` of `[p, q]`
/// holding `body`.
fn argument_binding(body: &str) -> String {
    case(&format!(
        "{}{}{}",
        ARGUMENT.package_in_case("p", &ARGUMENT.child_el("c0", "")),
        ARGUMENT.package_in_case("q", ""),
        ARGUMENT.binding_in_case("b", " participantPackage=\"p q\"", body)
    ))
}

#[test]
fn sacm23_val_013_an_argument_binding_allows_citations_and_asserted_relationships() {
    let row = VAL_013;
    // The five AssertedRelationship kinds, no citations: none of this row.
    for kind in [
        "AssertedInference",
        "AssertedEvidence",
        "AssertedContext",
        "AssertedArtifactSupport",
        "AssertedArtifactContext",
    ] {
        let what = format!("a non-citation {kind} in an ArgumentPackageBinding");
        let body = format!(
            "{}{}",
            ARGUMENT.citation("ci", "c0"),
            el(
                "argumentElement",
                kind,
                "r",
                " source=\"ci\" target=\"ci\"",
                ""
            )
        );
        let doc = read_ok(row, &what, &argument_binding(&body));
        assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
        assert_eq!(of_code(&doc, PKG_001), [], "{row}: {what}: a SACM-PKG-001");
    }
    // Every other kind: one Error each.
    for (kind, attributes) in [
        ("Claim", ""),
        ("ArgumentReasoning", ""),
        ("ArtifactReference", ""),
        ("ArgumentGroup", ""),
        ("ArgumentPackage", ""),
    ] {
        let what = format!("a non-citation {kind} in an ArgumentPackageBinding");
        let doc = read_ok(
            row,
            &what,
            &argument_binding(&el("argumentElement", kind, "c", attributes, "")),
        );
        assert_eq!(
            of_code(&doc, PKG_001),
            [f(
                "SACM-PKG-001/not-a-citation-or-relationship",
                ERROR,
                row,
                ARGUMENT.binding_clause,
                &["b", "c"]
            )],
            "{row}: {what}: not one Error with clause 11.5"
        );
        let one = the_one(
            row,
            &what,
            &doc,
            "SACM-PKG-001/not-a-citation-or-relationship",
        );
        assert_eq!(
            arguments(&one),
            pairs(&[
                ("class", "ArgumentPackageBinding"),
                ("id", "b"),
                ("child_class", kind),
                ("child_id", "c"),
                ("clause", "11.5"),
                ("relationship", "AssertedRelationship"),
            ]),
            "{row}: {what}: the arguments"
        );
        if kind == "Claim" {
            assert_eq!(
                one.message(),
                "ArgumentPackageBinding \"b\" contains Claim \"c\", which is no citation \
                 (isCitation true and a citedElement), but §11.5 allows only citations and \
                 AssertedRelationships in it",
                "{row}: {what}: the message"
            );
        }
    }
}

#[test]
fn sacm23_val_013_an_artifact_binding_allows_citations_and_artifact_asset_relationships() {
    let row = VAL_013;
    let family = ARTIFACT;
    let input = |body: &str| {
        case(&format!(
            "{}{}{}",
            family.package_in_case("t", &family.child_el("c0", "")),
            family.package_in_case("u", ""),
            family.binding_in_case("b", " participantPackage=\"t u\"", body)
        ))
    };
    let what = "a non-citation ArtifactAssetRelationship in an ArtifactPackageBinding";
    let body = format!(
        "{}{}",
        family.citation("ci", "c0"),
        el(
            "artifactElement",
            "ArtifactAssetRelationship",
            "r",
            " source=\"ci\" target=\"ci\"",
            ""
        )
    );
    let doc = read_ok(row, what, &input(&body));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    let what = "a non-citation Artifact in an ArtifactPackageBinding";
    let doc = read_ok(row, what, &input(&family.child_el("c", "")));
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/not-a-citation-or-relationship",
            ERROR,
            row,
            family.binding_clause,
            &["b", "c"]
        )],
        "{row}: {what}: not one Error with clause 12.4"
    );
    let one = the_one(
        row,
        what,
        &doc,
        "SACM-PKG-001/not-a-citation-or-relationship",
    );
    assert_eq!(
        arguments(&one),
        pairs(&[
            ("class", "ArtifactPackageBinding"),
            ("id", "b"),
            ("child_class", "Artifact"),
            ("child_id", "c"),
            ("clause", "12.4"),
            ("relationship", "ArtifactAssetRelationship"),
        ]),
        "{row}: {what}: the arguments"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the three families' cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_val_013_a_citation_of_the_wrong_kind_in_an_interface_is_a_warning() {
    let row = VAL_013;
    // In p, i (implements p) holds ci, which cites the ArgumentPackage q in p.
    let what = "an ArgumentPackageInterface citing an ArgumentPackage inside its package";
    let input = root(
        "ArgumentPackage",
        "p",
        "",
        &format!(
            "{}{}",
            ARGUMENT.nested_package("q", ""),
            ARGUMENT.interface_el("i", " implements=\"p\"", &ARGUMENT.citation("ci", "q"))
        ),
    );
    let doc = read_ok(row, what, &input);
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/cited-kind",
            WARNING,
            row,
            "11.6",
            &["i", "ci", "q"]
        )],
        "{row}: {what}: not one cited-kind Warning"
    );
    let one = the_one(row, what, &doc, "SACM-PKG-001/cited-kind");
    assert_eq!(
        arguments(&one),
        pairs(&[
            ("class", "ArgumentPackageInterface"),
            ("id", "i"),
            ("child_class", "Claim"),
            ("child_id", "ci"),
            ("target", "q"),
            ("target_class", "ArgumentPackage"),
            ("clause", "11.6"),
            ("expected", ARGUMENT.asset),
        ]),
        "{row}: {what}: the arguments"
    );
    assert_eq!(
        one.message(),
        "ArgumentPackageInterface \"i\" contains Claim \"ci\", which cites \"q\" of the kind \
         ArgumentPackage, but §11.6 has an interface cite only ArgumentAssets",
        "{row}: {what}: the message"
    );
    // An ArtifactPackageInterface citing a Claim inside its package (an
    // ArtifactPackage admits every ArtifactElement, D-ARTPKG).
    let what = "an ArtifactPackageInterface citing a Claim inside its package";
    let input = root(
        "ArtifactPackage",
        "p",
        "",
        &format!(
            "{}{}",
            el("artifactElement", "Claim", "cl", "", ""),
            ARTIFACT.interface_el("i", " implements=\"p\"", &ARTIFACT.citation("ci", "cl"))
        ),
    );
    let doc = read_ok(row, what, &input);
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/cited-kind",
            WARNING,
            row,
            "12.5",
            &["i", "ci", "cl"]
        )],
        "{row}: {what}: not one cited-kind Warning"
    );
    let one = the_one(row, what, &doc, "SACM-PKG-001/cited-kind");
    assert_eq!(
        (one.message(), arguments(&one).last().cloned()),
        (
            "ArtifactPackageInterface \"i\" contains Artifact \"ci\", which cites \"cl\" of the \
             kind Claim, but §12.5 has an interface cite only ArtifactAssets",
            Some(("expected".to_owned(), ARTIFACT.asset.to_owned()))
        ),
        "{row}: {what}: the message and expected"
    );
    // A TerminologyPackageInterface citing a Claim elsewhere: both Warnings.
    let what = "a TerminologyPackageInterface citing a Claim in another package";
    let input = case(&format!(
        "{}{}",
        ARGUMENT.package_in_case("ap", &ARGUMENT.child_el("cl", "")),
        TERMINOLOGY.package_in_case(
            "p",
            &TERMINOLOGY.interface_el("i", " implements=\"p\"", &TERMINOLOGY.citation("ci", "cl"))
        )
    ));
    let doc = read_ok(row, what, &input);
    assert_eq!(
        findings(&doc),
        [
            f(
                "SACM-PKG-001/cited-kind",
                WARNING,
                row,
                "10.5",
                &["i", "ci", "cl"]
            ),
            f(
                "SACM-PKG-001/cited-outside",
                WARNING,
                row,
                "10.5",
                &["i", "ci", "cl"]
            ),
        ],
        "{row}: {what}: not cited-kind and cited-outside"
    );
    assert_eq!(
        argument_named(
            &the_one(row, what, &doc, "SACM-PKG-001/cited-kind"),
            "expected"
        ),
        Some(TERMINOLOGY.asset.to_owned()),
        "{row}: {what}: expected"
    );
}

/// The value of the argument `name` of `diagnostic`, as text.
fn argument_named(diagnostic: &Diagnostic, name: &str) -> Option<String> {
    arguments(diagnostic)
        .into_iter()
        .find(|(given, _)| given == name)
        .map(|(_, value)| value)
}

#[test]
fn sacm23_val_013_a_citation_of_an_element_outside_the_implemented_package_is_a_warning() {
    let row = VAL_013;
    for family in FAMILIES {
        // ci cites d, an element of another package of the family.
        let what = format!(
            "an {} citing an element of another package",
            family.interface
        );
        let input = case(&format!(
            "{}{}",
            family.package_in_case(
                "p",
                &family.interface_el("i", " implements=\"p\"", &family.citation("ci", "d"))
            ),
            family.package_in_case("q", &family.child_el("d", ""))
        ));
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/cited-outside",
                WARNING,
                row,
                family.interface_clause,
                &["i", "ci", "d"]
            )],
            "{row}: {what}: not one cited-outside Warning"
        );
        let one = the_one(row, &what, &doc, "SACM-PKG-001/cited-outside");
        assert_eq!(
            arguments(&one),
            pairs(&[
                ("class", family.interface),
                ("id", "i"),
                ("child_class", family.child),
                ("child_id", "ci"),
                ("target", "d"),
                ("clause", family.interface_clause),
                ("package", "p"),
            ]),
            "{row}: {what}: the arguments"
        );
        if family.interface == ARGUMENT.interface {
            assert_eq!(
                one.message(),
                "ArgumentPackageInterface \"i\" contains Claim \"ci\", which cites \"d\", but \
                 §11.6 has an interface cite only elements within the package it implements, \
                 \"p\"",
                "{row}: {what}: the message"
            );
        }
        // Any depth: p{q{d}, i{ci -> d}} gives none.
        let what = format!(
            "an {} citing an element deeper in its package",
            family.interface
        );
        let input = root(
            family.package,
            "p",
            "",
            &format!(
                "{}{}",
                family.nested_package("q", &family.child_el("d", "")),
                family.interface_el("i", " implements=\"p\"", &family.citation("ci", "d"))
            ),
        );
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_val_013_kind_and_place_need_a_resolved_target() {
    let row = VAL_013;
    let family = ARGUMENT;
    // A cited element that does not resolve to a typed element: neither
    // Warning, whatever the implements.
    // An element the ArgumentPackage role does not admit: preserved content,
    // whose id names no typed element (SACM-REF-003).
    let preserved_q = el("argumentElement", "Term", "z", " value=\"z\"", "");
    for (what, citation, extra, own) in [
        (
            "a dangling citedElement",
            family.citation("ci", "gone"),
            String::new(),
            vec![f(
                "SACM-REF-001/dangling",
                ERROR,
                "SACM23-VAL-003",
                "8.2",
                &["ci", "gone"],
            )],
        ),
        (
            "an href citedElement",
            el(
                "argumentElement",
                "Claim",
                "ci",
                " isCitation=\"true\"",
                "<citedElement href=\"o.xmi#q\"/>",
            ),
            String::new(),
            Vec::new(),
        ),
        (
            "a citedElement naming preserved content",
            family.citation("ci", "z"),
            family.package_in_case("other", &preserved_q),
            vec![f(
                "SACM-REF-003/preserved-target",
                WARNING,
                "LIB-COMPAT-003",
                "8.2",
                &["ci", "z"],
            )],
        ),
    ] {
        let input = case(&format!(
            "{}{extra}",
            family.package_in_case(
                "p",
                &family.interface_el("i", " implements=\"p\"", &citation)
            )
        ));
        let doc = tolerant(row, what, &input);
        assert_eq!(
            findings(&doc),
            own,
            "{row}: {what}: not only the reference's own finding"
        );
    }
    // An implements that is missing, dangling, an href, preserved content or
    // of the wrong kind: no cited-outside, but cited-kind when the cited
    // element resolves. ci cites the ArgumentPackage q, a sibling of p, so a
    // resolved implements p would give both.
    let cited_kind = f(
        "SACM-PKG-001/cited-kind",
        WARNING,
        row,
        "11.6",
        &["i", "ci", "q"],
    );
    for (what, attributes, body, extra) in [
        ("no implements", "", "", String::new()),
        (
            "a dangling implements",
            " implements=\"gone\"",
            "",
            String::new(),
        ),
        (
            "an href implements",
            "",
            "<implements href=\"o.xmi#p\"/>",
            String::new(),
        ),
        (
            "an implements naming preserved content",
            " implements=\"z\"",
            "",
            family.package_in_case("other", &preserved_q),
        ),
        (
            "an implements naming a Claim",
            " implements=\"c0\"",
            "",
            String::new(),
        ),
    ] {
        let input = case(&format!(
            "{}{}{extra}",
            family.package_in_case(
                "p",
                &format!(
                    "{}{}",
                    family.child_el("c0", ""),
                    family.interface_el(
                        "i",
                        attributes,
                        &format!("{body}{}", family.citation("ci", "q"))
                    )
                )
            ),
            family.package_in_case("q", "")
        ));
        let doc = tolerant(row, what, &input);
        assert_eq!(
            of_row(&doc, row),
            std::slice::from_ref(&cited_kind),
            "{row}: {what}: not cited-kind alone"
        );
        assert_eq!(
            of_row(&doc, VAL_023),
            [],
            "{row}: {what}: an unresolved implements is checked for placement"
        );
    }
    // The control: implements p resolves, so both.
    let input = case(&format!(
        "{}{}",
        family.package_in_case(
            "p",
            &family.interface_el("i", " implements=\"p\"", &family.citation("ci", "q"))
        ),
        family.package_in_case("q", "")
    ));
    let doc = read_ok(row, "the control", &input);
    assert_eq!(
        of_row(&doc, row),
        [
            cited_kind,
            f(
                "SACM-PKG-001/cited-outside",
                WARNING,
                row,
                "11.6",
                &["i", "ci", "q"]
            ),
        ],
        "{row}: the control: a resolved implements gives both Warnings"
    );
}

#[test]
fn sacm23_val_013_binding_citations_are_not_checked_for_kind_or_place() {
    let row = VAL_013;
    for family in FAMILIES {
        // The binding b of the interfaces i (in p) and j (in q) holds a
        // citation of the plain package r, of no cited kind and outside both.
        let what = format!(
            "a citation of the wrong kind and place in an {}",
            family.binding
        );
        let input = case(&format!(
            "{}{}{}{}",
            family.package_in_case("p", &family.interface_el("i", " implements=\"p\"", "")),
            family.package_in_case("q", &family.interface_el("j", " implements=\"q\"", "")),
            family.package_in_case("r", ""),
            family.binding_in_case(
                "b",
                " participantPackage=\"i j\"",
                &family.citation("ci", "r")
            )
        ));
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
        // The control: the same citation in the interface i gives both
        // Warnings.
        let what = format!("the control: the citation in the {}", family.interface);
        let input = case(&format!(
            "{}{}",
            family.package_in_case(
                "p",
                &family.interface_el("i", " implements=\"p\"", &family.citation("ci", "rc"))
            ),
            family.package_in_case("r", &family.child_el("rc", "")),
        ));
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            of_row(&doc, row),
            [f(
                "SACM-PKG-001/cited-outside",
                WARNING,
                row,
                family.interface_clause,
                &["i", "ci", "rc"]
            )],
            "{row}: {what}: not one cited-outside Warning"
        );
    }
}

#[test]
fn sacm23_val_013_assurance_case_interfaces_and_bindings_have_no_citation_rule() {
    let row = VAL_013;
    // A non-citation ArgumentPackageInterface in an
    // AssuranceCasePackageInterface: no finding of this row.
    let what = "a non-citation ArgumentPackageInterface in an AssuranceCasePackageInterface";
    let input = case(&format!(
        "{}{}",
        ARGUMENT.package_in_case("p", ""),
        el(
            "assuranceCasePackage",
            CASE_INTERFACE,
            "ai",
            " implements=\"case\"",
            &el(
                "argumentPackage",
                "ArgumentPackageInterface",
                "pi",
                " implements=\"p\"",
                ""
            )
        )
    ));
    let doc = read_ok(row, what, &input);
    assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
    // The control: the same interface in an ArgumentPackageInterface is no
    // citation there.
    let what =
        "the control: a non-citation ArgumentPackageInterface in an ArgumentPackageInterface";
    let doc = read_ok(
        row,
        what,
        &ARGUMENT.i_in_p(&ARGUMENT.interface_el("pi", " implements=\"p\"", "")),
    );
    assert_eq!(
        of_row(&doc, row),
        [f(
            "SACM-PKG-001/not-a-citation",
            ERROR,
            row,
            "11.6",
            &["i", "pi"]
        )],
        "{row}: {what}: not one Error"
    );
    // Plain packages in an AssuranceCasePackageBinding.
    let what = "plain packages in an AssuranceCasePackageBinding";
    let input = case(&format!(
        "{}{}{}",
        el("assuranceCasePackage", "AssuranceCasePackage", "a1", "", ""),
        el("assuranceCasePackage", "AssuranceCasePackage", "a2", "", ""),
        el(
            "assuranceCasePackage",
            CASE_BINDING,
            "ab",
            " participantPackage=\"a1 a2\"",
            &format!(
                "{}{}{}",
                ARGUMENT.package_in_case("bp", &ARGUMENT.child_el("bc", "")),
                ARTIFACT.package_in_case("bt", ""),
                TERMINOLOGY.package_in_case("bu", "")
            )
        )
    ));
    let doc = read_ok(row, what, &input);
    assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
}

#[test]
fn sacm23_val_013_utility_elements_and_preserved_content_are_not_content() {
    let row = VAL_013;
    for family in FAMILIES {
        let what = format!("an {} holding only utility elements", family.interface);
        let utilities = "<description xmi:id=\"u_d\"><content><value content=\"D\"/></content></description>\
             <note xmi:id=\"u_n\"><content><value content=\"N\"/></content></note>\
             <taggedValue xmi:id=\"u_tv\"><key><value content=\"K\"/></key></taggedValue>";
        let doc = read_ok(row, &what, &family.i_in_p(utilities));
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
        // A vendor element is preserved content of the interface: not its
        // content.
        let what = format!("an {} holding preserved content", family.interface);
        let preserved = "<v:extra xmi:id=\"u_v\"><v:more/></v:extra>";
        let doc = tolerant(row, &what, &family.i_in_p(preserved));
        assert!(
            doc.get("i").is_some_and(SacmElement::has_preserved_content),
            "{row}: setup: {what}: the interface holds no preserved content"
        );
        assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
        // The control: a plain element beside the utility elements is one
        // Error.
        let what = format!(
            "the control: an {} holding utility elements and a plain element",
            family.interface
        );
        let doc = read_ok(
            row,
            &what,
            &family.i_in_p(&format!("{utilities}{}", family.child_el("c", ""))),
        );
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/not-a-citation",
                ERROR,
                row,
                family.interface_clause,
                &["i", "c"]
            )],
            "{row}: {what}: not one Error"
        );
    }
}

#[test]
fn sacm23_val_013_each_holder_is_checked_for_its_own_children() {
    let row = VAL_013;
    // i{ citation ArgumentPackageInterface j{ non-citation Claim c } }: one
    // Error, [j, c]. j cites by href, so no Warning applies to it.
    let what = "a citation interface j inside i holding a non-citation Claim";
    let j = el(
        "argumentElement",
        "ArgumentPackageInterface",
        "j",
        " isCitation=\"true\" implements=\"p\"",
        &format!(
            "<citedElement href=\"o.xmi#j\"/>{}",
            ARGUMENT.child_el("c", "")
        ),
    );
    let doc = read_ok(row, what, &ARGUMENT.i_in_p(&j));
    assert_eq!(
        of_row(&doc, row),
        [f(
            "SACM-PKG-001/not-a-citation",
            ERROR,
            row,
            "11.6",
            &["j", "c"]
        )],
        "{row}: {what}: not one Error for j's Claim"
    );
    // i{ non-citation ArgumentPackage q{ Claim } }: one, [i, q].
    let what = "a non-citation ArgumentPackage inside i holding a Claim";
    let doc = read_ok(
        row,
        what,
        &ARGUMENT.i_in_p(&ARGUMENT.nested_package("q", &ARGUMENT.child_el("qc", ""))),
    );
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/not-a-citation",
            ERROR,
            row,
            "11.6",
            &["i", "q"]
        )],
        "{row}: {what}: not one Error for q alone"
    );
}

#[test]
fn sacm23_val_013_edits_that_break_it_succeed_and_validate_reports() {
    let row = VAL_013;
    let mut doc = read_ok(
        row,
        "i citing c0",
        &ARGUMENT.i_in_p(&ARGUMENT.citation("ci", "c0")),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: setup: the document has findings"
    );
    applied(
        row,
        &mut doc,
        CreateClaim::new("i", named("C2")).with_id("c2"),
    );
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/not-a-citation",
            ERROR,
            row,
            "11.6",
            &["i", "c2"]
        )],
        "{row}: a Claim created under the interface"
    );
    applied(row, &mut doc, SetFlag::new("ci", Flag::IsCitation, false));
    assert_eq!(
        findings(&doc),
        [
            f(
                "SACM-CITE-001/not-a-citation",
                ERROR,
                "SACM23-VAL-007",
                "8.2",
                &["ci"]
            ),
            f(
                "SACM-PKG-001/not-a-citation",
                ERROR,
                row,
                "11.6",
                &["i", "c2"]
            ),
            f(
                "SACM-PKG-001/not-a-citation",
                ERROR,
                row,
                "11.6",
                &["i", "ci"]
            ),
        ],
        "{row}: isCitation set false on the citation"
    );
}

/// The nine findings of the fixture, as `Display`, in order.
const FIXTURE_FINDINGS: [&str; 9] = [
    "error SACM-MULT-001 [SACM23-VAL-014, clause 11.5]: ArgumentPackageBinding \"b\" has 1 distinct participantPackage (participantPackage:ArgumentPackage[2..*])",
    "warning SACM-MULT-001 [SACM23-VAL-022, clause 11.5]: ArgumentPackageBinding \"b\" holds \"i\" 2 times in its participantPackage, whose ids are unique in SACM 2.3 (the repeat is kept)",
    "error SACM-MULT-001 [SACM23-VAL-014, clause 11.6]: ArgumentPackageInterface \"k\" has no implements (implements:ArgumentPackage[1])",
    "error SACM-PKG-001 [SACM23-VAL-015, clause 9.3]: AssuranceCasePackageInterface \"ai\" contains ArgumentPackage \"ap\" in its argumentPackage, but §9.3 allows only ArgumentPackageInterfaces there",
    "error SACM-PKG-001 [SACM23-VAL-013, clause 11.5]: ArgumentPackageBinding \"b\" contains Claim \"b_plain\", which is no citation (isCitation true and a citedElement), but §11.5 allows only citations and AssertedRelationships in it",
    "warning SACM-PKG-001 [SACM23-VAL-014, clause 11.5]: ArgumentPackageBinding \"b2\" has the participantPackage \"b\", of the kind ArgumentPackageBinding: a binding binds packages, not bindings",
    "warning SACM-PKG-001 [SACM23-VAL-013, clause 11.6]: ArgumentPackageInterface \"i\" contains Claim \"i_far\", which cites \"d\", but §11.6 has an interface cite only elements within the package it implements, \"p\"",
    "error SACM-PKG-001 [SACM23-VAL-013, clause 11.6]: ArgumentPackageInterface \"i\" contains Claim \"i_plain\", which is no citation (isCitation true and a citedElement), but §11.6 allows only citations in it",
    "warning SACM-PKG-002 [SACM23-VAL-023, clause 11.6]: ArgumentPackageInterface \"j\" implements the ArgumentPackage \"q\" but does not reside inside it, as §11.6 says an interface does",
];

/// The affected ids of the nine findings, in the same order.
const FIXTURE_AFFECTED: [&[&str]; 9] = [
    &["b"],
    &["b", "i"],
    &["k"],
    &["ai", "ap"],
    &["b", "b_plain"],
    &["b2", "b"],
    &["i", "i_far", "d"],
    &["i", "i_plain"],
    &["j", "q"],
];

#[test]
fn sacm23_val_013_the_modularity_fixture_gives_exactly_its_nine_findings_and_writes() {
    let row = VAL_013;
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{row}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok(row, "the findings fixture", &text);
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(ToString::to_string).collect::<Vec<_>>(),
        FIXTURE_FINDINGS,
        "{row}: the fixture's findings, as Display, in order"
    );
    assert_eq!(
        found
            .iter()
            .map(|one| one
                .affected()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        FIXTURE_AFFECTED
            .iter()
            .map(|ids| ids.to_vec())
            .collect::<Vec<_>>(),
        "{row}: the fixture's affected ids, in order"
    );
    assert!(
        found.iter().all(|one| one.location().is_none()),
        "{row}: a finding of the fixture has a location"
    );
    assert_both_saves_write(row, "the findings fixture", &doc);
}

#[test]
fn sacm23_val_013_the_catalog_pkg_001_row_names_the_row_and_its_templates() {
    let row = VAL_013;
    let line = catalog_row(row, PKG_001);
    for part in [
        "| SACM-PKG-001 | Error / Warning |",
        "`Code::PACKAGE_CONTENT_INVALID`",
        "SACM23-VAL-012",
        "SACM23-VAL-013",
        "SACM23-VAL-014",
        "SACM23-VAL-015",
        "10.5",
        "10.6",
        "11.5",
        "11.6",
        "12.4",
        "12.5",
        "9.3",
        "D-IFACE-CITE",
    ] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-PKG-001 row does not name {part:?}: {line}"
        );
    }
    assert!(
        !line.contains("reserved"),
        "{row}: the catalog's SACM-PKG-001 row still reserves a part: {line}"
    );
    let catalog = catalog(row);
    for template in [
        "SACM-PKG-001/binding-participant",
        "SACM-PKG-001/cited-kind",
        "SACM-PKG-001/cited-outside",
        "SACM-PKG-001/mixed-content",
        "SACM-PKG-001/not-a-citation",
        "SACM-PKG-001/not-a-citation-or-relationship",
        "SACM-PKG-001/not-an-interface",
    ] {
        let cell = format!("| `{template}` | `Diagnostic` |");
        assert_eq!(
            catalog
                .lines()
                .filter(|line| line.starts_with(&cell))
                .count(),
            1,
            "{row}: the template table has not one row for {template}"
        );
    }
}

// =================================================================================
// SACM23-VAL-014: implements and distinct participants
// =================================================================================

/// The four interface classes, the package class `implements` names, and the
/// clause.
const INTERFACES: [(&str, &str, &str); 4] = [
    (
        "AssuranceCasePackageInterface",
        "AssuranceCasePackage",
        "9.3",
    ),
    ("TerminologyPackageInterface", "TerminologyPackage", "10.5"),
    ("ArgumentPackageInterface", "ArgumentPackage", "11.6"),
    ("ArtifactPackageInterface", "ArtifactPackage", "12.5"),
];

/// The four binding classes, the package class `participantPackage` names,
/// and the clause.
const BINDINGS: [(&str, &str, &str); 4] = [
    ("AssuranceCasePackageBinding", "AssuranceCasePackage", "9.4"),
    ("TerminologyPackageBinding", "TerminologyPackage", "10.6"),
    ("ArgumentPackageBinding", "ArgumentPackage", "11.5"),
    ("ArtifactPackageBinding", "ArtifactPackage", "12.4"),
];

#[test]
fn sacm23_val_014_an_interface_with_no_implements_is_one_error() {
    let row = VAL_014;
    for (interface, package, clause) in INTERFACES {
        let what = format!("an {interface} root with no implements");
        let doc = read_ok(row, &what, &root(interface, "i", "", ""));
        assert_eq!(
            findings(&doc),
            [f("SACM-MULT-001/missing", ERROR, row, clause, &["i"])],
            "{row}: {what}: not one SACM-MULT-001 Error with clause {clause}"
        );
        let one = the_one(row, &what, &doc, "SACM-MULT-001/missing");
        assert_eq!(
            (one.message().to_owned(), arguments(&one)),
            (
                format!("{interface} \"i\" has no implements (implements:{package}[1])"),
                pairs(&[
                    ("class", interface),
                    ("id", "i"),
                    ("property", "implements"),
                    ("type", package),
                    ("multiplicity", "1"),
                ])
            ),
            "{row}: {what}: the message and arguments"
        );
    }
}

#[test]
fn sacm23_val_014_a_binding_with_no_participant_is_one_missing_error() {
    let row = VAL_014;
    for (binding, package, clause) in BINDINGS {
        let what = format!("an {binding} root with no participant");
        let doc = read_ok(row, &what, &root(binding, "b", "", ""));
        assert_eq!(
            findings(&doc),
            [f("SACM-MULT-001/missing", ERROR, row, clause, &["b"])],
            "{row}: {what}: not one SACM-MULT-001 Error with clause {clause}"
        );
        let one = the_one(row, &what, &doc, "SACM-MULT-001/missing");
        assert_eq!(
            (one.message().to_owned(), arguments(&one)),
            (
                format!(
                    "{binding} \"b\" has no participantPackage \
                     (participantPackage:{package}[2..*])"
                ),
                pairs(&[
                    ("class", binding),
                    ("id", "b"),
                    ("property", "participantPackage"),
                    ("type", package),
                    ("multiplicity", "2..*"),
                ])
            ),
            "{row}: {what}: the message and arguments"
        );
    }
}

/// The AssuranceCasePackage `case` holding the `ArgumentPackage`s `a` and `c`
/// and the `ArgumentPackageBinding` `b` with `attributes` and `body`.
fn b_of(attributes: &str, body: &str) -> String {
    case(&format!(
        "{}{}{}",
        ARGUMENT.package_in_case("a", ""),
        ARGUMENT.package_in_case("c", ""),
        ARGUMENT.binding_in_case("b", attributes, body)
    ))
}

#[test]
fn sacm23_val_014_one_distinct_participant_is_one_too_few_error() {
    let row = VAL_014;
    let too_few = f("SACM-MULT-001/too-few", ERROR, row, "11.5", &["b"]);
    let what = "participantPackage=\"a\"";
    let doc = read_ok(row, what, &b_of(" participantPackage=\"a\"", ""));
    assert_eq!(
        findings(&doc),
        std::slice::from_ref(&too_few),
        "{row}: {what}"
    );
    let one = the_one(row, what, &doc, "SACM-MULT-001/too-few");
    assert_eq!(
        (one.message().to_owned(), arguments(&one)),
        (
            "ArgumentPackageBinding \"b\" has 1 distinct participantPackage \
             (participantPackage:ArgumentPackage[2..*])"
                .to_owned(),
            pairs(&[
                ("class", "ArgumentPackageBinding"),
                ("id", "b"),
                ("count", "1"),
                ("property", "participantPackage"),
                ("type", "ArgumentPackage"),
                ("multiplicity", "2..*"),
            ])
        ),
        "{row}: {what}: the message and arguments"
    );
    assert_eq!(
        one.arguments()
            .iter()
            .find(|argument| argument.name() == "count")
            .and_then(|argument| argument.value().as_count()),
        Some(1),
        "{row}: {what}: count is not the count 1"
    );
    let what = "participantPackage=\"a a\"";
    let doc = read_ok(row, what, &b_of(" participantPackage=\"a a\"", ""));
    assert_eq!(
        findings(&doc),
        [
            too_few,
            f(
                "SACM-MULT-001/repeated-reference",
                WARNING,
                "SACM23-VAL-022",
                "11.5",
                &["b", "a"]
            ),
        ],
        "{row}: {what}: not too-few and one SACM23-VAL-022 Warning"
    );
    let what = "participantPackage=\"a gone\"";
    let doc = read_ok(row, what, &b_of(" participantPackage=\"a gone\"", ""));
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-REF-001/dangling",
            ERROR,
            "SACM23-VAL-003",
            "11.5",
            &["b", "gone"]
        )],
        "{row}: {what}: a dangling participant does not count"
    );
    let what = "participantPackage=\"a c\"";
    let doc = read_ok(row, what, &b_of(" participantPackage=\"a c\"", ""));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
}

#[test]
fn sacm23_val_014_hrefs_count_as_distinct_participants_and_as_implements() {
    let row = VAL_014;
    let what = "ids [a] and hrefs [o.xmi#p]";
    let doc = read_ok(
        row,
        what,
        &b_of(
            "",
            "<participantPackage xmi:idref=\"a\"/><participantPackage href=\"o.xmi#p\"/>",
        ),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    let what = "hrefs [o.xmi#p, o.xmi#p]";
    let doc = read_ok(
        row,
        what,
        &b_of(
            "",
            "<participantPackage href=\"o.xmi#p\"/><participantPackage href=\"o.xmi#p\"/>",
        ),
    );
    assert_eq!(
        findings(&doc),
        [
            f("SACM-MULT-001/too-few", ERROR, row, "11.5", &["b"]),
            f(
                "SACM-MULT-001/repeated-reference",
                WARNING,
                "SACM23-VAL-022",
                "11.5",
                &["b"]
            ),
        ],
        "{row}: {what}: not too-few and one SACM23-VAL-022 Warning"
    );
    for (interface, package, _) in INTERFACES {
        let what = format!("an {interface} root whose implements is an href");
        let doc = read_ok(
            row,
            &what,
            &root(interface, "i", "", "<implements href=\"o.xmi#p\"/>"),
        );
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding (implements:{package}[1])"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the four families and the special cases of one acceptance item read best as one list"
)]
fn sacm23_val_014_a_binding_named_as_a_participant_is_one_warning() {
    let row = VAL_014;
    // b2 = [a, b], b an ArgumentPackageBinding of [a, c].
    let what = "an ArgumentPackageBinding naming an ArgumentPackageBinding";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}{}{}",
            ARGUMENT.package_in_case("a", ""),
            ARGUMENT.package_in_case("c", ""),
            ARGUMENT.binding_in_case("b", " participantPackage=\"a c\"", ""),
            ARGUMENT.binding_in_case("b2", " participantPackage=\"a b\"", "")
        )),
    );
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-PKG-001/binding-participant",
            WARNING,
            row,
            "11.5",
            &["b2", "b"]
        )],
        "{row}: {what}: not one Warning with clause 11.5"
    );
    let one = the_one(row, what, &doc, "SACM-PKG-001/binding-participant");
    assert_eq!(
        (one.message().to_owned(), arguments(&one)),
        (
            "ArgumentPackageBinding \"b2\" has the participantPackage \"b\", of the kind \
             ArgumentPackageBinding: a binding binds packages, not bindings"
                .to_owned(),
            pairs(&[
                ("class", "ArgumentPackageBinding"),
                ("id", "b2"),
                ("target", "b"),
                ("target_class", "ArgumentPackageBinding"),
            ])
        ),
        "{row}: {what}: the message and arguments"
    );
    // Every family: its binding of [x, y], and b2 of [x, b].
    for (binding, package, clause) in BINDINGS {
        let (case_role, package_class) = match package {
            "AssuranceCasePackage" => ("assuranceCasePackage", package),
            "TerminologyPackage" => ("terminologyPackage", package),
            "ArgumentPackage" => ("argumentPackage", package),
            _ => ("artifactPackage", package),
        };
        let what = format!("an {binding} naming an {binding}");
        let doc = read_ok(
            row,
            &what,
            &case(&format!(
                "{}{}{}{}",
                el(case_role, package_class, "x", "", ""),
                el(case_role, package_class, "y", "", ""),
                el(case_role, binding, "b", " participantPackage=\"x y\"", ""),
                el(case_role, binding, "b2", " participantPackage=\"x b\"", "")
            )),
        );
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/binding-participant",
                WARNING,
                row,
                clause,
                &["b2", "b"]
            )],
            "{row}: {what}: not one Warning with clause {clause}"
        );
        // A binding naming itself.
        let what = format!("an {binding} naming itself");
        let doc = read_ok(
            row,
            &what,
            &case(&format!(
                "{}{}",
                el(case_role, package_class, "x", "", ""),
                el(case_role, binding, "b", " participantPackage=\"x b\"", "")
            )),
        );
        assert_eq!(
            findings(&doc),
            [f(
                "SACM-PKG-001/binding-participant",
                WARNING,
                row,
                clause,
                &["b", "b"]
            )],
            "{row}: {what}: not one Warning affecting [b, b]"
        );
    }
    // [b, b]: one Warning, beside too-few and the repeat.
    let what = "b2 = [b, b]";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}{}{}",
            ARGUMENT.package_in_case("a", ""),
            ARGUMENT.package_in_case("a2", ""),
            ARGUMENT.binding_in_case("b", " participantPackage=\"a a2\"", ""),
            ARGUMENT.binding_in_case("b2", " participantPackage=\"b b\"", "")
        )),
    );
    assert_eq!(
        of_code(&doc, PKG_001),
        [f(
            "SACM-PKG-001/binding-participant",
            WARNING,
            row,
            "11.5",
            &["b2", "b"]
        )],
        "{row}: {what}: not one binding-participant Warning"
    );
    assert_eq!(
        of_code(&doc, MULT_001),
        [
            f("SACM-MULT-001/too-few", ERROR, row, "11.5", &["b2"]),
            f(
                "SACM-MULT-001/repeated-reference",
                WARNING,
                "SACM23-VAL-022",
                "11.5",
                &["b2", "b"]
            ),
        ],
        "{row}: {what}: not too-few and one repeat"
    );
    // An ArtifactPackageBinding participant of an ArgumentPackageBinding:
    // only SACM-REF-002.
    let what = "an ArgumentPackageBinding naming an ArtifactPackageBinding";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}{}{}{}",
            ARGUMENT.package_in_case("a", ""),
            ARTIFACT.package_in_case("t", ""),
            ARTIFACT.package_in_case("u", ""),
            ARTIFACT.binding_in_case("tb", " participantPackage=\"t u\"", ""),
            ARGUMENT.binding_in_case("b2", " participantPackage=\"a tb\"", "")
        )),
    );
    assert_eq!(
        findings(&doc),
        [f(
            "SACM-REF-002/wrong-type",
            ERROR,
            "SACM23-VAL-004",
            "11.5",
            &["b2", "tb"]
        )],
        "{row}: {what}: not only SACM-REF-002"
    );
}

#[test]
fn sacm23_val_014_a_file_below_the_bounds_reads_and_an_edit_repairs_it() {
    let row = VAL_014;
    let mut doc = read_ok(
        row,
        "a binding of one participant",
        &b_of(" participantPackage=\"a\"", ""),
    );
    assert_eq!(
        findings(&doc),
        [f("SACM-MULT-001/too-few", ERROR, row, "11.5", &["b"])],
        "{row}: the binding read with one participant"
    );
    let mutation = applied(
        row,
        &mut doc,
        SetReferences::new("b", ReferenceEnd::ParticipantPackage, ["a", "c"]),
    );
    assert_eq!(
        mutation
            .changed_ids()
            .iter()
            .map(ElementId::as_str)
            .collect::<Vec<_>>(),
        ["b"],
        "{row}: the edit's changed ids"
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: the edit does not clear the Error"
    );
}

#[test]
fn sacm23_val_014_the_findings_do_not_refuse_a_strict_write() {
    let row = VAL_014;
    let input = case(&format!(
        "{}{}{}{}{}",
        ARGUMENT.package_in_case("a", ""),
        ARGUMENT.package_in_case("c", &ARGUMENT.interface_el("k", "", "")),
        ARGUMENT.binding_in_case("b", " participantPackage=\"a a\"", ""),
        ARGUMENT.binding_in_case("b2", " participantPackage=\"a b\"", ""),
        ARGUMENT.binding_in_case("b3", "", "")
    ));
    let doc = read_ok(row, "bindings and an interface below their bounds", &input);
    assert_eq!(
        of_row(&doc, row).len(),
        4,
        "{row}: setup: not four findings of this row: {:?}",
        of_row(&doc, row)
    );
    assert_both_saves_write(row, "bindings and an interface below their bounds", &doc);
}

#[test]
fn sacm23_val_014_the_catalog_names_the_row() {
    let row = VAL_014;
    let line = catalog_row(row, MULT_001);
    assert!(
        !line.contains("is reserved for `validate` (SACM23-VAL-014)"),
        "{row}: the catalog's SACM-MULT-001 row still reserves this row's part: {line}"
    );
    for part in ["SACM23-VAL-014", "`implements`", "`too-few`", "`missing`"] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-MULT-001 row does not name {part:?}: {line}"
        );
    }
    let line = catalog_row(row, PKG_001);
    for part in ["SACM23-VAL-014", "D-BIND-PART", "participant"] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-PKG-001 row does not name {part:?}: {line}"
        );
    }
    let catalog = catalog(row);
    assert_eq!(
        catalog
            .lines()
            .filter(|line| line.starts_with(
                "| `SACM-MULT-001/too-few` | `Diagnostic` | `class`, `id`, `count` (count), \
                 `property`, `type`, `multiplicity` |"
            ))
            .count(),
        1,
        "{row}: the template table has not one row for SACM-MULT-001/too-few"
    );
}

// =================================================================================
// SACM23-VAL-015: AssuranceCasePackageInterface content
// =================================================================================

/// The AssuranceCasePackage `case` holding the `AssuranceCasePackageInterface`
/// `ai`, which implements `case` and holds `body`.
fn ai_holding(body: &str) -> String {
    case(&el(
        "assuranceCasePackage",
        CASE_INTERFACE,
        "ai",
        " implements=\"case\"",
        body,
    ))
}

#[test]
fn sacm23_val_015_each_sub_package_that_is_not_an_interface_is_one_error() {
    let row = VAL_015;
    for (role, class, expected) in [
        (
            "argumentPackage",
            "ArgumentPackage",
            "ArgumentPackageInterface",
        ),
        (
            "argumentPackage",
            "ArgumentPackageBinding",
            "ArgumentPackageInterface",
        ),
        (
            "artifactPackage",
            "ArtifactPackage",
            "ArtifactPackageInterface",
        ),
        (
            "artifactPackage",
            "ArtifactPackageBinding",
            "ArtifactPackageInterface",
        ),
        (
            "terminologyPackage",
            "TerminologyPackage",
            "TerminologyPackageInterface",
        ),
        (
            "terminologyPackage",
            "TerminologyPackageBinding",
            "TerminologyPackageInterface",
        ),
        (
            "assuranceCasePackage",
            "AssuranceCasePackage",
            "AssuranceCasePackageInterface",
        ),
        (
            "assuranceCasePackage",
            "AssuranceCasePackageBinding",
            "AssuranceCasePackageInterface",
        ),
    ] {
        let what = format!("a {class} in an AssuranceCasePackageInterface's {role}");
        let doc = read_ok(row, &what, &ai_holding(&el(role, class, "p", "", "")));
        assert_eq!(
            of_row(&doc, row),
            [f(
                "SACM-PKG-001/not-an-interface",
                ERROR,
                row,
                "9.3",
                &["ai", "p"]
            )],
            "{row}: {what}: not one Error with clause 9.3"
        );
        let one = the_one(row, &what, &doc, "SACM-PKG-001/not-an-interface");
        assert_eq!(
            (one.message().to_owned(), arguments(&one)),
            (
                format!(
                    "AssuranceCasePackageInterface \"ai\" contains {class} \"p\" in its {role}, \
                     but §9.3 allows only {expected}s there"
                ),
                pairs(&[
                    ("class", "AssuranceCasePackageInterface"),
                    ("id", "ai"),
                    ("child_class", class),
                    ("child_id", "p"),
                    ("role", role),
                    ("expected", expected),
                ])
            ),
            "{row}: {what}: the message and arguments"
        );
        if class == "ArgumentPackage" {
            assert_eq!(
                findings(&doc),
                [f(
                    "SACM-PKG-001/not-an-interface",
                    ERROR,
                    row,
                    "9.3",
                    &["ai", "p"]
                )],
                "{row}: {what}: validate reports another finding"
            );
        }
    }
}

#[test]
fn sacm23_val_015_interfaces_utility_elements_and_an_empty_interface_give_no_finding() {
    let row = VAL_015;
    let what = "an AssuranceCasePackageInterface of the four interfaces and utility elements";
    let body = format!(
        "<description xmi:id=\"ai_d\"><content><value content=\"D\"/></content></description>\
         <note xmi:id=\"ai_n\"><content><value content=\"N\"/></content></note>{}{}{}{}",
        el(
            "argumentPackage",
            "ArgumentPackageInterface",
            "pi",
            " isCitation=\"true\" citedElement=\"case\"",
            "<implements href=\"o.xmi#p\"/>"
        ),
        el(
            "artifactPackage",
            "ArtifactPackageInterface",
            "ti",
            " isCitation=\"true\" citedElement=\"case\"",
            "<implements href=\"o.xmi#t\"/>"
        ),
        el(
            "terminologyPackage",
            "TerminologyPackageInterface",
            "ui",
            " isCitation=\"true\" citedElement=\"case\"",
            "<implements href=\"o.xmi#u\"/>"
        ),
        el(
            "assuranceCasePackage",
            CASE_INTERFACE,
            "ci",
            " isCitation=\"true\" citedElement=\"case\" implements=\"case\"",
            ""
        ),
    );
    let doc = read_ok(row, what, &ai_holding(&body));
    assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    let what = "an empty AssuranceCasePackageInterface";
    let doc = read_ok(row, what, &ai_holding(""));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
}

#[test]
fn sacm23_val_015_a_citation_is_checked_like_any_child() {
    let row = VAL_015;
    let what = "a citation ArgumentPackage in an AssuranceCasePackageInterface";
    let doc = read_ok(
        row,
        what,
        &ai_holding(&el(
            "argumentPackage",
            "ArgumentPackage",
            "p",
            " isCitation=\"true\" citedElement=\"case\"",
            "",
        )),
    );
    assert_eq!(
        of_row(&doc, row),
        [f(
            "SACM-PKG-001/not-an-interface",
            ERROR,
            row,
            "9.3",
            &["ai", "p"]
        )],
        "{row}: {what}: not one Error"
    );
    let what = "a citation ArgumentPackageInterface in an AssuranceCasePackageInterface";
    let doc = read_ok(
        row,
        what,
        &ai_holding(&el(
            "argumentPackage",
            "ArgumentPackageInterface",
            "pi",
            " isCitation=\"true\" citedElement=\"case\"",
            "<implements href=\"o.xmi#p\"/>",
        )),
    );
    assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
}

#[test]
fn sacm23_val_015_an_assurance_case_package_or_binding_owner_is_out_of_scope() {
    let row = VAL_015;
    let sub_packages = format!(
        "{}{}{}{}",
        ARGUMENT.package_in_case("p", ""),
        ARTIFACT.package_in_case("t", ""),
        TERMINOLOGY.package_in_case("u", ""),
        el("assuranceCasePackage", "AssuranceCasePackage", "a", "", "")
    );
    let what = "plain sub-packages in an AssuranceCasePackage";
    let doc = read_ok(row, what, &case(&sub_packages));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    let what = "plain sub-packages in an AssuranceCasePackageBinding";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}{}",
            el("assuranceCasePackage", "AssuranceCasePackage", "x", "", ""),
            el("assuranceCasePackage", "AssuranceCasePackage", "y", "", ""),
            el(
                "assuranceCasePackage",
                CASE_BINDING,
                "ab",
                " participantPackage=\"x y\"",
                &sub_packages
            )
        )),
    );
    assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
    // The control: the same sub-packages in an AssuranceCasePackageInterface
    // are four Errors.
    let what = "the control: plain sub-packages in an AssuranceCasePackageInterface";
    let doc = read_ok(row, what, &ai_holding(&sub_packages));
    assert_eq!(
        of_row(&doc, row).len(),
        4,
        "{row}: {what}: not four Errors: {:?}",
        of_row(&doc, row)
    );
}

#[test]
fn sacm23_val_015_the_findings_do_not_refuse_a_strict_write() {
    let row = VAL_015;
    let what = "an AssuranceCasePackageInterface holding plain packages";
    let doc = read_ok(
        row,
        what,
        &ai_holding(&format!(
            "{}{}",
            ARGUMENT.package_in_case("p", &ARGUMENT.child_el("c", "")),
            TERMINOLOGY.package_in_case("u", "")
        )),
    );
    assert_eq!(
        of_row(&doc, row).len(),
        2,
        "{row}: setup: not two findings of this row: {:?}",
        findings(&doc)
    );
    assert_both_saves_write(row, what, &doc);
}

// =================================================================================
// SACM23-VAL-023: an interface resides inside the package it implements
// =================================================================================

/// The placement Warning for `interface`, `id`, implementing `package`.
fn placement(clause: &str, interface: &str, package: &str) -> Found {
    f(
        "SACM-PKG-002/outside-implemented-package",
        WARNING,
        VAL_023,
        clause,
        &[interface, package],
    )
}

#[test]
fn sacm23_val_023_an_interface_outside_its_package_is_one_warning() {
    let row = VAL_023;
    for family in FAMILIES {
        let j = |role: &str| el(role, family.interface, "j", " implements=\"q\"", "");
        for (what, input) in [
            (
                "a root",
                wrapped(&[
                    root(family.package, "q", "", ""),
                    root(family.interface, "j", " implements=\"q\"", ""),
                ]),
            ),
            (
                "a sibling of its package",
                case(&format!(
                    "{}{}",
                    family.package_in_case("q", ""),
                    j(family.case_role)
                )),
            ),
            (
                "inside another package",
                case(&format!(
                    "{}{}",
                    family.package_in_case("q", ""),
                    family.package_in_case("r", &j(family.role))
                )),
            ),
        ] {
            let what = format!("an {} implementing q as {what}", family.interface);
            let doc = read_ok(row, &what, &input);
            assert_eq!(
                findings(&doc),
                [placement(family.interface_clause, "j", "q")],
                "{row}: {what}: not one SACM-PKG-002 Warning with clause {}",
                family.interface_clause
            );
            let one = the_one(row, &what, &doc, "SACM-PKG-002/outside-implemented-package");
            assert_eq!(
                (one.message().to_owned(), arguments(&one)),
                (
                    format!(
                        "{} \"j\" implements the {} \"q\" but does not reside inside it, as §{} \
                         says an interface does",
                        family.interface, family.package, family.interface_clause
                    ),
                    pairs(&[
                        ("class", family.interface),
                        ("id", "j"),
                        ("package_class", family.package),
                        ("package", "q"),
                        ("clause", family.interface_clause),
                    ])
                ),
                "{row}: {what}: the message and arguments"
            );
        }
    }
}

#[test]
fn sacm23_val_023_an_interface_inside_its_package_at_any_depth_gives_none() {
    let row = VAL_023;
    for family in FAMILIES {
        let i = family.interface_el("i", " implements=\"p\"", "");
        for (what, body) in [
            ("p{i}", i.clone()),
            ("p{q{i}}", family.nested_package("q", &i)),
        ] {
            let what = format!("{what} of the {} family", family.package);
            let doc = read_ok(row, &what, &root(family.package, "p", "", &body));
            assert_eq!(
                findings(&doc),
                [],
                "{row}: {what}: validate reports a finding"
            );
        }
        // The control: i beside p, at the roots, is one Warning.
        let what = format!(
            "the control: an {} beside the package it implements",
            family.interface
        );
        let doc = read_ok(
            row,
            &what,
            &wrapped(&[
                root(family.package, "p", "", ""),
                root(family.interface, "i", " implements=\"p\"", ""),
            ]),
        );
        assert_eq!(
            findings(&doc),
            [placement(family.interface_clause, "i", "p")],
            "{row}: {what}: not one Warning"
        );
    }
}

#[test]
fn sacm23_val_023_an_interface_implementing_itself_or_a_package_it_holds_is_reported() {
    let row = VAL_023;
    for family in FAMILIES {
        let what = format!("an {} implementing itself", family.interface);
        let doc = read_ok(
            row,
            &what,
            &root(
                family.package,
                "p",
                "",
                &family.interface_el("i", " implements=\"i\"", ""),
            ),
        );
        assert_eq!(
            findings(&doc),
            [placement(family.interface_clause, "i", "i")],
            "{row}: {what}: not one Warning affecting [i, i]"
        );
        let what = format!("an {} implementing a package it holds", family.interface);
        let doc = read_ok(
            row,
            &what,
            &root(
                family.package,
                "p",
                "",
                &family.interface_el("i", " implements=\"q\"", &family.nested_package("q", "")),
            ),
        );
        assert_eq!(
            of_row(&doc, row),
            [placement(family.interface_clause, "i", "q")],
            "{row}: {what}: not one Warning affecting [i, q]"
        );
    }
}

#[test]
fn sacm23_val_023_citations_and_unresolved_implements_are_not_checked() {
    let row = VAL_023;
    for family in FAMILIES {
        // The interface j sits in r, a sibling of q, with `attributes` and
        // `body`; `extra` adds content to the case.
        let input = |attributes: &str, body: &str, extra: &str| {
            case(&format!(
                "{}{}{extra}",
                family.package_in_case("q", &family.child_el("qc", "")),
                family.package_in_case("r", &family.interface_el("j", attributes, body))
            ))
        };
        let what = format!("a citation {} outside q", family.interface);
        let doc = read_ok(
            row,
            &what,
            &input(
                " isCitation=\"true\" implements=\"q\"",
                "<citedElement href=\"o.xmi#j\"/>",
                "",
            ),
        );
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
        // A Term in an ArgumentPackage is preserved content (SACM-REF-003).
        let preserved =
            ARGUMENT.package_in_case("w", &el("argumentElement", "Term", "z", " value=\"z\"", ""));
        for (what, attributes, body, extra, code) in [
            (
                "no implements",
                "",
                "",
                String::new(),
                Some("SACM-MULT-001"),
            ),
            (
                "a dangling implements",
                " implements=\"gone\"",
                "",
                String::new(),
                Some("SACM-REF-001"),
            ),
            (
                "an href implements",
                "",
                "<implements href=\"o.xmi#q\"/>",
                String::new(),
                None,
            ),
            (
                "an implements naming preserved content",
                " implements=\"z\"",
                "",
                preserved.clone(),
                Some("SACM-REF-003"),
            ),
            (
                "an implements naming an element of no package kind",
                " implements=\"qc\"",
                "",
                String::new(),
                Some("SACM-REF-002"),
            ),
        ] {
            let what = format!("an {} with {what}", family.interface);
            let doc = tolerant(row, &what, &input(attributes, body, &extra));
            assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
            let codes: Vec<String> = findings(&doc)
                .into_iter()
                .filter(|one| one.affected.first().map(String::as_str) == Some("j"))
                .map(|one| one.code)
                .collect();
            assert_eq!(
                codes,
                code.map(str::to_owned).into_iter().collect::<Vec<_>>(),
                "{row}: {what}: not only its own finding"
            );
        }
    }
}

#[test]
fn sacm23_val_023_assurance_case_interfaces_and_bindings_are_not_checked() {
    let row = VAL_023;
    // An AssuranceCasePackageInterface outside the package it implements.
    let what = "an AssuranceCasePackageInterface implementing a sibling";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}",
            el("assuranceCasePackage", "AssuranceCasePackage", "a", "", ""),
            el(
                "assuranceCasePackage",
                CASE_INTERFACE,
                "ai",
                " implements=\"a\"",
                ""
            )
        )),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    // The control: an ArgumentPackageInterface there is one Warning.
    let what = "the control: an ArgumentPackageInterface implementing a sibling";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}",
            ARGUMENT.package_in_case("a", ""),
            el(
                "argumentPackage",
                "ArgumentPackageInterface",
                "ai",
                " implements=\"a\"",
                ""
            )
        )),
    );
    assert_eq!(
        findings(&doc),
        [placement("11.6", "ai", "a")],
        "{row}: {what}: not one Warning"
    );
    // Bindings anywhere: no placement rule.
    for family in FAMILIES {
        let what = format!("an {} inside a package it does not bind", family.binding);
        let doc = read_ok(
            row,
            &what,
            &case(&format!(
                "{}{}{}",
                family.package_in_case("x", ""),
                family.package_in_case("y", ""),
                family.package_in_case(
                    "r",
                    &el(
                        family.role,
                        family.binding,
                        "b",
                        " participantPackage=\"x y\"",
                        ""
                    )
                )
            )),
        );
        assert_eq!(of_row(&doc, row), [], "{row}: {what}: reported by this row");
    }
}

#[test]
fn sacm23_val_023_moving_an_interface_causes_and_clears_the_warning() {
    let row = VAL_023;
    let mut doc = read_ok(
        row,
        "i in p",
        &case(&ARGUMENT.package_in_case("p", &ARGUMENT.interface_el("i", " implements=\"p\"", ""))),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: setup: the document has findings"
    );
    applied(row, &mut doc, MoveElement::root("i"));
    assert_eq!(
        findings(&doc),
        [placement("11.6", "i", "p")],
        "{row}: i moved to the roots"
    );
    applied(row, &mut doc, MoveElement::new("i", "p"));
    assert_eq!(findings(&doc), [], "{row}: i moved back into p");
}

#[test]
fn sacm23_val_023_the_warning_does_not_refuse_a_strict_write() {
    let row = VAL_023;
    let what = "an interface outside the package it implements";
    let doc = read_ok(
        row,
        what,
        &case(&format!(
            "{}{}",
            ARGUMENT.package_in_case("q", ""),
            el(
                "argumentPackage",
                "ArgumentPackageInterface",
                "j",
                " implements=\"q\"",
                ""
            )
        )),
    );
    assert_eq!(
        findings(&doc),
        [placement("11.6", "j", "q")],
        "{row}: setup: {what}"
    );
    assert_both_saves_write(row, what, &doc);
}

#[test]
fn sacm23_val_023_interface_placement_is_sacm_pkg_002_and_its_catalog_row_names_it() {
    let row = VAL_023;
    {
        use sacm::diagnostic::Code;

        assert_eq!(
            Code::INTERFACE_PLACEMENT.as_str(),
            PKG_002,
            "{row}: Code::INTERFACE_PLACEMENT is not SACM-PKG-002"
        );
        assert_eq!(
            Code::INTERFACE_PLACEMENT.to_string(),
            PKG_002,
            "{row}: the Display of Code::INTERFACE_PLACEMENT"
        );
    }
    let line = catalog_row(row, PKG_002);
    assert!(
        line.starts_with("| SACM-PKG-002 | Warning |"),
        "{row}: the catalog's SACM-PKG-002 row is not a Warning: {line}"
    );
    for part in [
        "`Code::INTERFACE_PLACEMENT`",
        "SACM23-VAL-023",
        "§11.6",
        "§12.5",
        "§10.5",
        "`implements`",
    ] {
        assert!(
            line.contains(part),
            "{row}: the catalog's SACM-PKG-002 row does not name {part:?}: {line}"
        );
    }
    let catalog = catalog(row);
    assert_eq!(
        catalog
            .lines()
            .filter(|line| line
                .starts_with("| `SACM-PKG-002/outside-implemented-package` | `Diagnostic` |"))
            .count(),
        1,
        "{row}: the template table has not one row for SACM-PKG-002"
    );
}
