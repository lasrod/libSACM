//! Tests for `AssuranceCasePackageInterface` and
//! `AssuranceCasePackageBinding`, with the ends `interface`, `implements` and
//! `participantPackage` (SACM23-AC-002, SACM23-AC-003; formal/23-05-08 §9.2
//! "`interface: AssuranceCasePackageInterface [0..*]` - a number of optional
//! assurance case package interfaces that the current package may
//! implement", §9.3 "`implements:AssuranceCasePackage[1]` - the
//! `AssuranceCasePackage` that the `AssuranceCasePackageInterface` declares.",
//! §9.4 "`participantPackage:AssuranceCasePackage[2..*]`"; ptc/22-03-13
//! `AssuranceCase`; the M5 decisions Q1, Q2, Q3, Q4, Q7, Q8 and Q9; ADR 0039).
//!
//! Both kinds are `AssuranceCasePackage`s by inheritance: they read typed in
//! `assuranceCasePackage` of any AssuranceCasePackage kind, in
//! `artifactElement` of an ArtifactPackage and as roots, hold their children
//! in the four roles of an AssuranceCasePackage, and carry the package's
//! `interface` end. In an ArgumentPackage's `argumentElement`, a
//! TerminologyPackage's `terminologyElement` and an AssuranceCasePackage's
//! `argumentPackage`, which admit no AssuranceCasePackage, they are kept whole
//! as preserved content of the holder. The ends are kept as written and never
//! synced (Q1); since slice 0068 a missing `implements` and fewer than two
//! participants each give one SACM-MULT-001 (SACM23-VAL-014), and a
//! sub-package of an interface that is not an interface one SACM-PKG-001
//! (the §9.3 OCL, SACM23-VAL-015); an `AssuranceCasePackageInterface` has no
//! placement rule (SACM23-VAL-023); a lower bound counts distinct ids (Q7).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use std::collections::BTreeSet;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ElementKind, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

use sacm::compare::Difference;
use sacm::edit::CrossPackageReferencePolicy::ApplyReferencePolicy;
use sacm::edit::PackageDeletePolicy::{DeleteRecursively, RejectIfNonEmpty};
use sacm::edit::ReferenceDeletePolicy::ScrubReferences;
use sacm::edit::{
    self, AddReference, CopyElements, CreateAssuranceCasePackage, DeleteElement, EditError,
    MoveElement, Mutation, Operation, RemoveReference, ReorderElements, SetFlag, SetGid, SetName,
    SetReferences,
};
use sacm::model::{Flag, LangString, ReferenceEnd};

// The creates of slice 0067 (SACM23-AC-002, SACM23-AC-003).
use sacm::edit::{CreateAssuranceCasePackageBinding, CreateAssuranceCasePackageInterface};

const AC_002: &str = "SACM23-AC-002";
const AC_003: &str = "SACM23-AC-003";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a bare root: `sacm`, `xmi` and a foreign
/// `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:v=\"urn:v\"",
);

/// The assurance case fixture of slice 0067 and its golden output.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/assurance-case-package-interfaces-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/assurance-case-package-interfaces.golden.sacm.xmi"
);

const INTERFACE: &str = "AssuranceCasePackageInterface";
const BINDING: &str = "AssuranceCasePackageBinding";

// --- Inputs --------------------------------------------------------------------------

/// An element of the class `class`, `id`, typed in the role `role`, with
/// `attributes`, named after its id, holding `body`.
fn typed_in(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// An `AssuranceCasePackageInterface` `id` in the role `role`.
fn interface_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    typed_in(role, INTERFACE, id, attributes, body)
}

/// An `AssuranceCasePackageBinding` `id` in the role `role`.
fn binding_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    typed_in(role, BINDING, id, attributes, body)
}

/// An `AssuranceCasePackageInterface` `id` in an AssuranceCasePackage's
/// `assuranceCasePackage`.
fn interface(id: &str, attributes: &str, body: &str) -> String {
    interface_in("assuranceCasePackage", id, attributes, body)
}

/// An `AssuranceCasePackageBinding` `id` in an AssuranceCasePackage's
/// `assuranceCasePackage`.
fn binding(id: &str, attributes: &str, body: &str) -> String {
    binding_in("assuranceCasePackage", id, attributes, body)
}

/// The root AssuranceCasePackage `case`, one element per line:
/// - the AssuranceCasePackage `a`, with `a_attributes`, holding `a_body`;
/// - the empty AssuranceCasePackage `c`;
/// - the ArgumentPackage `p`, holding the Claim `cl` and the
///   `ArgumentPackageInterface` `p_api` (implementing `p`), then `p_body`;
/// - the ArtifactPackage `arts`, holding the Artifact `report`, then
///   `arts_body`;
/// - the TerminologyPackage `terms`, holding the Term `blade`, then
///   `terms_body`;
/// - then `extra`.
fn case(
    a_attributes: &str,
    a_body: &str,
    p_body: &str,
    arts_body: &str,
    terms_body: &str,
    extra: &str,
) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">
  <name content=\"Case\"/>
  <assuranceCasePackage xmi:id=\"a\"{a_attributes}><name content=\"A\"/>
    {a_body}
  </assuranceCasePackage>
  <assuranceCasePackage xmi:id=\"c\"><name content=\"C\"/></assuranceCasePackage>
  <argumentPackage xmi:id=\"p\"><name content=\"P\"/>
    <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"cl\"><name content=\"Cl\"/></argumentElement>
    <argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"p_api\" implements=\"p\"><name content=\"P API\"/></argumentElement>
    {p_body}
  </argumentPackage>
  <artifactPackage xmi:id=\"arts\"><name content=\"Arts\"/>
    <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"report\"><name content=\"Report\"/></artifactElement>
    {arts_body}
  </artifactPackage>
  <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/>
    <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\"><name content=\"Blade\"/></terminologyElement>
    {terms_body}
  </terminologyPackage>
  {extra}
</sacm:AssuranceCasePackage>"
    )
}

/// [`case`] with only `a_body` and `extra`.
fn case_with(a_body: &str, extra: &str) -> String {
    case("", a_body, "", "", "", extra)
}

/// [`case`] with the interface `i` in `a`, with `attributes` and `body`.
fn i_in_a(attributes: &str, body: &str) -> String {
    case_with(&interface("i", attributes, body), "")
}

/// [`case`] with the binding `b` in `case`, with `attributes` and `body`.
fn b_in_case(attributes: &str, body: &str) -> String {
    case_with("", &binding("b", attributes, body))
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!(
            "SACM23-AC-002: setup: SourceLocation::new({line}, {column}) refused a 1-based position"
        );
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

// --- Reading, writing and validating -------------------------------------------------

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
                panic!("{row}: {what}: a {mode:?} read refused it: {error:?} ({error})\n{input}")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The strict read's refusal of `input`.
fn refused(row: &str, what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{row}: {what}: a strict read returned a document of {} elements",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// The tolerant read of `input`, which must succeed.
fn tolerant(row: &str, what: &str, input: &str) -> ReadOutput {
    match read(Mode::Tolerant, input) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?} ({error})"),
    }
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, an Error in strict mode and a
/// Warning in tolerant mode. Returns the strict document.
fn assert_preserved_at(row: &str, what: &str, template: &str, holder: &str) -> Document {
    let (input, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{row}: setup: {what} has not one mark");
    };
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{row}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
            ),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(*at),
                vec![ElementId::from(holder)]
            )],
            "{row}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{row}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn write_in(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
fn round_trip(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
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
        "{row}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{row}: {what}: the output reads back !=");
    assert_eq!(
        write_in(row, what, &back, mode),
        text,
        "{row}: {what}: writing what was read back gives other bytes"
    );
    text
}

fn element<'d>(row: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    found
}

fn parent_of(row: &str, doc: &Document, id: &str) -> Option<String> {
    element(row, doc, id)
        .parent()
        .map(|parent| parent.id().as_str().to_owned())
}

fn children(row: &str, doc: &Document, id: &str) -> Vec<String> {
    element(row, doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

type Finding = (
    Code,
    Severity,
    String,
    Option<String>,
    Option<SourceLocation>,
    Vec<ElementId>,
);

fn finding(found: &Diagnostic) -> Finding {
    (
        found.code(),
        found.severity(),
        found.requirement_id().to_owned(),
        found.clause().map(str::to_owned),
        found.location(),
        found.affected().to_vec(),
    )
}

/// The text argument `name` of `found`, if it has one.
fn argument_of<'d>(found: &'d Diagnostic, name: &str) -> Option<&'d str> {
    found
        .arguments()
        .iter()
        .find(|argument| argument.name() == name)
        .and_then(|argument| argument.value().as_text())
}

/// The findings of `validate` on `doc`.
fn findings(doc: &Document) -> Vec<Finding> {
    validation::validate(doc).iter().map(finding).collect()
}

/// One validate finding with the code, row, clause and affected ids given,
/// an Error but for SACM-MULT-001 (a repeat), which is a Warning.
fn expected(code: Code, row: &str, clause: &str, affected: &[&str]) -> Finding {
    let severity = if code == Code::MULTIPLICITY_VIOLATION {
        Severity::Warning
    } else {
        Severity::Error
    };
    (
        code,
        severity,
        row.to_owned(),
        Some(clause.to_owned()),
        None,
        list(affected),
    )
}

/// One validate finding with the code, severity, row, clause and affected
/// ids given: for the findings of slice 0068 (SACM23-VAL-014, -015, -023).
fn finding_of(
    code: Code,
    severity: Severity,
    row: &str,
    clause: &str,
    affected: &[&str],
) -> Finding {
    (
        code,
        severity,
        row.to_owned(),
        Some(clause.to_owned()),
        None,
        list(affected),
    )
}

/// `found`, sorted, for comparing findings whatever their order.
fn sorted(mut found: Vec<Finding>) -> Vec<Finding> {
    found.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    found
}

