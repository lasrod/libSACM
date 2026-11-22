//! Tests for `ArgumentPackageInterface` and `ArgumentPackageBinding`, with the
//! ends `interface`, `implements` and `participantPackage` (SACM23-ARG-014,
//! SACM23-ARG-015; formal/23-05-08 §11.5
//! "`participantPackage:ArgumentPackage[2..*]` - the `ArgumentPackages` being
//! mapped together by the `ArgumentPackageBinding`.", §11.6
//! "`implements:ArgumentPackage[1]` - a reference to the ArgumentPackage which
//! the `ArgumentPackageInterface` declares."; ptc/22-03-13
//! `ArgumentPackage::interface` [0..*]; the M5 decisions Q1, Q2, Q5, Q7, Q8 and
//! Q9; D-PKG-HOMOG, D-BIND-PART; ADR 0039).
//!
//! Both kinds are `ArgumentPackage`s by inheritance: they read typed in every
//! role that admits an ArgumentPackage and as roots, hold their children in
//! `argumentElement`, and carry the package's `interface` end. `interface`
//! and `implements` are kept as written and never synced (Q1). A missing
//! `implements` and fewer than two participants read and write back, and
//! since slice 0068 `validate` gives one SACM-MULT-001 for each
//! (SACM23-VAL-014); an interface outside the package it implements gives
//! one SACM-PKG-002 Warning (SACM23-VAL-023), and an interface's or a
//! binding's content that is no citation one SACM-PKG-001 (SACM23-VAL-013).
//! A lower bound counts distinct ids (Q7, ADR 0039 D3).
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
use sacm::edit::{
    self, AddReference, CopyElements, CreateArgumentPackageBinding, CreateArgumentPackageInterface,
    EditError, MoveElement, Mutation, Operation, RemoveReference, ReorderElements, SetFlag, SetGid,
    SetName, SetReferences,
};
use sacm::model::{Flag, LangString, ReferenceEnd};

const ARG_014: &str = "SACM23-ARG-014";
const ARG_015: &str = "SACM23-ARG-015";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The worked fixture of slice 0066 and its golden output.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/argument-package-interfaces-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/argument-package-interfaces.golden.sacm.xmi"
);

// --- Inputs --------------------------------------------------------------------------

/// An `ArgumentPackageInterface` `id` in the role `role`, with `attributes`,
/// named after its id, holding `body`.
fn interface_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// An `ArgumentPackageBinding` `id` in the role `role`, with `attributes`,
/// named after its id, holding `body`.
fn binding_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// An `ArgumentPackageInterface` `id` in an ArgumentPackage's `argumentElement`.
fn interface(id: &str, attributes: &str, body: &str) -> String {
    interface_in("argumentElement", id, attributes, body)
}

/// An `ArgumentPackageBinding` `id` in an AssuranceCasePackage's `argumentPackage`.
fn binding(id: &str, attributes: &str, body: &str) -> String {
    binding_in("argumentPackage", id, attributes, body)
}

/// A typed `argumentElement` of the kind `kind`, `id`, with `attributes`,
/// named after its id, holding `body`.
fn argument(kind: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// A Claim `id` with `attributes`.
fn claim(id: &str, attributes: &str) -> String {
    argument("Claim", id, attributes, "")
}

/// The AssuranceCasePackage `case`, one element per line:
/// - the ArgumentPackage `p`, with `p_attributes`, holding the Claim `c`, then `p_body`;
/// - the ArgumentPackage `q`, holding the Claim `d`;
/// - the ArtifactPackage `evidence`, holding the Artifact `report` and the
///   `ArtifactPackageInterface` `evidence_api` (implementing `evidence`), then
///   `evidence_body`;
/// - the empty TerminologyPackage `terms`;
/// - then `extra`.
fn case(p_attributes: &str, p_body: &str, evidence_body: &str, extra: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">
  <name content=\"Case\"/>
  <argumentPackage xmi:id=\"p\"{p_attributes}><name content=\"P\"/>
    {c}
    {p_body}
  </argumentPackage>
  <argumentPackage xmi:id=\"q\"><name content=\"Q\"/>
    {d}
  </argumentPackage>
  <artifactPackage xmi:id=\"evidence\"><name content=\"Evidence\"/>
    <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"report\"><name content=\"Report\"/></artifactElement>
    <artifactElement xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"evidence_api\" implements=\"evidence\"><name content=\"Evidence API\"/></artifactElement>
    {evidence_body}
  </artifactPackage>
  <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/></terminologyPackage>
  {extra}
</sacm:AssuranceCasePackage>",
        c = claim("c", ""),
        d = claim("d", ""),
    )
}

/// [`case`] with the interface `i` in `p`, with `attributes` and `body`.
fn i_in_p(attributes: &str, body: &str) -> String {
    case("", &interface("i", attributes, body), "", "")
}

/// [`case`] with the binding `b` in `case`, with `attributes` and `body`.
fn b_in_case(attributes: &str, body: &str) -> String {
    case("", "", "", &binding("b", attributes, body))
}

/// An ArgumentPackage root `p` holding `body`.
fn root_p(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}</sacm:ArgumentPackage>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!(
            "SACM23-ARG-014: setup: SourceLocation::new({line}, {column}) refused a 1-based position"
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
/// implements, clause 11.6 (SACM23-VAL-023).
fn placement(interface: &str, package: &str) -> Finding {
    finding_of(
        Code::new("SACM-PKG-002"),
        Severity::Warning,
        "SACM23-VAL-023",
        "11.6",
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
        .unwrap_or_else(|error| panic!("SACM23-ARG-014: cannot read {path}: {error}"))
}

// --- Edits (slice 0066 API) ------------------------------------------------------------

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
    let input = root_p("").replacen(
        "<name content=\"P\"/>",
        "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\"/>",
        1,
    );
    let doc = read_ok(row, "a name with no expression", &input);
    element(row, &doc, "p")
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
        None => panic!("SACM23-ARG-014: {source:?} was not copied: {pairs:?}"),
    }
}

// =====================================================================================
// SACM23-ARG-014: ArgumentPackageInterface
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_reads_typed_in_every_role_and_as_a_root() {
    let row = ARG_014;
    let kind = ElementKind::ArgumentPackageInterface;
    let roles = [
        (
            "argumentElement in an ArgumentPackage",
            i_in_p(" implements=\"p\"", ""),
            Some("p"),
        ),
        (
            "argumentPackage in an AssuranceCasePackage",
            case(
                "",
                "",
                "",
                &interface_in("argumentPackage", "i", " implements=\"p\"", ""),
            ),
            Some("case"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                &interface_in("artifactElement", "i", " implements=\"p\"", ""),
                "",
            ),
            Some("evidence"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:ArgumentPackageInterface{NS} xmi:id=\"i\" implements=\"i\"><name content=\"i\"/>\
                 </sacm:ArgumentPackageInterface>"
            ),
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>\
                 <sacm:ArgumentPackageInterface xmi:id=\"i\" implements=\"p\"><name content=\"i\"/>\
                 </sacm:ArgumentPackageInterface></xmi:XMI>"
            ),
            None,
        ),
    ];
    for (what, input, parent) in &roles {
        let doc = read_ok(row, what, input);
        assert_eq!(
            (
                element(row, &doc, "i").kind(),
                parent_of(row, &doc, "i").as_deref()
            ),
            (kind, *parent),
            "{row}: {what}: not an ArgumentPackageInterface under {parent:?}"
        );
        assert!(
            kind.is_kind_of(ElementKind::ArgumentPackage),
            "{row}: an ArgumentPackageInterface is no ArgumentPackage"
        );
    }
    // Its name, utility children, gid, flags, citedElement and abstractForm,
    // and its children in argumentElement.
    let what = "an interface with every inherited feature";
    let doc = read_ok(
        row,
        what,
        &i_in_p(
            " implements=\"p\" gid=\"I-1\" isCitation=\"true\" citedElement=\"q\" \
             isAbstract=\"true\" abstractForm=\"i\"",
            "<description xmi:id=\"i_d\"><content><value content=\"D\"/></content></description>\
             <implementationConstraint xmi:id=\"i_ic\"/>\
             <note xmi:id=\"i_n\"><content><value content=\"N\"/></content></note>\
             <taggedValue xmi:id=\"i_tv\"><key><value content=\"K\"/></key></taggedValue>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"ic\"><name content=\"IC\"/></argumentElement>\
             <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ip\"><name content=\"IP\"/></argumentElement>",
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
        (Some("i"), Some("I-1"), true, Some("q"), true, Some("i")),
        "{row}: {what}: the inherited features"
    );
    assert_eq!(
        children(row, &doc, "i")
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["i_d", "i_ic", "i_n", "i_tv", "ic", "ip"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "{row}: {what}: the children"
    );
    assert_eq!(
        (
            element(row, &doc, "ic").kind(),
            element(row, &doc, "ip").kind(),
            element(row, &doc, "i_n").kind()
        ),
        (
            ElementKind::Claim,
            ElementKind::ArgumentPackage,
            ElementKind::Note
        ),
        "{row}: {what}: the kinds of the children"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "ic").starts_with("<argumentElement xmi:type=\"sacm:Claim\""),
        "{row}: {what}: the interface's Claim is not written in argumentElement\n{text}"
    );
    // An untyped argumentElement is MissingType; an untyped argumentPackage
    // is a plain ArgumentPackage.
    let input = case(
        "",
        "<argumentElement xmi:id=\"x\"><name content=\"x\"/></argumentElement>",
        "",
        "",
    );
    let error = refused(row, "an untyped argumentElement", &input);
    assert!(
        matches!(
            &error,
            ReadError::MissingType { role, declared_type, .. }
                if role == "argumentElement" && declared_type == "ArgumentationElement"
        ),
        "{row}: an untyped argumentElement is not MissingType: {error:?}"
    );
    let doc = read_ok(
        row,
        "an untyped argumentPackage",
        &case(
            "",
            "",
            "",
            "<argumentPackage xmi:id=\"x\"><name content=\"x\"/></argumentPackage>",
        ),
    );
    assert_eq!(
        element(row, &doc, "x").kind(),
        ElementKind::ArgumentPackage,
        "{row}: an untyped argumentPackage is not a plain ArgumentPackage"
    );
}

