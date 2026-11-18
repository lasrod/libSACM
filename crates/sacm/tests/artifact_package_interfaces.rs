//! Tests for `ArtifactPackageInterface` and `ArtifactPackageBinding`, with the
//! ends `interface`, `implements` and `participantPackage` (SACM23-ART-012,
//! SACM23-ART-013; formal/23-05-08 §12.4
//! "`participantPackage:ArtifactPackage[2..*]` - the `ArtifactPackages`
//! containing the `ArtifactAssets` being related together by the
//! `ArtifactPackageBinding`.", §12.5 "`implements:ArtifactPackage[1]` - a
//! reference to the ArtifactPackage which the `ArtifactPackageInterface`
//! declares."; ptc/22-03-13 `ArtifactPackage::interface` [0..*]; the M5
//! decisions Q1, Q2, Q7, Q8 and Q9; ADR 0039).
//!
//! Both kinds are `ArtifactPackage`s by inheritance: they read typed in
//! `artifactPackage` and `artifactElement` and as roots, hold their children
//! in `artifactElement`, and carry the package's `interface` end. In an
//! ArgumentPackage's `argumentElement`, which admits no ArtifactPackage, an
//! `ArtifactPackageInterface` is kept whole as preserved content of the
//! package. The ends are kept as written and never synced (Q1); a missing
//! `implements` and fewer than two participants give no finding in this
//! slice; a lower bound counts distinct ids (Q7). §11.4's homogeneity rule
//! does not apply to an ArtifactPackage.
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
    self, AddReference, CopyElements, CreateArtifactPackageBinding, CreateArtifactPackageInterface,
    EditError, MoveElement, Mutation, Operation, RemoveReference, ReorderElements, SetFlag, SetGid,
    SetName, SetReferences,
};
use sacm::model::{Flag, LangString, ReferenceEnd};

mod common;

const ART_012: &str = "SACM23-ART-012";
const ART_013: &str = "SACM23-ART-013";

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

/// The artifact fixture of slice 0066 and its golden output.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-package-interfaces-valid.sacm.xmi"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/artifact-package-interfaces.golden.sacm.xmi"
);

// --- Inputs --------------------------------------------------------------------------

/// An `ArtifactPackageInterface` `id` in the role `role`, with `attributes`,
/// named after its id, holding `body`.
fn interface_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// An `ArtifactPackageBinding` `id` in the role `role`, with `attributes`,
/// named after its id, holding `body`.
fn binding_in(role: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<{role} xmi:type=\"sacm:ArtifactPackageBinding\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</{role}>"
    )
}

/// An `ArtifactPackageInterface` `id` in an ArtifactPackage's `artifactElement`.
fn interface(id: &str, attributes: &str, body: &str) -> String {
    interface_in("artifactElement", id, attributes, body)
}

/// An `ArtifactPackageBinding` `id` in an AssuranceCasePackage's `artifactPackage`.
fn binding(id: &str, attributes: &str, body: &str) -> String {
    binding_in("artifactPackage", id, attributes, body)
}

/// A typed `artifactElement` of the kind `kind`, `id`, with `attributes`,
/// named after its id, holding `body`.
fn artifact_element(kind: &str, id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</artifactElement>"
    )
}

/// An Artifact `id` with `attributes`.
fn artifact(id: &str, attributes: &str) -> String {
    artifact_element("Artifact", id, attributes, "")
}

/// The AssuranceCasePackage `case`, one element per line:
/// - the ArtifactPackage `t`, with `t_attributes`, holding the Artifact
///   `report`, then `t_body`;
/// - the ArtifactPackage `u`, holding the Artifact `spec`;
/// - the ArgumentPackage `p`, holding the Claim `c` and the
///   `ArgumentPackageInterface` `p_api` (implementing `p`), then `p_body`;
/// - the empty TerminologyPackage `terms`;
/// - then `extra`.
fn case(t_attributes: &str, t_body: &str, p_body: &str, extra: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">
  <name content=\"Case\"/>
  <artifactPackage xmi:id=\"t\"{t_attributes}><name content=\"T\"/>
    {report}
    {t_body}
  </artifactPackage>
  <artifactPackage xmi:id=\"u\"><name content=\"U\"/>
    {spec}
  </artifactPackage>
  <argumentPackage xmi:id=\"p\"><name content=\"P\"/>
    <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><name content=\"C\"/></argumentElement>
    <argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"p_api\" implements=\"p\"><name content=\"P API\"/></argumentElement>
    {p_body}
  </argumentPackage>
  <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/></terminologyPackage>
  {extra}
</sacm:AssuranceCasePackage>",
        report = artifact("report", ""),
        spec = artifact("spec", ""),
    )
}

