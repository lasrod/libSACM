//! Tests for `TerminologyPackageInterface` and `TerminologyPackageBinding`,
//! with the ends `interface`, `implements` and `participantPackage`
//! (SACM23-TERM-007, SACM23-TERM-008; formal/23-05-08 §10.5
//! "`implements:TerminologyPackage[1]` - the TerminologyPackage that the
//! `TerminologyPackageInterface` declares.", §10.6 "Superclass
//! TerminologyPackage"; ptc/22-03-13 `TerminologyPackage::interface` [0..*]
//! and `TerminologyPackageBinding::participantPackage` [2..*]; the M5
//! decisions Q1, Q2, Q3, Q7, Q8 and Q9; ADR 0039).
//!
//! Both kinds are `TerminologyPackage`s by inheritance (the model, and
//! §10.5's first sentence; D-TEXT-MODEL): they read typed in
//! `terminologyElement` of any TerminologyPackage kind, in `terminologyPackage`
//! of any AssuranceCasePackage kind, in `artifactElement` of an ArtifactPackage
//! and as roots, hold their children in `terminologyElement`, and carry the
//! package's `interface` end. In an ArgumentPackage's `argumentElement` and an
//! AssuranceCasePackage's `assuranceCasePackage`, which admit no
//! TerminologyPackage, they are kept whole as preserved content of the holder.
//! The ends are kept as written and never synced (Q1); since slice 0068 a
//! missing `implements` and fewer than two participants each give one
//! SACM-MULT-001 (SACM23-VAL-014), an interface outside the package it
//! implements one SACM-PKG-002 Warning (SACM23-VAL-023), and content that is
//! no citation one SACM-PKG-001 (SACM23-VAL-013); a lower bound counts
//! distinct ids (Q7).
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
    self, AddReference, CopyElements, CreateTerm, CreateTerminologyPackage, DeleteElement,
    EditError, MoveElement, Mutation, Operation, RemoveReference, ReorderElements, SetFlag, SetGid,
    SetName, SetReferences,
};
use sacm::model::{Flag, LangString, ReferenceEnd};

// The creates of slice 0067 (SACM23-TERM-007, SACM23-TERM-008).
use sacm::edit::{CreateTerminologyPackageBinding, CreateTerminologyPackageInterface};

const TERM_007: &str = "SACM23-TERM-007";
const TERM_008: &str = "SACM23-TERM-008";

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

/// The terminology fixture of slice 0067 and its golden output.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/terminology-package-interfaces-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/terminology-package-interfaces.golden.sacm.xmi"
);

const INTERFACE: &str = "TerminologyPackageInterface";
const BINDING: &str = "TerminologyPackageBinding";

// --- Inputs --------------------------------------------------------------------------

/// An element of the class `class`, `id`, typed in the role `role`, with
/// `attributes`, named after its id, holding `body`.
fn typed_in(role: &str, class: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// A `TerminologyPackageInterface` `id` in the role `role`.
fn interface_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    typed_in(role, INTERFACE, id, attributes, body)
}

/// A `TerminologyPackageBinding` `id` in the role `role`.
fn binding_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    typed_in(role, BINDING, id, attributes, body)
}

/// A `TerminologyPackageInterface` `id` in a TerminologyPackage's
/// `terminologyElement`.
fn interface(id: &str, attributes: &str, body: &str) -> String {
    interface_in("terminologyElement", id, attributes, body)
}

/// A `TerminologyPackageBinding` `id` in an AssuranceCasePackage's
/// `terminologyPackage`.
fn binding(id: &str, attributes: &str, body: &str) -> String {
    binding_in("terminologyPackage", id, attributes, body)
}

/// A Term `id` in `terminologyElement`, with the value `id` and `attributes`.
fn term(id: &str, attributes: &str) -> String {
    typed_in(
        "terminologyElement",
        "Term",
        id,
        &format!(" value=\"{id}\"{attributes}"),
        "",
    )
}

/// An `AssuranceCasePackageInterface` `id` in `assuranceCasePackage`,
/// implementing `case`, holding `body`.
fn case_interface(id: &str, body: &str) -> String {
    typed_in(
        "assuranceCasePackage",
        "AssuranceCasePackageInterface",
        id,
        " implements=\"case\"",
        body,
    )
}

/// The AssuranceCasePackage `case`, one element per line:
/// - the TerminologyPackage `t`, with `t_attributes`, holding the Term
///   `blade`, then `t_body`;
/// - the TerminologyPackage `u`, holding the Term `cutter`;
/// - the ArgumentPackage `p`, holding the Claim `c` and the
///   `ArgumentPackageInterface` `p_api` (implementing `p`), then `p_body`;
/// - the ArtifactPackage `arts`, holding the Artifact `report`, then
///   `arts_body`;
/// - then `extra`.
fn case(t_attributes: &str, t_body: &str, p_body: &str, arts_body: &str, extra: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">
  <name content=\"Case\"/>
  <terminologyPackage xmi:id=\"t\"{t_attributes}><name content=\"T\"/>
    {blade}
    {t_body}
  </terminologyPackage>
  <terminologyPackage xmi:id=\"u\"><name content=\"U\"/>
    {cutter}
  </terminologyPackage>
  <argumentPackage xmi:id=\"p\"><name content=\"P\"/>
    <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"C\"/></argumentElement>
    <argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"p_api\" implements=\"p\"><name content=\"P API\"/></argumentElement>
    {p_body}
  </argumentPackage>
  <artifactPackage xmi:id=\"arts\"><name content=\"Arts\"/>
    <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"report\"><name content=\"Report\"/></artifactElement>
    {arts_body}
  </artifactPackage>
  {extra}
</sacm:AssuranceCasePackage>",
        blade = term("blade", ""),
        cutter = term("cutter", ""),
    )
}

/// [`case`] with only `t_body` and `extra`.
fn case_with(t_body: &str, extra: &str) -> String {
    case("", t_body, "", "", extra)
}

/// [`case`] with the interface `i` in `t`, with `attributes` and `body`.
fn i_in_t(attributes: &str, body: &str) -> String {
    case_with(&interface("i", attributes, body), "")
}