#[test]
fn sacm23_arg_014_implements_and_interface_read_in_attribute_and_element_form() {
    let row = ARG_014;
    let attribute = read_ok(row, "implements=\"p\"", &i_in_p(" implements=\"p\"", ""));
    for (what, body) in [
        (
            "<implements xmi:idref=\"p\"/>",
            "<implements xmi:idref=\"p\"/>",
        ),
        ("<implements href=\"#p\"/>", "<implements href=\"#p\"/>"),
    ] {
        let doc = read_ok(row, what, &i_in_p("", body));
        assert!(
            doc == attribute,
            "{row}: {what} does not read == to the attribute"
        );
    }
    assert_eq!(
        element(row, &attribute, "i")
            .implements()
            .map(ElementId::as_str),
        Some("p"),
        "{row}: implements() of i"
    );
    // interface on an ArgumentPackage, and inherited by an interface.
    let both = format!(
        "{}{}",
        interface("i1", " implements=\"p\"", ""),
        interface("i2", " implements=\"p\"", "")
    );
    let on_p = read_ok(
        row,
        "interface=\"i1 i2\" on p",
        &case(" interface=\"i1 i2\"", &both, "", ""),
    );
    let elements = read_ok(
        row,
        "interface elements on p",
        &case(
            "",
            &format!("<interface xmi:idref=\"i1\"/><interface href=\"#i2\"/>{both}"),
            "",
            "",
        ),
    );
    assert!(
        elements == on_p,
        "{row}: interface elements on p do not read == to the attribute"
    );
    let inherited = read_ok(
        row,
        "interface=\"i1 i2\" on an interface",
        &i_in_p(" implements=\"p\" interface=\"i1 i2\"", &both),
    );
    let inherited_elements = read_ok(
        row,
        "interface elements on an interface",
        &i_in_p(
            " implements=\"p\"",
            &format!("<interface xmi:idref=\"i1\"/><interface xmi:idref=\"i2\"/>{both}"),
        ),
    );
    assert!(
        inherited_elements == inherited,
        "{row}: interface elements on an interface do not read == to the attribute"
    );
    {
        assert_eq!(
            (
                element(row, &on_p, "p").interfaces(),
                element(row, &inherited, "i").interfaces(),
                element(row, &inherited, "i").references(ReferenceEnd::Interface),
            ),
            (
                list(&["i1", "i2"]).as_slice(),
                list(&["i1", "i2"]).as_slice(),
                list(&["i1", "i2"]).as_slice(),
            ),
            "{row}: interfaces() in order"
        );
        assert!(
            element(row, &on_p, "c").interfaces().is_empty()
                && element(row, &on_p, "p").implements().is_none(),
            "{row}: a Claim holds interfaces, or a package an implements"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_repeated_empty_and_qualified_implements_follow_the_reference_rules() {
    let row = ARG_014;
    for (what, attributes, body, first) in [
        (
            "implements=\"p q\"",
            format!(" implements=\"{MARK}p {MARK}q\""),
            String::new(),
            None,
        ),
        (
            "implements as an attribute and an element",
            format!(" {MARK}implements=\"p\""),
            format!("{MARK}<implements xmi:idref=\"q\"/>"),
            Some("p"),
        ),
    ] {
        let (input, marks) = marked(&i_in_p(&attributes, &body));
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
        // Tolerant: the repeat is kept as preserved content (LIB-COMPAT-001).
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
        assert!(
            element(row, output.document(), "i").has_preserved_content(),
            "{row}: {what}: the repeat is not kept"
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
        ("implements=\" \"", format!(" {MARK}implements=\" \"")),
        ("sacm:implements", format!(" {MARK}sacm:implements=\"p\"")),
    ] {
        let doc = assert_preserved_at(row, what, &i_in_p(&attribute, ""), "i");
        assert!(
            element(row, &doc, "i").implements().is_none(),
            "{row}: {what}: read as the end"
        );
    }
    // A repeated interface is held twice: one SACM23-VAL-022 Warning.
    let what = "interface=\"i i\"";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i i\"",
            &interface("i", " implements=\"p\"", ""),
            "",
            "",
        ),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "11.4",
            &["p", "i"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 11.4"
    );
    assert_eq!(
        element(row, &doc, "p").interfaces(),
        list(&["i", "i"]).as_slice(),
        "{row}: {what}: the repeat is not held"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "p").ends_with(" interface=\"i i\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_arg_014_missing_implements_reads_and_writes_back_unchanged() {
    let row = ARG_014;
    for (what, input) in [
        ("an interface in p", i_in_p("", "")),
        (
            "an interface root",
            format!(
                "<sacm:ArgumentPackageInterface{NS} xmi:id=\"i\"><name content=\"i\"/>\
                 </sacm:ArgumentPackageInterface>"
            ),
        ),
    ] {
        let what = format!("{what} with no implements");
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            findings(&doc),
            [finding_of(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                "SACM23-VAL-014",
                "11.6",
                &["i"]
            )],
            "{row}: {what}: validate does not give one SACM-MULT-001 missing (SACM23-VAL-014)"
        );
        assert!(
            element(row, &doc, "i").implements().is_none(),
            "{row}: {what}: implements() is not None"
        );
        for mode in MODES {
            let text = round_trip(row, &what, &doc, mode);
            let line = line_with(row, &text, "i");
            assert!(
                line.ends_with("xmi:id=\"i\">"),
                "{row}: {what}: a {mode:?} write added to the start tag: {line}"
            );
        }
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_dangling_and_wrongly_typed_ends_report_their_clause_and_type() {
    let row = ARG_014;
    // implements="gone": SACM-REF-001, clause 11.6.
    let what = "implements=\"gone\"";
    let doc = read_ok(row, what, &i_in_p(" implements=\"gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "11.6",
            &["i", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 11.6"
    );
    assert_eq!(
        found[0].message(),
        "ArgumentPackageInterface \"i\" has the implements \"gone\", which names no element of \
         the document (the end's type is ArgumentPackage)",
        "{row}: {what}: the message"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "i", " implements=\"gone\">");
    // A binding to name, with two participants.
    let with_binding = |attributes: &str| {
        case(
            "",
            &interface("i", attributes, ""),
            "",
            &binding("b", " participantPackage=\"p q\"", ""),
        )
    };
    // implements naming each kind: an ArgumentPackage kind gives no
    // reference finding; since slice 0068 one i does not reside in gives one
    // SACM-PKG-002 Warning (SACM23-VAL-023).
    for (target, want) in [
        ("p", Vec::new()),
        ("q", vec![placement("i", "q")]),
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
        ("c", "Claim"),
        ("evidence", "ArtifactPackage"),
        ("evidence_api", "ArtifactPackageInterface"),
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
                "11.6",
                &["i", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 11.6"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("ArgumentPackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
        // An IDREF is untyped: both writes keep it.
        for mode in MODES {
            let text = write_in(row, &what, &doc, mode);
            assert!(
                line_with(row, &text, "i").ends_with(&format!(" implements=\"{target}\">")),
                "{row}: {what}: a {mode:?} write does not keep it\n{text}"
            );
        }
    }
    // interface naming anything but an ArgumentPackageInterface: REF-002,
    // clause 11.4.
    for (target, class) in [
        ("c", "Claim"),
        ("b", "ArgumentPackageBinding"),
        ("q", "ArgumentPackage"),
        ("evidence_api", "ArtifactPackageInterface"),
    ] {
        let what = format!("interface=\"{target}\" (a {class}) on p");
        let doc = read_ok(
            row,
            &what,
            &case(
                &format!(" interface=\"{target}\""),
                "",
                "",
                &binding("b", " participantPackage=\"p q\"", ""),
            ),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "11.4",
                &["p", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 11.4"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("ArgumentPackageInterface"),
            "{row}: {what}: the end's type"
        );
    }
    // interface="gone": REF-001, clause 11.4; a strict write refuses it.
    let what = "interface=\"gone\" on p";
    let doc = read_ok(row, what, &case(" interface=\"gone\"", "", "", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "11.4",
            &["p", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 11.4"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "p", " interface=\"gone\">");
}

#[test]
fn sacm23_arg_014_writes_attribute_order_and_the_golden_file() {
    let row = ARG_014;
    let what = "an interface with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i\" abstractForm=\"p\" gid=\"P-1\"",
            &interface(
                "i",
                " implements=\"p\" interface=\"i\" abstractForm=\"i\" citedElement=\"i\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"I-1\"",
                "",
            ),
            "",
            "",
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" gid=\"I-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"i\" abstractForm=\"i\" \
         interface=\"i\" implements=\"p\">",
        "{row}: {what}: not written in order\n{text}"
    );
    assert_eq!(
        line_with(row, &text, "p"),
        "<argumentPackage xmi:id=\"p\" gid=\"P-1\" abstractForm=\"p\" interface=\"i\">",
        "{row}: {what}: the package's interface is not written last\n{text}"
    );
    // The interface in each role is written typed in that role.
    let doc = read_ok(
        row,
        "an interface in each role",
        &case(
            "",
            &interface("i1", " implements=\"p\"", ""),
            &interface_in("artifactElement", "i3", " implements=\"p\"", ""),
            &interface_in("argumentPackage", "i2", " implements=\"p\"", ""),
        ),
    );
    let text = round_trip(row, "an interface in each role", &doc, Mode::Strict);
    for (id, role) in [
        ("i1", "argumentElement"),
        ("i2", "argumentPackage"),
        ("i3", "artifactElement"),
    ] {
        assert_eq!(
            line_with(row, &text, id),
            format!(
                "<{role} xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"{id}\" implements=\"p\">"
            ),
            "{row}: {id} is not written typed in {role}\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_014_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let row = ARG_014;
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
    // ARG-015 shares the fixture: the binding and its content.
    assert_eq!(
        (
            element(row, &doc, "binding").kind(),
            children(row, &doc, "binding"),
            element(row, &doc, "binding_link").kind()
        ),
        (
            ElementKind::ArgumentPackageBinding,
            vec![
                "binding_braking".to_owned(),
                "binding_vehicle".to_owned(),
                "binding_link".to_owned()
            ],
            ElementKind::AssertedInference
        ),
        "{ARG_015}: the fixture's binding"
    );
    assert_eq!(
        (
            element(row, &doc, "braking").interfaces(),
            element(row, &doc, "braking_api")
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "binding").participant_packages(),
        ),
        (
            list(&["braking_api"]).as_slice(),
            Some("braking"),
            list(&["braking_api", "vehicle_api"]).as_slice(),
        ),
        "{row}: the fixture's ends"
    );
}

#[test]
fn sacm23_arg_014_reference_end_tables_bounds_and_serde() {
    let row = ARG_014;
    let (interface, implements) = (ReferenceEnd::Interface, ReferenceEnd::Implements);
    assert_eq!(
        (interface.name(), implements.name()),
        ("interface", "implements"),
        "{row}: the names"
    );
    assert_eq!(
        implements.bounds(ElementKind::ArgumentPackageInterface),
        Some((1, Some(1))),
        "{row}: implements on an ArgumentPackageInterface is [1]"
    );
    for kind in [
        ElementKind::ArgumentPackage,
        ElementKind::ArgumentPackageInterface,
        ElementKind::ArgumentPackageBinding,
    ] {
        assert_eq!(
            interface.bounds(kind),
            Some((0, None)),
            "{row}: interface on a {kind} is [0..*]"
        );
    }
    // Since slice 0067 the Terminology and AssuranceCase kinds have the
    // ends too (SACM23-TERM-007, SACM23-AC-002).
    assert_eq!(
        (
            implements.bounds(ElementKind::ArgumentPackage),
            implements.bounds(ElementKind::ArgumentPackageBinding),
            interface.bounds(ElementKind::Claim),
        ),
        (None, None, None),
        "{row}: the kinds without the ends"
    );
    let interface_owners = [
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
    let implements_owners = [
        ElementKind::AssuranceCasePackageInterface,
        ElementKind::ArgumentPackageInterface,
        ElementKind::ArtifactPackageInterface,
        ElementKind::TerminologyPackageInterface,
    ];
    for &kind in ElementKind::ALL {
        assert_eq!(
            (interface.applies_to(kind), implements.applies_to(kind)),
            (
                interface_owners.contains(&kind),
                implements_owners.contains(&kind)
            ),
            "{row}: applies_to on a {kind}"
        );
    }
    // In ALL after TerminologyGroupMember and before Expression.
    let all = ReferenceEnd::ALL;
    let at = all
        .iter()
        .position(|&end| end == ReferenceEnd::TerminologyGroupMember)
        .map(|index| &all[index + 1..]);
    assert_eq!(
        at,
        Some(
            [
                ReferenceEnd::Interface,
                ReferenceEnd::Implements,
                ReferenceEnd::ParticipantPackage,
                ReferenceEnd::Expression
            ]
            .as_slice()
        ),
        "{row}: the three ends do not follow TerminologyGroupMember in ALL"
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

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
fn sacm23_arg_014_create_reports_what_it_holds_and_checks_in_order() {
    let row = ARG_014;
    let operation = CreateArgumentPackageInterface::root(named("Braking interface"), "p");
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.name(),
            operation.implements().map(ElementId::as_str)
        ),
        (None, None, &named("Braking interface"), Some("p")),
        "{row}: root()"
    );
    let operation = CreateArgumentPackageInterface::new("p", named("I"), "p").with_id("i");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            operation.implements().map(ElementId::as_str)
        ),
        (Some("p"), Some("i"), Some("p")),
        "{row}: new() and with_id()"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateArgumentPackageInterface",
        "{row}: operation_name()"
    );
    let mut doc = read_ok(row, "the case", &case("", "", "", ""));
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            strs(mutation.changed_ids()),
            strs(mutation.deleted_ids()),
            parent_of(row, &doc, "i"),
            element(row, &doc, "i").kind(),
            element(row, &doc, "i").implements().map(ElementId::as_str),
            element(row, &doc, "p").interfaces().len(),
        ),
        (
            vec!["i"],
            vec![],
            vec![],
            Some("p".to_owned()),
            ElementKind::ArgumentPackageInterface,
            Some("p"),
            0
        ),
        "{row}: the created interface; p's interface end is unchanged"
    );
    let text = round_trip(row, "the created interface", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" implements=\"p\">",
        "{row}: the created interface's start tag\n{text}"
    );
    // Generated ids, and every legal parent: an AssuranceCasePackage, an
    // ArgumentPackage, an ArtifactPackage, and their interfaces and bindings.
    let mut doc = read_ok(
        row,
        "the case with a binding",
        &case("", "", "", &binding("b", " participantPackage=\"p q\"", "")),
    );
    let mut generated = Vec::new();
    for parent in ["case", "p", "evidence", "evidence_api", "b"] {
        let mutation = applied(
            row,
            &mut doc,
            CreateArgumentPackageInterface::new(parent, named("N"), "q"),
        );
        let created = strs(mutation.created_ids())
            .first()
            .map(|id| (*id).to_owned())
            .unwrap_or_default();
        assert_eq!(
            parent_of(row, &doc, &created).as_deref(),
            Some(parent),
            "{row}: the interface under {parent}"
        );
        generated.push(created);
    }
    let first = generated[0].clone();
    let mutation = applied(
        row,
        &mut doc,
        CreateArgumentPackageInterface::new(first.as_str(), named("N"), "q"),
    );
    generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    let root = applied(
        row,
        &mut doc,
        CreateArgumentPackageInterface::root(named("Root"), "q"),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=7)
            .map(|n| format!("argument_package_interface_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    assert!(
        doc.roots()
            .any(|found| found.id().as_str() == "argument_package_interface_7"),
        "{row}: the root create is not a root"
    );
    round_trip(row, "the created interfaces", &doc, Mode::Strict);
    for (parent, parent_kind) in [
        ("c", ElementKind::Claim),
        ("terms", ElementKind::TerminologyPackage),
        ("report", ElementKind::Artifact),
    ] {
        let error = refuse(
            row,
            &mut doc,
            CreateArgumentPackageInterface::new(parent, named("N"), "q"),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::ArgumentPackageInterface
            ),
            "{row}: an interface under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    // The check order.
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, implements: &str| {
        CreateArgumentPackageInterface::new(parent, name, implements)
    };
    let cases: Vec<(&str, CreateArgumentPackageInterface, _)> = vec![
        (
            "a missing parent",
            create("nowhere", unnamed.clone(), "a b").with_id("1i"),
            k("ParentNotFound", None, None),
        ),
        (
            "an illegal parent",
            create("c", unnamed.clone(), "a b").with_id("1i"),
            k("InvalidParent", None, None),
        ),
        (
            "an invalid id",
            create("p", unnamed.clone(), "a b").with_id("1i"),
            k("InvalidId", None, None),
        ),
        (
            "a duplicate id",
            create("p", unnamed.clone(), "a b").with_id("c"),
            k("DuplicateId", None, None),
        ),
        (
            "5a: a name with no expression before an invalid implements",
            create("p", unnamed.clone(), "a b"),
            k("TooFewReferences", Some("expression"), None),
        ),
        (
            "an implements with white space",
            create("p", named("N").with_expression("nowhere"), "a b"),
            k("InvalidReference", Some("implements"), Some("a b")),
        ),
        (
            "a missing implements before the name's expression",
            create("p", named("N").with_expression("nowhere"), "nope"),
            k("ReferenceNotFound", Some("implements"), Some("nope")),
        ),
        (
            "the name's missing expression",
            create("p", named("N").with_expression("nowhere"), "q"),
            k("ReferenceNotFound", Some("expression"), Some("nowhere")),
        ),
        (
            "a Claim as implements",
            create("p", named("N").with_expression("nowhere"), "c"),
            k("ReferenceNotFound", Some("expression"), Some("nowhere")),
        ),
        (
            "a Claim as implements, with a plain name",
            create("p", named("N"), "c"),
            k("WrongReferenceType", Some("implements"), Some("c")),
        ),
        (
            "an ArtifactPackage as implements",
            create("p", named("N"), "evidence"),
            k("WrongReferenceType", Some("implements"), Some("evidence")),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "ArgumentPackage", "{row}: {what}: expected");
        }
    }
    let next = applied(
        row,
        &mut doc,
        CreateArgumentPackageInterface::new("p", named("N"), "q"),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["argument_package_interface_8"],
        "{row}: a refusal moved the next generated id"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_reference_edits_keep_implements_at_exactly_one() {
    let row = ARG_014;
    let implements = ReferenceEnd::Implements;
    let mut doc = read_ok(row, "i in p", &i_in_p(" implements=\"p\"", ""));
    for (what, operation) in [
        (
            "SetReferences::clear",
            Operation::from(SetReferences::clear("i", implements)),
        ),
        (
            "RemoveReference",
            RemoveReference::new("i", implements, "p").into(),
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
        assert_eq!(
            error.code(),
            Code::MULTIPLICITY_VIOLATION,
            "{row}: {what}: the code"
        );
    }
    for (what, operation) in [
        (
            "SetReferences [p, q]",
            Operation::from(SetReferences::new("i", implements, ["p", "q"])),
        ),
        (
            "AddReference q onto p",
            AddReference::new("i", implements, "q").into(),
        ),
        (
            "AddReference p onto p",
            AddReference::new("i", implements, "p").into(),
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
    let mutation = applied(row, &mut doc, SetReferences::new("i", implements, ["q"]));
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "i").implements().map(ElementId::as_str)
        ),
        (vec!["i"], Some("q")),
        "{row}: SetReferences [q]"
    );
    for (ids, wrong) in [(["c"], "c"), (["evidence"], "evidence")] {
        let error = refuse(row, &mut doc, SetReferences::new("i", implements, ids));
        assert!(
            matches!(
                &error,
                EditError::WrongReferenceType { end: "implements", id, expected: "ArgumentPackage", .. }
                    if id.as_str() == wrong
            ),
            "{row}: implements [{wrong}]: {error:?}"
        );
    }
    let error = refuse(row, &mut doc, SetReferences::new("i", implements, ["nope"]));
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound {
                end: "implements",
                ..
            }
        ),
        "{row}: implements [nope]: {error:?}"
    );
    // The package's interface end.
    applied(
        row,
        &mut doc,
        SetReferences::new("p", ReferenceEnd::Interface, ["i"]),
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("p", ReferenceEnd::Interface, "c"),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType {
                end: "interface",
                expected: "ArgumentPackageInterface",
                ..
            }
        ),
        "{row}: interface [c]: {error:?}"
    );
    applied(
        row,
        &mut doc,
        SetReferences::clear("p", ReferenceEnd::Interface),
    );
    // The ends on kinds without them.
    let error = refuse(row, &mut doc, SetReferences::new("p", implements, ["q"]));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind: ElementKind::ArgumentPackage, feature: "implements", .. }
                if element.as_str() == "p"
        ),
        "{row}: implements on p: {error:?}"
    );
    assert_eq!(
        (error.code(), error.to_string()),
        (
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "the ArgumentPackage \"p\" has no implements".to_owned()
        ),
        "{row}: the Display of implements on an ArgumentPackage"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("c", ReferenceEnd::Interface, "i"),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind: ElementKind::Claim, feature: "interface", .. }
                if element.as_str() == "c"
        ),
        "{row}: interface on the Claim c: {error:?}"
    );
    // Since slice 0067 a TerminologyPackage and an AssuranceCasePackage have
    // the end too, typed to their own family (SACM23-TERM-007,
    // SACM23-AC-002): an ArgumentPackageInterface is the wrong type there.
    for (target, expected) in [
        ("terms", "TerminologyPackageInterface"),
        ("case", "AssuranceCasePackageInterface"),
    ] {
        let error = refuse(
            row,
            &mut doc,
            AddReference::new(target, ReferenceEnd::Interface, "i"),
        );
        assert!(
            matches!(
                &error,
                EditError::WrongReferenceType { end: "interface", id, expected: found, .. }
                    if id.as_str() == "i" && *found == expected
            ),
            "{row}: interface i on {target}: {error:?}"
        );
    }
}

#[test]
fn sacm23_arg_014_interface_and_implements_are_never_synced() {
    let row = ARG_014;
    let mut doc = read_ok(row, "the case", &case("", "", "", ""));
    applied(
        row,
        &mut doc,
        CreateArgumentPackageInterface::new("p", named("I"), "p").with_id("i"),
    );
    assert!(
        element(row, &doc, "p").interfaces().is_empty(),
        "{row}: creating i filled p's interface end"
    );
    let mutation = applied(
        row,
        &mut doc,
        AddReference::new("p", ReferenceEnd::Interface, "i"),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "i").implements().map(ElementId::as_str)
        ),
        (vec!["p"], Some("p")),
        "{row}: AddReference(p, interface, i) changed more than p"
    );
    assert_eq!(
        (referrers(&doc, "p"), referrers(&doc, "i")),
        (
            vec![("i".to_owned(), ReferenceEnd::Implements, 0)],
            vec![("p".to_owned(), ReferenceEnd::Interface, 0)]
        ),
        "{row}: the referrers of p and i"
    );
    let mutation = applied(
        row,
        &mut doc,
        SetReferences::new("i", ReferenceEnd::Implements, ["q"]),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "p").interfaces(),
            element(row, &doc, "q").interfaces()
        ),
        (vec!["i"], list(&["i"]).as_slice(), [].as_slice()),
        "{row}: changing implements changed an interface end"
    );
    let mutation = applied(
        row,
        &mut doc,
        RemoveReference::new("p", ReferenceEnd::Interface, "i"),
    );
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "i").implements().map(ElementId::as_str)
        ),
        (vec!["p"], Some("q")),
        "{row}: removing the interface changed implements"
    );
    // The unsynced ends give no finding; i, in p, now implements q, so
    // since slice 0068 it gives the placement Warning (SACM23-VAL-023).
    assert_eq!(
        findings(&doc),
        [placement("i", "q")],
        "{row}: the unsynced ends give a finding"
    );
}