/// [`case`] with the interface `i` in `t`, with `attributes` and `body`.
fn i_in_t(attributes: &str, body: &str) -> String {
    case("", &interface("i", attributes, body), "", "")
}

/// [`case`] with the binding `b` in `case`, with `attributes` and `body`.
fn b_in_case(attributes: &str, body: &str) -> String {
    case("", "", "", &binding("b", attributes, body))
}

/// An ArtifactPackage root `t` holding `body`.
fn root_t(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"t\"><name content=\"T\"/>{body}</sacm:ArtifactPackage>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!(
            "SACM23-ART-012: setup: SourceLocation::new({line}, {column}) refused a 1-based position"
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
        .unwrap_or_else(|error| panic!("SACM23-ART-012: cannot read {path}: {error}"))
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
    let input = root_t("").replacen(
        "<name content=\"T\"/>",
        "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\"/>",
        1,
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
        None => panic!("SACM23-ART-012: {source:?} was not copied: {pairs:?}"),
    }
}

// =====================================================================================
// SACM23-ART-012: ArtifactPackageInterface
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_art_012_reads_typed_in_every_role_and_as_a_root() {
    let row = ART_012;
    let kind = ElementKind::ArtifactPackageInterface;
    let roles = [
        (
            "artifactElement in an ArtifactPackage",
            i_in_t(" implements=\"t\"", ""),
            Some("t"),
        ),
        (
            "artifactPackage in an AssuranceCasePackage",
            case(
                "",
                "",
                "",
                &interface_in("artifactPackage", "i", " implements=\"t\"", ""),
            ),
            Some("case"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:ArtifactPackageInterface{NS} xmi:id=\"i\" implements=\"i\"><name content=\"i\"/>\
                 </sacm:ArtifactPackageInterface>"
            ),
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArtifactPackage xmi:id=\"t\"><name content=\"T\"/></sacm:ArtifactPackage>\
                 <sacm:ArtifactPackageInterface xmi:id=\"i\" implements=\"t\"><name content=\"i\"/>\
                 </sacm:ArtifactPackageInterface></xmi:XMI>"
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
            "{row}: {what}: not an ArtifactPackageInterface under {parent:?}"
        );
    }
    assert!(
        kind.is_kind_of(ElementKind::ArtifactPackage),
        "{row}: an ArtifactPackageInterface is no ArtifactPackage"
    );
    // Its inherited features, and its children in artifactElement.
    let what = "an interface with every inherited feature";
    let doc = read_ok(
        row,
        what,
        &i_in_t(
            " implements=\"t\" gid=\"I-1\" isCitation=\"true\" citedElement=\"u\" \
             isAbstract=\"true\" abstractForm=\"i\"",
            &format!(
                "<description xmi:id=\"i_d\"><content><value content=\"D\"/></content></description>\
                 <note xmi:id=\"i_n\"><content><value content=\"N\"/></content></note>\
                 <taggedValue xmi:id=\"i_tv\"><key><value content=\"K\"/></key></taggedValue>\
                 {}{}",
                artifact("ia", " isCitation=\"true\" citedElement=\"report\""),
                artifact_element("ArtifactPackage", "it", "", "")
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
        ["i_d", "i_n", "i_tv", "ia", "it"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "{row}: {what}: the children"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "ia").starts_with("<artifactElement xmi:type=\"sacm:Artifact\""),
        "{row}: {what}: the interface's Artifact is not written in artifactElement\n{text}"
    );
    // In an ArgumentPackage's argumentElement, which admits no
    // ArtifactPackage: kept whole as preserved content of the package.
    let template = case(
        "",
        "",
        &format!(
            "{MARK}{}",
            interface_in(
                "argumentElement",
                "x",
                " implements=\"t\"",
                &artifact("xa", "")
            )
        ),
        "",
    );
    let doc = assert_preserved_at(row, "an interface in argumentElement", &template, "p");
    assert!(
        doc.get("x").is_none() && doc.get("xa").is_none(),
        "{row}: the interface in argumentElement was read as an element"
    );
}

#[test]
fn sacm23_art_012_implements_and_interface_read_in_attribute_and_element_form() {
    let row = ART_012;
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
        &case(" interface=\"i1 i2\"", &both, "", ""),
    );
    let elements = read_ok(
        row,
        "interface elements on t",
        &case(
            "",
            &format!("<interface xmi:idref=\"i1\"/><interface href=\"#i2\"/>{both}"),
            "",
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

#[test]
fn sacm23_art_012_repeated_empty_and_qualified_implements_follow_the_reference_rules() {
    let row = ART_012;
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
        let _ = first;
    }
    for (what, attribute) in [
        ("implements=\"\"", format!(" {MARK}implements=\"\"")),
        ("sacm:implements", format!(" {MARK}sacm:implements=\"t\"")),
    ] {
        assert_preserved_at(row, what, &i_in_t(&attribute, ""), "i");
    }
    let what = "interface=\"i i\"";
    let doc = read_ok(
        row,
        what,
        &case(
            " interface=\"i i\"",
            &interface("i", " implements=\"t\"", ""),
            "",
            "",
        ),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "12.2",
            &["t", "i"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 12.2"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "t").ends_with(" interface=\"i i\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_art_012_missing_implements_reads_and_writes_back_unchanged() {
    let row = ART_012;
    let what = "an interface with no implements";
    let doc = read_ok(row, what, &i_in_t("", ""));
    assert_eq!(
        findings(&doc),
        [],
        "{row}: {what}: validate reports a finding in this slice"
    );
    for mode in MODES {
        let text = round_trip(row, what, &doc, mode);
        assert!(
            line_with(row, &text, "i").ends_with("xmi:id=\"i\">"),
            "{row}: {what}: a {mode:?} write added to the start tag\n{text}"
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_art_012_dangling_and_wrongly_typed_ends_report_their_clause_and_type() {
    let row = ART_012;
    let what = "implements=\"gone\"";
    let doc = read_ok(row, what, &i_in_t(" implements=\"gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "12.5",
            &["i", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 12.5"
    );
    assert_eq!(
        found[0].message(),
        "ArtifactPackageInterface \"i\" has the implements \"gone\", which names no element of \
         the document (the end's type is ArtifactPackage)",
        "{row}: {what}: the message"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "i", " implements=\"gone\">");
    let with_binding = |attributes: &str| {
        case(
            "",
            &interface("i", attributes, ""),
            "",
            &binding("b", " participantPackage=\"t u\"", ""),
        )
    };
    for target in ["t", "u", "i", "b"] {
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
        ("report", "Artifact"),
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
                "12.5",
                &["i", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 12.5"
        );
        assert_eq!(
            (
                argument_of(&found[0], "type"),
                argument_of(&found[0], "target_class")
            ),
            (Some("ArtifactPackage"), Some(class)),
            "{row}: {what}: the end's type and the target's class"
        );
    }
    for (target, class) in [
        ("report", "Artifact"),
        ("b", "ArtifactPackageBinding"),
        ("u", "ArtifactPackage"),
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
                &binding("b", " participantPackage=\"t u\"", ""),
            ),
        );
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [expected(
                Code::REF_WRONG_TYPE,
                "SACM23-VAL-004",
                "12.2",
                &["t", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 12.2"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("ArtifactPackageInterface"),
            "{row}: {what}: the end's type"
        );
    }
    let what = "interface=\"gone\" on t";
    let doc = read_ok(row, what, &case(" interface=\"gone\"", "", "", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "12.2",
            &["t", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 12.2"
    );
}

#[test]
fn sacm23_art_012_writes_attribute_order_and_the_golden_file() {
    let row = ART_012;
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
        ),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<artifactElement xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"i\" gid=\"I-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"i\" abstractForm=\"i\" \
         interface=\"i\" implements=\"t\">",
        "{row}: {what}: not written in order\n{text}"
    );
    assert_eq!(
        line_with(row, &text, "t"),
        "<artifactPackage xmi:id=\"t\" gid=\"T-1\" abstractForm=\"t\" interface=\"i\">",
        "{row}: {what}: the package's interface is not written last\n{text}"
    );
    let doc = read_ok(
        row,
        "an interface in each role",
        &case(
            "",
            &interface("i1", " implements=\"t\"", ""),
            "",
            &interface_in("artifactPackage", "i2", " implements=\"t\"", ""),
        ),
    );
    let text = round_trip(row, "an interface in each role", &doc, Mode::Strict);
    for (id, role) in [("i1", "artifactElement"), ("i2", "artifactPackage")] {
        assert_eq!(
            line_with(row, &text, id),
            format!(
                "<{role} xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"{id}\" implements=\"t\">"
            ),
            "{row}: {id} is not written typed in {role}\n{text}"
        );
    }
}

#[test]
fn sacm23_art_012_the_fixture_reads_validates_and_writes_exactly_the_golden_file() {
    let row = ART_012;
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
            element(row, &doc, "binding_verifies").kind()
        ),
        (
            ElementKind::ArtifactPackageBinding,
            vec![
                "binding_report".to_owned(),
                "binding_spec".to_owned(),
                "binding_verifies".to_owned()
            ],
            ElementKind::ArtifactAssetRelationship
        ),
        "{ART_013}: the fixture's binding"
    );
    assert_eq!(
        (
            element(row, &doc, "tests").interfaces(),
            element(row, &doc, "tests_api")
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "binding").participant_packages(),
        ),
        (
            list(&["tests_api"]).as_slice(),
            Some("tests"),
            list(&["tests_api", "requirements_api"]).as_slice(),
        ),
        "{row}: the fixture's ends"
    );
}

#[test]
fn sacm23_art_012_reference_end_tables_bounds_and_serde() {
    let row = ART_012;
    let (interface, implements) = (ReferenceEnd::Interface, ReferenceEnd::Implements);
    assert_eq!(
        (
            implements.bounds(ElementKind::ArtifactPackageInterface),
            implements.bounds(ElementKind::ArtifactPackage),
            implements.bounds(ElementKind::ArtifactPackageBinding),
            interface.bounds(ElementKind::ArtifactPackage),
            interface.bounds(ElementKind::ArtifactPackageInterface),
            interface.bounds(ElementKind::ArtifactPackageBinding),
            interface.bounds(ElementKind::Artifact),
        ),
        (
            Some((1, Some(1))),
            None,
            None,
            Some((0, None)),
            Some((0, None)),
            Some((0, None)),
            None
        ),
        "{row}: the bounds on the Artifact kinds"
    );
    #[cfg(feature = "serde")]
    for (end, name) in [(interface, "\"interface\""), (implements, "\"implements\"")] {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, name, "{row}: the serde name of {end:?}");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the check order of one create reads best as one list"
)]
fn sacm23_art_012_create_reports_what_it_holds_and_checks_in_order() {
    let row = ART_012;
    let operation = CreateArtifactPackageInterface::root(named("Brake tests interface"), "t");
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            operation.name(),
            operation.implements().as_str()
        ),
        (None, None, &named("Brake tests interface"), "t"),
        "{row}: root()"
    );
    let operation = CreateArtifactPackageInterface::new("t", named("I"), "t").with_id("i");
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateArtifactPackageInterface",
        "{row}: operation_name()"
    );
    let mut doc = read_ok(row, "the case", &case("", "", "", ""));
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            strs(mutation.created_ids()),
            strs(mutation.changed_ids()),
            parent_of(row, &doc, "i"),
            element(row, &doc, "i").implements().map(ElementId::as_str),
            element(row, &doc, "t").interfaces().len(),
        ),
        (vec!["i"], vec![], Some("t".to_owned()), Some("t"), 0),
        "{row}: the created interface; t's interface end is unchanged"
    );
    let text = round_trip(row, "the created interface", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "i"),
        "<artifactElement xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"i\" implements=\"t\">",
        "{row}: the created interface's start tag\n{text}"
    );
    let mut doc = read_ok(
        row,
        "the case with a binding",
        &case("", "", "", &binding("b", " participantPackage=\"t u\"", "")),
    );
    let mut generated = Vec::new();
    for parent in ["case", "t", "b"] {
        let mutation = applied(
            row,
            &mut doc,
            CreateArtifactPackageInterface::new(parent, named("N"), "u"),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let first = generated[0].clone();
    let mutation = applied(
        row,
        &mut doc,
        CreateArtifactPackageInterface::new(first.as_str(), named("N"), "u"),
    );
    generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    let root = applied(
        row,
        &mut doc,
        CreateArtifactPackageInterface::root(named("Root"), "u"),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=5)
            .map(|n| format!("artifact_package_interface_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (parent, parent_kind) in [
        ("p", ElementKind::ArgumentPackage),
        ("p_api", ElementKind::ArgumentPackageInterface),
        ("terms", ElementKind::TerminologyPackage),
        ("report", ElementKind::Artifact),
    ] {
        let error = refuse(
            row,
            &mut doc,
            CreateArtifactPackageInterface::new(parent, named("N"), "u"),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::ArtifactPackageInterface
            ),
            "{row}: an interface under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, implements: &str| {
        CreateArtifactPackageInterface::new(parent, name, implements)
    };
    let cases: Vec<(&str, CreateArtifactPackageInterface, _)> = vec![
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
            create("t", unnamed.clone(), "a b").with_id("report"),
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
            "an Artifact as implements",
            create("t", named("N"), "report"),
            k("WrongReferenceType", Some("implements"), Some("report")),
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
            assert_eq!(*expected, "ArtifactPackage", "{row}: {what}: expected");
        }
    }
    let next = applied(
        row,
        &mut doc,
        CreateArtifactPackageInterface::new("t", named("N"), "u"),
    );
    assert_eq!(
        strs(next.created_ids()),
        ["artifact_package_interface_6"],
        "{row}: a refusal moved the next generated id"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_art_012_reference_edits_keep_implements_at_exactly_one() {
    let row = ART_012;
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
                expected: "ArtifactPackage",
                ..
            }
        ),
        "{row}: implements [p]: {error:?}"
    );
    applied(
        row,
        &mut doc,
        SetReferences::new("t", ReferenceEnd::Interface, ["i"]),
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
                expected: "ArtifactPackageInterface",
                ..
            }
        ),
        "{row}: interface [p_api] on t: {error:?}"
    );
    let error = refuse(row, &mut doc, SetReferences::new("t", implements, ["u"]));
    assert_eq!(
        error.to_string(),
        "the ArtifactPackage \"t\" has no implements",
        "{row}: implements on an ArtifactPackage: {error:?}"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("report", ReferenceEnd::Interface, "i"),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::Artifact,
                feature: "interface",
                ..
            }
        ),
        "{row}: interface on an Artifact: {error:?}"
    );
}