/// Asserts that a strict write of `doc` is refused as `Invalid` and that a
/// tolerant write writes the line of `id` ending with `tail`.
fn assert_strict_refuses_tolerant_writes(
    row: &str,
    what: &str,
    doc: &Document,
    id: &str,
    tail: &str,
) {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{row}: {what}: a strict write accepted it:\n{text}"),
        Err(error) => assert!(
            matches!(error, WriteError::Invalid { .. }),
            "{row}: {what}: a strict write is not Invalid: {error:?}"
        ),
    }
    let text = write_in(row, what, doc, Mode::Tolerant);
    assert!(
        line_with(row, &text, id).ends_with(tail),
        "{row}: {what}: the tolerant write does not keep it\n{text}"
    );
}

fn fixture_text(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("SACM23-AC-002: cannot read {path}: {error}"))
}

/// One place an element `id` of `class` is read: what it is, the input, its
/// parent, and the role it is written in (`None` for a root).
type Place = (
    &'static str,
    String,
    Option<&'static str>,
    Option<&'static str>,
);

/// Asserts that the element `id` of `class` reads in each of `places` with no
/// diagnostic, as a `kind` under its parent, and writes back in its role (or
/// as a root) with `xmi:type`, in both modes.
fn assert_reads_in(row: &str, class: &str, kind: ElementKind, id: &str, places: &[Place]) {
    for (what, input, parent, role) in places {
        let doc = read_ok(row, what, input);
        assert_eq!(
            (
                element(row, &doc, id).kind(),
                parent_of(row, &doc, id).as_deref()
            ),
            (kind, *parent),
            "{row}: {what}: not a {class} under {parent:?}"
        );
        for mode in MODES {
            let text = round_trip(row, what, &doc, mode);
            let start = match role {
                Some(role) => format!("<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\""),
                None => format!("<sacm:{class} "),
            };
            assert!(
                line_with(row, &text, id).starts_with(&start),
                "{row}: {what}: a {mode:?} write does not write {id} as {start}\n{text}"
            );
        }
    }
}

// --- Edits ---------------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it and `edit::preview` gives the mutation
/// `apply` gives.
fn applied(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{row}: check refuses {operation:?}, which apply accepts"
    );
    assert_eq!(
        previewed.as_ref(),
        Ok(&mutation),
        "{row}: preview of {operation:?} does not give apply's mutation"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` and `preview` refusing it the same, and
/// returns why.
fn refuse(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{row}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{row}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{row}: check or preview answer {operation:?} otherwise than apply"
    );
    error
}

/// The variant, end and id of a refusal, for comparing check orders.
fn key(error: &EditError) -> (String, Option<&'static str>, Option<String>) {
    let variant = format!("{error:?}")
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned();
    let (end, id) = match error {
        EditError::TooFewReferences { end, .. } | EditError::TooManyReferences { end, .. } => {
            (Some(*end), None)
        }
        EditError::InvalidReference { end, id, .. }
        | EditError::ReferenceNotFound { end, id, .. }
        | EditError::WrongReferenceType { end, id, .. } => {
            (Some(*end), Some(id.as_str().to_owned()))
        }
        _ => (None, None),
    };
    (variant, end, id)
}

fn k(
    variant: &str,
    end: Option<&'static str>,
    id: Option<&str>,
) -> (String, Option<&'static str>, Option<String>) {
    (variant.to_owned(), end, id.map(str::to_owned))
}

/// A name that is an `ExpressionLangString` with no `expression`, read from
/// XMI (the API builds none).
fn name_without_expression(row: &str) -> LangString {
    let input = format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"t\">\
         <name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\"/>\
         </sacm:TerminologyPackage>"
    );
    let doc = read_ok(row, "a name with no expression", &input);
    element(row, &doc, "t")
        .name()
        .cloned()
        .unwrap_or_else(|| panic!("{row}: setup: the name with no expression is not read"))
}

/// The ids of `ids`, as strings.
fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

/// The (holder, end, position) of each referrer of `id`.
fn referrers(doc: &Document, id: &str) -> Vec<(String, ReferenceEnd, usize)> {
    doc.referrers(id)
        .into_iter()
        .map(|referrer| {
            (
                referrer.holder().id().as_str().to_owned(),
                referrer.end(),
                referrer.position(),
            )
        })
        .collect()
}

/// The copy of each source id, by `source_ids()` and `created_ids()`.
fn copies(copy: &CopyElements, mutation: &Mutation) -> Vec<(String, String)> {
    copy.source_ids()
        .iter()
        .zip(mutation.created_ids())
        .map(|(from, to)| (from.as_str().to_owned(), to.as_str().to_owned()))
        .collect()
}

fn copy_of(pairs: &[(String, String)], source: &str) -> String {
    match pairs.iter().find(|(from, _)| from == source) {
        Some((_, to)) => to.clone(),
        None => panic!("SACM23-AC-002: {source:?} was not copied: {pairs:?}"),
    }
}

/// The (holder, end, id, crosses) of each dangling reference the preview of
/// `operation` on `doc` lists.
fn dangling(
    row: &str,
    doc: &Document,
    operation: impl Into<Operation>,
) -> Vec<(String, &'static str, String, bool)> {
    let operation = operation.into();
    match edit::preview(doc, &operation) {
        Ok(preview) => preview
            .dangling_references()
            .iter()
            .map(|reference| {
                (
                    reference.holder().as_str().to_owned(),
                    reference.end().name(),
                    reference.id().as_str().to_owned(),
                    reference.crosses_package(),
                )
            })
            .collect(),
        Err(error) => panic!("{row}: the preview of {operation:?} was refused: {error}"),
    }
}

/// A dangling reference as plain values.
fn dangles(
    holder: &str,
    end: &'static str,
    id: &str,
    crosses: bool,
) -> (String, &'static str, String, bool) {
    (holder.to_owned(), end, id.to_owned(), crosses)
}

/// The delete of `element` with `ScrubReferences`, `DeleteRecursively` and
/// `ApplyReferencePolicy`.
fn scrubbing(element: &str) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(ScrubReferences)
        .with_package_policy(DeleteRecursively)
        .with_cross_package_policy(ApplyReferencePolicy)
}

/// The case of the create tests: [`case`] with the
/// `AssuranceCasePackageInterface` `ai` in `a`, the `ArgumentPackageBinding`
/// `p_b` in `p`, the `ArtifactPackageInterface` `arts_api` and the
/// `ArtifactPackageBinding` `arts_b` in `arts`, the
/// `TerminologyPackageInterface` `terms_api` and the
/// `TerminologyPackageBinding` `terms_b` in `terms`, and the binding `b` in
/// `case`.
fn create_case() -> String {
    case(
        "",
        &interface("ai", " implements=\"a\"", ""),
        &typed_in(
            "argumentElement",
            "ArgumentPackageBinding",
            "p_b",
            " participantPackage=\"p p\"",
            "",
        ),
        &format!(
            "{}{}",
            typed_in(
                "artifactElement",
                "ArtifactPackageInterface",
                "arts_api",
                " implements=\"arts\"",
                ""
            ),
            typed_in(
                "artifactElement",
                "ArtifactPackageBinding",
                "arts_b",
                " participantPackage=\"arts arts\"",
                ""
            ),
        ),
        &format!(
            "{}{}",
            typed_in(
                "terminologyElement",
                "TerminologyPackageInterface",
                "terms_api",
                " implements=\"terms\"",
                ""
            ),
            typed_in(
                "terminologyElement",
                "TerminologyPackageBinding",
                "terms_b",
                " participantPackage=\"terms terms\"",
                ""
            ),
        ),
        &binding("b", " participantPackage=\"a c\"", ""),
    )
}

/// The legal parents of an Assurance Case interface or binding in
/// [`create_case`]: an AssuranceCasePackage and an ArtifactPackage, each of
/// every kind.
const LEGAL_PARENTS: [&str; 7] = ["case", "a", "ai", "b", "arts", "arts_api", "arts_b"];

/// The parents of [`create_case`] that are `InvalidParent` for an
/// Assurance Case interface or binding.
const ILLEGAL_PARENTS: [(&str, ElementKind); 7] = [
    ("p", ElementKind::ArgumentPackage),
    ("p_api", ElementKind::ArgumentPackageInterface),
    ("p_b", ElementKind::ArgumentPackageBinding),
    ("terms", ElementKind::TerminologyPackage),
    ("terms_api", ElementKind::TerminologyPackageInterface),
    ("terms_b", ElementKind::TerminologyPackageBinding),
    ("cl", ElementKind::Claim),
];

/// Fixture A of the design, built with edits, in pre-order `a, c, ai, aj,
/// ci, b`: the empty root `AssuranceCasePackage`s `a` and `c`; the root
/// interface `ai` implementing `a`, holding the interface `aj` implementing
/// `c`; the empty root interface `ci` implementing `c`; the empty root
/// binding `b` of `[ai, ci]`.
fn fixture_a(row: &str) -> Document {
    let mut doc = Document::new();
    for package in ["a", "c"] {
        applied(
            row,
            &mut doc,
            CreateAssuranceCasePackage::root(named(package)).with_id(package),
        );
    }
    applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::root(named("ai"), "a").with_id("ai"),
    );
    applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::new("ai", named("aj"), "c").with_id("aj"),
    );
    applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::root(named("ci"), "c").with_id("ci"),
    );
    applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageBinding::root(named("b"), ["ai", "ci"]).with_id("b"),
    );
    let order: Vec<&str> = doc
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        order,
        ["a", "c", "ai", "aj", "ci", "b"],
        "{row}: setup: fixture A is not in the pre-order of the design"
    );
    doc
}