#[test]
fn sacm23_arg_014_compare_reports_interfaces_and_implements() {
    let row = ARG_014;
    let left = read_ok(
        row,
        "left",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"p\"", ""),
            "",
            "",
        ),
    );
    let right = read_ok(row, "right", &case("", &interface("i", "", ""), "", ""));
    let found = semantic_compare(&left, &right);
    assert_eq!(
        found,
        [
            Difference::Interfaces {
                id: ElementId::from("p"),
                left: list(&["i"]),
                right: Vec::new(),
            },
            Difference::Implements {
                id: ElementId::from("i"),
                left: Some(ElementId::from("p")),
                right: None,
            },
        ],
        "{row}: not one Interfaces and one Implements difference"
    );
    assert_eq!(
        found.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "element \"p\" differs in interface: [\"i\"] on the left, [] on the right",
            "element \"i\" differs in implements: \"p\" on the left, no implements on the right",
        ],
        "{row}: the Display of the differences"
    );
    assert_eq!(
        (found[0].element_id(), found[1].element_id()),
        (Some(&ElementId::from("p")), Some(&ElementId::from("i"))),
        "{row}: element_id()"
    );
    // On one element: after AbstractForm, Interfaces before Implements.
    let left = read_ok(
        row,
        "left",
        &i_in_p(" implements=\"p\" interface=\"i\" abstractForm=\"i\"", ""),
    );
    let right = read_ok(row, "right", &i_in_p(" implements=\"q\"", ""));
    let found = semantic_compare(&left, &right);
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::AbstractForm { .. },
                Difference::Interfaces { .. },
                Difference::Implements { left: Some(l), right: Some(r), .. },
            ] if l.as_str() == "p" && r.as_str() == "q"
        ),
        "{row}: not AbstractForm, Interfaces and Implements of i in that order: {found:?}"
    );
}