#[test]
fn sacm23_art_012_interface_and_implements_are_never_synced() {
    let row = ART_012;
    let mut doc = read_ok(row, "the case", &case("", "", "", ""));
    applied(
        row,
        &mut doc,
        CreateArtifactPackageInterface::new("t", named("I"), "t").with_id("i"),
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
fn sacm23_art_012_compare_reports_interfaces_and_implements() {
    let row = ART_012;
    let left = read_ok(
        row,
        "left",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"t\"", ""),
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

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_art_012_move_copy_and_the_element_edits_apply() {
    let row = ART_012;
    let mut doc = read_ok(
        row,
        "i in t",
        &case(
            " interface=\"i\"",
            &interface("i", " implements=\"t\"", &artifact("ia", "")),
            "",
            "",
        ),
    );
    applied(row, &mut doc, SetName::new("i", named("Renamed")));
    applied(row, &mut doc, SetGid::new("i", "I-1"));
    applied(row, &mut doc, SetFlag::new("i", Flag::IsAbstract, true));
    applied(row, &mut doc, ReorderElements::new("t", ["i", "report"]));
    assert_eq!(
        children(row, &doc, "t"),
        ["i", "report"],
        "{row}: the reorder"
    );
    let ends = |doc: &Document| {
        (
            element(row, doc, "t").interfaces().to_vec(),
            element(row, doc, "i").implements().cloned(),
        )
    };
    let held = ends(&doc);
    let before = findings(&doc);
    for (what, operation, parent) in [
        ("to the roots", MoveElement::root("i"), None),
        ("into case", MoveElement::new("i", "case"), Some("case")),
        ("into u", MoveElement::new("i", "u"), Some("u")),
        ("back into t", MoveElement::new("i", "t"), Some("t")),
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
        ("p", ElementKind::ArgumentPackage),
        ("terms", ElementKind::TerminologyPackage),
    ] {
        let error = refuse(row, &mut doc, MoveElement::new("i", parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent_kind: found, kind: ElementKind::ArtifactPackageInterface, .. }
                    if *found == parent_kind
            ),
            "{row}: an ArtifactPackageInterface into the {parent_kind} {parent}: {error:?}"
        );
    }
    // Copying t follows the copied ids; copying i alone keeps implements t.
    let copy = CopyElements::new(&doc, ["t"], "case")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of t: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    let (t2, i2) = (copy_of(&pairs, "t"), copy_of(&pairs, "i"));
    assert_eq!(
        (
            element(row, &doc, &t2).interfaces().to_vec(),
            element(row, &doc, &i2).implements().cloned()
        ),
        (
            vec![ElementId::from(i2.as_str())],
            Some(ElementId::from(t2.as_str()))
        ),
        "{row}: the copy of t does not follow the copied ids: {pairs:?}"
    );
    let copy = CopyElements::new(&doc, ["i"], "u")
        .unwrap_or_else(|error| panic!("{row}: setup: the copy of i: {error}"));
    let mutation = applied(row, &mut doc, copy.clone());
    let pairs = copies(&copy, &mutation);
    assert_eq!(
        (
            element(row, &doc, &copy_of(&pairs, "i"))
                .implements()
                .map(ElementId::as_str),
            element(row, &doc, "u").interfaces()
        ),
        (Some("t"), [].as_slice()),
        "{row}: the copy of i under u"
    );
    let mut doc = read_ok(row, "an interface without implements", &i_in_t("", ""));
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
}

// =====================================================================================
// SACM23-ART-013: ArtifactPackageBinding
// =====================================================================================

#[test]
fn sacm23_art_013_reads_typed_in_every_role_and_as_a_root() {
    let row = ART_013;
    let kind = ElementKind::ArtifactPackageBinding;
    let attributes = " participantPackage=\"t u\"";
    let roles = [
        (
            "artifactPackage in an AssuranceCasePackage",
            b_in_case(attributes, ""),
            Some("case"),
        ),
        (
            "artifactElement in an ArtifactPackage",
            case(
                "",
                &binding_in("artifactElement", "b", attributes, ""),
                "",
                "",
            ),
            Some("t"),
        ),
        (
            "a bare root",
            format!(
                "<sacm:ArtifactPackageBinding{NS} xmi:id=\"b\" participantPackage=\"x y\">\
                 <name content=\"b\"/></sacm:ArtifactPackageBinding>"
            ),
            None,
        ),
        (
            "a root inside xmi:XMI",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArtifactPackage xmi:id=\"t\"><name content=\"T\"/></sacm:ArtifactPackage>\
                 <sacm:ArtifactPackageBinding xmi:id=\"b\" participantPackage=\"t t\">\
                 <name content=\"b\"/></sacm:ArtifactPackageBinding></xmi:XMI>"
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
            "{row}: {what}: not an ArtifactPackageBinding under {parent:?}"
        );
    }
    // Non-citation content reads and writes in both modes (Q2).
    let what = "a binding holding a non-citation ArtifactAssetRelationship";
    let doc = read_ok(
        row,
        what,
        &b_in_case(
            " participantPackage=\"t u\" gid=\"B-1\"",
            &format!(
                "{}{}{}",
                artifact("b1", ""),
                artifact("b2", ""),
                artifact_element(
                    "ArtifactAssetRelationship",
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
            element(row, &doc, "b_link").targets()
        ),
        (
            vec!["b1".to_owned(), "b2".to_owned(), "b_link".to_owned()],
            Some("B-1"),
            list(&["b2"]).as_slice()
        ),
        "{row}: {what}: what it holds"
    );
    for mode in MODES {
        round_trip(row, what, &doc, mode);
    }
}

#[test]
fn sacm23_art_013_participants_read_in_attribute_and_element_form_in_order() {
    let row = ART_013;
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
            "<participantPackage xmi:idref=\"u\"/><participantPackage href=\"#t\"/>",
        ),
    );
    assert!(
        elements == attribute,
        "{row}: the element forms do not read == to the attribute"
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
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "12.4",
            &["b", "t"]
        )],
        "{row}: {what}: not one SACM-MULT-001 Warning with clause 12.4"
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "b").ends_with(" participantPackage=\"t t u\">"),
        "{row}: {what}: the repeat is not written back\n{text}"
    );
}