/// [`case`] with the binding `b` in `case`, with `attributes` and `body`.
fn b_in_case(attributes: &str, body: &str) -> String {
    case_with("", &binding("b", attributes, body))
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!(
            "SACM23-TERM-007: setup: SourceLocation::new({line}, {column}) refused a 1-based position"
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
/// ids given: for the findings of slice 0068 (SACM23-VAL-013, -014, -023).
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

/// The SACM-PKG-002 Warning of `interface` outside the package `package` it
/// implements, clause 10.5 (SACM23-VAL-023).
fn placement(interface: &str, package: &str) -> Finding {
    finding_of(
        Code::new("SACM-PKG-002"),
        Severity::Warning,
        "SACM23-VAL-023",
        "10.5",
        &[interface, package],
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
        .unwrap_or_else(|error| panic!("SACM23-TERM-007: cannot read {path}: {error}"))
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
        None => panic!("SACM23-TERM-007: {source:?} was not copied: {pairs:?}"),
    }
}

/// The case of the create tests: [`case`] with a Note `t_note` in `t`, the
/// `ArgumentPackageBinding` `p_b` in `p`, the `ArtifactPackageInterface`
/// `arts_api` and the `ArtifactPackageBinding` `arts_b` in `arts`, and in
/// `case` the binding `b`, the `AssuranceCasePackageInterface` `ci` and the
/// `AssuranceCasePackageBinding` `cb`.
fn create_case() -> String {
    case(
        "",
        "<note xmi:id=\"t_note\"><content><value content=\"N\"/></content></note>",
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
            "{}{}{}",
            binding("b", " participantPackage=\"t u\"", ""),
            case_interface("ci", ""),
            typed_in(
                "assuranceCasePackage",
                "AssuranceCasePackageBinding",
                "cb",
                " participantPackage=\"case ci\"",
                ""
            ),
        ),
    )
}

/// The legal parents of a Terminology interface or binding in
/// [`create_case`]: an AssuranceCasePackage, a TerminologyPackage and an
/// ArtifactPackage, each of every kind.
const LEGAL_PARENTS: [&str; 8] = ["case", "ci", "cb", "t", "b", "arts", "arts_api", "arts_b"];

/// The parents of [`create_case`] that are `InvalidParent` for a
/// Terminology interface or binding.
const ILLEGAL_PARENTS: [(&str, ElementKind); 7] = [
    ("p", ElementKind::ArgumentPackage),
    ("p_api", ElementKind::ArgumentPackageInterface),
    ("p_b", ElementKind::ArgumentPackageBinding),
    ("blade", ElementKind::Term),
    ("c", ElementKind::Claim),
    ("report", ElementKind::Artifact),
    ("t_note", ElementKind::Note),
];

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

/// Fixture T of the design, built with edits, in pre-order `t, u, ti, tx,
/// ui, b`: the empty root `TerminologyPackage`s `t` and `u`; the root
/// interface `ti` implementing `t`, holding the Term `tx` (value `x`); the
/// empty root interface `ui` implementing `u`; the empty root binding `b` of
/// `[ti, ui]`.
fn fixture_t(row: &str) -> Document {
    let mut doc = Document::new();
    for package in ["t", "u"] {
        applied(
            row,
            &mut doc,
            CreateTerminologyPackage::root(named(package)).with_id(package),
        );
    }
    applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::root(named("ti"), "t").with_id("ti"),
    );
    applied(
        row,
        &mut doc,
        CreateTerm::new("ti", named("tx"), "x").with_id("tx"),
    );
    applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::root(named("ui"), "u").with_id("ui"),
    );
    applied(
        row,
        &mut doc,
        CreateTerminologyPackageBinding::root(named("b"), ["ti", "ui"]).with_id("b"),
    );
    let order: Vec<&str> = doc
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        order,
        ["t", "u", "ti", "tx", "ui", "b"],
        "{row}: setup: fixture T is not in the pre-order of the design"
    );
    doc
}

/// The delete of `element` with `ScrubReferences`, `DeleteRecursively` and
/// `ApplyReferencePolicy`.
fn scrubbing(element: &str) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(ScrubReferences)
        .with_package_policy(DeleteRecursively)
        .with_cross_package_policy(ApplyReferencePolicy)
}

// =====================================================================================
// SACM23-TERM-007: TerminologyPackageInterface
// =====================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_term_007_reads_typed_in_every_role_and_as_a_root() {
    let row = TERM_007;
    let kind = ElementKind::TerminologyPackageInterface;
    let implementing = " implements=\"t\"";
    let places: Vec<Place> = vec![
        (
            "terminologyElement in a TerminologyPackage",
            i_in_t(implementing, ""),
            Some("t"),
            Some("terminologyElement"),
        ),
        (
            "terminologyElement in a TerminologyPackageInterface",
            case_with(
                &interface("ti", implementing, &interface("i", implementing, "")),
                "",
            ),
            Some("ti"),
            Some("terminologyElement"),
        ),
        (
            "terminologyElement in a TerminologyPackageBinding",
            case_with(
                "",
                &binding(
                    "b",
                    " participantPackage=\"t u\"",
                    &interface("i", implementing, ""),
                ),
            ),
            Some("b"),
            Some("terminologyElement"),
        ),
        (
            "terminologyPackage in an AssuranceCasePackage",
            case_with(
                "",
                &interface_in("terminologyPackage", "i", implementing, ""),
            ),
            Some("case"),
            Some("terminologyPackage"),
        ),
        (
            "terminologyPackage in an AssuranceCasePackageInterface",
            case_with(
                "",
                &case_interface(
                    "ci",
                    &interface_in("terminologyPackage", "i", implementing, ""),
                ),
            ),
            Some("ci"),
            Some("terminologyPackage"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                "",
                &interface_in("artifactElement", "i", implementing, ""),
                "",
            ),
            Some("arts"),
            Some("artifactElement"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:TerminologyPackageInterface{NS} xmi:id=\"i\" implements=\"i\">\
                 <name content=\"i\"/></sacm:TerminologyPackageInterface>"
            ),
            None,
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:TerminologyPackage xmi:id=\"t\"><name content=\"T\"/></sacm:TerminologyPackage>\
                 <sacm:TerminologyPackageInterface xmi:id=\"i\" implements=\"t\"><name content=\"i\"/>\
                 </sacm:TerminologyPackageInterface></xmi:XMI>"
            ),
            None,
            None,
        ),
    ];
    assert_reads_in(row, INTERFACE, kind, "i", &places);
    assert!(
        kind.is_kind_of(ElementKind::TerminologyPackage),
        "{row}: a TerminologyPackageInterface is no TerminologyPackage"
    );
    // Its name, utility children, gid, flags, citedElement and abstractForm,
    // and its children in terminologyElement.
    let what = "an interface with every inherited feature";
    let doc = read_ok(
        row,
        what,
        &i_in_t(
            " implements=\"t\" gid=\"I-1\" isCitation=\"true\" citedElement=\"u\" \
             isAbstract=\"true\" abstractForm=\"i\"",
            &format!(
                "<description xmi:id=\"i_d\"><content><value content=\"D\"/></content></description>\
                 <implementationConstraint xmi:id=\"i_ic\"/>\
                 <note xmi:id=\"i_n\"><content><value content=\"N\"/></content></note>\
                 <taggedValue xmi:id=\"i_tv\"><key><value content=\"K\"/></key></taggedValue>\
                 {}{}{}",
                term("it", " isCitation=\"true\" citedElement=\"blade\""),
                typed_in("terminologyElement", "Category", "ik", "", ""),
                typed_in("terminologyElement", "TerminologyPackage", "ip", "", ""),
            ),
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
        (Some("i"), Some("I-1"), true, Some("u"), true, Some("i")),
        "{row}: {what}: the inherited features"
    );
    assert_eq!(
        children(row, &doc, "i")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["i_d", "i_ic", "i_n", "i_tv", "it", "ik", "ip"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "{row}: {what}: the children"
    );
    assert_eq!(
        (
            element(row, &doc, "it").kind(),
            element(row, &doc, "ik").kind(),
            element(row, &doc, "ip").kind(),
            element(row, &doc, "i_n").kind()
        ),
        (
            ElementKind::Term,
            ElementKind::Category,
            ElementKind::TerminologyPackage,
            ElementKind::Note
        ),
        "{row}: {what}: the kinds of the children"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "it").starts_with("<terminologyElement xmi:type=\"sacm:Term\""),
        "{row}: {what}: the interface's Term is not written in terminologyElement\n{text}"
    );
    // In an ArgumentPackage's argumentElement and an AssuranceCasePackage's
    // assuranceCasePackage, which admit no TerminologyPackage: kept whole as
    // preserved content of the holder.
    let held = interface_in("argumentElement", "x", implementing, &term("xt", ""));
    let template = case("", "", &format!("{MARK}{held}"), "", "");
    let doc = assert_preserved_at(row, "an interface in argumentElement", &template, "p");
    assert!(
        doc.get("x").is_none() && doc.get("xt").is_none(),
        "{row}: the interface in argumentElement was read as an element"
    );
    let held = interface_in("assuranceCasePackage", "x", implementing, &term("xt", ""));
    let template = case_with("", &format!("{MARK}{held}"));
    let doc = assert_preserved_at(
        row,
        "an interface in assuranceCasePackage",
        &template,
        "case",
    );
    assert!(
        doc.get("x").is_none() && doc.get("xt").is_none(),
        "{row}: the interface in assuranceCasePackage was read as an element"
    );
    // An untyped terminologyPackage is a plain TerminologyPackage; an untyped
    // terminologyElement is MissingType.
    let doc = read_ok(
        row,
        "an untyped terminologyPackage",
        &case_with(
            "",
            "<terminologyPackage xmi:id=\"x\"><name content=\"x\"/></terminologyPackage>",
        ),
    );
    assert_eq!(
        element(row, &doc, "x").kind(),
        ElementKind::TerminologyPackage,
        "{row}: an untyped terminologyPackage is not a plain TerminologyPackage"
    );
    let input = i_in_t(
        implementing,
        "<terminologyElement xmi:id=\"x\"><name content=\"x\"/></terminologyElement>",
    );
    let error = refused(
        row,
        "an untyped terminologyElement in the interface",
        &input,
    );
    assert!(
        matches!(
            &error,
            ReadError::MissingType { role, declared_type, .. }
                if role == "terminologyElement" && declared_type == "TerminologyElement"
        ),
        "{row}: an untyped terminologyElement in the interface is not MissingType: {error:?}"
    );
    // The §10.4 spelling TerminologyElement under the interface reads in
    // tolerant mode with one SACM-XMI-013 (SACM23-XMI-028).
    let what = "the spelling TerminologyElement under the interface";
    let (input, marks) = marked(&i_in_t(
        implementing,
        &format!(
            "{MARK}<TerminologyElement xmi:type=\"sacm:Term\" xmi:id=\"x\" value=\"x\">\
             <name content=\"x\"/></TerminologyElement>"
        ),
    ));
    let output = tolerant(row, what, &input);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|found| (
                found.code(),
                found.severity(),
                found.location(),
                found.affected().to_vec()
            ))
            .collect::<Vec<_>>(),
        [(
            Code::XMI_SPELLING_VARIANT,
            Severity::Warning,
            marks.first().copied(),
            list(&["i"])
        )],
        "{row}: {what}: not one SACM-XMI-013 Warning at it affecting i"
    );
    assert_eq!(
        (
            element(row, output.document(), "x").kind(),
            parent_of(row, output.document(), "x").as_deref()
        ),
        (ElementKind::Term, Some("i")),
        "{row}: {what}: the Term is not read as a child of i"
    );
}