#[test]
fn sacm23_arg_014_homogeneity_exemption_on_the_read_path() {
    let row = ARG_014;
    // SACM23-VAL-012's findings: since slice 0068 the content of an interface
    // is SACM23-VAL-013's, with the same code.
    let homogeneity = |doc: &Document| -> Vec<Finding> {
        findings(doc)
            .into_iter()
            .filter(|found| found.0 == Code::PACKAGE_CONTENT_INVALID && found.2 == "SACM23-VAL-012")
            .collect()
    };
    let i = interface("i", " implements=\"p\"", "");
    let q = argument("ArgumentPackage", "q", "", "");
    let c = claim("c", "");
    for (what, body, want) in [
        ("p{i, c}", format!("{i}{c}"), Vec::new()),
        (
            "p{q, i, c}",
            format!("{q}{i}{c}"),
            vec![expected(
                Code::PACKAGE_CONTENT_INVALID,
                "SACM23-VAL-012",
                "11.4",
                &["p", "c"],
            )],
        ),
        (
            "i{q, c} in p",
            interface("i", " implements=\"p\"", &format!("{q}{c}")),
            Vec::new(),
        ),
    ] {
        let doc = read_ok(row, what, &root_p(&body));
        assert_eq!(
            homogeneity(&doc),
            want,
            "{row}: {what}: the SACM-PKG-001 findings"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_move_copy_and_the_element_edits_apply() {
    let row = ARG_014;
    let mut doc = read_ok(
        row,
        "i in p",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"p\"", &claim("ic", "")),
            "",
            "",
        ),
    );
    applied(row, &mut doc, SetName::new("i", named("Renamed")));
    applied(row, &mut doc, SetGid::new("i", "I-1"));
    applied(row, &mut doc, SetFlag::new("i", Flag::IsAbstract, true));
    let error = refuse(row, &mut doc, SetFlag::new("i", Flag::IsCounter, true));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                feature: "isCounter",
                ..
            }
        ),
        "{row}: isCounter on an interface: {error:?}"
    );
    applied(row, &mut doc, ReorderElements::new("p", ["i", "c"]));
    assert_eq!(children(row, &doc, "p"), ["i", "c"], "{row}: the reorder");
    let i = element(row, &doc, "i");
    assert_eq!(
        (
            i.name().and_then(|name| name.content()),
            i.gid(),
            i.is_abstract()
        ),
        (Some("Renamed"), Some("I-1"), true),
        "{row}: the element edits"
    );
    // Move: to the roots, into an ArtifactPackage and back. The references
    // are unchanged; outside p, the package it implements, i gets one
    // SACM-PKG-002 Warning, which moving it back clears (SACM23-VAL-023).
    let ends = |doc: &Document| {
        (
            element(row, doc, "p").interfaces().to_vec(),
            element(row, doc, "i").implements().cloned(),
        )
    };
    let held = ends(&doc);
    let findings_after_edits = findings(&doc);
    let outside = sorted(
        findings_after_edits
            .iter()
            .cloned()
            .chain([placement("i", "p")])
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
            "into evidence",
            MoveElement::new("i", "evidence"),
            Some("evidence"),
            outside.clone(),
        ),
        (
            "back into p",
            MoveElement::new("i", "p"),
            Some("p"),
            sorted(findings_after_edits.clone()),
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
    for (what, operation, parent, parent_kind, kind) in [
        (
            "an interface into a TerminologyPackage",
            MoveElement::new("i", "terms"),
            "terms",
            ElementKind::TerminologyPackage,
            ElementKind::ArgumentPackageInterface,
        ),
        (
            "an ArtifactPackageInterface into an ArgumentPackage",
            MoveElement::new("evidence_api", "p"),
            "p",
            ElementKind::ArgumentPackage,
            ElementKind::ArtifactPackageInterface,
        ),
    ] {
        let error = refuse(row, &mut doc, operation);
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: pk, kind: k, .. }
                    if found.as_str() == parent && *pk == parent_kind && *k == kind
            ),
            "{row}: {what}: not InvalidParent: {error:?}"
        );
    }
    // CopyElements: the copy of i under q keeps implements p.
    let copy = CopyElements::new(&doc, ["i"], "q")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of i: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let copied = copy_of(&pairs, "i");
    assert_eq!(
        (
            element(row, &doc, &copied)
                .implements()
                .map(ElementId::as_str),
            parent_of(row, &doc, &copied).as_deref(),
            element(row, &doc, &copied).kind()
        ),
        (Some("p"), Some("q"), ElementKind::ArgumentPackageInterface),
        "{row}: the copy of i"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_014_copy_follows_copied_ids() {
    let row = ARG_014;
    let source = root_p(&format!(
        "{}{}",
        claim("c", ""),
        interface(
            "i",
            " implements=\"p\"",
            &claim("ci", " isCitation=\"true\" citedElement=\"c\""),
        )
    ))
    .replacen("xmi:id=\"p\"", "xmi:id=\"p\" interface=\"i\"", 1);
    let mut doc = read_ok(row, "p with its interface", &source);
    // Copying p: every reference to a copied element names its copy.
    let copy = CopyElements::root(&doc, ["p"])
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of p: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let (p2, c2, i2, ci2) = (
        copy_of(&pairs, "p"),
        copy_of(&pairs, "c"),
        copy_of(&pairs, "i"),
        copy_of(&pairs, "ci"),
    );
    assert_eq!(
        (
            element(row, &doc, &p2).interfaces().to_vec(),
            element(row, &doc, &i2).implements().cloned(),
            element(row, &doc, &ci2).cited_element().cloned(),
        ),
        (
            vec![ElementId::from(i2.as_str())],
            Some(ElementId::from(p2.as_str())),
            Some(ElementId::from(c2.as_str())),
        ),
        "{row}: the copy of p does not follow the copied ids: {pairs:?}"
    );
    // Copying i alone under another package keeps implements p.
    let mut doc = read_ok(
        row,
        "the case",
        &case(
            " interface=\"i\"",
            &interface(
                "i",
                " implements=\"p\"",
                &claim("ci", " isCitation=\"true\" citedElement=\"c\""),
            ),
            "",
            "",
        ),
    );
    let copy = CopyElements::new(&doc, ["i"], "q")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of i: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        (
            element(row, &doc, &copy_of(&pairs, "i"))
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, &copy_of(&pairs, "ci"))
                .cited_element()
                .map(ElementId::as_str),
            element(row, &doc, "q").interfaces(),
            element(row, &doc, "p").interfaces(),
        ),
        (Some("p"), Some("c"), [].as_slice(), list(&["i"]).as_slice()),
        "{row}: the copy of i under q"
    );
    // Copying a binding into an empty document: its participants must resolve.
    let source = read_ok(
        row,
        "a binding",
        &case(
            "",
            &interface("i", " implements=\"p\"", ""),
            "",
            &binding("b", " participantPackage=\"i q\"", ""),
        ),
    );
    let copy = CopyElements::root(&source, ["b"])
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let mut empty = Document::new();
    let error = refuse(row, &mut empty, copy);
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound { end: "participantPackage", id, .. } if id.as_str() == "i"
        ),
        "{ARG_015}: the copy of b into an empty document: {error:?}"
    );
    // An interface read with no implements, and a binding read with one
    // distinct participant, are not copied.
    let mut doc = read_ok(
        row,
        "an interface without implements and a binding of [p, p]",
        &case(
            "",
            &interface("i", "", ""),
            "",
            &binding("b", " participantPackage=\"p p\"", ""),
        ),
    );
    let copy = CopyElements::new(&doc, ["i"], "q")
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
        "{ARG_015}: the copy of a binding of [p, p]: {error:?}"
    );
}