#[test]
fn sacm23_art_013_too_few_participants_read_and_write_back() {
    let row = ART_013;
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
            [],
            "{row}: {what}: validate reports a finding in this slice"
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
fn sacm23_art_013_dangling_and_wrongly_typed_participants() {
    let row = ART_013;
    let what = "participantPackage=\"t gone\"";
    let doc = read_ok(row, what, &b_in_case(" participantPackage=\"t gone\"", ""));
    let found = validation::validate(&doc);
    assert_eq!(
        found.iter().map(finding).collect::<Vec<_>>(),
        [expected(
            Code::REF_DANGLING,
            "SACM23-VAL-003",
            "12.4",
            &["b", "gone"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 12.4"
    );
    assert_eq!(
        argument_of(&found[0], "type"),
        Some("ArtifactPackage"),
        "{row}: {what}: the end's type"
    );
    assert_strict_refuses_tolerant_writes(row, what, &doc, "b", " participantPackage=\"t gone\">");
    let doc = read_ok(
        row,
        "participants of every ArtifactPackage kind",
        &case(
            "",
            &interface("i", " implements=\"t\"", ""),
            "",
            &format!(
                "{}{}",
                binding("b", " participantPackage=\"t i b2\"", ""),
                binding("b2", " participantPackage=\"t u\"", "")
            ),
        ),
    );
    assert_eq!(
        findings(&doc),
        [],
        "{row}: participants of every ArtifactPackage kind"
    );
    for (target, class) in [
        ("report", "Artifact"),
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
                "12.4",
                &["b", target]
            )],
            "{row}: {what}: not one SACM-REF-002 Error with clause 12.4"
        );
        assert_eq!(
            argument_of(&found[0], "type"),
            Some("ArtifactPackage"),
            "{row}: {what}: the end's type"
        );
    }
}

#[test]
fn sacm23_art_013_href_participants_stay_as_today() {
    let row = ART_013;
    let what = "a cross-document participant";
    let (input, marks) = marked(&b_in_case(
        "",
        &format!(
            "<participantPackage xmi:idref=\"t\"/><participantPackage {MARK}href=\"other.xmi#u\"/>"
        ),
    ));
    let error = refused(row, what, &input);
    assert!(
        matches!(&error, ReadError::Unsupported { location, .. } if *location == marks.first().copied())
            && error
                .to_string()
                .ends_with("(a limit of this version; SACM23-XMI-027 reads it)"),
        "{row}: {what}: not Unsupported at the href naming SACM23-XMI-027: {error}"
    );
    assert_eq!(
        error.code().as_str(),
        common::unsupported_code(&error.to_string()),
        "{row}: {what}: the code is not SACM-XMI-007"
    );
    let output = tolerant(row, what, &input);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|one| (one.code(), one.severity(), one.location()))
            .collect::<Vec<_>>(),
        [(Code::XMI_EXTERNAL_HREF, Severity::Warning, error.location())],
        "{row}: {what}: a tolerant read does not keep the href with one SACM-XMI-007 Warning"
    );
    assert!(
        element(row, output.document(), "b").has_preserved_content(),
        "{row}: {what}: the href is not kept"
    );
}