#[test]
fn sacm23_term_007_implements_and_interface_read_in_attribute_and_element_form() {
    let row = TERM_007;
    let attribute = read_ok(row, "implements=\"t\"", &i_in_t(" implements=\"t\"", ""));
    for (what, body) in [
        (
            "<implements xmi:idref=\"t\"/>",
            "<implements xmi:idref=\"t\"/>",
        ),
        ("<implements href=\"#t\"/>", "<implements href=\"#t\"/>"),
    ] {
        let doc = read_ok(row, what, &i_in_t("", body));
        assert!(
            doc == attribute,
            "{row}: {what} does not read == to the attribute"
        );
    }
    assert_eq!(
        element(row, &attribute, "i")
            .implements()
            .map(ElementId::as_str),
        Some("t"),
        "{row}: implements() of i"
    );
    let both = format!(
        "{}{}",
        interface("i1", " implements=\"t\"", ""),
        interface("i2", " implements=\"t\"", "")
    );
    let on_t = read_ok(
        row,
        "interface=\"i1 i2\" on t",
        &case(" interface=\"i1 i2\"", &both, "", "", ""),
    );
    let elements = read_ok(
        row,
        "interface elements on t",
        &case_with(
            &format!("<interface xmi:idref=\"i1\"/><interface href=\"#i2\"/>{both}"),
            "",
        ),
    );
    assert!(
        elements == on_t,
        "{row}: interface elements on t do not read == to the attribute"
    );
    let inherited = read_ok(
        row,
        "interface=\"i1 i2\" on an interface",
        &i_in_t(" implements=\"t\" interface=\"i1 i2\"", &both),
    );
    let inherited_elements = read_ok(
        row,
        "interface elements on an interface",
        &i_in_t(
            " implements=\"t\"",
            &format!("<interface xmi:idref=\"i1\"/><interface xmi:idref=\"i2\"/>{both}"),
        ),
    );
    assert!(
        inherited_elements == inherited,
        "{row}: interface elements on an interface do not read == to the attribute"
    );
    assert_eq!(
        (
            element(row, &on_t, "t").interfaces(),
            element(row, &inherited, "i").interfaces(),
        ),
        (
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
fn sacm23_term_007_repeated_empty_and_qualified_implements_follow_the_reference_rules() {
    let row = TERM_007;
    for (what, attributes, body, first) in [
        (
            "implements=\"t u\"",
            format!(" implements=\"{MARK}t {MARK}u\""),
            String::new(),
            None,
        ),
        (
            "implements as an attribute and an element",
            format!(" {MARK}implements=\"t\""),
            format!("{MARK}<implements xmi:idref=\"u\"/>"),
            Some("t"),
        ),
    ] {
        let (input, marks) = marked(&i_in_t(&attributes, &body));
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
        ("sacm:implements", format!(" {MARK}sacm:implements=\"t\"")),
        (
            "v:interface on the interface",
            format!(" implements=\"t\" {MARK}v:interface=\"i\""),
        ),
    ] {
        assert_preserved_at(row, what, &i_in_t(&attribute, ""), "i");
    }
    // Moved from interface_ends.rs: an interface in another namespace is not
    // the end, on a TerminologyPackage as an attribute and as an element.
    assert_preserved_at(
        row,
        "v:interface=\"i\" on t",
        &case(&format!(" {MARK}v:interface=\"i\""), "", "", "", ""),
        "t",
    );
    assert_preserved_at(
        row,
        "a v:interface element on t",
        &case_with(&format!("{MARK}<v:interface xmi:idref=\"i\"/>"), ""),
        "t",
    );
    let what = "interface=\"i i\"";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i i\"",
            &interface("i", " implements=\"t\"", ""),
            "",
            "",
            "",
        ),
    );
    assert_eq!(
        element(row, &doc, "t").interfaces(),
        list(&["i", "i"]).as_slice(),
        "{row}: {what}: not held twice"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "10.4",
            &["t", "i"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 10.4"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "t").ends_with(" interface=\"i i\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_term_007_missing_implements_reads_and_writes_back_unchanged() {
    let row = TERM_007;
    let what = "an interface with no implements";
    let doc = read_ok(row, what, &i_in_t("", ""));
    assert_eq!(
        findings(&doc),
        [finding_of(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Error,
            "SACM23-VAL-014",
            "10.5",
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
fn sacm23_term_007_dangling_and_wrongly_typed_ends_report_their_clause_and_type() {
    let row = TERM_007;
    let what = "implements=\"gone\"";
    let doc = read_ok(row, what, &i_in_t(" implements=\"gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "10.5",
            &["i", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 10.5"
    );
    assert_eq!(
        found[0].message(),
        "TerminologyPackageInterface \"i\" has the implements \"gone\", which names no element \
         of the document (the end's type is TerminologyPackage)",
        "{row}: {what}: the message"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "i", " implements=\"gone\">");
    let with_binding = |attributes: &str| {
        case_with(
            &interface("i", attributes, ""),
            &binding("b", " participantPackage=\"t u\"", ""),
        )
    };
    // A TerminologyPackage kind gives no reference finding; since slice 0068
    // one i does not reside in gives one SACM-PKG-002 Warning
    // (SACM23-VAL-023).
    for (target, want) in [
        ("t", Vec::new()),
        ("u", vec![placement("i", "u")]),
        ("i", vec![placement("i", "i")]),
        ("b", vec![placement("i", "b")]),
    ] {
        let what = format!("implements=\"{target}\"");
        let doc = read_ok(
            row,
            &what,
            &with_binding(&format!(" implements=\"{target}\"")),
        );
        assert_eq!(
            findings(&doc),
            want,
            "{row}: {what}: validate reports another finding"
        );
    }
    for (target, class) in [
        ("blade", "Term"),
        ("p", "ArgumentPackage"),
        ("p_api", "ArgumentPackageInterface"),
        ("case", "AssuranceCasePackage"),
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
                "10.5",
                &["i", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 10.5"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("TerminologyPackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
    }
    for (target, class) in [
        ("blade", "Term"),
        ("b", "TerminologyPackageBinding"),
        ("u", "TerminologyPackage"),
        ("p_api", "ArgumentPackageInterface"),
    ] {
        let what = format!("interface=\"{target}\" (a {class}) on t");
        let doc = read_ok(
            row,
            &what,
            &case(
                &format!(" interface=\"{target}\""),
                "",
                "",
                "",
                &binding("b", " participantPackage=\"t u\"", ""),
            ),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "10.4",
                &["t", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 10.4"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("TerminologyPackageInterface"),
            "{row}: {what}: the end's type"
        );
    }
    let what = "interface=\"gone\" on t";
    let doc = read_ok(row, what, &case(" interface=\"gone\"", "", "", "", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "10.4",
            &["t", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 10.4"
    );
}

#[test]
fn sacm23_term_007_writes_attribute_order_and_the_golden_file() {
    let row = TERM_007;
    let what = "an interface with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i\" abstractForm=\"t\" gid=\"T-1\"",
            &interface(
                "i",
                " implements=\"t\" interface=\"i\" abstractForm=\"i\" citedElement=\"i\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"I-1\"",
                "",
            ),
            "",
            "",
            "",
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<terminologyElement xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"i\" \
         gid=\"I-1\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"i\" \
         abstractForm=\"i\" interface=\"i\" implements=\"t\">",
        "{row}: {what}: not written in order\n{text}"
    );
    assert_eq!(
        line_with(row, &text, "t"),
        "<terminologyPackage xmi:id=\"t\" gid=\"T-1\" abstractForm=\"t\" interface=\"i\">",
        "{row}: {what}: the package's interface is not written last\n{text}"
    );
    let what = "an interface in each role";
    let doc = read_ok(
        row,
        what,
        &case(
            "",
            &interface("i1", " implements=\"t\"", ""),
            "",
            &interface_in("artifactElement", "i3", " implements=\"t\"", ""),
            &interface_in("terminologyPackage", "i2", " implements=\"t\"", ""),
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    for (id, role) in [
        ("i1", "terminologyElement"),
        ("i2", "terminologyPackage"),
        ("i3", "artifactElement"),
    ] {
        assert_eq!(
            line_with(row, &text, id),
            format!(
                "<{role} xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"{id}\" \
                 implements=\"t\">"
            ),
            "{row}: {id} is not written typed in {role}\n{text}"
        );
    }
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<terminologyElement xmi:type=\"sacm:TerminologyPackageInterface\" \
             xmi:id=\"hazard_terms_api\" implements=\"hazard_terms\">"
        ),
        "{row}: the golden file has no interface line"
    );
}

#[test]
fn sacm23_term_007_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let row = TERM_007;
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
            children(row, &doc, "binding"),
            element(row, &doc, "binding_blade").kind(),
            element(row, &doc, "hazard_terms_api").kind(),
        ),
        (
            ElementKind::TerminologyPackageBinding,
            vec!["binding_blade".to_owned(), "binding_cutter".to_owned()],
            ElementKind::Term,
            ElementKind::TerminologyPackageInterface,
        ),
        "{TERM_008}: the fixture's binding and interface"
    );
    assert_eq!(
        (
            element(row, &doc, "hazard_terms").interfaces(),
            element(row, &doc, "design_terms").interfaces(),
            element(row, &doc, "hazard_terms_api")
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "binding").participant_packages(),
        ),
        (
            list(&["hazard_terms_api"]).as_slice(),
            list(&["design_terms_api"]).as_slice(),
            Some("hazard_terms"),
            list(&["hazard_terms_api", "design_terms_api"]).as_slice(),
        ),
        "{row}: the fixture's ends"
    );
}

#[test]
fn sacm23_term_007_reference_end_tables_bounds_and_serde() {
    let row = TERM_007;
    let (interface, implements, participants) = (
        ReferenceEnd::Interface,
        ReferenceEnd::Implements,
        ReferenceEnd::ParticipantPackage,
    );
    assert_eq!(
        (
            implements.bounds(ElementKind::TerminologyPackageInterface),
            interface.bounds(ElementKind::TerminologyPackage),
            interface.bounds(ElementKind::TerminologyPackageInterface),
            interface.bounds(ElementKind::TerminologyPackageBinding),
            implements.bounds(ElementKind::TerminologyPackage),
            interface.bounds(ElementKind::Term),
        ),
        (
            Some((1, Some(1))),
            Some((0, None)),
            Some((0, None)),
            Some((0, None)),
            None,
            None
        ),
        "{row}: the bounds on the Terminology kinds"
    );
    let packages = [
        ElementKind::AssuranceCasePackage,
        ElementKind::AssuranceCasePackageInterface,
        ElementKind::AssuranceCasePackageBinding,
        ElementKind::ArgumentPackage,
        ElementKind::ArgumentPackageInterface,
        ElementKind::ArgumentPackageBinding,
        ElementKind::ArtifactPackage,
        ElementKind::ArtifactPackageInterface,
        ElementKind::ArtifactPackageBinding,
        ElementKind::TerminologyPackage,
        ElementKind::TerminologyPackageInterface,
        ElementKind::TerminologyPackageBinding,
    ];
    let interfaces = [
        ElementKind::AssuranceCasePackageInterface,
        ElementKind::ArgumentPackageInterface,
        ElementKind::ArtifactPackageInterface,
        ElementKind::TerminologyPackageInterface,
    ];
    let bindings = [
        ElementKind::AssuranceCasePackageBinding,
        ElementKind::ArgumentPackageBinding,
        ElementKind::ArtifactPackageBinding,
        ElementKind::TerminologyPackageBinding,
    ];
    for &kind in ElementKind::ALL {
        assert_eq!(
            (
                interface.applies_to(kind),
                implements.applies_to(kind),
                participants.applies_to(kind)
            ),
            (
                packages.contains(&kind),
                interfaces.contains(&kind),
                bindings.contains(&kind)
            ),
            "{row}: applies_to of interface, implements and participantPackage on a {kind}"
        );
    }
    assert_eq!(
        ReferenceEnd::ALL.len(),
        20,
        "{row}: ReferenceEnd::ALL no longer has 20 entries"
    );
    #[cfg(feature = "serde")]
    for (end, name) in [(interface, "\"interface\""), (implements, "\"implements\"")] {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, name, "{row}: the serde name of {end:?}");
        let back: ReferenceEnd = serde_json::from_str(name)
            .unwrap_or_else(|error| panic!("{row}: {name} does not deserialize: {error}"));
        assert_eq!(back, end, "{row}: {name} reads back as another end");
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
#[test]
fn sacm23_term_007_create_reports_what_it_holds_and_checks_in_order() {
    let row = TERM_007;
    let operation =
        CreateTerminologyPackageInterface::root(named("Hazard terminology interface"), "t");
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.name(),
            operation.implements().as_str()
        ),
        (None, None, &named("Hazard terminology interface"), "t"),
        "{row}: root()"
    );
    let operation = CreateTerminologyPackageInterface::new("t", named("I"), "t").with_id("i");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            operation.implements().as_str()
        ),
        (Some("t"), Some("i"), "t"),
        "{row}: new() and with_id()"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateTerminologyPackageInterface",
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
            element(row, &doc, "t").interfaces().len(),
        ),
        (
            vec!["i"],
            vec![],
            Some("t".to_owned()),
            ElementKind::TerminologyPackageInterface,
            Some("t"),
            0
        ),
        "{row}: the created interface; t's interface end is unchanged"
    );
    let text = round_trip(row, "the created interface", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<terminologyElement xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"i\" \
         implements=\"t\">",
        "{row}: the created interface's start tag\n{text}"
    );
    // Every legal parent, the first generated interface and the roots.
    let mut doc = read_ok(row, "the create case", &create_case());
    let mut generated = Vec::new();
    for parent in LEGAL_PARENTS {
        let mutation = applied(
            row,
            &mut doc,
            CreateTerminologyPackageInterface::new(parent, named("N"), "u"),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let first = generated[0].clone();
    let mutation = applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::new(first.as_str(), named("N"), "u"),
    );
    generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    let root = applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::root(named("Root"), "u"),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=10)
            .map(|n| format!("terminology_package_interface_{n}"))
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
            CreateTerminologyPackageInterface::new(parent, named("N"), "u"),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::TerminologyPackageInterface
            ),
            "{row}: an interface under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, implements: &str| {
        CreateTerminologyPackageInterface::new(parent, name, implements)
    };
    let cases: Vec<(&str, CreateTerminologyPackageInterface, _)> = vec![
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
            create("t", unnamed.clone(), "a b").with_id("1i"),
            k("InvalidId", None, None),
        ),
        (
            "a duplicate id",
            create("t", unnamed.clone(), "a b").with_id("blade"),
            k("DuplicateId", None, None),
        ),
        (
            "5a: a name with no expression before an invalid implements",
            create("t", unnamed.clone(), "a b"),
            k("TooFewReferences", Some("expression"), None),
        ),
        (
            "an implements with white space",
            create("t", named("N").with_expression("nowhere"), "a b"),
            k("InvalidReference", Some("implements"), Some("a b")),
        ),
        (
            "a missing implements before the name's expression",
            create("t", named("N").with_expression("nowhere"), "nope"),
            k("ReferenceNotFound", Some("implements"), Some("nope")),
        ),
        (
            "the name's missing expression",
            create("t", named("N").with_expression("nowhere"), "u"),
            k("ReferenceNotFound", Some("expression"), Some("nowhere")),
        ),
        (
            "a Term as implements",
            create("t", named("N"), "blade"),
            k("WrongReferenceType", Some("implements"), Some("blade")),
        ),
        (
            "an ArgumentPackage as implements",
            create("t", named("N"), "p"),
            k("WrongReferenceType", Some("implements"), Some("p")),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "TerminologyPackage", "{row}: {what}: expected");
        }
    }
    let next = applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::new("t", named("N"), "u"),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["terminology_package_interface_11"],
        "{row}: a refusal moved the next generated id"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_term_007_reference_edits_keep_implements_at_exactly_one() {
    let row = TERM_007;
    let implements = ReferenceEnd::Implements;
    let mut doc = read_ok(row, "i in t", &i_in_t(" implements=\"t\"", ""));
    for (what, operation) in [
        (
            "SetReferences::clear",
            Operation::from(SetReferences::clear("i", implements)),
        ),
        (
            "RemoveReference",
            RemoveReference::new("i", implements, "t").into(),
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
            "SetReferences [t, u]",
            Operation::from(SetReferences::new("i", implements, ["t", "u"])),
        ),
        (
            "AddReference u onto t",
            AddReference::new("i", implements, "u").into(),
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
    let error = refuse(row, &mut doc, SetReferences::new("i", implements, ["p"]));
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "implements",
                expected: "TerminologyPackage",
                ..
            }
        ),
        "{row}: implements [p]: {error:?}"
    );
    // Before this slice the end was FeatureNotApplicable on a
    // TerminologyPackage.
    applied(
        row,
        &mut doc,
        SetReferences::new("t", ReferenceEnd::Interface, ["i"]),
    );
    assert_eq!(
        element(row, &doc, "t").interfaces(),
        list(&["i"]).as_slice(),
        "{row}: SetReferences(t, interface, [i])"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("t", ReferenceEnd::Interface, "p_api"),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "interface",
                expected: "TerminologyPackageInterface",
                ..
            }
        ),
        "{row}: interface [p_api] on t: {error:?}"
    );
    let error = refuse(row, &mut doc, SetReferences::new("t", implements, ["u"]));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::TerminologyPackage,
                feature: "implements",
                ..
            }
        ),
        "{row}: implements on a TerminologyPackage: {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the TerminologyPackage \"t\" has no implements",
        "{row}: implements on a TerminologyPackage: the Display"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("blade", ReferenceEnd::Interface, "i"),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::Term,
                feature: "interface",
                ..
            }
        ),
        "{row}: interface on a Term: {error:?}"
    );
}