// =====================================================================================
// SACM23-ARG-015: ArgumentPackageBinding
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_015_reads_typed_in_every_role_and_as_a_root() {
    let row = ARG_015;
    let kind = ElementKind::ArgumentPackageBinding;
    let attributes = " participantPackage=\"p q\"";
    let roles = [
        (
            "argumentElement in an ArgumentPackage",
            case(
                "",
                &binding_in("argumentElement", "b", attributes, ""),
                "",
                "",
            ),
            Some("p"),
        ),
        (
            "argumentPackage in an AssuranceCasePackage",
            b_in_case(attributes, ""),
            Some("case"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                "",
                &binding_in("artifactElement", "b", attributes, ""),
                "",
            ),
            Some("evidence"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:ArgumentPackageBinding{NS} xmi:id=\"b\" participantPackage=\"x y\">\
                 <name content=\"b\"/></sacm:ArgumentPackageBinding>"
            ),
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>\
                 <sacm:ArgumentPackageBinding xmi:id=\"b\" participantPackage=\"p p\">\
                 <name content=\"b\"/></sacm:ArgumentPackageBinding></xmi:XMI>"
            ),
            None,
        ),
    ];
    for (what, input, parent) in &roles {
        let doc = read_ok(row, what, input);
        assert_eq!(
            (
                element(row, &doc, "b").kind(),
                parent_of(row, &doc, "b").as_deref()
            ),
            (kind, *parent),
            "{row}: {what}: not an ArgumentPackageBinding under {parent:?}"
        );
    }
    // A binding holding non-citation content reads and writes in both modes
    // (Q2): its content rules are slice 0068's.
    let what = "a binding holding a non-citation AssertedInference";
    let doc = read_ok(
        row,
        what,
        &b_in_case(
            " participantPackage=\"p q\" gid=\"B-1\" isAbstract=\"true\"",
            &format!(
                "<note xmi:id=\"b_n\"><content><value content=\"N\"/></content></note>{}{}{}",
                claim("b1", ""),
                claim("b2", ""),
                argument(
                    "AssertedInference",
                    "b_link",
                    " source=\"b1\" target=\"b2\"",
                    ""
                )
            ),
        ),
    );
    assert_eq!(
        (
            children(row, &doc, "b"),
            element(row, &doc, "b").gid(),
            element(row, &doc, "b").is_abstract(),
            element(row, &doc, "b_link").target().map(ElementId::as_str)
        ),
        (
            vec![
                "b_n".to_owned(),
                "b1".to_owned(),
                "b2".to_owned(),
                "b_link".to_owned()
            ],
            Some("B-1"),
            true,
            Some("b2")
        ),
        "{row}: {what}: what it holds"
    );
    for mode in MODES {
        round_trip(row, what, &doc, mode);
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_arg_015_participants_read_in_attribute_and_element_form_in_order() {
    let row = ARG_015;
    let attribute = read_ok(
        row,
        "participantPackage=\"q p\"",
        &b_in_case(" participantPackage=\"q p\"", ""),
    );
    let elements = read_ok(
        row,
        "participantPackage elements",
        &b_in_case(
            "",
            "<participantPackage xmi:idref=\"q\"/><participantPackage href=\"#p\"/>",
        ),
    );
    assert!(
        elements == attribute,
        "{row}: the element forms do not read == to the attribute"
    );
    assert_eq!(
        element(row, &attribute, "b").participant_packages(),
        list(&["q", "p"]).as_slice(),
        "{row}: participant_packages() in order"
    );
    // An attribute and elements: RepeatedValue in strict mode, the repeat kept
    // in tolerant mode (LIB-COMPAT-001).
    let what = "an attribute and a participant element";
    let (input, marks) = marked(&b_in_case(
        &format!(" {MARK}participantPackage=\"p\""),
        &format!("{MARK}<participantPackage xmi:idref=\"q\"/>"),
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
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.requirement_id().to_owned(),
                found.location(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "LIB-COMPAT-001".to_owned(),
            marks.get(1).copied()
        )],
        "{row}: {what}: a tolerant read does not give one SACM-MULT-001 Warning at the repeat"
    );
    assert_eq!(
        element(row, output.document(), "b").participant_packages(),
        list(&["p"]).as_slice(),
        "{row}: {what}: a tolerant read does not keep the attribute"
    );
    // "p p q": held as three ids; one SACM23-VAL-022 Warning.
    let what = "participantPackage=\"p p q\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"p p q\"", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "11.5",
            &["b", "p"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 11.5"
    );
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["p", "p", "q"]).as_slice(),
        "{row}: {what}: the repeat is not held"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "b").ends_with(" participantPackage=\"p p q\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
    // An empty or qualified attribute is preserved content of the binding.
    for (what, attribute) in [
        (
            "participantPackage=\"\"",
            format!(" {MARK}participantPackage=\"\""),
        ),
        (
            "sacm:participantPackage",
            format!(" {MARK}sacm:participantPackage=\"p q\""),
        ),
    ] {
        assert_preserved_at(row, what, &b_in_case(&attribute, ""), "b");
    }
}