#[test]
fn sacm23_art_013_writes_attribute_order_and_the_golden_file() {
    let row = ART_013;
    let what = "a binding with every attribute, read out of order";
    let doc = read_ok(
        row,
        what,
        &case(
            "",
            &interface("i", " implements=\"t\"", ""),
            "",
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
        "<artifactPackage xmi:type=\"sacm:ArtifactPackageBinding\" xmi:id=\"b\" gid=\"B-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"b\" abstractForm=\"b\" \
         interface=\"i\" participantPackage=\"u i\">",
        "{row}: {what}: not written in order\n{text}"
    );
    let golden = fixture_text(GOLDEN);
    assert!(
        golden.contains(
            "<artifactPackage xmi:type=\"sacm:ArtifactPackageBinding\" xmi:id=\"binding\" \
             participantPackage=\"tests_api requirements_api\">"
        ),
        "{row}: the golden file has no binding line"
    );
}

#[test]
fn sacm23_art_013_reference_end_tables_bounds_and_serde() {
    let row = ART_013;
    let end = ReferenceEnd::ParticipantPackage;
    assert_eq!(
        (
            end.bounds(ElementKind::ArtifactPackageBinding),
            end.bounds(ElementKind::ArtifactPackageInterface),
            end.bounds(ElementKind::ArtifactPackage),
        ),
        (Some((2, None)), None, None),
        "{row}: the bounds"
    );
    #[cfg(feature = "serde")]
    {
        let written = serde_json::to_string(&end)
            .unwrap_or_else(|error| panic!("{row}: {end:?} does not serialize: {error}"));
        assert_eq!(written, "\"participantPackage\"", "{row}: the serde name");
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_art_013_create_reports_what_it_holds_and_checks_in_order() {
    let row = ART_013;
    let operation =
        CreateArtifactPackageBinding::root(named("The tests verify the requirement"), ["t", "u"]);
    assert_eq!(
        (
            operation.parent(),
            operation.id(),
            strs(operation.participant_packages())
        ),
        (None, None, vec!["t", "u"]),
        "{row}: root()"
    );
    let operation = CreateArtifactPackageBinding::new("case", named("B"), ["u", "t"]).with_id("b");
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateArtifactPackageBinding",
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
            list(&["u", "t"]).as_slice()
        ),
        "{row}: the created binding"
    );
    let text = write_in(row, "the created binding", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "b"),
        "<artifactPackage xmi:type=\"sacm:ArtifactPackageBinding\" xmi:id=\"b\" \
         participantPackage=\"u t\">",
        "{row}: the created binding's start tag\n{text}"
    );
    let mut generated = Vec::new();
    for parent in ["case", "t", "b"] {
        let mutation = applied(
            row,
            &mut doc,
            CreateArtifactPackageBinding::new(parent, named("N"), ["t", "u"]),
        );
        generated.extend(strs(mutation.created_ids()).into_iter().map(str::to_owned));
    }
    let root = applied(
        row,
        &mut doc,
        CreateArtifactPackageBinding::root(named("N"), ["t", "u"]),
    );
    generated.extend(strs(root.created_ids()).into_iter().map(str::to_owned));
    assert_eq!(
        generated,
        (1..=4)
            .map(|n| format!("artifact_package_binding_{n}"))
            .collect::<Vec<_>>(),
        "{row}: the generated ids"
    );
    for (parent, parent_kind) in [
        ("p", ElementKind::ArgumentPackage),
        ("terms", ElementKind::TerminologyPackage),
    ] {
        let error = refuse(
            row,
            &mut doc,
            CreateArtifactPackageBinding::new(parent, named("N"), ["t", "u"]),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent_kind: found, kind: ElementKind::ArtifactPackageBinding, .. }
                    if *found == parent_kind
            ),
            "{row}: a binding under the {parent_kind} {parent}: {error:?}"
        );
    }
    let unnamed = name_without_expression(row);
    let create = |parent: &str, name: LangString, participants: &[&str]| {
        CreateArtifactPackageBinding::new(parent, name, participants.iter().copied())
    };
    let cases: Vec<(&str, CreateArtifactPackageBinding, _)> = vec![
        (
            "a duplicate id",
            create("case", unnamed.clone(), &["a b"]).with_id("report"),
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
            create("case", named("N"), &["report", "nope"]),
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
            "an Artifact participant",
            create("case", named("N"), &["report", "u"]),
            k(
                "WrongReferenceType",
                Some("participantPackage"),
                Some("report"),
            ),
        ),
    ];
    for (what, operation, want) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(key(&error), want, "{row}: {what}: {error:?}");
        if let EditError::WrongReferenceType { expected, .. } = &error {
            assert_eq!(*expected, "ArtifactPackage", "{row}: {what}: expected");
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
}