/// The children of an AssuranceCasePackage kind in its four roles: the
/// `AssuranceCasePackageInterface` `ii`, the `ArgumentPackageInterface` `iap`
/// and the plain ArgumentPackage `iplain` in `argumentPackage`, the
/// `ArtifactPackageInterface` `iar` in `artifactPackage` and the
/// `TerminologyPackageInterface` `itp` in `terminologyPackage`.
fn four_roles() -> String {
    format!(
        "{}{}{}{}{}",
        interface("ii", " implements=\"c\"", ""),
        typed_in(
            "argumentPackage",
            "ArgumentPackageInterface",
            "iap",
            " implements=\"p\"",
            ""
        ),
        "<argumentPackage xmi:id=\"iplain\"><name content=\"iplain\"/></argumentPackage>",
        typed_in(
            "artifactPackage",
            "ArtifactPackageInterface",
            "iar",
            " implements=\"arts\"",
            ""
        ),
        typed_in(
            "terminologyPackage",
            "TerminologyPackageInterface",
            "itp",
            " implements=\"terms\"",
            ""
        ),
    )
}

/// Asserts that `holder` in `doc` holds the children of [`four_roles`],
/// each of its kind, and that `validate` gives the findings of slice 0068
/// only: the SACM-PKG-002 Warning of each of `iap`, `iar` and `itp`, which
/// reside outside the package they implement (SACM23-VAL-023), and, when
/// `holder` is an `AssuranceCasePackageInterface`, the SACM-PKG-001 Error
/// of its plain ArgumentPackage `iplain` (the §9.3 OCL, SACM23-VAL-015).
fn assert_holds_four_roles(row: &str, what: &str, doc: &Document, holder: &str) {
    assert_eq!(
        children(row, doc, holder)
            .iter()
            .map(|id| (id.clone(), element(row, doc, id).kind()))
            .collect::<Vec<_>>(),
        [
            ("ii", ElementKind::AssuranceCasePackageInterface),
            ("iap", ElementKind::ArgumentPackageInterface),
            ("iplain", ElementKind::ArgumentPackage),
            ("iar", ElementKind::ArtifactPackageInterface),
            ("itp", ElementKind::TerminologyPackageInterface),
        ]
        .map(|(id, kind)| (id.to_owned(), kind)),
        "{row}: {what}: the children in the four roles"
    );
    let placement = |interface: &str, package: &str, clause: &str| {
        finding_of(
            Code::new("SACM-PKG-002"),
            Severity::Warning,
            "SACM23-VAL-023",
            clause,
            &[interface, package],
        )
    };
    let mut want = vec![
        placement("iap", "p", "11.6"),
        placement("iar", "arts", "12.5"),
        placement("itp", "terms", "10.5"),
    ];
    if element(row, doc, holder).kind() == ElementKind::AssuranceCasePackageInterface {
        want.push(finding_of(
            Code::PACKAGE_CONTENT_INVALID,
            Severity::Error,
            "SACM23-VAL-015",
            "9.3",
            &[holder, "iplain"],
        ));
    }
    assert_eq!(
        sorted(findings(doc)),
        sorted(want),
        "{row}: {what}: validate does not give the findings of slice 0068 alone"
    );
}

// =====================================================================================
// SACM23-AC-002: AssuranceCasePackageInterface
// =====================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_002_reads_typed_in_every_role_and_as_a_root() {
    let row = AC_002;
    let kind = ElementKind::AssuranceCasePackageInterface;
    let implementing = " implements=\"a\"";
    let places: Vec<Place> = vec![
        (
            "assuranceCasePackage in an AssuranceCasePackage",
            i_in_a(implementing, ""),
            Some("a"),
            Some("assuranceCasePackage"),
        ),
        (
            "assuranceCasePackage in an AssuranceCasePackageInterface",
            case_with(
                &interface("ai", implementing, &interface("i", implementing, "")),
                "",
            ),
            Some("ai"),
            Some("assuranceCasePackage"),
        ),
        (
            "assuranceCasePackage in an AssuranceCasePackageBinding",
            case_with(
                "",
                &binding(
                    "b",
                    " participantPackage=\"a c\"",
                    &interface("i", implementing, ""),
                ),
            ),
            Some("b"),
            Some("assuranceCasePackage"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                "",
                &interface_in("artifactElement", "i", implementing, ""),
                "",
                "",
            ),
            Some("arts"),
            Some("artifactElement"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:AssuranceCasePackageInterface{NS} xmi:id=\"i\" implements=\"i\">\
                 <name content=\"i\"/></sacm:AssuranceCasePackageInterface>"
            ),
            None,
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:AssuranceCasePackage xmi:id=\"a\"><name content=\"A\"/></sacm:AssuranceCasePackage>\
                 <sacm:AssuranceCasePackageInterface xmi:id=\"i\" implements=\"a\"><name content=\"i\"/>\
                 </sacm:AssuranceCasePackageInterface></xmi:XMI>"
            ),
            None,
            None,
        ),
    ];
    assert_reads_in(row, INTERFACE, kind, "i", &places);
    assert!(
        kind.is_kind_of(ElementKind::AssuranceCasePackage),
        "{row}: an AssuranceCasePackageInterface is no AssuranceCasePackage"
    );
    // Its children in all four roles; a plain ArgumentPackage child reads,
    // and `validate` reports it (SACM23-VAL-015, slice 0068).
    let what = "an interface holding a child in each role";
    let doc = read_ok(row, what, &i_in_a(implementing, &four_roles()));
    assert_holds_four_roles(row, what, &doc, "i");
    let text = round_trip(row, what, &doc, Mode::Strict);
    for (id, start) in [
        (
            "ii",
            "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageInterface\"",
        ),
        (
            "iap",
            "<argumentPackage xmi:type=\"sacm:ArgumentPackageInterface\"",
        ),
        ("iplain", "<argumentPackage xmi:id=\"iplain\""),
        (
            "iar",
            "<artifactPackage xmi:type=\"sacm:ArtifactPackageInterface\"",
        ),
        (
            "itp",
            "<terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\"",
        ),
    ] {
        assert!(
            line_with(row, &text, id).starts_with(start),
            "{row}: {what}: {id} is not written as {start}\n{text}"
        );
    }
    // Its name, utility children, gid, flags, citedElement and abstractForm.
    let what = "an interface with every inherited feature";
    let doc = read_ok(
        row,
        what,
        &i_in_a(
            " implements=\"a\" gid=\"I-1\" isCitation=\"true\" citedElement=\"c\" \
             isAbstract=\"true\" abstractForm=\"i\"",
            "<description xmi:id=\"i_d\"><content><value content=\"D\"/></content></description>\
             <implementationConstraint xmi:id=\"i_ic\"/>\
             <note xmi:id=\"i_n\"><content><value content=\"N\"/></content></note>\
             <taggedValue xmi:id=\"i_tv\"><key><value content=\"K\"/></key></taggedValue>",
        ),
    );
    let i = element(row, &doc, "i");
    assert_eq!(
        (
            i.name().and_then(|name| name.content()),
            i.gid(),
            i.is_citation(),
            i.cited_element().map(ElementId::as_str),
            i.is_abstract(),
            i.abstract_form().map(ElementId::as_str),
        ),
        (Some("i"), Some("I-1"), true, Some("c"), true, Some("i")),
        "{row}: {what}: the inherited features"
    );
    assert_eq!(
        children(row, &doc, "i")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["i_d", "i_ic", "i_n", "i_tv"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "{row}: {what}: the utility children"
    );
    round_trip(row, what, &doc, Mode::Strict);
    // In an ArgumentPackage's argumentElement, a TerminologyPackage's
    // terminologyElement and an AssuranceCasePackage's argumentPackage, which
    // admit no AssuranceCasePackage: kept whole as preserved content of the
    // holder.
    let held = |role: &str| interface_in(role, "x", implementing, &interface("xi", "", ""));
    for (what, template, holder) in [
        (
            "an interface in argumentElement",
            case(
                "",
                "",
                &format!("{MARK}{}", held("argumentElement")),
                "",
                "",
                "",
            ),
            "p",
        ),
        (
            "an interface in terminologyElement",
            case(
                "",
                "",
                "",
                "",
                &format!("{MARK}{}", held("terminologyElement")),
                "",
            ),
            "terms",
        ),
        (
            "an interface in argumentPackage",
            case_with("", &format!("{MARK}{}", held("argumentPackage"))),
            "case",
        ),
    ] {
        let doc = assert_preserved_at(row, what, &template, holder);
        assert!(
            doc.get("x").is_none() && doc.get("xi").is_none(),
            "{row}: {what}: read as an element"
        );
    }
    // An untyped assuranceCasePackage is a plain AssuranceCasePackage.
    let doc = read_ok(
        row,
        "an untyped assuranceCasePackage",
        &case_with(
            "<assuranceCasePackage xmi:id=\"x\"><name content=\"x\"/></assuranceCasePackage>",
            "",
        ),
    );
    assert_eq!(
        (
            element(row, &doc, "x").kind(),
            parent_of(row, &doc, "x").as_deref()
        ),
        (ElementKind::AssuranceCasePackage, Some("a")),
        "{row}: an untyped assuranceCasePackage is not a plain AssuranceCasePackage"
    );
}