#[test]
fn sacm23_arg_015_too_few_participants_read_and_write_back() {
    let row = ARG_015;
    for (what, attribute) in [
        ("no participant", ""),
        ("one participant", " participantPackage=\"p\""),
        ("one distinct participant", " participantPackage=\"q q\""),
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
                "11.5",
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
fn sacm23_arg_015_dangling_and_wrongly_typed_participants() {
    let row = ARG_015;
    let what = "participantPackage=\"p gone\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"p gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "11.5",
            &["b", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 11.5"
    );
    assert_eq!(
        argument_of(&found[0], "type"),
        Some("ArgumentPackage"),
        "{row}: {what}: the end's type"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "b", " participantPackage=\"p gone\">");
    // An ArgumentPackage of any kind gives no reference finding; since slice
    // 0068 the binding b2 among them gives one SACM-PKG-001 Warning
    // (D-BIND-PART, SACM23-VAL-014).
    let doc = read_ok(
        row,
        "participants of every ArgumentPackage kind",
        &case(
            "",
            &interface("i", " implements=\"p\"", ""),
            "",
            &format!(
                "{}{}",
                binding("b", " participantPackage=\"p i b2\"", ""),
                binding("b2", " participantPackage=\"p q\"", "")
            ),
        ),
    );
    assert_eq!(
        findings(&doc),
        [finding_of(
            Code::PACKAGE_CONTENT_INVALID,
            Severity::Warning,
            "SACM23-VAL-014",
            "11.5",
            &["b", "b2"]
        )],
        "{row}: participants of every ArgumentPackage kind"
    );
    for (target, class) in [
        ("c", "Claim"),
        ("evidence", "ArtifactPackage"),
        ("evidence_api", "ArtifactPackageInterface"),
        ("case", "AssuranceCasePackage"),
    ] {
        let what = format!("participantPackage=\"p {target}\" (a {class})");
        let doc = read_ok(
            row,
            &what,
            &b_in_case(&format!(" participantPackage=\"p {target}\""), ""),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "11.5",
                &["b", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 11.5"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("ArgumentPackage"),
            "{row}: {what}: the end's type"
        );
    }
}

#[test]
fn sacm23_arg_015_writes_attribute_order_and_the_golden_file() {
    let row = ARG_015;
    let what = "a binding with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case(
            "",
            &interface("i", " implements=\"p\"", ""),
            "",
            &binding(
                "b",
                " participantPackage=\"q i\" interface=\"i\" abstractForm=\"b\" citedElement=\"b\" \
                 isAbstract=\"true\" isCitation=\"true\" gid=\"B-1\"",
                "",
            ),
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<argumentPackage xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"b\" gid=\"B-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"b\" abstractForm=\"b\" \
         interface=\"i\" participantPackage=\"q i\">",
        "{row}: {what}: not written in order\n{text}"
    );
    // Elements are written as one attribute, in order.
    let doc = read_ok(
        row,
        "participant elements",
        &b_in_case(
            "",
            "<participantPackage href=\"#q\"/><participantPackage xmi:idref=\"p\"/>",
        ),
    );
    let text = round_trip(row, "participant elements", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<argumentPackage xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"b\" \
         participantPackage=\"q p\">",
        "{row}: the participant elements are not written as one attribute\n{text}"
    );
    // The golden file is the shared fixture's
    // (sacm23_arg_014_the_fixture_reads_validates_and_writes_exactly_the_golden_file).
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<argumentPackage xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"binding\" \
             participantPackage=\"braking_api vehicle_api\">"
        ),
        "{row}: the golden file has no binding line"
    );
}