#[test]
fn sacm23_term_007_interface_and_implements_are_never_synced() {
    let row = TERM_007;
    let mut doc = read_ok(row, "the case", &case_with("", ""));
    applied(
        row,
        &mut doc,
        CreateTerminologyPackageInterface::new("t", named("I"), "t").with_id("i"),
    );
    assert!(
        element(row, &doc, "t").interfaces().is_empty(),
        "{row}: creating i filled t's interface end"
    );
    let mutation = applied(
        row,
        &mut doc,
        AddReference::new("t", ReferenceEnd::Interface, "i"),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "i").implements().map(ElementId::as_str)
        ),
        (vec!["t"], Some("t")),
        "{row}: AddReference(t, interface, i) changed more than t"
    );
    assert_eq!(
        (referrers(&doc, "t"), referrers(&doc, "i")),
        (
            vec![("i".to_owned(), ReferenceEnd::Implements, 0)],
            vec![("t".to_owned(), ReferenceEnd::Interface, 0)]
        ),
        "{row}: the referrers of t and i"
    );
    applied(
        row,
        &mut doc,
        SetReferences::new("i", ReferenceEnd::Implements, ["u"]),
    );
    assert_eq!(
        (
            element(row, &doc, "t").interfaces(),
            element(row, &doc, "u").interfaces()
        ),
        (list(&["i"]).as_slice(), [].as_slice()),
        "{row}: changing implements changed an interface end"
    );
}