#[test]
fn sacm23_art_013_reference_edits_count_distinct_participants() {
    let row = ART_013;
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
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            "SACM23-VAL-022",
            "12.4",
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
                expected: "ArtifactPackage",
                ..
            }
        ),
        "{row}: participants [t, p]: {error:?}"
    );
}

#[test]
fn sacm23_art_013_compare_reports_participant_packages() {
    let row = ART_013;
    let left = read_ok(row, "left", &b_in_case(" participantPackage=\"t u\"", ""));
    let right = read_ok(row, "right", &b_in_case(" participantPackage=\"u t\"", ""));
    let found = semantic_compare(&left, &right);
    assert_eq!(
        found,
        [Difference::ParticipantPackages {
            id: ElementId::from("b"),
            left: list(&["t", "u"]),
            right: list(&["u", "t"]),
        }],
        "{row}: not one ParticipantPackages difference"
    );
    assert_eq!(
        found[0].to_string(),
        "element \"b\" differs in participantPackage: [\"t\", \"u\"] on the left, [\"u\", \"t\"] \
         on the right",
        "{row}: the Display"
    );
}

#[test]
fn sacm23_art_013_move_copy_and_the_element_edits_apply() {
    let row = ART_013;
    let mut doc = read_ok(
        row,
        "b in case",
        &b_in_case(
            " participantPackage=\"t u\"",
            &format!("{}{}", artifact("b1", ""), artifact("b2", "")),
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
                kind: ElementKind::ArtifactPackageBinding,
                ..
            }
        ),
        "{row}: an ArtifactPackageBinding into an ArgumentPackage: {error:?}"
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