#[test]
fn sacm23_arg_015_reference_end_tables_bounds_and_serde() {
    let row = ARG_015;
    let end = ReferenceEnd::ParticipantPackage;
    assert_eq!(end.name(), "participantPackage", "{row}: the name");
    // Since slice 0067 the Terminology and AssuranceCase bindings have the
    // end too (SACM23-TERM-008, SACM23-AC-003).
    assert_eq!(
        (
            end.bounds(ElementKind::ArgumentPackageBinding),
            end.bounds(ElementKind::ArtifactPackageBinding),
            end.bounds(ElementKind::ArgumentPackageInterface),
            end.bounds(ElementKind::ArgumentPackage),
        ),
        (Some((2, None)), Some((2, None)), None, None),
        "{row}: the bounds"
    );
    for &kind in ElementKind::ALL {
        assert_eq!(
            end.applies_to(kind),
            [
                ElementKind::AssuranceCasePackageBinding,
                ElementKind::ArgumentPackageBinding,
                ElementKind::ArtifactPackageBinding,
                ElementKind::TerminologyPackageBinding,
            ]
            .contains(&kind),
            "{row}: applies_to on a {kind}"
        );
    }
    #[cfg(feature = "serde")]
    {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, "\"participantPackage\"", "{row}: the serde name");
        let back: ReferenceEnd = serde_json::from_str(&written)
            .unwrap_or_else(|error| panic!("{row}: {written} does not deserialize: {error}"));
        assert_eq!(back, end, "{row}: the serde name reads back as another end");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
fn sacm23_arg_015_create_reports_what_it_holds_and_checks_in_order() {
    let row = ARG_015;
    let operation =
        CreateArgumentPackageBinding::root(named("Braking supports the vehicle"), ["p", "q"]);
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
            &named("Braking supports the vehicle"),
            vec!["p", "q"]
        ),
        "{row}: root()"
    );
    let operation =
        CreateArgumentPackageBinding::new("case", named("B"), ["q", "p", "q"]).with_id("b");
    assert_eq!(
        (
            operation.parent().map(ElementId::as_str),
            operation.id().map(ElementId::as_str),
            strs(operation.participant_packages())
        ),
        (Some("case"), Some("b"), vec!["q", "p", "q"]),
        "{row}: new() and with_id() keep the participants in order with repeats"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateArgumentPackageBinding",
        "{row}: operation_name()"
    );
    let mut doc = read_ok(row, "the case", &case("", "", "", ""));
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            strs(mutation.changed_ids()),
            parent_of(row, &doc, "b"),
            element(row, &doc, "b").participant_packages(),
        ),
        (
            vec!["b"],
            vec![],
            Some("case".to_owned()),
            list(&["q", "p", "q"]).as_slice()
        ),
        "{row}: the created binding"
    );
    let text = write_in(row, "the created binding", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<argumentPackage xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"b\" \
         participantPackage=\"q p q\">",
        "{row}: the created binding's start tag\n{text}"
    );
    // The generated ids and the legal parents.
    let mut generated = Vec::new();
    for parent in ["case", "p", "evidence", "b", "evidence_api"] {
        let mutation = applied(
            row,
            &mut doc,
            CreateArgumentPackageBinding::new(parent, named("N"), ["p", "q"]),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let root = applied(
        row,
        &mut doc,
        CreateArgumentPackageBinding::root(named("N"), ["p", "q"]),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=6)
            .map(|n| format!("argument_package_binding_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (parent, parent_kind) in [
        ("c", ElementKind::Claim),
        ("terms", ElementKind::TerminologyPackage),
    ] {
        let error = refuse(
            row,
            &mut doc,
            CreateArgumentPackageBinding::new(parent, named("N"), ["p", "q"]),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: pk, kind: ElementKind::ArgumentPackageBinding, .. }
                    if found.as_str() == parent && *pk == parent_kind
            ),
            "{row}: a binding under the {parent_kind} {parent}: {error:?}"
        );
    }
    // The check order: 1-4, then TooFewReferences, then 5a, then 6-8 over
    // the participants in order, then the name's expression.
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, participants: &[&str]| {
        CreateArgumentPackageBinding::new(parent, name, participants.iter().copied())
    };
    let cases: Vec<(&str, CreateArgumentPackageBinding, _)> = vec![
        (
            "a missing parent",
            create("nowhere", unnamed.clone(), &["a b"]).with_id("1b"),
            k("ParentNotFound", None, None),
        ),
        (
            "an illegal parent",
            create("c", unnamed.clone(), &["a b"]).with_id("1b"),
            k("InvalidParent", None, None),
        ),
        (
            "an invalid id",
            create("case", unnamed.clone(), &["a b"]).with_id("1b"),
            k("InvalidId", None, None),
        ),
        (
            "a duplicate id",
            create("case", unnamed.clone(), &["a b"]).with_id("c"),
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
            create(
                "case",
                named("N").with_expression("nowhere"),
                &["nope", "a b"],
            ),
            k("InvalidReference", Some("participantPackage"), Some("a b")),
        ),
        (
            "a missing participant before the name's expression",
            create(
                "case",
                named("N").with_expression("nowhere"),
                &["p", "nope"],
            ),
            k(
                "ReferenceNotFound",
                Some("participantPackage"),
                Some("nope"),
            ),
        ),
        (
            "a missing participant before a wrong type",
            create("case", named("N"), &["c", "nope"]),
            k(
                "ReferenceNotFound",
                Some("participantPackage"),
                Some("nope"),
            ),
        ),
        (
            "the name's missing expression",
            create("case", named("N").with_expression("nowhere"), &["p", "q"]),
            k("ReferenceNotFound", Some("expression"), Some("nowhere")),
        ),
        (
            "a Claim participant",
            create("case", named("N"), &["p", "c"]),
            k("WrongReferenceType", Some("participantPackage"), Some("c")),
        ),
        (
            "an ArtifactPackage participant",
            create("case", named("N"), &["evidence", "p"]),
            k(
                "WrongReferenceType",
                Some("participantPackage"),
                Some("evidence"),
            ),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "ArgumentPackage", "{row}: {what}: expected");
        }
    }
    for (what, participants, given) in [
        ("no participant", Vec::new(), 0),
        ("one participant", vec!["p"], 1),
        ("one distinct participant", vec!["p", "p"], 1),
        (
            "one distinct participant, three times",
            vec!["q", "q", "q"],
            1,
        ),
    ] {
        let error = refuse(row, &mut doc, create("case", named("N"), &participants));
        assert!(
            matches!(
                &error,
                EditError::TooFewReferences { end: "participantPackage", given: g, required: 2, .. }
                    if *g == given
            ),
            "{row}: {what}: not TooFewReferences {{ participantPackage, {given}, 2 }}: {error:?}"
        );
    }
    let next = applied(
        row,
        &mut doc,
        CreateArgumentPackageBinding::new("case", named("N"), ["p", "q", "p"]),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["argument_package_binding_7"],
        "{row}: a refusal moved the next generated id"
    );
}