#[test]
fn sacm23_term_007_compare_reports_interfaces_and_implements() {
    let row = TERM_007;
    let left = read_ok(
        row,
        "left",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"t\"", ""),
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
                id: ElementId::from("t"),
                left: list(&["i"]),
                right: Vec::new(),
            },
            Difference::Implements {
                id: ElementId::from("i"),
                left: Some(ElementId::from("t")),
                right: None,
            },
        ],
        "{row}: not one Interfaces and one Implements difference"
    );
    assert_eq!(
        found.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "element \"t\" differs in interface: [\"i\"] on the left, [] on the right",
            "element \"i\" differs in implements: \"t\" on the left, no implements on the right",
        ],
        "{row}: the Display of the differences"
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_term_007_move_copy_and_the_element_edits_apply() {
    let row = TERM_007;
    let mut doc = read_ok(
        row,
        "i in t",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"t\"", &term("ia", "")),
            "",
            "",
            "",
        ),
    );
    applied(row, &mut doc, SetName::new("i", named("Renamed")));
    applied(row, &mut doc, SetGid::new("i", "I-1"));
    applied(row, &mut doc, SetFlag::new("i", Flag::IsAbstract, true));
    applied(row, &mut doc, ReorderElements::new("t", ["i", "blade"]));
    assert_eq!(
        children(row, &doc, "t"),
        ["i", "blade"],
        "{row}: the reorder"
    );
    let ends = |doc: &Document| {
        (
            element(row, doc, "t").interfaces().to_vec(),
            element(row, doc, "i").implements().cloned(),
        )
    };
    let held = ends(&doc);
    // Outside t, the package it implements, i gets one SACM-PKG-002
    // Warning, which moving it back clears (SACM23-VAL-023).
    let before = findings(&doc);
    let outside = sorted(
        before
            .iter()
            .cloned()
            .chain([placement("i", "t")])
            .collect(),
    );
    for (what, operation, parent, want) in [
        (
            "to the roots",
            MoveElement::root("i"),
            None,
            outside.clone(),
        ),
        (
            "into case",
            MoveElement::new("i", "case"),
            Some("case"),
            outside.clone(),
        ),
        (
            "into arts",
            MoveElement::new("i", "arts"),
            Some("arts"),
            outside.clone(),
        ),
        (
            "back into t",
            MoveElement::new("i", "t"),
            Some("t"),
            sorted(before.clone()),
        ),
    ] {
        applied(row, &mut doc, operation);
        assert_eq!(
            (
                parent_of(row, &doc, "i").as_deref(),
                ends(&doc),
                sorted(findings(&doc))
            ),
            (parent, held.clone(), want),
            "{row}: a move {what} changed a reference, or not the placement finding"
        );
    }
    let error = refuse(row, &mut doc, MoveElement::new("i", "p"));
    assert!(
        matches!(
            &error,
            EditError::InvalidParent {
                parent_kind: ElementKind::ArgumentPackage,
                kind: ElementKind::TerminologyPackageInterface,
                ..
            }
        ),
        "{row}: a TerminologyPackageInterface into an ArgumentPackage: {error:?}"
    );
    // Copying hazard_terms follows the copied ids; copying its interface
    // alone keeps implements hazard_terms.
    let mut doc = read_ok(row, "the fixture", &fixture_text(FIXTURE));
    let copy = CopyElements::new(&doc, ["hazard_terms"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of hazard_terms: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let (package, api, blade, cited) = (
        copy_of(&pairs, "hazard_terms"),
        copy_of(&pairs, "hazard_terms_api"),
        copy_of(&pairs, "blade"),
        copy_of(&pairs, "blade_cited"),
    );
    assert_eq!(
        (
            element(row, &doc, &package).interfaces().to_vec(),
            element(row, &doc, &api).implements().cloned(),
            element(row, &doc, &cited).cited_element().cloned(),
        ),
        (
            vec![ElementId::from(api.as_str())],
            Some(ElementId::from(package.as_str())),
            Some(ElementId::from(blade.as_str())),
        ),
        "{row}: the copy of hazard_terms does not follow the copied ids: {pairs:?}"
    );
    let copy = CopyElements::new(&doc, ["hazard_terms_api"], "design_terms")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of hazard_terms_api: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        (
            element(row, &doc, &copy_of(&pairs, "hazard_terms_api"))
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "design_terms").interfaces(),
        ),
        (Some("hazard_terms"), list(&["design_terms_api"]).as_slice()),
        "{row}: the copy of hazard_terms_api under design_terms"
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
                if id.as_str() == "hazard_terms_api"
        ),
        "{TERM_008}: the copy of binding into an empty document: {error:?}"
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
    let copy = CopyElements::new(&doc, ["i"], "u")
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
        "{TERM_008}: the copy of a binding of [i, i]: {error:?}"
    );
}