#[test]
fn sacm23_ac_002_implements_and_interface_read_in_attribute_and_element_form() {
    let row = AC_002;
    let attribute = read_ok(row, "implements=\"a\"", &i_in_a(" implements=\"a\"", ""));
    for (what, body) in [
        (
            "<implements xmi:idref=\"a\"/>",
            "<implements xmi:idref=\"a\"/>",
        ),
        ("<implements href=\"#a\"/>", "<implements href=\"#a\"/>"),
    ] {
        let doc = read_ok(row, what, &i_in_a("", body));
        assert!(
            doc == attribute,
            "{row}: {what} does not read == to the attribute"
        );
    }
    assert_eq!(
        element(row, &attribute, "i")
            .implements()
            .map(ElementId::as_str),
        Some("a"),
        "{row}: implements() of i"
    );
    let both = format!(
        "{}{}",
        interface("i1", " implements=\"a\"", ""),
        interface("i2", " implements=\"a\"", "")
    );
    // On the child a, on the root case, and inherited on an interface.
    let on_a = read_ok(
        row,
        "interface=\"i1 i2\" on a",
        &case(" interface=\"i1 i2\"", &both, "", "", "", ""),
    );
    let elements = read_ok(
        row,
        "interface elements on a",
        &case_with(
            &format!("<interface xmi:idref=\"i1\"/><interface href=\"#i2\"/>{both}"),
            "",
        ),
    );
    assert!(
        elements == on_a,
        "{row}: interface elements on a do not read == to the attribute"
    );
    let on_case = read_ok(
        row,
        "interface=\"i1 i2\" on the root case",
        &case_with(&both, "").replacen("xmi:id=\"case\"", "xmi:id=\"case\" interface=\"i1 i2\"", 1),
    );
    let case_elements = read_ok(
        row,
        "interface elements on the root case",
        &case_with(&both, "").replacen(
            "<name content=\"Case\"/>",
            "<interface xmi:idref=\"i1\"/><interface xmi:idref=\"i2\"/><name content=\"Case\"/>",
            1,
        ),
    );
    assert!(
        case_elements == on_case,
        "{row}: interface elements on case do not read == to the attribute"
    );
    let inherited = read_ok(
        row,
        "interface=\"i1 i2\" on an interface",
        &i_in_a(" implements=\"a\" interface=\"i1 i2\"", &both),
    );
    let inherited_elements = read_ok(
        row,
        "interface elements on an interface",
        &i_in_a(
            " implements=\"a\"",
            &format!("<interface xmi:idref=\"i1\"/><interface xmi:idref=\"i2\"/>{both}"),
        ),
    );
    assert!(
        inherited_elements == inherited,
        "{row}: interface elements on an interface do not read == to the attribute"
    );
    assert_eq!(
        (
            element(row, &on_a, "a").interfaces(),
            element(row, &on_case, "case").interfaces(),
            element(row, &inherited, "i").interfaces(),
        ),
        (
            list(&["i1", "i2"]).as_slice(),
            list(&["i1", "i2"]).as_slice(),
            list(&["i1", "i2"]).as_slice()
        ),
        "{row}: interfaces() in order"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_002_repeated_empty_and_qualified_implements_follow_the_reference_rules() {
    let row = AC_002;
    for (what, attributes, body, first) in [
        (
            "implements=\"a c\"",
            format!(" implements=\"{MARK}a {MARK}c\""),
            String::new(),
            None,
        ),
        (
            "implements as an attribute and an element",
            format!(" {MARK}implements=\"a\""),
            format!("{MARK}<implements xmi:idref=\"c\"/>"),
            Some("a"),
        ),
    ] {
        let (input, marks) = marked(&i_in_a(&attributes, &body));
        let error = refused(row, what, &input);
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property, .. }
                    if element.as_str() == "i" && property == "implements"
            ),
            "{row}: {what}: not RepeatedValue {{ i, implements }}: {error:?}"
        );
        assert_eq!(
            (error.code(), error.locations()),
            (Code::MULTIPLICITY_VIOLATION, marks.as_slice()),
            "{row}: {what}: not SACM-MULT-001 at both occurrences"
        );
        let output = tolerant(row, what, &input);
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.template(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                "SACM-MULT-001/repeat-kept",
                list(&["i"])
            )],
            "{row}: {what}: a tolerant read does not give one SACM-MULT-001 repeat-kept Warning"
        );
        assert_eq!(
            element(row, output.document(), "i")
                .implements()
                .map(ElementId::as_str),
            first,
            "{row}: {what}: what the end holds after a tolerant read"
        );
    }
    for (what, attribute) in [
        ("implements=\"\"", format!(" {MARK}implements=\"\"")),
        ("sacm:implements", format!(" {MARK}sacm:implements=\"a\"")),
        (
            "v:interface on the interface",
            format!(" implements=\"a\" {MARK}v:interface=\"i\""),
        ),
    ] {
        assert_preserved_at(row, what, &i_in_a(&attribute, ""), "i");
    }
    // Moved from interface_ends.rs: an interface in another namespace is not
    // the end, on an AssuranceCasePackage root and child, as an attribute and
    // as an element.
    for (what, template, holder) in [
        (
            "v:interface=\"i\" on a",
            case(&format!(" {MARK}v:interface=\"i\""), "", "", "", "", ""),
            "a",
        ),
        (
            "a v:interface element on a",
            case_with(&format!("{MARK}<v:interface xmi:idref=\"i\"/>"), ""),
            "a",
        ),
        (
            "v:interface=\"i\" on the root case",
            case_with("", "").replacen(
                "xmi:id=\"case\"",
                &format!("xmi:id=\"case\" {MARK}v:interface=\"i\""),
                1,
            ),
            "case",
        ),
        (
            "a v:interface element on the root case",
            case_with("", "").replacen(
                "<name content=\"Case\"/>",
                &format!("<name content=\"Case\"/>{MARK}<v:interface xmi:idref=\"i\"/>"),
                1,
            ),
            "case",
        ),
    ] {
        assert_preserved_at(row, what, &template, holder);
    }
    let what = "interface=\"i i\"";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i i\"",
            &interface("i", " implements=\"a\"", ""),
            "",
            "",
            "",
            "",
        ),
    );
    assert_eq!(
        element(row, &doc, "a").interfaces(),
        list(&["i", "i"]).as_slice(),
        "{row}: {what}: not held twice"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "9.2",
            &["a", "i"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 9.2"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "a").ends_with(" interface=\"i i\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_ac_002_missing_implements_reads_and_writes_back_unchanged() {
    let row = AC_002;
    let what = "an interface with no implements";
    let doc = read_ok(row, what, &i_in_a("", ""));
    assert_eq!(
        findings(&doc),
        [finding_of(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Error,
            "SACM23-VAL-014",
            "9.3",
            &["i"]
        )],
        "{row}: {what}: validate does not give one SACM-MULT-001 missing (SACM23-VAL-014)"
    );
    for mode in MODES {
        let text = round_trip(row, what, &doc, mode);
        assert!(
            line_with(row, &text, "i").ends_with("xmi:id=\"i\">"),
            "{row}: {what}: a {mode:?} write added to the start tag\n{text}"
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_002_dangling_and_wrongly_typed_ends_report_their_clause_and_type() {
    let row = AC_002;
    let what = "implements=\"gone\"";
    let doc = read_ok(row, what, &i_in_a(" implements=\"gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "9.3",
            &["i", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 9.3"
    );
    assert_eq!(
        found[0].message(),
        "AssuranceCasePackageInterface \"i\" has the implements \"gone\", which names no \
         element of the document (the end's type is AssuranceCasePackage)",
        "{row}: {what}: the message"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "i", " implements=\"gone\">");
    let with_binding = |attributes: &str| {
        case_with(
            &interface("i", attributes, ""),
            &binding("b", " participantPackage=\"a c\"", ""),
        )
    };
    for target in ["case", "a", "c", "i", "b"] {
        let what = format!("implements=\"{target}\"");
        let doc = read_ok(
            row,
            &what,
            &with_binding(&format!(" implements=\"{target}\"")),
        );
        assert_eq!(
            findings(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
    for (target, class) in [
        ("terms", "TerminologyPackage"),
        ("p", "ArgumentPackage"),
        ("p_api", "ArgumentPackageInterface"),
        ("cl", "Claim"),
    ] {
        let what = format!("implements=\"{target}\" (a {class})");
        let doc = read_ok(
            row,
            &what,
            &with_binding(&format!(" implements=\"{target}\"")),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "9.3",
                &["i", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 9.3"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("AssuranceCasePackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
    }
    for (target, class) in [
        ("c", "AssuranceCasePackage"),
        ("b", "AssuranceCasePackageBinding"),
        ("p_api", "ArgumentPackageInterface"),
    ] {
        let what = format!("interface=\"{target}\" (a {class}) on a");
        let doc = read_ok(
            row,
            &what,
            &case(
                &format!(" interface=\"{target}\""),
                "",
                "",
                "",
                "",
                &binding("b", " participantPackage=\"a c\"", ""),
            ),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "9.2",
                &["a", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 9.2"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("AssuranceCasePackageInterface"),
            "{row}: {what}: the end's type"
        );
    }
    let what = "interface=\"gone\" on a";
    let doc = read_ok(row, what, &case(" interface=\"gone\"", "", "", "", "", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "9.2",
            &["a", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 9.2"
    );
}

#[test]
fn sacm23_ac_002_writes_attribute_order_and_the_golden_file() {
    let row = AC_002;
    let what = "an interface with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i\" abstractForm=\"a\" gid=\"A-1\"",
            &interface(
                "i",
                " implements=\"a\" interface=\"i\" abstractForm=\"i\" citedElement=\"i\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"I-1\"",
                "",
            ),
            "",
            "",
            "",
            "",
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageInterface\" xmi:id=\"i\" \
         gid=\"I-1\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"i\" \
         abstractForm=\"i\" interface=\"i\" implements=\"a\">",
        "{row}: {what}: not written in order\n{text}"
    );
    assert_eq!(
        line_with(row, &text, "a"),
        "<assuranceCasePackage xmi:id=\"a\" gid=\"A-1\" abstractForm=\"a\" interface=\"i\">",
        "{row}: {what}: the package's interface is not written last\n{text}"
    );
    let what = "an interface in each role";
    let doc = read_ok(
        row,
        what,
        &case(
            "",
            &interface("i1", " implements=\"a\"", ""),
            "",
            &interface_in("artifactElement", "i2", " implements=\"a\"", ""),
            "",
            "",
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    for (id, role) in [("i1", "assuranceCasePackage"), ("i2", "artifactElement")] {
        assert_eq!(
            line_with(row, &text, id),
            format!(
                "<{role} xmi:type=\"sacm:AssuranceCasePackageInterface\" xmi:id=\"{id}\" \
                 implements=\"a\">"
            ),
            "{row}: {id} is not written typed in {role}\n{text}"
        );
    }
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageInterface\" \
             xmi:id=\"braking_case_api\" implements=\"braking_case\">"
        ),
        "{row}: the golden file has no interface line"
    );
}

#[test]
fn sacm23_ac_002_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let row = AC_002;
    let doc = read_ok(row, "the fixture", &fixture_text(FIXTURE));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: the fixture: validate reports a finding"
    );
    let golden = fixture_text(GOLDEN);
    let text = write_in(row, "the fixture", &doc, Mode::Strict);
    assert_eq!(
        text, golden,
        "{row}: a strict write of the fixture is not exactly the golden file"
    );
    let back = read_ok(row, "the golden file", &golden);
    assert!(
        back == doc,
        "{row}: the golden file does not read == to the fixture"
    );
    assert_eq!(
        write_in(row, "the golden file", &back, Mode::Strict),
        golden,
        "{row}: writing the golden file again gives other bytes"
    );
    assert_eq!(
        (
            element(row, &doc, "binding").kind(),
            element(row, &doc, "braking_case_api").kind(),
            children(row, &doc, "braking_case_api"),
            element(row, &doc, "braking_api_cited").kind(),
        ),
        (
            ElementKind::AssuranceCasePackageBinding,
            ElementKind::AssuranceCasePackageInterface,
            vec!["braking_api_cited".to_owned()],
            ElementKind::ArgumentPackageInterface,
        ),
        "{row}: the fixture's interface and binding"
    );
    assert_eq!(
        (
            element(row, &doc, "braking_case").interfaces(),
            element(row, &doc, "vehicle_case").interfaces(),
            element(row, &doc, "braking_case_api")
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "binding").participant_packages(),
        ),
        (
            list(&["braking_case_api"]).as_slice(),
            list(&["vehicle_case_api"]).as_slice(),
            Some("braking_case"),
            list(&["braking_case_api", "vehicle_case_api"]).as_slice(),
        ),
        "{row}: the fixture's ends"
    );
}

#[test]
fn sacm23_ac_002_reference_end_tables_bounds_and_serde() {
    let row = AC_002;
    let (interface, implements) = (ReferenceEnd::Interface, ReferenceEnd::Implements);
    assert_eq!(
        (
            implements.bounds(ElementKind::AssuranceCasePackageInterface),
            interface.bounds(ElementKind::AssuranceCasePackage),
            interface.bounds(ElementKind::AssuranceCasePackageInterface),
            interface.bounds(ElementKind::AssuranceCasePackageBinding),
            implements.bounds(ElementKind::AssuranceCasePackage),
            implements.bounds(ElementKind::AssuranceCasePackageBinding),
        ),
        (
            Some((1, Some(1))),
            Some((0, None)),
            Some((0, None)),
            Some((0, None)),
            None,
            None
        ),
        "{row}: the bounds on the AssuranceCase kinds"
    );
    assert!(
        interface.applies_to(ElementKind::AssuranceCasePackage)
            && implements.applies_to(ElementKind::AssuranceCasePackageInterface),
        "{row}: the ends do not apply to the AssuranceCase kinds"
    );
    #[cfg(feature = "serde")]
    for (end, name) in [(interface, "\"interface\""), (implements, "\"implements\"")] {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, name, "{row}: the serde name of {end:?}");
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
#[test]
fn sacm23_ac_002_create_reports_what_it_holds_and_checks_in_order() {
    let row = AC_002;
    let operation = CreateAssuranceCasePackageInterface::root(named("Braking case interface"), "a");
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.name(),
            operation.implements().map(ElementId::as_str)
        ),
        (None, None, &named("Braking case interface"), Some("a")),
        "{row}: root()"
    );
    let operation = CreateAssuranceCasePackageInterface::new("a", named("I"), "a").with_id("i");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            operation.implements().map(ElementId::as_str)
        ),
        (Some("a"), Some("i"), Some("a")),
        "{row}: new() and with_id()"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateAssuranceCasePackageInterface",
        "{row}: operation_name()"
    );
    let mut doc = read_ok(row, "the case", &case_with("", ""));
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            strs(mutation.changed_ids()),
            parent_of(row, &doc, "i"),
            element(row, &doc, "i").kind(),
            element(row, &doc, "i").implements().map(ElementId::as_str),
            element(row, &doc, "a").interfaces().len(),
        ),
        (
            vec!["i"],
            vec![],
            Some("a".to_owned()),
            ElementKind::AssuranceCasePackageInterface,
            Some("a"),
            0
        ),
        "{row}: the created interface; a's interface end is unchanged"
    );
    let text = round_trip(row, "the created interface", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageInterface\" xmi:id=\"i\" \
         implements=\"a\">",
        "{row}: the created interface's start tag\n{text}"
    );
    let mut doc = read_ok(row, "the create case", &create_case());
    let mut generated = Vec::new();
    for parent in LEGAL_PARENTS {
        let mutation = applied(
            row,
            &mut doc,
            CreateAssuranceCasePackageInterface::new(parent, named("N"), "c"),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let first = generated[0].clone();
    let mutation = applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::new(first.as_str(), named("N"), "c"),
    );
    generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    let root = applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::root(named("Root"), "c"),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=9)
            .map(|n| format!("assurance_case_package_interface_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (id, parent) in generated.iter().zip(LEGAL_PARENTS) {
        assert_eq!(
            parent_of(row, &doc, id).as_deref(),
            Some(parent),
            "{row}: {id} is not under {parent}"
        );
    }
    for (parent, parent_kind) in ILLEGAL_PARENTS {
        let error = refuse(
            row,
            &mut doc,
            CreateAssuranceCasePackageInterface::new(parent, named("N"), "c"),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::AssuranceCasePackageInterface
            ),
            "{row}: an interface under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, implements: &str| {
        CreateAssuranceCasePackageInterface::new(parent, name, implements)
    };
    let cases: Vec<(&str, CreateAssuranceCasePackageInterface, _)> = vec![
        (
            "a missing parent",
            create("nowhere", unnamed.clone(), "a b").with_id("1i"),
            k("ParentNotFound", None, None),
        ),
        (
            "an illegal parent",
            create("p", unnamed.clone(), "a b").with_id("1i"),
            k("InvalidParent", None, None),
        ),
        (
            "an invalid id",
            create("a", unnamed.clone(), "a b").with_id("1i"),
            k("InvalidId", None, None),
        ),
        (
            "a duplicate id",
            create("a", unnamed.clone(), "a b").with_id("blade"),
            k("DuplicateId", None, None),
        ),
        (
            "5a: a name with no expression before an invalid implements",
            create("a", unnamed.clone(), "a b"),
            k("TooFewReferences", Some("expression"), None),
        ),
        (
            "an implements with white space",
            create("a", named("N").with_expression("nowhere"), "a b"),
            k("InvalidReference", Some("implements"), Some("a b")),
        ),
        (
            "a missing implements before the name's expression",
            create("a", named("N").with_expression("nowhere"), "nope"),
            k("ReferenceNotFound", Some("implements"), Some("nope")),
        ),
        (
            "the name's missing expression",
            create("a", named("N").with_expression("nowhere"), "c"),
            k("ReferenceNotFound", Some("expression"), Some("nowhere")),
        ),
        (
            "a TerminologyPackage as implements",
            create("a", named("N"), "terms"),
            k("WrongReferenceType", Some("implements"), Some("terms")),
        ),
        (
            "an ArgumentPackage as implements",
            create("a", named("N"), "p"),
            k("WrongReferenceType", Some("implements"), Some("p")),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "AssuranceCasePackage", "{row}: {what}: expected");
        }
    }
    let next = applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::new("a", named("N"), "c"),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["assurance_case_package_interface_10"],
        "{row}: a refusal moved the next generated id"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_002_reference_edits_keep_implements_at_exactly_one() {
    let row = AC_002;
    let implements = ReferenceEnd::Implements;
    let mut doc = read_ok(row, "i in a", &i_in_a(" implements=\"a\"", ""));
    for (what, operation) in [
        (
            "SetReferences::clear",
            Operation::from(SetReferences::clear("i", implements)),
        ),
        (
            "RemoveReference",
            RemoveReference::new("i", implements, "a").into(),
        ),
    ] {
        let error = refuse(row, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::TooFewReferences {
                    end: "implements",
                    given: 0,
                    required: 1,
                    ..
                }
            ),
            "{row}: {what}: not TooFewReferences {{ implements, 0, 1 }}: {error:?}"
        );
    }
    for (what, operation) in [
        (
            "SetReferences [a, c]",
            Operation::from(SetReferences::new("i", implements, ["a", "c"])),
        ),
        (
            "AddReference c onto a",
            AddReference::new("i", implements, "c").into(),
        ),
    ] {
        let error = refuse(row, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::TooManyReferences {
                    end: "implements",
                    given: 2,
                    allowed: 1,
                    ..
                }
            ),
            "{row}: {what}: not TooManyReferences {{ implements, 2, 1 }}: {error:?}"
        );
    }
    let error = refuse(
        row,
        &mut doc,
        SetReferences::new("i", implements, ["terms"]),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "implements",
                expected: "AssuranceCasePackage",
                ..
            }
        ),
        "{row}: implements [terms]: {error:?}"
    );
    // Before this slice the end was FeatureNotApplicable on an
    // AssuranceCasePackage.
    applied(
        row,
        &mut doc,
        SetReferences::new("a", ReferenceEnd::Interface, ["i"]),
    );
    applied(
        row,
        &mut doc,
        SetReferences::new("case", ReferenceEnd::Interface, ["i"]),
    );
    assert_eq!(
        (
            element(row, &doc, "a").interfaces(),
            element(row, &doc, "case").interfaces()
        ),
        (list(&["i"]).as_slice(), list(&["i"]).as_slice()),
        "{row}: SetReferences(a | case, interface, [i])"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("a", ReferenceEnd::Interface, "p_api"),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "interface",
                expected: "AssuranceCasePackageInterface",
                ..
            }
        ),
        "{row}: interface [p_api] on a: {error:?}"
    );
    let error = refuse(row, &mut doc, SetReferences::new("case", implements, ["a"]));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::AssuranceCasePackage,
                feature: "implements",
                ..
            }
        ),
        "{row}: implements on an AssuranceCasePackage: {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the AssuranceCasePackage \"case\" has no implements",
        "{row}: implements on an AssuranceCasePackage: the Display"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("cl", ReferenceEnd::Interface, "i"),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::Claim,
                feature: "interface",
                ..
            }
        ),
        "{row}: interface on a Claim: {error:?}"
    );
}