#[test]
fn sacm23_arg_015_reference_edits_count_distinct_participants() {
    let row = ARG_015;
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
        "b of [p, q]",
        &b_in_case(" participantPackage=\"p q\"", ""),
    );
    for (what, ids) in [("[p]", vec!["p"]), ("[p, p]", vec!["p", "p"])] {
        let error = refuse(row, &mut doc, SetReferences::new("b", end, ids));
        assert!(too_few(&error, 1), "{row}: SetReferences {what}: {error:?}");
    }
    let error = refuse(row, &mut doc, RemoveReference::new("b", end, "q"));
    assert!(too_few(&error, 1), "{row}: removing one of two: {error:?}");
    let mutation = applied(row, &mut doc, AddReference::new("b", end, "p"));
    assert_eq!(
        (
            strs(mutation.changed_ids()),
            element(row, &doc, "b").participant_packages()
        ),
        (vec!["b"], list(&["p", "q", "p"]).as_slice()),
        "{row}: AddReference of a held participant"
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "11.5",
            &["b", "p"]
        )],
        "{row}: the repeat warns"
    );
    let error = refuse(row, &mut doc, RemoveReference::new("b", end, "p"));
    assert!(
        too_few(&error, 1),
        "{row}: removing p of [p, q, p] leaves one distinct participant: {error:?}"
    );
    // A binding read with one participant: adding it again is refused,
    // adding another succeeds.
    let mut doc = read_ok(row, "b of [p]", &b_in_case(" participantPackage=\"p\"", ""));
    let error = refuse(row, &mut doc, AddReference::new("b", end, "p"));
    assert!(
        too_few(&error, 1),
        "{row}: AddReference p onto [p]: {error:?}"
    );
    applied(row, &mut doc, AddReference::new("b", end, "q"));
    assert_eq!(
        element(row, &doc, "b").participant_packages(),
        list(&["p", "q"]).as_slice(),
        "{row}: AddReference q onto [p]"
    );
    for (ids, wrong) in [(["p", "c"], "c"), (["p", "evidence"], "evidence")] {
        let error = refuse(row, &mut doc, SetReferences::new("b", end, ids));
        assert!(
            matches!(
                &error,
                EditError::WrongReferenceType { end: "participantPackage", id, expected: "ArgumentPackage", .. }
                    if id.as_str() == wrong
            ),
            "{row}: participants [p, {wrong}]: {error:?}"
        );
    }
    let error = refuse(row, &mut doc, SetReferences::new("p", end, ["p", "q"]));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::ArgumentPackage,
                feature: "participantPackage",
                ..
            }
        ),
        "{row}: participantPackage on an ArgumentPackage: {error:?}"
    );
}

#[test]
fn sacm23_arg_015_compare_reports_participant_packages() {
    let row = ARG_015;
    let with = |participants: &str| {
        case(
            "",
            &interface("i", " implements=\"p\"", ""),
            "",
            &format!(
                "<argumentPackage xmi:id=\"r\"><name content=\"R\"/>{}</argumentPackage>{}",
                interface("j", " implements=\"r\"", ""),
                binding("b", &format!(" participantPackage=\"{participants}\""), "")
            ),
        )
    };
    let left = read_ok(row, "left", &with("i j"));
    let right = read_ok(row, "right", &with("j i"));
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
fn sacm23_arg_015_homogeneity_exemption_on_the_read_path() {
    let row = ARG_015;
    // SACM23-VAL-012's findings: since slice 0068 the content of a binding is
    // SACM23-VAL-013's, with the same code.
    let homogeneity = |doc: &Document| -> Vec<Finding> {
        findings(doc)
            .into_iter()
            .filter(|found| found.0 == Code::PACKAGE_CONTENT_INVALID && found.2 == "SACM23-VAL-012")
            .collect()
    };
    let b = |body: &str| binding_in("argumentElement", "b", " participantPackage=\"p q\"", body);
    let q = argument("ArgumentPackage", "q", "", "");
    let c = claim("c", "");
    for (what, body, want) in [
        ("p{b, c}", format!("{}{c}", b("")), Vec::new()),
        (
            "p{q, b, c}",
            format!("{q}{}{c}", b("")),
            vec![expected(
                Code::PACKAGE_CONTENT_INVALID,
                "SACM23-VAL-012",
                "11.4",
                &["p", "c"],
            )],
        ),
        ("b{q, c} in p", b(&format!("{q}{c}")), Vec::new()),
    ] {
        let doc = read_ok(row, what, &root_p(&body));
        assert_eq!(
            homogeneity(&doc),
            want,
            "{row}: {what}: the SACM-PKG-001 findings"
        );
    }
}

#[test]
fn sacm23_arg_015_move_copy_and_the_element_edits_apply() {
    let row = ARG_015;
    let mut doc = read_ok(
        row,
        "b in case",
        &b_in_case(
            " participantPackage=\"p q\"",
            &format!("{}{}", claim("b1", ""), claim("b2", "")),
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
        (
            "into evidence",
            MoveElement::new("b", "evidence"),
            Some("evidence"),
        ),
        ("into p", MoveElement::new("b", "p"), Some("p")),
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
    let error = refuse(row, &mut doc, MoveElement::new("b", "terms"));
    assert!(
        matches!(
            &error,
            EditError::InvalidParent {
                kind: ElementKind::ArgumentPackageBinding,
                ..
            }
        ),
        "{row}: a binding into a TerminologyPackage: {error:?}"
    );
    let copy = CopyElements::new(&doc, ["b"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of b: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let copied = copy_of(&pairs, "b");
    assert_eq!(
        (
            element(row, &doc, &copied).participant_packages(),
            element(row, &doc, &copied).kind(),
            children(row, &doc, &copied).len()
        ),
        (
            list(&["p", "q"]).as_slice(),
            ElementKind::ArgumentPackageBinding,
            2
        ),
        "{row}: the copy of b keeps its participants as written"
    );
}