#[test]
fn sacm23_term_007_deleting_the_implemented_package_follows_lib_edit_019() {
    let row = TERM_007;
    // (a) By default, ti's implements crossing to t refuses.
    let mut doc = fixture_t(row);
    let error = refuse(row, &mut doc, DeleteElement::new("t"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end: "implements", id, .. }
                if holder.as_str() == "ti" && id.as_str() == "t"
        ),
        "{row}: (a) deleting t by default: not CrossPackageReference {{ ti, implements, t }}: \
         {error:?}"
    );
    // (b) Scrubbing, recursively: ti loses its implements and goes with tx,
    // and b, left with ui alone, goes too.
    let operation = scrubbing("t");
    assert_eq!(
        dangling(row, &doc, operation.clone()),
        [
            dangles("ti", "implements", "t", true),
            dangles("b", "participantPackage", "ti", true)
        ],
        "{row}: (b) the dangling references of deleting t"
    );
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["t", "ti", "tx", "b"], Vec::<&str>::new()),
        "{row}: (b) the deleted and changed ids"
    );
    // (c) The same with RejectIfNonEmpty: ti holds tx.
    let mut doc = fixture_t(row);
    let error = refuse(
        row,
        &mut doc,
        scrubbing("t").with_package_policy(RejectIfNonEmpty),
    );
    assert!(
        matches!(
            &error,
            EditError::NotEmpty { element, contained: 1, .. } if element.as_str() == "ti"
        ),
        "{row}: (c) not NotEmpty {{ ti, 1 }}: {error:?}"
    );
}

// =====================================================================================
// SACM23-TERM-008: TerminologyPackageBinding
// =====================================================================================