#[test]
fn sacm23_ac_002_interface_and_implements_are_never_synced() {
    let row = AC_002;
    let mut doc = read_ok(row, "the case", &case_with("", ""));
    applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageInterface::new("a", named("I"), "a").with_id("i"),
    );
    assert!(
        element(row, &doc, "a").interfaces().is_empty(),
        "{row}: creating i filled a's interface end"
    );
    let mutation = applied(
        row,
        &mut doc,
        AddReference::new("a", ReferenceEnd::Interface, "i"),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "i").implements().map(ElementId::as_str)
        ),
        (vec!["a"], Some("a")),
        "{row}: AddReference(a, interface, i) changed more than a"
    );
    assert_eq!(
        (referrers(&doc, "a"), referrers(&doc, "i")),
        (
            vec![("i".to_owned(), ReferenceEnd::Implements, 0)],
            vec![("a".to_owned(), ReferenceEnd::Interface, 0)]
        ),
        "{row}: the referrers of a and i"
    );
    applied(
        row,
        &mut doc,
        SetReferences::new("i", ReferenceEnd::Implements, ["c"]),
    );
    assert_eq!(
        (
            element(row, &doc, "a").interfaces(),
            element(row, &doc, "c").interfaces()
        ),
        (list(&["i"]).as_slice(), [].as_slice()),
        "{row}: changing implements changed an interface end"
    );
}