#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
#[test]
fn sacm23_term_008_reads_typed_in_every_role_and_as_a_root() {
    let row = TERM_008;
    let kind = ElementKind::TerminologyPackageBinding;
    let attributes = " participantPackage=\"t u\"";
    let places: Vec<Place> = vec![
        (
            "terminologyElement in a TerminologyPackage",
            case_with(&binding_in("terminologyElement", "b", attributes, ""), ""),
            Some("t"),
            Some("terminologyElement"),
        ),
        (
            "terminologyElement in a TerminologyPackageInterface",
            case_with(
                &interface(
                    "ti",
                    " implements=\"t\"",
                    &binding_in("terminologyElement", "b", attributes, ""),
                ),
                "",
            ),
            Some("ti"),
            Some("terminologyElement"),
        ),
        (
            "terminologyElement in a TerminologyPackageBinding",
            case_with(
                "",
                &binding(
                    "outer",
                    attributes,
                    &binding_in("terminologyElement", "b", attributes, ""),
                ),
            ),
            Some("outer"),
            Some("terminologyElement"),
        ),
        (
            "terminologyPackage in an AssuranceCasePackage",
            b_in_case(attributes, ""),
            Some("case"),
            Some("terminologyPackage"),
        ),
        (
            "terminologyPackage in an AssuranceCasePackageInterface",
            case_with("", &case_interface("ci", &binding("b", attributes, ""))),
            Some("ci"),
            Some("terminologyPackage"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                "",
                &binding_in("artifactElement", "b", attributes, ""),
                "",
            ),
            Some("arts"),
            Some("artifactElement"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:TerminologyPackageBinding{NS} xmi:id=\"b\">\
                 <name content=\"b\"/></sacm:TerminologyPackageBinding>"
            ),
            None,
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:TerminologyPackage xmi:id=\"t\"><name content=\"T\"/></sacm:TerminologyPackage>\
                 <sacm:TerminologyPackageBinding xmi:id=\"b\" participantPackage=\"t t\">\
                 <name content=\"b\"/></sacm:TerminologyPackageBinding></xmi:XMI>"
            ),
            None,
            None,
        ),
    ];
    assert_reads_in(row, BINDING, kind, "b", &places);
    assert!(
        kind.is_kind_of(ElementKind::TerminologyPackage),
        "{row}: a TerminologyPackageBinding is no TerminologyPackage"
    );
    // Non-citation content reads and writes in both modes (Q2).
    let what = "a binding holding a non-citation Term and a Category";
    let doc = read_ok(
        row,
        what,
        &b_in_case(
            " participantPackage=\"t u\" gid=\"B-1\"",
            &format!(
                "{}{}",
                term("b1", ""),
                typed_in("terminologyElement", "Category", "b2", "", "")
            ),
        ),
    );
    assert_eq!(
        (
            children(row, &doc, "b"),
            element(row, &doc, "b").gid(),
            element(row, &doc, "b1").is_citation()
        ),
        (vec!["b1".to_owned(), "b2".to_owned()], Some("B-1"), false),
        "{row}: {what}: what it holds"
    );
    for mode in MODES {
        round_trip(row, what, &doc, mode);
    }
    // In an ArgumentPackage's argumentElement and an AssuranceCasePackage's
    // assuranceCasePackage: kept whole as preserved content of the holder.
    let held = binding_in("argumentElement", "x", attributes, &term("xt", ""));
    let template = case("", "", &format!("{MARK}{held}"), "", "");
    let doc = assert_preserved_at(row, "a binding in argumentElement", &template, "p");
    assert!(
        doc.get("x").is_none() && doc.get("xt").is_none(),
        "{row}: the binding in argumentElement was read as an element"
    );
    let held = binding_in("assuranceCasePackage", "x", attributes, &term("xt", ""));
    let template = case_with("", &format!("{MARK}{held}"));
    let doc = assert_preserved_at(row, "a binding in assuranceCasePackage", &template, "case");
    assert!(
        doc.get("x").is_none() && doc.get("xt").is_none(),
        "{row}: the binding in assuranceCasePackage was read as an element"
    );
}

#[test]
fn sacm23_term_008_participants_read_in_attribute_and_element_form_in_order() {
    let row = TERM_008;
    let attribute = read_ok(
        row,
        "participantPackage=\"u t\"",
        &b_in_case(" participantPackage=\"u t\"", ""),
    );
    let elements = read_ok(
        row,
        "participantPackage elements",
        &b_in_case(
            "",
            "<participantPackage xmi:idref=\"u\"/><participantPackage xmi:idref=\"t\"/>",
        ),
    );
    assert!(
        elements == attribute,
        "{row}: the element forms do not read == to the attribute"
    );
    let href = read_ok(
        row,
        "participantPackage elements by xmi:idref and href",
        &b_in_case(
            "",
            "<participantPackage xmi:idref=\"u\"/><participantPackage href=\"#t\"/>",
        ),
    );
    assert!(
        href == attribute,
        "{row}: the xmi:idref and href forms do not read == to the attribute"
    );
    assert_eq!(
        element(row, &attribute, "b").participant_packages(),
        list(&["u", "t"]).as_slice(),
        "{row}: participant_packages() in order"
    );
    let what = "an attribute and a participant element";
    let (input, marks) = marked(&b_in_case(
        &format!(" {MARK}participantPackage=\"t\""),
        &format!("{MARK}<participantPackage xmi:idref=\"u\"/>"),
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
    let what = "participantPackage=\"t t u\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"t t u\"", ""));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["t", "t", "u"]).as_slice(),
        "{row}: {what}: not held as three ids"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "10.6",
            &["b", "t"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 10.6"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "b").ends_with(" participantPackage=\"t t u\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_term_008_too_few_participants_read_and_write_back() {
    let row = TERM_008;
    for (what, attribute) in [
        ("no participant", ""),
        ("one participant", " participantPackage=\"t\""),
        ("one distinct participant", " participantPackage=\"u u\""),
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
                "10.6",
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
fn sacm23_term_008_dangling_and_wrongly_typed_participants() {
    let row = TERM_008;
    let what = "participantPackage=\"t gone\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"t gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "10.6",
            &["b", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 10.6"
    );
    assert_eq!(
        argument_of(&found[0], "type"),
        Some("TerminologyPackage"),
        "{row}: {what}: the end's type"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "b", " participantPackage=\"t gone\">");
    let doc = read_ok(
        row,
        "participants of every TerminologyPackage kind",
        &case_with(
            &interface("i", " implements=\"t\"", ""),
            &format!(
                "{}{}",
                binding("b", " participantPackage=\"t i b2\"", ""),
                binding("b2", " participantPackage=\"t u\"", "")
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
            "10.6",
            &["b", "b2"]
        )],
        "{row}: participants of every TerminologyPackage kind"
    );
    for (target, class) in [
        ("blade", "Term"),
        ("p", "ArgumentPackage"),
        ("p_api", "ArgumentPackageInterface"),
        ("case", "AssuranceCasePackage"),
    ] {
        let what = format!("participantPackage=\"t {target}\" (a {class})");
        let doc = read_ok(
            row,
            &what,
            &b_in_case(&format!(" participantPackage=\"t {target}\""), ""),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "10.6",
                &["b", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 10.6"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("TerminologyPackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
    }
}

#[test]
fn sacm23_term_008_writes_attribute_order_and_the_golden_file() {
    let row = TERM_008;
    let what = "a binding with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case_with(
            &interface("i", " implements=\"t\"", ""),
            &binding(
                "b",
                " participantPackage=\"u i\" interface=\"i\" abstractForm=\"b\" citedElement=\"b\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"B-1\"",
                "",
            ),
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<terminologyPackage xmi:type=\"sacm:TerminologyPackageBinding\" xmi:id=\"b\" \
         gid=\"B-1\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"b\" \
         abstractForm=\"b\" interface=\"i\" participantPackage=\"u i\">",
        "{row}: {what}: not written in order\n{text}"
    );
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<terminologyPackage xmi:type=\"sacm:TerminologyPackageBinding\" xmi:id=\"binding\" \
             participantPackage=\"hazard_terms_api design_terms_api\">"
        ),
        "{row}: the golden file has no binding line"
    );
}

#[test]
fn sacm23_term_008_reference_end_tables_bounds_and_serde() {
    let row = TERM_008;
    let end = ReferenceEnd::ParticipantPackage;
    assert_eq!(
        (
            end.bounds(ElementKind::TerminologyPackageBinding),
            end.bounds(ElementKind::TerminologyPackageInterface),
            end.bounds(ElementKind::TerminologyPackage),
        ),
        (Some((2, None)), None, None),
        "{row}: the bounds"
    );
    assert!(
        end.applies_to(ElementKind::TerminologyPackageBinding),
        "{row}: participantPackage does not apply to a TerminologyPackageBinding"
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
fn sacm23_term_008_create_reports_what_it_holds_and_checks_in_order() {
    let row = TERM_008;
    let operation = CreateTerminologyPackageBinding::root(
        named("Hazard terms map to design terms"),
        ["t", "u"],
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
            &named("Hazard terms map to design terms"),
            vec!["t", "u"]
        ),
        "{row}: root()"
    );
    let operation =
        CreateTerminologyPackageBinding::new("case", named("B"), ["u", "t", "u"]).with_id("b");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            strs(operation.participant_packages())
        ),
        (Some("case"), Some("b"), vec!["u", "t", "u"]),
        "{row}: new() and with_id() keep the participants in order with repeats"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateTerminologyPackageBinding",
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
            ElementKind::TerminologyPackageBinding,
            list(&["u", "t", "u"]).as_slice()
        ),
        "{row}: the created binding"
    );
    let text = write_in(row, "the created binding", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<terminologyPackage xmi:type=\"sacm:TerminologyPackageBinding\" xmi:id=\"b\" \
         participantPackage=\"u t u\">",
        "{row}: the created binding's start tag\n{text}"
    );
    let mut doc = read_ok(row, "the create case", &create_case());
    let mut generated = Vec::new();
    for parent in LEGAL_PARENTS {
        let mutation = applied(
            row,
            &mut doc,
            CreateTerminologyPackageBinding::new(parent, named("N"), ["t", "u"]),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let root = applied(
        row,
        &mut doc,
        CreateTerminologyPackageBinding::root(named("N"), ["t", "u"]),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=9)
            .map(|n| format!("terminology_package_binding_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (parent, parent_kind) in ILLEGAL_PARENTS {
        let error = refuse(
            row,
            &mut doc,
            CreateTerminologyPackageBinding::new(parent, named("N"), ["t", "u"]),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::TerminologyPackageBinding
            ),
            "{row}: a binding under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, participants: &[&str]| {
        CreateTerminologyPackageBinding::new(parent, name, participants.iter().copied())
    };
    let cases: Vec<(&str, CreateTerminologyPackageBinding, _)> = vec![
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
            create("case", named("N"), &["blade", "nope"]),
            k(
                "ReferenceNotFound",
                Some("participantPackage"),
                Some("nope"),
            ),
        ),
        (
            "an ArgumentPackage participant",
            create("case", named("N"), &["t", "p"]),
            k("WrongReferenceType", Some("participantPackage"), Some("p")),
        ),
        (
            "a Term participant",
            create("case", named("N"), &["blade", "u"]),
            k(
                "WrongReferenceType",
                Some("participantPackage"),
                Some("blade"),
            ),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "TerminologyPackage", "{row}: {what}: expected");
        }
    }
    let error = refuse(row, &mut doc, create("case", named("N"), &["t", "t"]));
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
        "{row}: [t, t]: {error:?}"
    );
    let next = applied(row, &mut doc, create("case", named("N"), &["t", "u"]));
    assert_eq!(
        strs(next.created_ids()),
        ["terminology_package_binding_10"],
        "{row}: a refusal moved the next generated id"
    );
}

#[test]
fn sacm23_term_008_reference_edits_count_distinct_participants() {
    let row = TERM_008;
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
        "b of [t, u]",
        &b_in_case(" participantPackage=\"t u\"", ""),
    );
    for (what, ids) in [("[t]", vec!["t"]), ("[t, t]", vec!["t", "t"])] {
        let error = refuse(row, &mut doc, SetReferences::new("b", end, ids));
        assert!(too_few(&error, 1), "{row}: SetReferences {what}: {error:?}");
    }
    let error = refuse(row, &mut doc, RemoveReference::new("b", end, "u"));
    assert!(too_few(&error, 1), "{row}: removing one of two: {error:?}");
    applied(row, &mut doc, AddReference::new("b", end, "t"));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["t", "u", "t"]).as_slice(),
        "{row}: adding a held id"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "10.6",
            &["b", "t"]
        )],
        "{row}: the repeat warns"
    );
    let mut doc = read_ok(row, "b of [t]", &b_in_case(" participantPackage=\"t\"", ""));
    let error = refuse(row, &mut doc, AddReference::new("b", end, "t"));
    assert!(
        too_few(&error, 1),
        "{row}: AddReference t onto [t]: {error:?}"
    );
    applied(row, &mut doc, AddReference::new("b", end, "u"));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["t", "u"]).as_slice(),
        "{row}: AddReference u onto [t]"
    );
    let error = refuse(row, &mut doc, SetReferences::new("b", end, ["t", "p"]));
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "participantPackage",
                expected: "TerminologyPackage",
                ..
            }
        ),
        "{row}: participants [t, p]: {error:?}"
    );
}

#[test]
fn sacm23_term_008_compare_reports_participant_packages() {
    let row = TERM_008;
    let left = read_ok(row, "left", &b_in_case(" participantPackage=\"i j\"", ""));
    let right = read_ok(row, "right", &b_in_case(" participantPackage=\"j i\"", ""));
    let found = semantic_compare(&left, &right);
    assert_eq!(
        found,
        [Difference::ParticipantPackages {
            id: ElementId::from("b"),
            left: list(&["i", "j"]),
            right: list(&["j", "i"]),
        }],
        "{row}: not one ParticipantPackages difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"b\" differs in participantPackage: [\"i\", \"j\"] on the left, [\"j\", \"i\"] \
         on the right",
        "{row}: the Display"
    );
}

#[test]
fn sacm23_term_008_move_copy_and_the_element_edits_apply() {
    let row = TERM_008;
    let mut doc = read_ok(
        row,
        "b in case",
        &b_in_case(
            " participantPackage=\"t u\"",
            &format!("{}{}", term("b1", ""), term("b2", "")),
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
        ("into t", MoveElement::new("b", "t"), Some("t")),
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
    let error = refuse(row, &mut doc, MoveElement::new("b", "p"));
    assert!(
        matches!(
            &error,
            EditError::InvalidParent {
                kind: ElementKind::TerminologyPackageBinding,
                ..
            }
        ),
        "{row}: a TerminologyPackageBinding into an ArgumentPackage: {error:?}"
    );
    let copy = CopyElements::new(&doc, ["b"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        element(row, &doc, &copy_of(&pairs, "b")).participant_packages(),
        list(&["t", "u"]).as_slice(),
        "{row}: the copy of b keeps its participants as written"
    );
    let mut doc = read_ok(
        row,
        "b of [t, t]",
        &b_in_case(" participantPackage=\"t t\"", ""),
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
        "{row}: the copy of a binding of [t, t]: {error:?}"
    );
}

#[test]
fn sacm23_term_008_a_binding_left_below_two_participants_is_deleted_as_lib_edit_019_says() {
    let row = TERM_008;
    let mut doc = fixture_t(row);
    let operation = scrubbing("ui");
    assert_eq!(
        dangling(row, &doc, operation.clone()),
        [dangles("b", "participantPackage", "ui", true)],
        "{row}: the dangling references of deleting ui"
    );
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["ui", "b"], Vec::<&str>::new()),
        "{row}: b, left with ti alone, goes with ui"
    );
    assert_eq!(
        element(row, &doc, "ti").implements().map(ElementId::as_str),
        Some("t"),
        "{row}: ti is unchanged"
    );
}