#[test]
fn sacm23_ac_002_compare_reports_interfaces_and_implements() {
    let row = AC_002;
    let left = read_ok(
        row,
        "left",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"a\"", ""),
            "",
            "",
            "",
            "",
        ),
    );
    let right = read_ok(row, "right", &case_with(&interface("i", "", ""), ""));
    let found = semantic_compare(&left, &right);
    assert_eq!(
        found,
        [
            Difference::Interfaces {
                id: ElementId::from("a"),
                left: list(&["i"]),
                right: Vec::new(),
            },
            Difference::Implements {
                id: ElementId::from("i"),
                left: Some(ElementId::from("a")),
                right: None,
            },
        ],
        "{row}: not one Interfaces and one Implements difference"
    );
    assert_eq!(
        found.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "element \"a\" differs in interface: [\"i\"] on the left, [] on the right",
            "element \"i\" differs in implements: \"a\" on the left, no implements on the right",
        ],
        "{row}: the Display of the differences"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_002_move_copy_and_the_element_edits_apply() {
    let row = AC_002;
    let mut doc = read_ok(
        row,
        "i in a",
        &case(
            " interface=\"i\"",
            &format!(
                "{}{}",
                interface("x", " implements=\"c\"", ""),
                interface(
                    "i",
                    " implements=\"a\"",
                    &interface("ia", " implements=\"c\"", "")
                )
            ),
            "",
            "",
            "",
            "",
        ),
    );
    applied(row, &mut doc, SetName::new("i", named("Renamed")));
    applied(row, &mut doc, SetGid::new("i", "I-1"));
    applied(row, &mut doc, SetFlag::new("i", Flag::IsAbstract, true));
    applied(row, &mut doc, ReorderElements::new("a", ["i", "x"]));
    assert_eq!(children(row, &doc, "a"), ["i", "x"], "{row}: the reorder");
    let ends = |doc: &Document| {
        (
            element(row, doc, "a").interfaces().to_vec(),
            element(row, doc, "i").implements().cloned(),
        )
    };
    let held = ends(&doc);
    let before = findings(&doc);
    for (what, operation, parent) in [
        ("to the roots", MoveElement::root("i"), None),
        ("into case", MoveElement::new("i", "case"), Some("case")),
        ("into arts", MoveElement::new("i", "arts"), Some("arts")),
        ("back into a", MoveElement::new("i", "a"), Some("a")),
    ] {
        applied(row, &mut doc, operation);
        assert_eq!(
            (
                parent_of(row, &doc, "i").as_deref(),
                ends(&doc),
                findings(&doc)
            ),
            (parent, held.clone(), before.clone()),
            "{row}: a move {what} changed a reference or a finding"
        );
    }
    for (parent, parent_kind) in [
        ("terms", ElementKind::TerminologyPackage),
        ("p", ElementKind::ArgumentPackage),
    ] {
        let error = refuse(row, &mut doc, MoveElement::new("i", parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent_kind: found, kind: ElementKind::AssuranceCasePackageInterface, .. }
                    if *found == parent_kind
            ),
            "{row}: an AssuranceCasePackageInterface into the {parent_kind} {parent}: {error:?}"
        );
    }
    // Copying braking_case follows the copied ids; copying its interface
    // alone keeps implements braking_case.
    let mut doc = read_ok(row, "the fixture", &fixture_text(FIXTURE));
    let copy = CopyElements::new(&doc, ["braking_case"], "system")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of braking_case: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let (package, api, arguments, arguments_api, cited) = (
        copy_of(&pairs, "braking_case"),
        copy_of(&pairs, "braking_case_api"),
        copy_of(&pairs, "braking"),
        copy_of(&pairs, "braking_api"),
        copy_of(&pairs, "braking_api_cited"),
    );
    assert_eq!(
        (
            element(row, &doc, &package).interfaces().to_vec(),
            element(row, &doc, &api).implements().cloned(),
            element(row, &doc, &cited).cited_element().cloned(),
            element(row, &doc, &cited).implements().cloned(),
        ),
        (
            vec![ElementId::from(api.as_str())],
            Some(ElementId::from(package.as_str())),
            Some(ElementId::from(arguments_api.as_str())),
            Some(ElementId::from(arguments.as_str())),
        ),
        "{row}: the copy of braking_case does not follow the copied ids: {pairs:?}"
    );
    let copy = CopyElements::new(&doc, ["braking_case_api"], "vehicle_case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of braking_case_api: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        (
            element(row, &doc, &copy_of(&pairs, "braking_case_api"))
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, &copy_of(&pairs, "braking_api_cited"))
                .cited_element()
                .map(ElementId::as_str),
            element(row, &doc, "vehicle_case").interfaces(),
        ),
        (
            Some("braking_case"),
            Some("braking_api"),
            list(&["vehicle_case_api"]).as_slice()
        ),
        "{row}: the copy of braking_case_api under vehicle_case"
    );
    // Copying the binding into an empty document: its participants must
    // resolve.
    let source = read_ok(row, "the fixture", &fixture_text(FIXTURE));
    let copy = CopyElements::root(&source, ["binding"])
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of binding: {error}"));
    let mut empty = Document::new();
    let error = refuse(row, &mut empty, copy);
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound { end: "participantPackage", id, .. }
                if id.as_str() == "braking_case_api"
        ),
        "{AC_003}: the copy of binding into an empty document: {error:?}"
    );
    // An interface read with no implements, and a binding read with one
    // distinct participant, are not copied.
    let mut doc = read_ok(
        row,
        "an interface without implements and a binding of [i, i]",
        &case_with(
            &interface("i", "", ""),
            &binding("b", " participantPackage=\"i i\"", ""),
        ),
    );
    let copy = CopyElements::new(&doc, ["i"], "c")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of i: {error}"));
    let error = refuse(row, &mut doc, copy);
    assert!(
        matches!(
            &error,
            EditError::TooFewReferences {
                end: "implements",
                given: 0,
                required: 1,
                ..
            }
        ),
        "{row}: the copy of an interface without implements: {error:?}"
    );
    let copy = CopyElements::root(&doc, ["b"])
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let error = refuse(row, &mut doc, copy);
    assert!(
        matches!(
            &error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{AC_003}: the copy of a binding of [i, i]: {error:?}"
    );
}

#[test]
fn sacm23_ac_002_deleting_the_implemented_package_follows_lib_edit_019() {
    let row = AC_002;
    // (a) By default, ai's implements crossing to a refuses.
    let mut doc = fixture_a(row);
    let error = refuse(row, &mut doc, DeleteElement::new("a"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end: "implements", id, .. }
                if holder.as_str() == "ai" && id.as_str() == "a"
        ),
        "{row}: (a) deleting a by default: not CrossPackageReference {{ ai, implements, a }}: \
         {error:?}"
    );
    // (b) Scrubbing, recursively: ai loses its implements and goes with aj,
    // and b, left with ci alone, goes too.
    let operation = scrubbing("a");
    assert_eq!(
        dangling(row, &doc, operation.clone()),
        [
            dangles("ai", "implements", "a", true),
            dangles("b", "participantPackage", "ai", true)
        ],
        "{row}: (b) the dangling references of deleting a"
    );
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["a", "ai", "aj", "b"], Vec::<&str>::new()),
        "{row}: (b) the deleted and changed ids"
    );
    // (c) The same with RejectIfNonEmpty: ai holds aj.
    let mut doc = fixture_a(row);
    let error = refuse(
        row,
        &mut doc,
        scrubbing("a").with_package_policy(RejectIfNonEmpty),
    );
    assert!(
        matches!(
            &error,
            EditError::NotEmpty { element, contained: 1, .. } if element.as_str() == "ai"
        ),
        "{row}: (c) not NotEmpty {{ ai, 1 }}: {error:?}"
    );
}

// =====================================================================================
// SACM23-AC-003: AssuranceCasePackageBinding
// =====================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_ac_003_reads_typed_in_every_role_and_as_a_root() {
    let row = AC_003;
    let kind = ElementKind::AssuranceCasePackageBinding;
    let attributes = " participantPackage=\"a c\"";
    let places: Vec<Place> = vec![
        (
            "assuranceCasePackage in an AssuranceCasePackage",
            b_in_case(attributes, ""),
            Some("case"),
            Some("assuranceCasePackage"),
        ),
        (
            "assuranceCasePackage in an AssuranceCasePackageInterface",
            case_with(
                &interface("ai", " implements=\"a\"", &binding("b", attributes, "")),
                "",
            ),
            Some("ai"),
            Some("assuranceCasePackage"),
        ),
        (
            "assuranceCasePackage in an AssuranceCasePackageBinding",
            case_with(
                "",
                &binding("outer", attributes, &binding("b", attributes, "")),
            ),
            Some("outer"),
            Some("assuranceCasePackage"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                "",
                &binding_in("artifactElement", "b", attributes, ""),
                "",
                "",
            ),
            Some("arts"),
            Some("artifactElement"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:AssuranceCasePackageBinding{NS} xmi:id=\"b\">\
                 <name content=\"b\"/></sacm:AssuranceCasePackageBinding>"
            ),
            None,
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:AssuranceCasePackage xmi:id=\"a\"><name content=\"A\"/></sacm:AssuranceCasePackage>\
                 <sacm:AssuranceCasePackageBinding xmi:id=\"b\" participantPackage=\"a a\">\
                 <name content=\"b\"/></sacm:AssuranceCasePackageBinding></xmi:XMI>"
            ),
            None,
            None,
        ),
    ];
    assert_reads_in(row, BINDING, kind, "b", &places);
    assert!(
        kind.is_kind_of(ElementKind::AssuranceCasePackage),
        "{row}: an AssuranceCasePackageBinding is no AssuranceCasePackage"
    );
    let what = "a binding holding a child in each role";
    let doc = read_ok(
        row,
        what,
        &b_in_case(" participantPackage=\"a c\" gid=\"B-1\"", &four_roles()),
    );
    assert_holds_four_roles(row, what, &doc, "b");
    assert_eq!(
        element(row, &doc, "b").gid(),
        Some("B-1"),
        "{row}: {what}: the gid"
    );
    for mode in MODES {
        round_trip(row, what, &doc, mode);
    }
    let held = |role: &str| binding_in(role, "x", attributes, &interface("xi", "", ""));
    for (what, template, holder) in [
        (
            "a binding in argumentElement",
            case(
                "",
                "",
                &format!("{MARK}{}", held("argumentElement")),
                "",
                "",
                "",
            ),
            "p",
        ),
        (
            "a binding in terminologyElement",
            case(
                "",
                "",
                "",
                "",
                &format!("{MARK}{}", held("terminologyElement")),
                "",
            ),
            "terms",
        ),
        (
            "a binding in argumentPackage",
            case_with("", &format!("{MARK}{}", held("argumentPackage"))),
            "case",
        ),
    ] {
        let doc = assert_preserved_at(row, what, &template, holder);
        assert!(
            doc.get("x").is_none() && doc.get("xi").is_none(),
            "{row}: {what}: read as an element"
        );
    }
}

#[test]
fn sacm23_ac_003_participants_read_in_attribute_and_element_form_in_order() {
    let row = AC_003;
    let attribute = read_ok(
        row,
        "participantPackage=\"c a\"",
        &b_in_case(" participantPackage=\"c a\"", ""),
    );
    let elements = read_ok(
        row,
        "participantPackage elements",
        &b_in_case(
            "",
            "<participantPackage xmi:idref=\"c\"/><participantPackage href=\"#a\"/>",
        ),
    );
    assert!(
        elements == attribute,
        "{row}: the element forms do not read == to the attribute"
    );
    assert_eq!(
        element(row, &attribute, "b").participant_packages(),
        list(&["c", "a"]).as_slice(),
        "{row}: participant_packages() in order"
    );
    let what = "an attribute and a participant element";
    let (input, marks) = marked(&b_in_case(
        &format!(" {MARK}participantPackage=\"a\""),
        &format!("{MARK}<participantPackage xmi:idref=\"c\"/>"),
    ));
    let error = refused(row, what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "b" && property == "participantPackage"
        ),
        "{row}: {what}: not RepeatedValue {{ b, participantPackage }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations()),
        (Code::MULTIPLICITY_VIOLATION, marks.as_slice()),
        "{row}: {what}: not SACM-MULT-001 at both occurrences"
    );
    let output = tolerant(row, what, &input);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|found| (
                found.code(),
                found.severity(),
                found.requirement_id().to_owned()
            ))
            .collect::<Vec<_>>(),
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "LIB-COMPAT-001".to_owned()
        )],
        "{row}: {what}: a tolerant read does not give one SACM-MULT-001 Warning"
    );
    let what = "participantPackage=\"a a c\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"a a c\"", ""));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["a", "a", "c"]).as_slice(),
        "{row}: {what}: not held as three ids"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "9.4",
            &["b", "a"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 9.4"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "b").ends_with(" participantPackage=\"a a c\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_ac_003_too_few_participants_read_and_write_back() {
    let row = AC_003;
    for (what, attribute) in [
        ("no participant", ""),
        ("one participant", " participantPackage=\"a\""),
        ("one distinct participant", " participantPackage=\"c c\""),
    ] {
        let doc = read_ok(row, what, &b_in_case(attribute, ""));
        let found: Vec<Finding> = findings(&doc)
            .into_iter()
            .filter(|found| found.2 != "SACM23-VAL-022")
            .collect();
        assert_eq!(
            found,
            [finding_of(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                "SACM23-VAL-014",
                "9.4",
                &["b"]
            )],
            "{row}: {what}: validate does not give one SACM-MULT-001 (SACM23-VAL-014)"
        );
        for mode in MODES {
            let text = round_trip(row, what, &doc, mode);
            assert!(
                line_with(row, &text, "b").ends_with(&format!("xmi:id=\"b\"{attribute}>")),
                "{row}: {what}: a {mode:?} write changed the start tag\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_ac_003_dangling_and_wrongly_typed_participants() {
    let row = AC_003;
    let what = "participantPackage=\"a gone\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"a gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "9.4",
            &["b", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 9.4"
    );
    assert_eq!(
        argument_of(&found[0], "type"),
        Some("AssuranceCasePackage"),
        "{row}: {what}: the end's type"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "b", " participantPackage=\"a gone\">");
    let doc = read_ok(
        row,
        "participants of every AssuranceCasePackage kind",
        &case_with(
            &interface("i", " implements=\"a\"", ""),
            &format!(
                "{}{}",
                binding("b", " participantPackage=\"case a i b2\"", ""),
                binding("b2", " participantPackage=\"a c\"", "")
            ),
        ),
    );
    // No reference finding; since slice 0068 the binding b2 among them gives
    // one SACM-PKG-001 Warning (D-BIND-PART, SACM23-VAL-014).
    assert_eq!(
        findings(&doc),
        [finding_of(
            Code::PACKAGE_CONTENT_INVALID,
            Severity::Warning,
            "SACM23-VAL-014",
            "9.4",
            &["b", "b2"]
        )],
        "{row}: participants of every AssuranceCasePackage kind"
    );
    for (target, class) in [
        ("terms", "TerminologyPackage"),
        ("p", "ArgumentPackage"),
        ("p_api", "ArgumentPackageInterface"),
        ("cl", "Claim"),
    ] {
        let what = format!("participantPackage=\"a {target}\" (a {class})");
        let doc = read_ok(
            row,
            &what,
            &b_in_case(&format!(" participantPackage=\"a {target}\""), ""),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "9.4",
                &["b", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 9.4"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("AssuranceCasePackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
    }
}

#[test]
fn sacm23_ac_003_writes_attribute_order_and_the_golden_file() {
    let row = AC_003;
    let what = "a binding with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case_with(
            &interface("i", " implements=\"a\"", ""),
            &binding(
                "b",
                " participantPackage=\"c i\" interface=\"i\" abstractForm=\"b\" citedElement=\"b\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"B-1\"",
                "",
            ),
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageBinding\" xmi:id=\"b\" \
         gid=\"B-1\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"b\" \
         abstractForm=\"b\" interface=\"i\" participantPackage=\"c i\">",
        "{row}: {what}: not written in order\n{text}"
    );
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageBinding\" \
             xmi:id=\"binding\" participantPackage=\"braking_case_api vehicle_case_api\">"
        ),
        "{row}: the golden file has no binding line"
    );
}

#[test]
fn sacm23_ac_003_reference_end_tables_bounds_and_serde() {
    let row = AC_003;
    let end = ReferenceEnd::ParticipantPackage;
    assert_eq!(
        (
            end.bounds(ElementKind::AssuranceCasePackageBinding),
            end.bounds(ElementKind::AssuranceCasePackageInterface),
            end.bounds(ElementKind::AssuranceCasePackage),
        ),
        (Some((2, None)), None, None),
        "{row}: the bounds"
    );
    assert!(
        end.applies_to(ElementKind::AssuranceCasePackageBinding),
        "{row}: participantPackage does not apply to an AssuranceCasePackageBinding"
    );
    #[cfg(feature = "serde")]
    {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, "\"participantPackage\"", "{row}: the serde name");
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
#[test]
fn sacm23_ac_003_create_reports_what_it_holds_and_checks_in_order() {
    let row = AC_003;
    let operation = CreateAssuranceCasePackageBinding::root(
        named("The braking case supports the vehicle case"),
        ["a", "c"],
    );
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.name(),
            strs(operation.participant_packages())
        ),
        (
            None,
            None,
            &named("The braking case supports the vehicle case"),
            vec!["a", "c"]
        ),
        "{row}: root()"
    );
    let operation =
        CreateAssuranceCasePackageBinding::new("case", named("B"), ["c", "a", "c"]).with_id("b");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            strs(operation.participant_packages())
        ),
        (Some("case"), Some("b"), vec!["c", "a", "c"]),
        "{row}: new() and with_id() keep the participants in order with repeats"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateAssuranceCasePackageBinding",
        "{row}: operation_name()"
    );
    let mut doc = read_ok(row, "the case", &case_with("", ""));
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            strs(mutation.changed_ids()),
            parent_of(row, &doc, "b"),
            element(row, &doc, "b").kind(),
            element(row, &doc, "b").participant_packages(),
        ),
        (
            vec!["b"],
            vec![],
            Some("case".to_owned()),
            ElementKind::AssuranceCasePackageBinding,
            list(&["c", "a", "c"]).as_slice()
        ),
        "{row}: the created binding"
    );
    let text = write_in(row, "the created binding", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<assuranceCasePackage xmi:type=\"sacm:AssuranceCasePackageBinding\" xmi:id=\"b\" \
         participantPackage=\"c a c\">",
        "{row}: the created binding's start tag\n{text}"
    );
    let mut doc = read_ok(row, "the create case", &create_case());
    let mut generated = Vec::new();
    for parent in LEGAL_PARENTS {
        let mutation = applied(
            row,
            &mut doc,
            CreateAssuranceCasePackageBinding::new(parent, named("N"), ["a", "c"]),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let root = applied(
        row,
        &mut doc,
        CreateAssuranceCasePackageBinding::root(named("N"), ["a", "c"]),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=8)
            .map(|n| format!("assurance_case_package_binding_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (parent, parent_kind) in ILLEGAL_PARENTS {
        let error = refuse(
            row,
            &mut doc,
            CreateAssuranceCasePackageBinding::new(parent, named("N"), ["a", "c"]),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::AssuranceCasePackageBinding
            ),
            "{row}: a binding under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, participants: &[&str]| {
        CreateAssuranceCasePackageBinding::new(parent, name, participants.iter().copied())
    };
    let cases: Vec<(&str, CreateAssuranceCasePackageBinding, _)> = vec![
        (
            "a missing parent",
            create("nowhere", unnamed.clone(), &["a b"]).with_id("1b"),
            k("ParentNotFound", None, None),
        ),
        (
            "an illegal parent",
            create("p", unnamed.clone(), &["a b"]).with_id("1b"),
            k("InvalidParent", None, None),
        ),
        (
            "an invalid id",
            create("case", unnamed.clone(), &["a b"]).with_id("1b"),
            k("InvalidId", None, None),
        ),
        (
            "a duplicate id",
            create("case", unnamed.clone(), &["a b"]).with_id("blade"),
            k("DuplicateId", None, None),
        ),
        (
            "one participant, before 5a",
            create("case", unnamed.clone(), &["a b"]),
            k("TooFewReferences", Some("participantPackage"), None),
        ),
        (
            "5a: a name with no expression before an invalid participant",
            create("case", unnamed.clone(), &["a b", "nope"]),
            k("TooFewReferences", Some("expression"), None),
        ),
        (
            "a participant with white space before a missing one",
            create("case", named("N"), &["nope", "a b"]),
            k("InvalidReference", Some("participantPackage"), Some("a b")),
        ),
        (
            "a missing participant",
            create("case", named("N"), &["terms", "nope"]),
            k(
                "ReferenceNotFound",
                Some("participantPackage"),
                Some("nope"),
            ),
        ),
        (
            "a TerminologyPackage participant",
            create("case", named("N"), &["a", "terms"]),
            k(
                "WrongReferenceType",
                Some("participantPackage"),
                Some("terms"),
            ),
        ),
        (
            "an ArgumentPackage participant",
            create("case", named("N"), &["p", "c"]),
            k("WrongReferenceType", Some("participantPackage"), Some("p")),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "AssuranceCasePackage", "{row}: {what}: expected");
        }
    }
    let error = refuse(row, &mut doc, create("case", named("N"), &["a", "a"]));
    assert!(
        matches!(
            &error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{row}: [a, a]: {error:?}"
    );
    let next = applied(row, &mut doc, create("case", named("N"), &["a", "c"]));
    assert_eq!(
        strs(next.created_ids()),
        ["assurance_case_package_binding_9"],
        "{row}: a refusal moved the next generated id"
    );
}

#[test]
fn sacm23_ac_003_reference_edits_count_distinct_participants() {
    let row = AC_003;
    let end = ReferenceEnd::ParticipantPackage;
    let too_few = |error: &EditError, given: usize| {
        matches!(
            error,
            EditError::TooFewReferences { end: "participantPackage", given: g, required: 2, .. }
                if *g == given
        )
    };
    let mut doc = read_ok(
        row,
        "b of [a, c]",
        &b_in_case(" participantPackage=\"a c\"", ""),
    );
    for (what, ids) in [("[a]", vec!["a"]), ("[a, a]", vec!["a", "a"])] {
        let error = refuse(row, &mut doc, SetReferences::new("b", end, ids));
        assert!(too_few(&error, 1), "{row}: SetReferences {what}: {error:?}");
    }
    let error = refuse(row, &mut doc, RemoveReference::new("b", end, "c"));
    assert!(too_few(&error, 1), "{row}: removing one of two: {error:?}");
    applied(row, &mut doc, AddReference::new("b", end, "a"));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["a", "c", "a"]).as_slice(),
        "{row}: adding a held id"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "9.4",
            &["b", "a"]
        )],
        "{row}: the repeat warns"
    );
    let mut doc = read_ok(row, "b of [a]", &b_in_case(" participantPackage=\"a\"", ""));
    let error = refuse(row, &mut doc, AddReference::new("b", end, "a"));
    assert!(
        too_few(&error, 1),
        "{row}: AddReference a onto [a]: {error:?}"
    );
    applied(row, &mut doc, AddReference::new("b", end, "c"));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["a", "c"]).as_slice(),
        "{row}: AddReference c onto [a]"
    );
    let error = refuse(row, &mut doc, SetReferences::new("b", end, ["a", "terms"]));
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "participantPackage",
                expected: "AssuranceCasePackage",
                ..
            }
        ),
        "{row}: participants [a, terms]: {error:?}"
    );
}

#[test]
fn sacm23_ac_003_compare_reports_participant_packages() {
    let row = AC_003;
    let left = read_ok(row, "left", &b_in_case(" participantPackage=\"a c\"", ""));
    let right = read_ok(row, "right", &b_in_case(" participantPackage=\"c a\"", ""));
    let found = semantic_compare(&left, &right);
    assert_eq!(
        found,
        [Difference::ParticipantPackages {
            id: ElementId::from("b"),
            left: list(&["a", "c"]),
            right: list(&["c", "a"]),
        }],
        "{row}: not one ParticipantPackages difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"b\" differs in participantPackage: [\"a\", \"c\"] on the left, [\"c\", \"a\"] \
         on the right",
        "{row}: the Display"
    );
}

#[test]
fn sacm23_ac_003_move_copy_and_the_element_edits_apply() {
    let row = AC_003;
    let mut doc = read_ok(
        row,
        "b in case",
        &b_in_case(
            " participantPackage=\"a c\"",
            &format!(
                "{}{}",
                interface("b1", " implements=\"a\"", ""),
                interface("b2", " implements=\"c\"", "")
            ),
        ),
    );
    applied(row, &mut doc, SetName::new("b", named("Renamed")));
    applied(row, &mut doc, SetGid::new("b", "B-1"));
    applied(row, &mut doc, SetFlag::new("b", Flag::IsCitation, true));
    applied(row, &mut doc, ReorderElements::new("b", ["b2", "b1"]));
    assert_eq!(children(row, &doc, "b"), ["b2", "b1"], "{row}: the reorder");
    let held = element(row, &doc, "b").participant_packages().to_vec();
    let before = findings(&doc);
    for (what, operation, parent) in [
        ("to the roots", MoveElement::root("b"), None),
        ("into a", MoveElement::new("b", "a"), Some("a")),
        ("into arts", MoveElement::new("b", "arts"), Some("arts")),
        (
            "back into case",
            MoveElement::new("b", "case"),
            Some("case"),
        ),
    ] {
        applied(row, &mut doc, operation);
        assert_eq!(
            (
                parent_of(row, &doc, "b").as_deref(),
                element(row, &doc, "b").participant_packages().to_vec(),
                findings(&doc)
            ),
            (parent, held.clone(), before.clone()),
            "{row}: a move {what} changed a reference or a finding"
        );
    }
    for parent in ["terms", "p"] {
        let error = refuse(row, &mut doc, MoveElement::new("b", parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent {
                    kind: ElementKind::AssuranceCasePackageBinding,
                    ..
                }
            ),
            "{row}: an AssuranceCasePackageBinding into {parent}: {error:?}"
        );
    }
    let copy = CopyElements::new(&doc, ["b"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        element(row, &doc, &copy_of(&pairs, "b")).participant_packages(),
        list(&["a", "c"]).as_slice(),
        "{row}: the copy of b keeps its participants as written"
    );
    let mut doc = read_ok(
        row,
        "b of [a, a]",
        &b_in_case(" participantPackage=\"a a\"", ""),
    );
    let copy = CopyElements::new(&doc, ["b"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let error = refuse(row, &mut doc, copy);
    assert!(
        matches!(
            &error,
            EditError::TooFewReferences {
                end: "participantPackage",
                given: 1,
                required: 2,
                ..
            }
        ),
        "{row}: the copy of a binding of [a, a]: {error:?}"
    );
}

#[test]
fn sacm23_ac_003_a_binding_left_below_two_participants_is_deleted_as_lib_edit_019_says() {
    let row = AC_003;
    let mut doc = fixture_a(row);
    let operation = scrubbing("ci");
    assert_eq!(
        dangling(row, &doc, operation.clone()),
        [dangles("b", "participantPackage", "ci", true)],
        "{row}: the dangling references of deleting ci"
    );
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["ci", "b"], Vec::<&str>::new()),
        "{row}: b, left with ai alone, goes with ci"
    );
    assert_eq!(
        element(row, &doc, "ai").implements().map(ElementId::as_str),
        Some("a"),
        "{row}: ai is unchanged"
    );
}
