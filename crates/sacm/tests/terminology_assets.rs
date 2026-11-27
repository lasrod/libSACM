//! Tests for the Terminology assets of slice 0062 (ADR 0034 amendment 3):
//! - `Category` and its end `category : Category [0..*]` (SACM23-TERM-002;
//!   formal/23-05-08 §10.8 "Terms, Categories, and `ExpressionElements` can be
//!   said to belong to Categories. … Categories can contain Categories.";
//!   ptc/22-03-13 `Terminology::Category`; spec defect D21);
//! - the `ExpressionElement` features `value : String [1]` and `category`
//!   (SACM23-TERM-003; §10.9 "value:String[1] - the value of the
//!   expression.", "category: Category [0..*] - optionally associates the
//!   `ExpressionElement` with one or more terminology categories.");
//! - `Term`, with `externalReference : String [0..1]` and `origin :
//!   Base::ModelElement [0..1]` (SACM23-TERM-004; §10.11);
//! - `Expression`, with `element : ExpressionElement [0..*]`
//!   (SACM23-TERM-005; §10.10).
//!
//! On a Category, formal/23-05-08 does not say which way its `category` end
//! points (spec defect D21): no test here assumes a direction. The ids are
//! held, validated, written, edited and compared as written.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CopyElements, CreateProperty, CrossPackageReferencePolicy, DeleteElement, EditError,
    MoveElement, Mutation, Operation, ReferenceDeletePolicy, ReorderElements, SetFlag, SetGid,
    SetName, SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

use sacm::compare::Difference;
use sacm::edit::{
    AddReference, CreateCategory, CreateExpression, CreateTerm, RemoveReference,
    SetExternalReference, SetValue,
};
use sacm::xmi::{WriteAttribute, WriteError};

const TERM_002: &str = "SACM23-TERM-002";
const TERM_003: &str = "SACM23-TERM-003";
const TERM_004: &str = "SACM23-TERM-004";
const TERM_005: &str = "SACM23-TERM-005";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The worked fixture of slice 0062.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/terminology-valid.sacm.xmi"
);

/// The exact output of a strict write of [`FIXTURE`].
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/terminology.golden.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// A typed `terminologyElement` of class `class` with the id `id`, named
/// `id`, with `attributes` on its start tag and `content` after its name.
fn of_class(class: &str, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<terminologyElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{content}</terminologyElement>"
    )
}

fn category(id: &str, attributes: &str, content: &str) -> String {
    of_class("Category", id, attributes, content)
}

fn term(id: &str, attributes: &str, content: &str) -> String {
    of_class("Term", id, attributes, content)
}

fn expression(id: &str, attributes: &str, content: &str) -> String {
    of_class("Expression", id, attributes, content)
}

/// The AssuranceCasePackage root `case`, one element per line:
/// - the TerminologyPackage `terms`: the Category `hazards`, the Term `t1`
///   (value `t1`) and the Expression `e0` (value `e0`), then `body`;
/// - the TerminologyPackage `other`, holding `other`;
/// - the ArgumentPackage `p`: the Claim `c1`, which holds the Note `n1`;
/// - the ArtifactPackage `evidence`: the Artifact `manual`.
fn case(body: &str, other: &str) -> String {
    format!(
        r#"<sacm:AssuranceCasePackage{NS} xmi:id="case">
  <name content="Case"/>
  <terminologyPackage xmi:id="terms"><name content="Terms"/>
    {}
    {}
    {}
    {body}
  </terminologyPackage>
  <terminologyPackage xmi:id="other"><name content="Other"/>
    {other}
  </terminologyPackage>
  <argumentPackage xmi:id="p"><name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/><note xmi:id="n1"><content><value content="N"/></content></note></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="evidence"><name content="Evidence"/>
    <artifactElement xmi:type="sacm:Artifact" xmi:id="manual"><name content="Manual"/></artifactElement>
  </artifactPackage>
</sacm:AssuranceCasePackage>"#,
        category("hazards", "", ""),
        term("t1", " value=\"t1\"", ""),
        expression("e0", " value=\"e0\"", ""),
    )
}

/// [`case`] with `body` in `terms` and nothing in `other`.
fn in_terms(body: &str) -> String {
    case(body, "")
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(row: &str, template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(row, line, column));
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

/// `template` without its one mark, and the mark's location.
fn one_mark(row: &str, template: &str) -> (String, SourceLocation) {
    let (text, marks) = marked(row, template);
    let [at] = marks.as_slice() else {
        panic!(
            "{row}: setup: {template:?} has {} marks, not one",
            marks.len()
        );
    };
    (text, *at)
}

// --- Reading, writing and validating --------------------------------------------------

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
                panic!("{row}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in strict mode, which the test expects to refuse it, and
/// returns the refusal.
fn refused(row: &str, what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{row}: {what}: a strict read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// `(code, severity, location, affected)` of each diagnostic.
type Reported = Vec<(Code, Severity, Option<SourceLocation>, Vec<ElementId>)>;

fn reported(found: &[Diagnostic]) -> Reported {
    found
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.location(),
                found.affected().to_vec(),
            )
        })
        .collect()
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, an Error in strict mode and a
/// Warning in tolerant mode, and the same document in both. Returns the strict
/// document.
fn assert_preserved_at(row: &str, what: &str, template: &str, holder: &str) -> Document {
    let (input, at) = one_mark(row, template);
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{row}: {what}: a {mode:?} read refused it instead of preserving it: {error:?} \
                 ({error})"
            ),
        };
        assert_eq!(
            reported(output.diagnostics()),
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(at),
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

/// Asserts that `template`, with two marks at the two occurrences of
/// `property` on `holder`, is `RepeatedValue` in strict mode at both, and
/// that a tolerant read keeps the first, preserves the second and gives one
/// SACM-MULT-001 Warning there affecting `holder`. Returns the tolerant
/// document.
fn assert_repeated(
    row: &str,
    what: &str,
    template: &str,
    holder: &str,
    property: &str,
) -> Document {
    let (input, marks) = marked(row, template);
    assert_eq!(marks.len(), 2, "{row}: setup: {what} has not two marks");
    let error = refused(row, what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property: found, .. }
                if element.as_str() == holder && found == property
        ),
        "{row}: {what}: not RepeatedValue {{ {holder}, {property} }}: {error:?} ({error})"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Code::MULTIPLICITY_VIOLATION,
            marks.as_slice(),
            Some(marks[1])
        ),
        "{row}: {what}: not SACM-MULT-001 at both occurrences"
    );
    let output = match read(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?}"),
    };
    assert_eq!(
        reported(output.diagnostics()),
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            Some(marks[1]),
            vec![ElementId::from(holder)]
        )],
        "{row}: {what}: a tolerant read does not give one SACM-MULT-001 Warning at the repeat"
    );
    let doc = output.into_document();
    assert!(
        element(row, &doc, holder).has_preserved_content(),
        "{row}: {what}: a tolerant read does not keep the repeat as preserved content"
    );
    doc
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
    let back = read_ok(row, &format!("{what}, written in {mode:?} mode"), &text);
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

fn children(row: &str, doc: &Document, id: &str) -> Vec<String> {
    element(row, doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn strs(values: &[ElementId]) -> Vec<&str> {
    values.iter().map(ElementId::as_str).collect()
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// One validation finding as plain values: code, severity, requirement,
/// clause, location and affected ids.
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

fn findings(doc: &Document) -> Vec<Finding> {
    validation::validate(doc).iter().map(finding).collect()
}

/// A finding with no location.
fn expected(
    code: Code,
    severity: Severity,
    requirement: &str,
    clause: &str,
    affected: &[&str],
) -> Finding {
    (
        code,
        severity,
        requirement.to_owned(),
        Some(clause.to_owned()),
        None,
        list(affected),
    )
}

fn fixture_text(row: &str) -> String {
    std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{row}: cannot read {FIXTURE}: {error}"))
}

/// The fixture with `from` replaced by `to`, which must change it.
fn fixture_with(row: &str, from: &str, to: &str) -> String {
    let text = fixture_text(row);
    assert!(
        text.contains(from),
        "{row}: setup: the fixture has no {from:?}"
    );
    text.replacen(from, to, 1)
}

fn fixture_doc(row: &str) -> Document {
    read_ok(row, "the fixture", &fixture_text(row))
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

// --- Editing -----------------------------------------------------------------------

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

/// Asserts that `operation` is `FeatureNotApplicable` on `target`, of
/// `kind`, naming `feature`, with SACM-CMD-008, and returns its Display.
fn assert_not_applicable(
    row: &str,
    doc: &mut Document,
    operation: impl Into<Operation>,
    target: &str,
    kind: ElementKind,
    feature: &str,
) -> String {
    let error = refuse(row, doc, operation);
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind: found, feature: found_feature, .. }
                if element.as_str() == target && *found == kind && *found_feature == feature
        ),
        "{row}: {feature} on the {kind} {target}: not FeatureNotApplicable: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_FEATURE_NOT_APPLICABLE,
        "{row}: {feature} on the {kind} {target}: not SACM-CMD-008"
    );
    error.to_string()
}

/// The referrers of `id` as `(holder, end name, position)`.
fn referrers(doc: &Document, id: &str) -> Vec<(String, &'static str, usize)> {
    doc.referrers(id)
        .into_iter()
        .map(|referrer| {
            (
                referrer.holder().id().as_str().to_owned(),
                referrer.end().name(),
                referrer.position(),
            )
        })
        .collect()
}

/// The ids the element `id` holds in the end named `end`, read from the
/// strict output of `doc`: the value of the attribute `end` on its start tag.
fn written_end(row: &str, doc: &Document, id: &str, end: &str) -> Option<String> {
    let text = write_in(row, "the document", doc, Mode::Strict);
    let line = line_with(row, &text, id).to_owned();
    let needle = format!(" {end}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    rest.find('"').map(|end| rest[..end].to_owned())
}

// =====================================================================================
// SACM23-TERM-002 item 1: the forms of category on a Category
// =====================================================================================

#[test]
fn sacm23_term_002_category_reads_from_the_attribute_and_both_element_forms_alike() {
    let row = TERM_002;
    let attribute = read_ok(
        row,
        "category=\"hazards\"",
        &in_terms(&category("mechanical", " category=\"hazards\"", "")),
    );
    for (what, content) in [
        (
            "<category xmi:idref=\"hazards\"/>",
            "<category xmi:idref=\"hazards\"/>",
        ),
        (
            "<category href=\"#hazards\"/>",
            "<category href=\"#hazards\"/>",
        ),
    ] {
        let doc = read_ok(row, what, &in_terms(&category("mechanical", "", content)));
        assert!(
            doc == attribute,
            "{row}: {what}: does not read == to the attribute form"
        );
    }
    assert_eq!(
        (
            element(row, &attribute, "mechanical").kind(),
            children(row, &attribute, "terms")
        ),
        (
            ElementKind::Category,
            ids(&["hazards", "t1", "e0", "mechanical"])
        ),
        "{row}: the Category mechanical is not read in terms"
    );
    assert_eq!(
        validation::validate(&attribute),
        [],
        "{row}: a Category in a Category validates with a finding"
    );
    // In an ArtifactPackage's artifactElement.
    let what = "a Category typed in artifactElement";
    let doc = read_ok(
        row,
        what,
        &case(
            "",
            "",
        )
        .replace(
            "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"manual\"><name content=\"Manual\"/></artifactElement>",
            "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"manual\"><name content=\"Manual\"/></artifactElement>\
             <artifactElement xmi:type=\"sacm:Category\" xmi:id=\"kept\" category=\"hazards\"><name content=\"Kept\"/></artifactElement>",
        ),
    );
    assert_eq!(
        (
            element(row, &doc, "kept").kind(),
            children(row, &doc, "evidence")
        ),
        (ElementKind::Category, ids(&["manual", "kept"])),
        "{row}: {what}: not read in evidence"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: validates with a finding"
    );
}

#[test]
fn sacm23_term_002_categories_reads_the_category_end_as_written() {
    let row = TERM_002;
    let doc = read_ok(
        row,
        "category=\"hazards t1 hazards\"",
        &in_terms(&category(
            "mechanical",
            " category=\"hazards t1 hazards\"",
            "",
        )),
    );
    let mechanical = element(row, &doc, "mechanical");
    assert_eq!(
        (
            strs(mechanical.categories()),
            mechanical.categories(),
            strs(element(row, &doc, "hazards").categories())
        ),
        (
            vec!["hazards", "t1", "hazards"],
            mechanical.references(ReferenceEnd::Category),
            Vec::<&str>::new()
        ),
        "{row}: categories() is not the end as written, or not references(Category)"
    );
    // The fixture's Category, Term and Expression.
    let doc = fixture_doc(row);
    assert_eq!(
        (
            strs(element(row, &doc, "mechanical").categories()),
            strs(element(row, &doc, "blade").categories()),
            strs(element(row, &doc, "lid").categories()),
            strs(element(row, &doc, "blade_guarded").categories())
        ),
        (
            vec!["hazards"],
            vec!["mechanical"],
            vec!["mechanical"],
            Vec::new()
        ),
        "{row}: the fixture's categories()"
    );
    // Empty for every other kind.
    for id in ["terms", "iec", "reviewed", "blade_ref", "manual", "case"] {
        assert!(
            element(row, &doc, id).categories().is_empty(),
            "{row}: categories() of {id} is not empty"
        );
    }
}

#[test]
fn sacm23_term_002_an_attribute_and_an_element_of_category_are_repeated_value() {
    let row = TERM_002;
    assert_repeated(
        row,
        "category= and a category element",
        &in_terms(&category(
            "mechanical",
            &format!(" {MARK}category=\"hazards\""),
            &format!("{MARK}<category xmi:idref=\"hazards\"/>"),
        )),
        "mechanical",
        "category",
    );
}

#[test]
fn sacm23_term_002_an_empty_category_attribute_is_preserved_by_the_category() {
    let row = TERM_002;
    for (what, attribute) in [
        ("an empty category", format!(" {MARK}category=\"\"")),
        (
            "a category of white space",
            format!(" {MARK}category=\" \""),
        ),
    ] {
        assert_preserved_at(
            row,
            what,
            &in_terms(&category("mechanical", &attribute, "")),
            "mechanical",
        );
    }
}

// =====================================================================================
// SACM23-TERM-002 item 2: ReferenceEnd::Category
// =====================================================================================

#[test]
fn sacm23_term_002_reference_end_category_applies_to_the_terminology_assets() {
    let row = TERM_002;
    let end = ReferenceEnd::Category;
    assert_eq!(end.name(), "category", "{row}: Category.name()");
    let holders: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|&kind| end.applies_to(kind))
        .collect();
    assert_eq!(
        holders,
        [
            ElementKind::Category,
            ElementKind::Expression,
            ElementKind::Term
        ],
        "{row}: Category applies to other kinds than Category, Term and Expression"
    );
    for &kind in ElementKind::ALL {
        let bounds = if holders.contains(&kind) {
            Some((0, None))
        } else {
            None
        };
        assert_eq!(
            end.bounds(kind),
            bounds,
            "{row}: the bounds of category on a {kind}"
        );
    }
    let all = ReferenceEnd::ALL;
    let after = all
        .iter()
        .position(|&found| found == ReferenceEnd::ArtifactTarget)
        .map(|index| all.get(index + 1));
    assert_eq!(
        after,
        Some(Some(&ReferenceEnd::Category)),
        "{row}: ReferenceEnd::ALL does not list Category directly after ArtifactTarget"
    );
}

#[test]
fn sacm23_term_002_reference_end_all_is_the_sixteen_written_ends_then_expression() {
    let row = TERM_002;
    assert_eq!(
        ReferenceEnd::ALL,
        [
            ReferenceEnd::CitedElement,
            ReferenceEnd::AbstractForm,
            ReferenceEnd::MetaClaim,
            ReferenceEnd::Reasoning,
            ReferenceEnd::Source,
            ReferenceEnd::Target,
            ReferenceEnd::Structure,
            ReferenceEnd::ReferencedArtifactElement,
            ReferenceEnd::GroupMember,
            ReferenceEnd::ArtifactGroupMember,
            ReferenceEnd::ArtifactSource,
            ReferenceEnd::ArtifactTarget,
            ReferenceEnd::Category,
            ReferenceEnd::Element,
            ReferenceEnd::Origin,
            ReferenceEnd::TerminologyGroupMember,
            // The ends of slice 0066 (ADR 0039 D1).
            ReferenceEnd::Interface,
            ReferenceEnd::Implements,
            ReferenceEnd::ParticipantPackage,
            // The derived end of slice 0065 (SACM23-BASE-009), after the
            // written ends.
            ReferenceEnd::Expression,
        ],
        "{row}: ReferenceEnd::ALL"
    );
    // No kind holds two ends of one XMI name: the reader resolves an end by
    // the element's kind.
    for &kind in ElementKind::ALL {
        let names: Vec<&str> = ReferenceEnd::ALL
            .iter()
            .filter(|end| end.applies_to(kind))
            .map(|end| end.name())
            .collect();
        let mut distinct = names.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            names.len(),
            "{row}: a {kind} has two ends of one name: {names:?}"
        );
    }
}

// =====================================================================================
// SACM23-TERM-002 item 3: validating a Category's category
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_term_002_validate_checks_a_category_s_category_with_clause_10_8() {
    let row = TERM_002;
    // Naming a Term: SACM-REF-002.
    let what = "category=\"t1\" on a Category";
    let doc = read_ok(
        row,
        what,
        &in_terms(&category("mechanical", " category=\"t1\"", "")),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_WRONG_TYPE,
            Severity::Error,
            "SACM23-VAL-004",
            "10.8",
            &["mechanical", "t1"]
        )],
        "{row}: {what}: not one SACM-REF-002 Error with clause 10.8"
    );
    // Naming no element: SACM-REF-001.
    let what = "category=\"ghost\" on a Category";
    let doc = read_ok(
        row,
        what,
        &in_terms(&category("mechanical", " category=\"ghost\"", "")),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003",
            "10.8",
            &["mechanical", "ghost"]
        )],
        "{row}: {what}: not one SACM-REF-001 Error with clause 10.8"
    );
    // Naming an element kept only as preserved content: SACM-REF-003, with
    // the Category's clause too.
    let what = "category=\"z\" on a Category, z a Claim kept as preserved content";
    let input = in_terms(&format!(
        "{}<terminologyElement xmi:type=\"sacm:Claim\" xmi:id=\"z\"><name content=\"Z\"/>\
         </terminologyElement>",
        category("k", " category=\"z\"", "")
    ));
    let doc = match read(Mode::Tolerant, &input) {
        Ok(output) => {
            assert_eq!(
                output
                    .diagnostics()
                    .iter()
                    .map(Diagnostic::code)
                    .collect::<Vec<_>>(),
                [Code::XMI_UNKNOWN_CONTENT],
                "{row}: {what}: setup: z is not kept with one SACM-XMI-003"
            );
            output.into_document()
        }
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?}"),
    };
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_PRESERVED_TARGET,
            Severity::Warning,
            "LIB-COMPAT-003",
            "10.8",
            &["k", "z"]
        )],
        "{row}: {what}: not one SACM-REF-003 Warning with clause 10.8"
    );
    // A repeat: one SACM23-VAL-022 Warning.
    let what = "category=\"hazards hazards\" on a Category";
    let doc = read_ok(
        row,
        what,
        &in_terms(&category("mechanical", " category=\"hazards hazards\"", "")),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022",
            "10.8",
            &["mechanical", "hazards"]
        )],
        "{row}: {what}: not one SACM23-VAL-022 Warning with clause 10.8"
    );
    // A Category in another package, a self-reference and a two-Category
    // cycle: §10.8 states no rule, so no finding, whichever way it points.
    for (what, body, other) in [
        (
            "a Category naming one in another package",
            category("mechanical", " category=\"remote\"", ""),
            category("remote", "", ""),
        ),
        (
            "a Category naming itself",
            category("mechanical", " category=\"mechanical\"", ""),
            String::new(),
        ),
        (
            "two Categories naming each other",
            format!(
                "{}{}",
                category("a", " category=\"b\"", ""),
                category("b", " category=\"a\"", "")
            ),
            String::new(),
        ),
    ] {
        let doc = read_ok(row, what, &case(&body, &other));
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
}

// =====================================================================================
// SACM23-TERM-002 item 4: writing and comparing a Category's category
// =====================================================================================

#[test]
fn sacm23_term_002_category_is_written_after_abstract_form_kept_when_repeated_and_absent_when_empty()
 {
    let row = TERM_002;
    let what = "a Category with every SACMElement feature and category";
    let doc = read_ok(
        row,
        what,
        &in_terms(&category(
            "mechanical",
            " category=\"hazards hazards\" abstractForm=\"hazards\" citedElement=\"hazards\" \
             isAbstract=\"true\" isCitation=\"true\" gid=\"K-1\"",
            "",
        )),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "mechanical"),
        "<terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"mechanical\" gid=\"K-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"hazards\" \
         abstractForm=\"hazards\" category=\"hazards hazards\">",
        "{row}: {what}: not written in order with the repeat kept\n{text}"
    );
    // An element form is written as the attribute; no end, no attribute.
    let doc = read_ok(
        row,
        "two category elements",
        &in_terms(&category(
            "mechanical",
            "",
            "<category xmi:idref=\"hazards\"/><category href=\"#t1\"/>",
        )),
    );
    let text = round_trip(row, "two category elements", &doc, Mode::Strict);
    assert_eq!(
        (
            line_with(row, &text, "mechanical"),
            line_with(row, &text, "hazards")
        ),
        (
            "<terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"mechanical\" \
             category=\"hazards t1\">",
            "<terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"hazards\">"
        ),
        "{row}: the elements are not written as one attribute, or an empty end is written\n{text}"
    );
}

#[test]
fn sacm23_term_002_a_categories_difference_displays_its_end_and_both_lists() {
    let row = TERM_002;
    let left = fixture_doc(row);
    let right = read_ok(
        row,
        "the fixture without mechanical's category",
        &fixture_with(
            row,
            "xmi:id=\"mechanical\" category=\"hazards\"",
            "xmi:id=\"mechanical\"",
        ),
    );
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{row}: not one difference: {found:?}");
    };
    assert_eq!(
        (difference.to_string(), difference.element_id()),
        (
            "element \"mechanical\" differs in category: [\"hazards\"] on the left, [] on the right"
                .to_owned(),
            Some(&ElementId::from("mechanical"))
        ),
        "{row}: the Display of the category difference"
    );
}

#[test]
fn sacm23_term_002_semantic_compare_reports_one_categories_difference_after_artifact_targets() {
    let row = TERM_002;
    let left = fixture_doc(row);
    let right = read_ok(
        row,
        "the fixture without mechanical's category",
        &fixture_with(
            row,
            "xmi:id=\"mechanical\" category=\"hazards\"",
            "xmi:id=\"mechanical\" abstractForm=\"hazards\"",
        ),
    );
    assert_eq!(
        semantic_compare(&left, &right),
        [
            Difference::AbstractForm {
                id: ElementId::from("mechanical"),
                left: None,
                right: Some(ElementId::from("hazards")),
            },
            Difference::Categories {
                id: ElementId::from("mechanical"),
                left: list(&["hazards"]),
                right: Vec::new(),
            },
        ],
        "{row}: not AbstractForm, then Categories"
    );
    // Order and repeats count.
    let with = |value: &str| {
        read_ok(
            row,
            value,
            &in_terms(&category(
                "mechanical",
                &format!(" category=\"{value}\""),
                "",
            )),
        )
    };
    for (a, b) in [("hazards t1", "t1 hazards"), ("hazards", "hazards hazards")] {
        assert_eq!(
            semantic_compare(&with(a), &with(b)),
            [Difference::Categories {
                id: ElementId::from("mechanical"),
                left: a.split(' ').map(ElementId::from).collect(),
                right: b.split(' ').map(ElementId::from).collect(),
            }],
            "{row}: {a:?} vs {b:?}"
        );
    }
}

// =====================================================================================
// SACM23-TERM-002 item 5: CreateCategory, and the creates of TERM-004 and -005
// =====================================================================================

/// The document the creates are tested on: the AssuranceCasePackage `case`
/// holding the ArgumentPackage `p` (the Claim `c1` with the Note `n1`), the
/// ArtifactPackage `evidence` (the Artifact `manual`) and the
/// TerminologyPackage `terms` (the Category `hazards`, the Term `t1` and the
/// Expression `e0`); and the TerminologyPackage `other`, empty.
fn built(row: &str) -> Document {
    read_ok(row, "the create document", &case("", ""))
}

type Make = fn(Option<&str>, Option<&str>) -> Operation;

/// The variant of `error` and the ids it names, comparable.
fn refusal(error: &EditError) -> (String, Vec<String>) {
    let shown = format!("{error:?}");
    let variant = shown
        .split([' ', '{', '('])
        .next()
        .unwrap_or_default()
        .to_owned();
    let named = match error {
        EditError::ParentNotFound { parent, .. } => vec![parent.as_str().to_owned()],
        EditError::InvalidParent {
            parent,
            parent_kind,
            kind,
            ..
        } => vec![
            parent.as_str().to_owned(),
            parent_kind.name().to_owned(),
            kind.name().to_owned(),
        ],
        EditError::InvalidId { id, .. } | EditError::DuplicateId { id, .. } => {
            vec![id.as_str().to_owned()]
        }
        EditError::InvalidReference { end, id, .. }
        | EditError::ReferenceNotFound { end, id, .. } => {
            vec![(*end).to_owned(), id.as_str().to_owned()]
        }
        EditError::WrongReferenceType {
            end, id, expected, ..
        } => vec![
            (*end).to_owned(),
            id.as_str().to_owned(),
            (*expected).to_owned(),
        ],
        _ => Vec::new(),
    };
    (variant, named)
}

/// Applies a create the test expects to succeed, and returns the one id it
/// created, with nothing changed or deleted.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let mutation = applied(row, doc, operation);
    let [only] = mutation.created_ids() else {
        panic!(
            "{row}: created_ids() is {:?}, not the one new element",
            mutation.created_ids()
        );
    };
    assert_eq!(
        (mutation.changed_ids(), mutation.deleted_ids()),
        ([].as_slice(), [].as_slice()),
        "{row}: a create changed or deleted elements"
    );
    only.clone()
}

/// Asserts the creates of `kind` by `make` on [`built`]: the generated ids
/// `<prefix>_1` and `<prefix>_2`; created under `terms` (in
/// `terminologyElement`) and `evidence` (in `artifactElement`); `InvalidParent`
/// under each of `illegal`; the four checks of the parent and the id in
/// order; and a refusal that does not move the next generated id.
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn assert_create_rules(row: &str, kind: ElementKind, make: Make, prefix: &str, illegal: &[&str]) {
    let mut doc = built(row);
    let id1 = create(row, &mut doc, make(Some("terms"), None));
    let id2 = create(row, &mut doc, make(Some("terms"), None));
    assert_eq!(
        (id1.as_str().to_owned(), id2.as_str().to_owned()),
        (format!("{prefix}_1"), format!("{prefix}_2")),
        "{row}: the generated ids of {kind}"
    );
    for (index, (parent, role)) in [
        ("terms", "terminologyElement"),
        ("evidence", "artifactElement"),
    ]
    .iter()
    .enumerate()
    {
        let given = format!("given_{index}");
        create(row, &mut doc, make(Some(*parent), Some(given.as_str())));
        let created = element(row, &doc, &given);
        assert_eq!(
            (
                created.kind(),
                created
                    .parent()
                    .map(|parent| parent.id().as_str().to_owned()),
                created.name().cloned()
            ),
            (kind, Some((*parent).to_owned()), Some(named("N"))),
            "{row}: a {kind} with a caller id under {parent}"
        );
        assert_eq!(
            children(row, &doc, parent).last(),
            Some(&given),
            "{row}: the {kind} under {parent} is not its last child"
        );
        let text = write_in(row, "the built document", &doc, Mode::Strict);
        assert!(
            line_with(row, &text, &given).starts_with(&format!(
                "<{role} xmi:type=\"sacm:{}\" xmi:id=\"{given}\"",
                kind.name()
            )),
            "{row}: the {kind} under {parent} is not written typed in {role}\n{text}"
        );
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the built document does not validate cleanly"
    );
    round_trip(row, "the built document", &doc, Mode::Strict);
    for parent in illegal {
        let parent_kind = element(row, &doc, parent).kind();
        let error = refuse(row, &mut doc, make(Some(*parent), None));
        assert_eq!(
            (refusal(&error), error.code()),
            (
                (
                    "InvalidParent".to_owned(),
                    vec![
                        (*parent).to_owned(),
                        parent_kind.name().to_owned(),
                        kind.name().to_owned()
                    ]
                ),
                Code::CMD_INVALID_PARENT
            ),
            "{row}: a {kind} under the {parent_kind} {parent}"
        );
    }
    let illegal_kind = element(row, &doc, illegal[0]).kind().name();
    for (what, parent, id, expected) in [
        (
            "a missing parent, an invalid id",
            "nowhere",
            Some("1x"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "a missing parent, a duplicate id",
            "nowhere",
            Some("c1"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "an illegal parent, an invalid id",
            illegal[0],
            Some("1x"),
            ("InvalidParent", vec![illegal[0], illegal_kind, kind.name()]),
        ),
        (
            "an illegal parent, a duplicate id",
            illegal[0],
            Some("c1"),
            ("InvalidParent", vec![illegal[0], illegal_kind, kind.name()]),
        ),
        (
            "an invalid id",
            "terms",
            Some("1x"),
            ("InvalidId", vec!["1x"]),
        ),
        (
            "an id with a space",
            "terms",
            Some("a b"),
            ("InvalidId", vec!["a b"]),
        ),
        (
            "a duplicate id",
            "terms",
            Some("c1"),
            ("DuplicateId", vec!["c1"]),
        ),
        (
            "the id of the parent",
            "terms",
            Some("terms"),
            ("DuplicateId", vec!["terms"]),
        ),
    ] {
        let error = refuse(row, &mut doc, make(Some(parent), id));
        assert_eq!(
            refusal(&error),
            (
                expected.0.to_owned(),
                expected.1.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
            ),
            "{row}: {what}: the refusal of a {kind}"
        );
    }
    let next = create(row, &mut doc, make(Some("terms"), None));
    assert_eq!(
        next.as_str(),
        format!("{prefix}_3"),
        "{row}: a refusal moved the next generated id of {kind}"
    );
}

/// One refusal case: what it is, the operation, and the variant and the
/// names [`refusal`] gives.
type RefusalCase<'a> = (&'a str, Operation, (&'a str, Vec<&'a str>));

/// Asserts each `(operation, refusal)` of `cases` on [`built`].
fn assert_refusals(row: &str, cases: Vec<RefusalCase<'_>>) {
    let mut doc = built(row);
    for (what, operation, (variant, named)) in cases {
        let error = refuse(row, &mut doc, operation);
        assert_eq!(
            refusal(&error),
            (
                variant.to_owned(),
                named.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
            ),
            "{row}: {what}"
        );
    }
}

#[test]
fn sacm23_term_002_create_category_reports_what_it_was_given_and_creates_it() {
    let row = TERM_002;
    let operation = CreateCategory::new("terms", named("Mechanical hazards"));
    assert_eq!(
        (
            operation.parent().as_str(),
            operation.id(),
            operation.name(),
            operation.categories()
        ),
        ("terms", None, &named("Mechanical hazards"), [].as_slice()),
        "{row}: the defaults"
    );
    let operation = operation
        .with_categories(["t1"])
        .with_id("mechanical")
        .with_categories(["hazards", "hazards"]);
    assert_eq!(
        (
            operation.id().map(ElementId::as_str),
            strs(operation.categories())
        ),
        (Some("mechanical"), vec!["hazards", "hazards"]),
        "{row}: the builders replace what they were given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateCategory",
        "{row}: operation_name()"
    );
    let mut doc = built(row);
    create(row, &mut doc, operation);
    assert_eq!(
        (
            element(row, &doc, "mechanical").kind(),
            strs(element(row, &doc, "mechanical").categories())
        ),
        (ElementKind::Category, vec!["hazards", "hazards"]),
        "{row}: the created Category"
    );
    let text = write_in(row, "the created Category", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "mechanical"),
        "<terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"mechanical\" \
         category=\"hazards hazards\">",
        "{row}: the created Category's start tag\n{text}"
    );
}

#[test]
fn sacm23_term_002_create_category_legal_parents_refusals_and_generated_id() {
    let row = TERM_002;
    let make: Make = |parent, id| {
        let create = CreateCategory::new(parent.unwrap_or("terms"), named("N"));
        match id {
            Some(id) => create.with_id(id).into(),
            None => create.into(),
        }
    };
    assert_create_rules(
        row,
        ElementKind::Category,
        make,
        "category",
        &["case", "p", "t1", "hazards", "e0", "c1", "manual"],
    );
    let category_with = |ids: &[&str]| -> Operation {
        CreateCategory::new("terms", named("N"))
            .with_categories(ids.iter().copied())
            .into()
    };
    assert_refusals(
        row,
        vec![
            (
                "the parent before the categories",
                CreateCategory::new("nowhere", named("N"))
                    .with_categories(["a b"])
                    .into(),
                ("ParentNotFound", vec!["nowhere"]),
            ),
            (
                "the id before the categories",
                CreateCategory::new("terms", named("N"))
                    .with_id("c1")
                    .with_categories(["a b"])
                    .into(),
                ("DuplicateId", vec!["c1"]),
            ),
            (
                "an id with white space",
                category_with(&["hazards", "a b"]),
                ("InvalidReference", vec!["category", "a b"]),
            ),
            (
                "white space before a missing id",
                category_with(&["nope", "a b"]),
                ("InvalidReference", vec!["category", "a b"]),
            ),
            (
                "a missing Category",
                category_with(&["hazards", "nope"]),
                ("ReferenceNotFound", vec!["category", "nope"]),
            ),
            (
                "a missing id before a wrong type",
                category_with(&["t1", "nope"]),
                ("ReferenceNotFound", vec!["category", "nope"]),
            ),
            (
                "a Term",
                category_with(&["hazards", "t1"]),
                ("WrongReferenceType", vec!["category", "t1", "Category"]),
            ),
            (
                "a Claim",
                category_with(&["c1"]),
                ("WrongReferenceType", vec!["category", "c1", "Category"]),
            ),
        ],
    );
}

// =====================================================================================
// SACM23-TERM-002 item 6: the reference edits with Category
// =====================================================================================

#[test]
fn sacm23_term_002_set_add_and_remove_reference_edit_a_category_s_category() {
    let row = TERM_002;
    let mut doc = read_ok(
        row,
        "a second Category",
        &in_terms(&category("mechanical", "", "")),
    );
    let mutation = applied(
        row,
        &mut doc,
        SetReferences::new(
            "mechanical",
            ReferenceEnd::Category,
            ["hazards", "mechanical"],
        ),
    );
    assert_eq!(
        (
            mutation.changed_ids(),
            strs(element(row, &doc, "mechanical").categories())
        ),
        (
            list(&["mechanical"]).as_slice(),
            vec!["hazards", "mechanical"]
        ),
        "{row}: SetReferences"
    );
    applied(
        row,
        &mut doc,
        AddReference::new("mechanical", ReferenceEnd::Category, "hazards"),
    );
    applied(
        row,
        &mut doc,
        RemoveReference::new("mechanical", ReferenceEnd::Category, "mechanical"),
    );
    assert_eq!(
        strs(element(row, &doc, "mechanical").categories()),
        ["hazards", "hazards"],
        "{row}: AddReference appends and RemoveReference removes"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("mechanical", ReferenceEnd::Category, "t1"),
    );
    assert!(
        matches!(&error, EditError::WrongReferenceType { end, id, expected, .. }
            if *end == "category" && id.as_str() == "t1" && *expected == "Category"),
        "{row}: AddReference of a Term: {error:?}"
    );
    applied(
        row,
        &mut doc,
        SetReferences::clear("mechanical", ReferenceEnd::Category),
    );
    assert!(
        element(row, &doc, "mechanical").categories().is_empty(),
        "{row}: clear"
    );
}

#[test]
fn sacm23_term_002_category_on_a_claim_is_feature_not_applicable() {
    let row = TERM_002;
    let mut doc = built(row);
    for operation in [
        Operation::from(SetReferences::new(
            "c1",
            ReferenceEnd::Category,
            ["hazards"],
        )),
        AddReference::new("c1", ReferenceEnd::Category, "hazards").into(),
        RemoveReference::new("c1", ReferenceEnd::Category, "hazards").into(),
    ] {
        let shown = assert_not_applicable(
            row,
            &mut doc,
            operation,
            "c1",
            ElementKind::Claim,
            "category",
        );
        assert_eq!(
            shown, "the Claim \"c1\" has no category",
            "{row}: the Display of category on a Claim"
        );
    }
    // Nor on a TerminologyPackage or a TerminologyGroup.
    assert_not_applicable(
        row,
        &mut doc,
        SetReferences::new("terms", ReferenceEnd::Category, ["hazards"]),
        "terms",
        ElementKind::TerminologyPackage,
        "category",
    );
}

// =====================================================================================
// SACM23-TERM-002 item 7: deleting a Category that is named
// =====================================================================================

#[test]
fn sacm23_term_002_deleting_a_named_category_is_referenced_or_cross_package_reference() {
    let row = TERM_002;
    let mut doc = read_ok(
        row,
        "mechanical naming hazards",
        &in_terms(&category("mechanical", " category=\"hazards\"", "")),
    );
    let error = refuse(row, &mut doc, DeleteElement::new("hazards"));
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end, id, .. }
                if holder.as_str() == "mechanical" && *end == "category" && id.as_str() == "hazards"
        ),
        "{row}: deleting hazards: not Referenced {{ mechanical, category, hazards }}: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_DELETE_REFERENCED,
        "{row}: not SACM-CMD-002"
    );
    let mut doc = read_ok(
        row,
        "remote naming hazards from other",
        &case("", &category("remote", " category=\"hazards\"", "")),
    );
    let error = refuse(row, &mut doc, DeleteElement::new("hazards"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end, id, .. }
                if holder.as_str() == "remote" && *end == "category" && id.as_str() == "hazards"
        ),
        "{row}: deleting hazards named from other: not CrossPackageReference: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_EXTERNAL_REFERENCES,
        "{row}: not SACM-CMD-007"
    );
}

#[test]
fn sacm23_term_002_scrub_and_delete_referencing_relationships_keep_every_holder() {
    let row = TERM_002;
    let input = case(
        &format!(
            "{}{}{}",
            category("mechanical", " category=\"hazards\"", ""),
            term(
                "t2",
                " category=\"hazards mechanical hazards\" value=\"t2\"",
                ""
            ),
            expression("e2", " category=\"hazards\" value=\"e2\"", "")
        ),
        &category("remote", " category=\"hazards\"", ""),
    );
    let doc = read_ok(row, "three holders of hazards", &input);
    assert_eq!(
        referrers(&doc, "hazards"),
        [
            ("mechanical".to_owned(), "category", 0),
            ("t2".to_owned(), "category", 0),
            ("t2".to_owned(), "category", 2),
            ("e2".to_owned(), "category", 0),
            ("remote".to_owned(), "category", 0),
        ],
        "{row}: Document::referrers does not list the category end"
    );
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok(row, "three holders of hazards", &input);
        let mutation = applied(
            row,
            &mut doc,
            DeleteElement::new("hazards")
                .with_reference_policy(policy)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
        );
        assert_eq!(
            (
                mutation.deleted_ids(),
                mutation.changed_ids(),
                children(row, &doc, "terms"),
                referrers(&doc, "hazards")
            ),
            (
                list(&["hazards"]).as_slice(),
                list(&["mechanical", "t2", "e2", "remote"]).as_slice(),
                ids(&["t1", "e0", "mechanical", "t2", "e2"]),
                Vec::new()
            ),
            "{row}: {policy:?}: not every holder scrubbed and kept"
        );
        assert_eq!(
            (
                written_end(row, &doc, "mechanical", "category"),
                written_end(row, &doc, "t2", "category"),
                written_end(row, &doc, "e2", "category")
            ),
            (None, Some("mechanical".to_owned()), None),
            "{row}: {policy:?}: the scrubbed ends as written"
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: {policy:?}: the scrubbed document validates with a finding"
        );
    }
}

// =====================================================================================
// SACM23-TERM-003 item 1: value, verbatim, in each form
// =====================================================================================

/// The values read verbatim, each as written in an attribute.
const VALUES: [&str; 5] = [
    "blade",
    "",
    " a b ",
    "$Blade$ is covered by the $Lid$",
    "a&amp;b&#9;c",
];

#[test]
fn sacm23_term_003_value_reads_and_writes_verbatim_on_a_term_and_an_expression() {
    let row = TERM_003;
    for written in VALUES {
        for (class, make) in [
            ("Term", term as fn(&str, &str, &str) -> String),
            ("Expression", expression),
        ] {
            let what = format!("value=\"{written}\" on a {class}");
            let doc = read_ok(
                row,
                &what,
                &in_terms(&make("x", &format!(" value=\"{written}\""), "")),
            );
            assert_eq!(
                validation::validate(&doc),
                [],
                "{row}: {what}: validate reports a finding"
            );
            let text = round_trip(row, &what, &doc, Mode::Strict);
            assert_eq!(
                line_with(row, &text, "x"),
                format!(
                    "<terminologyElement xmi:type=\"sacm:{class}\" xmi:id=\"x\" value=\"{written}\">"
                ),
                "{row}: {what}: not written back as read\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_term_003_value_returns_the_value_exactly_as_written() {
    let row = TERM_003;
    for (written, held) in [
        ("blade", "blade"),
        ("", ""),
        (" a b ", " a b "),
        (
            "$Blade$ is covered by the $Lid$",
            "$Blade$ is covered by the $Lid$",
        ),
        ("a&amp;b&#9;c", "a&b\tc"),
    ] {
        let doc = read_ok(
            row,
            written,
            &in_terms(&term("x", &format!(" value=\"{written}\""), "")),
        );
        assert_eq!(
            element(row, &doc, "x").value(),
            Some(held),
            "{row}: value=\"{written}\""
        );
    }
    let doc = fixture_doc(row);
    assert_eq!(
        (
            element(row, &doc, "blade").value(),
            element(row, &doc, "lid").value(),
            element(row, &doc, "guard").value(),
            element(row, &doc, "blade_guarded").value(),
            element(row, &doc, "jar").value()
        ),
        (
            Some("blade"),
            Some("lid"),
            Some("{guard}"),
            Some("$Blade$ is covered by the $Lid$"),
            Some("jar")
        ),
        "{row}: the fixture's values"
    );
    for id in ["hazards", "terms", "reviewed", "manual", "blade_ref"] {
        assert_eq!(
            element(row, &doc, id).value(),
            None,
            "{row}: value() of {id}"
        );
    }
    let doc = read_ok(row, "a Term with no value", &in_terms(&term("x", "", "")));
    assert_eq!(
        element(row, &doc, "x").value(),
        None,
        "{row}: value() of a Term with no value"
    );
}

#[test]
fn sacm23_term_003_each_value_element_form_reads_and_writes_as_its_attribute_twin() {
    let row = TERM_003;
    let twin = read_ok(
        row,
        "value=\"lid\"",
        &in_terms(&term("x", " value=\"lid\"", "")),
    );
    let twin_text = write_in(row, "value=\"lid\"", &twin, Mode::Strict);
    for (what, content) in [
        ("<value>lid</value>", "<value>lid</value>"),
        ("a CDATA section", "<value><![CDATA[lid]]></value>"),
        ("a comment inside", "<value>l<!-- c -->id</value>"),
        ("a character reference", "<value>l&#105;d</value>"),
    ] {
        let doc = read_ok(row, what, &in_terms(&term("x", "", content)));
        assert!(
            doc == twin,
            "{row}: {what}: does not read == to value=\"lid\""
        );
        assert_eq!(
            write_in(row, what, &doc, Mode::Strict),
            twin_text,
            "{row}: {what}: not written byte-identical to its attribute twin"
        );
    }
    // A nil value element is absent.
    let without = read_ok(row, "no value", &in_terms(&term("x", "", "")));
    let doc = read_ok(
        row,
        "<value xsi:nil=\"true\"/>",
        &in_terms(&term("x", "", "<value xsi:nil=\"true\"/>")),
    );
    assert!(
        doc == without,
        "{row}: a nil value does not read == to no value"
    );
    // ...and counts as given.
    assert_repeated(
        row,
        "value= and a nil value element",
        &in_terms(&term(
            "x",
            &format!(" {MARK}value=\"lid\""),
            &format!("{MARK}<value xsi:nil=\"true\"/>"),
        )),
        "x",
        "value",
    );
}

#[test]
fn sacm23_term_003_value_is_written_after_the_ends_and_before_external_reference() {
    let row = TERM_003;
    let doc = read_ok(
        row,
        "a Term and an Expression with every end",
        &in_terms(&format!(
            "{}{}",
            term(
                "x",
                " externalReference=\"urn:x\" value=\"v\" origin=\"manual\" category=\"hazards\"",
                "",
            ),
            expression("y", " value=\"w\" element=\"x\" category=\"hazards\"", "")
        )),
    );
    let text = round_trip(row, "every end", &doc, Mode::Strict);
    assert_eq!(
        (line_with(row, &text, "x"), line_with(row, &text, "y")),
        (
            "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"x\" category=\"hazards\" \
             origin=\"manual\" value=\"v\" externalReference=\"urn:x\">",
            "<terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"y\" category=\"hazards\" \
             element=\"x\" value=\"w\">"
        ),
        "{row}: value is not written after the ends and before externalReference\n{text}"
    );
}

// =====================================================================================
// SACM23-TERM-003 item 2: value given twice, and value where it does not belong
// =====================================================================================

#[test]
fn sacm23_term_003_value_given_twice_is_repeated_value() {
    let row = TERM_003;
    for (what, attributes, content) in [
        (
            "value= and a value element",
            format!(" {MARK}value=\"a\""),
            format!("{MARK}<value>b</value>"),
        ),
        (
            "two value elements",
            String::new(),
            format!("{MARK}<value>a</value>{MARK}<value>b</value>"),
        ),
    ] {
        for (class, make) in [
            ("Term", term as fn(&str, &str, &str) -> String),
            ("Expression", expression),
        ] {
            let what = format!("{what} on a {class}");
            let doc = assert_repeated(
                row,
                &what,
                &in_terms(&make("x", &attributes, &content)),
                "x",
                "value",
            );
            // The first is kept: a tolerant write writes it as the value.
            let text = write_in(row, &what, &doc, Mode::Tolerant);
            assert!(
                line_with(row, &text, "x").contains(" value=\"a\""),
                "{row}: {what}: the first value is not kept\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_term_003_a_tolerant_read_of_a_repeated_value_keeps_the_first() {
    let row = TERM_003;
    let (input, _) = marked(
        row,
        &in_terms(&term("x", " value=\"a\"", "<value>b</value>")),
    );
    let doc = match read(Mode::Tolerant, &input) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: a tolerant read refused it: {error:?}"),
    };
    assert_eq!(
        element(row, &doc, "x").value(),
        Some("a"),
        "{row}: the first value is not kept"
    );
}

#[test]
fn sacm23_term_003_value_on_a_kind_without_it_is_preserved() {
    let row = TERM_003;
    for (what, template, holder) in [
        (
            "value= on a Category",
            in_terms(&category("x", &format!(" {MARK}value=\"x\""), "")),
            "x",
        ),
        (
            "value= on a TerminologyPackage",
            in_terms("").replace(
                "<terminologyPackage xmi:id=\"terms\">",
                &format!("<terminologyPackage xmi:id=\"terms\" {MARK}value=\"x\">"),
            ),
            "terms",
        ),
        (
            "value= on a Claim",
            in_terms("").replace(
                "xmi:id=\"c1\">",
                &format!("xmi:id=\"c1\" {MARK}value=\"x\">"),
            ),
            "c1",
        ),
        (
            "a value element on an Artifact",
            in_terms("").replace(
                "<name content=\"Manual\"/>",
                &format!("<name content=\"Manual\"/>{MARK}<value>x</value>"),
            ),
            "manual",
        ),
        (
            "a LangString value element on a Term",
            in_terms(&term(
                "x",
                "",
                &format!("{MARK}<value lang=\"en\" content=\"x\"/>"),
            )),
            "x",
        ),
    ] {
        assert_preserved_at(row, what, &template, holder);
    }
}

#[test]
fn sacm23_term_003_a_preserved_value_is_no_value() {
    let row = TERM_003;
    for (what, input, holder) in [
        (
            "value= on a Category",
            in_terms(&category("x", " value=\"x\"", "")),
            "x",
        ),
        (
            "a LangString value element on a Term",
            in_terms(&term("x", "", "<value lang=\"en\" content=\"x\"/>")),
            "x",
        ),
    ] {
        let doc = match read(Mode::Tolerant, &input) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?}"),
        };
        assert_eq!(
            element(row, &doc, holder).value(),
            None,
            "{row}: {what}: read as a value"
        );
    }
}

// =====================================================================================
// SACM23-TERM-003 item 3: a missing value
// =====================================================================================

#[test]
fn sacm23_term_003_validate_reports_a_term_or_expression_with_no_value() {
    let row = TERM_003;
    for (class, id) in [("Term", "t1"), ("Expression", "e1")] {
        let what = format!("the {class} {id} with no value");
        let input = format!(
            "<sacm:TerminologyPackage{NS} xmi:id=\"terms\"><name content=\"Terms\"/>\
             <terminologyElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\">\
             <name content=\"N\"/></terminologyElement></sacm:TerminologyPackage>"
        );
        let doc = read_ok(row, &what, &input);
        let found = validation::validate(&doc);
        assert_eq!(
            found.iter().map(finding).collect::<Vec<_>>(),
            [(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                TERM_003.to_owned(),
                Some("10.9".to_owned()),
                None,
                list(&[id])
            )],
            "{row}: {what}: not one SACM-MULT-001 Error with clause 10.9"
        );
        assert_eq!(
            found[0].message(),
            format!("{class} \"{id}\" has no value (value:String[1])"),
            "{row}: {what}: the message"
        );
        // A strict write writes it without a value.
        let text = write_in(row, &what, &doc, Mode::Strict);
        assert_eq!(
            line_with(row, &text, id),
            format!("<terminologyElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\">"),
            "{row}: {what}: not written without a value\n{text}"
        );
        // An empty value is a value.
        let doc = read_ok(
            row,
            "value=\"\"",
            &input.replace(
                &format!("xmi:id=\"{id}\">"),
                &format!("xmi:id=\"{id}\" value=\"\">"),
            ),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: the {class} {id} with value=\"\" is reported"
        );
    }
}

// =====================================================================================
// SACM23-TERM-003 item 4: SetValue
// =====================================================================================

#[test]
fn sacm23_term_003_set_value_sets_the_value_and_refuses_a_kind_without_it() {
    let row = TERM_003;
    let operation = SetValue::new("t1", " new ");
    assert_eq!(
        (operation.element().as_str(), operation.value()),
        ("t1", " new "),
        "{row}: SetValue holds what it was given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "SetValue",
        "{row}: operation_name()"
    );
    let mut doc = built(row);
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (mutation.changed_ids(), element(row, &doc, "t1").value()),
        (list(&["t1"]).as_slice(), Some(" new ")),
        "{row}: SetValue on a Term"
    );
    // The same value changes nothing.
    let mutation = applied(row, &mut doc, SetValue::new("t1", " new "));
    assert_eq!(
        mutation.changed_ids(),
        ([] as [ElementId; 0]).as_slice(),
        "{row}: setting the same value changed the Term"
    );
    let mutation = applied(row, &mut doc, SetValue::new("e0", ""));
    assert_eq!(
        (mutation.changed_ids(), element(row, &doc, "e0").value()),
        (list(&["e0"]).as_slice(), Some("")),
        "{row}: SetValue on an Expression"
    );
    // A Term with no value gets one.
    let mut doc = read_ok(row, "a Term with no value", &in_terms(&term("x", "", "")));
    let mutation = applied(row, &mut doc, SetValue::new("x", "x"));
    assert_eq!(
        (mutation.changed_ids(), validation::validate(&doc)),
        (list(&["x"]).as_slice(), Vec::new()),
        "{row}: SetValue on a Term with no value"
    );
    // ElementNotFound before FeatureNotApplicable.
    let mut doc = built(row);
    let error = refuse(row, &mut doc, SetValue::new("nowhere", "x"));
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
        "{row}: an unknown element: {error:?}"
    );
    let shown = assert_not_applicable(
        row,
        &mut doc,
        SetValue::new("hazards", "x"),
        "hazards",
        ElementKind::Category,
        "value",
    );
    assert_eq!(
        shown, "the Category \"hazards\" has no value",
        "{row}: the Display of value on a Category"
    );
    for (target, kind) in [
        ("terms", ElementKind::TerminologyPackage),
        ("c1", ElementKind::Claim),
        ("manual", ElementKind::Artifact),
    ] {
        assert_not_applicable(
            row,
            &mut doc,
            SetValue::new(target, "x"),
            target,
            kind,
            "value",
        );
    }
}

/// The element and attribute of the refusal of a write of `doc` in both
/// modes, which must refuse it for U+0001.
fn unrepresentable(row: &str, doc: &Document) -> (String, WriteAttribute) {
    let mut found = Vec::new();
    for mode in MODES {
        match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
            Ok(text) => panic!("{row}: written in {mode:?} mode: {text}"),
            Err(WriteError::UnrepresentableCharacter {
                element,
                character: '\u{1}',
                attribute,
                ..
            }) => found.push((element.as_str().to_owned(), attribute)),
            Err(other) => panic!("{row}: not UnrepresentableCharacter in {mode:?}: {other:?}"),
        }
    }
    assert_eq!(found[0], found[1], "{row}: the modes refuse otherwise");
    found.swap_remove(0)
}

#[test]
fn sacm23_term_003_a_value_holding_u_0001_is_accepted_and_refused_by_the_write_in_order() {
    let row = TERM_003;
    let mut doc = built(row);
    applied(row, &mut doc, SetGid::new("t1", "g\u{1}"));
    applied(row, &mut doc, SetValue::new("t1", "v\u{1}"));
    applied(row, &mut doc, SetExternalReference::new("t1", "u\u{1}"));
    applied(
        row,
        &mut doc,
        SetName::new("t1", LangString::new("N").with_lang("e\u{1}")),
    );
    assert_eq!(
        unrepresentable(row, &doc),
        ("t1".to_owned(), WriteAttribute::Gid),
        "{row}: the gid first"
    );
    applied(row, &mut doc, SetGid::remove("t1"));
    assert_eq!(
        unrepresentable(row, &doc),
        ("t1".to_owned(), WriteAttribute::Value),
        "{row}: the value after the gid"
    );
    applied(row, &mut doc, SetValue::new("t1", "v"));
    assert_eq!(
        unrepresentable(row, &doc),
        ("t1".to_owned(), WriteAttribute::ExternalReference),
        "{row}: the externalReference after the value"
    );
    applied(row, &mut doc, SetExternalReference::remove("t1"));
    assert_eq!(
        unrepresentable(row, &doc),
        ("t1".to_owned(), WriteAttribute::NameLang),
        "{row}: the name's lang after the externalReference"
    );
    // On an Expression too.
    let mut doc = built(row);
    applied(row, &mut doc, SetValue::new("e0", "\u{1}"));
    assert_eq!(
        unrepresentable(row, &doc),
        ("e0".to_owned(), WriteAttribute::Value),
        "{row}: an Expression's value"
    );
}

// =====================================================================================
// SACM23-TERM-003 item 5: Difference::Value
// =====================================================================================

#[test]
fn sacm23_term_003_a_value_difference_displays_both_values() {
    let row = TERM_003;
    let left = fixture_doc(row);
    let right = read_ok(
        row,
        "the fixture without blade's value",
        &fixture_with(
            row,
            " value=\"blade\" externalReference",
            " externalReference",
        ),
    );
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{row}: not one difference: {found:?}");
    };
    assert_eq!(
        (difference.to_string(), difference.element_id()),
        (
            "element \"blade\" differs in value: \"blade\" on the left, no value on the right"
                .to_owned(),
            Some(&ElementId::from("blade"))
        ),
        "{row}: the Display of the value difference"
    );
}

#[test]
fn sacm23_term_003_semantic_compare_reports_one_value_difference_compared_exactly() {
    let row = TERM_003;
    let with = |value: &str| {
        read_ok(
            row,
            value,
            &in_terms(&term("x", &format!(" value=\"{value}\""), "")),
        )
    };
    for (a, b) in [("a", "a "), ("a", "A"), ("", "x")] {
        assert_eq!(
            semantic_compare(&with(a), &with(b)),
            [Difference::Value {
                id: ElementId::from("x"),
                left: Some(a.to_owned()),
                right: Some(b.to_owned()),
            }],
            "{row}: {a:?} vs {b:?}"
        );
    }
    let without = read_ok(row, "no value", &in_terms(&term("x", "", "")));
    assert_eq!(
        semantic_compare(&without, &with("")),
        [Difference::Value {
            id: ElementId::from("x"),
            left: None,
            right: Some(String::new()),
        }],
        "{row}: no value vs an empty value"
    );
}

// =====================================================================================
// SACM23-TERM-003 item 6: category on a Term and an Expression
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_term_003_category_on_a_term_and_an_expression_reads_and_validates_with_clause_10_9() {
    let row = TERM_003;
    for (class, make) in [
        ("Term", term as fn(&str, &str, &str) -> String),
        ("Expression", expression),
    ] {
        // The three forms read alike, with no finding.
        let attribute = read_ok(
            row,
            "category=\"hazards\"",
            &in_terms(&make("x", " category=\"hazards\" value=\"x\"", "")),
        );
        for content in [
            "<category xmi:idref=\"hazards\"/>",
            "<category href=\"#hazards\"/>",
        ] {
            let doc = read_ok(row, content, &in_terms(&make("x", " value=\"x\"", content)));
            assert!(
                doc == attribute,
                "{row}: {content} on a {class}: does not read == to the attribute"
            );
        }
        assert_eq!(
            validation::validate(&attribute),
            [],
            "{row}: category=\"hazards\" on a {class}: a finding"
        );
        assert_repeated(
            row,
            &format!("category= and a category element on a {class}"),
            &in_terms(&make(
                "x",
                &format!(" {MARK}category=\"hazards\" value=\"x\""),
                &format!("{MARK}<category xmi:idref=\"hazards\"/>"),
            )),
            "x",
            "category",
        );
        assert_preserved_at(
            row,
            &format!("category=\"\" on a {class}"),
            &in_terms(&make("x", &format!(" value=\"x\" {MARK}category=\"\""), "")),
            "x",
        );
        for (what, value, finding) in [
            (
                "naming a Term",
                "t1",
                expected(
                    Code::REF_WRONG_TYPE,
                    Severity::Error,
                    "SACM23-VAL-004",
                    "10.9",
                    &["x", "t1"],
                ),
            ),
            (
                "naming a Claim",
                "c1",
                expected(
                    Code::REF_WRONG_TYPE,
                    Severity::Error,
                    "SACM23-VAL-004",
                    "10.9",
                    &["x", "c1"],
                ),
            ),
            (
                "naming no element",
                "ghost",
                expected(
                    Code::REF_DANGLING,
                    Severity::Error,
                    "SACM23-VAL-003",
                    "10.9",
                    &["x", "ghost"],
                ),
            ),
            (
                "naming one twice",
                "hazards hazards",
                expected(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Warning,
                    "SACM23-VAL-022",
                    "10.9",
                    &["x", "hazards"],
                ),
            ),
        ] {
            let what = format!("category=\"{value}\" on a {class}, {what}");
            let doc = read_ok(
                row,
                &what,
                &in_terms(&make(
                    "x",
                    &format!(" category=\"{value}\" value=\"x\""),
                    "",
                )),
            );
            assert_eq!(findings(&doc), [finding], "{row}: {what}");
        }
    }
    // The acceptance's case: an Expression in the fixture naming the Term blade.
    let doc = read_ok(
        row,
        "category=\"blade\" on blade_guarded",
        &fixture_with(
            row,
            "xmi:id=\"blade_guarded\" value=",
            "xmi:id=\"blade_guarded\" category=\"blade\" value=",
        ),
    );
    assert_eq!(
        findings(&doc),
        [expected(
            Code::REF_WRONG_TYPE,
            Severity::Error,
            "SACM23-VAL-004",
            "10.9",
            &["blade_guarded", "blade"]
        )],
        "{row}: category=\"blade\" on an Expression"
    );
}

#[test]
fn sacm23_term_003_deleting_a_category_of_a_term_is_referenced_and_a_scrub_keeps_the_term() {
    let row = TERM_003;
    let input = in_terms(&term("x", " category=\"hazards\" value=\"x\"", ""));
    let mut doc = read_ok(row, "a Term in hazards", &input);
    let error = refuse(row, &mut doc, DeleteElement::new("hazards"));
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end, id, .. }
                if holder.as_str() == "x" && *end == "category" && id.as_str() == "hazards"
        ),
        "{row}: deleting hazards: not Referenced {{ x, category, hazards }}: {error:?}"
    );
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok(row, "a Term in hazards", &input);
        let mutation = applied(
            row,
            &mut doc,
            DeleteElement::new("hazards").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.changed_ids(),
                doc.contains("x"),
                written_end(row, &doc, "x", "category")
            ),
            (list(&["x"]).as_slice(), true, None),
            "{row}: {policy:?}: the Term is not kept with its category scrubbed"
        );
    }
}

#[test]
fn sacm23_term_003_category_edits_work_on_a_term_and_an_expression() {
    let row = TERM_003;
    let mut doc = built(row);
    for holder in ["t1", "e0"] {
        applied(
            row,
            &mut doc,
            SetReferences::new(holder, ReferenceEnd::Category, ["hazards"]),
        );
        applied(
            row,
            &mut doc,
            AddReference::new(holder, ReferenceEnd::Category, "hazards"),
        );
        assert_eq!(
            strs(element(row, &doc, holder).categories()),
            ["hazards", "hazards"],
            "{row}: the category edits on {holder}"
        );
        applied(
            row,
            &mut doc,
            RemoveReference::new(holder, ReferenceEnd::Category, "hazards"),
        );
        assert!(
            element(row, &doc, holder).categories().is_empty(),
            "{row}: RemoveReference on {holder}"
        );
    }
}

// =====================================================================================
// SACM23-TERM-004 item 1: the fixture's Term and its forms
// =====================================================================================

#[test]
fn sacm23_term_004_the_fixture_s_term_blade_reports_its_features() {
    let row = TERM_004;
    let doc = fixture_doc(row);
    let blade = element(row, &doc, "blade");
    assert_eq!(
        (
            blade.kind(),
            strs(blade.categories()),
            blade.origin().map(ElementId::as_str),
            blade.value(),
            blade.external_reference()
        ),
        (
            ElementKind::Term,
            vec!["mechanical"],
            Some("manual"),
            Some("blade"),
            Some("https://example.org/terms/blade")
        ),
        "{row}: the Term blade"
    );
    assert_eq!(
        (
            element(row, &doc, "lid").origin(),
            element(row, &doc, "lid").external_reference(),
            element(row, &doc, "blade_guarded").origin(),
            element(row, &doc, "blade_guarded").external_reference(),
            element(row, &doc, "mechanical").external_reference()
        ),
        (None, None, None, None, None),
        "{row}: origin() and external_reference() of the others"
    );
}

#[test]
fn sacm23_term_004_origin_and_external_reference_read_alike_in_each_form() {
    let row = TERM_004;
    let attribute = read_ok(
        row,
        "the attributes",
        &in_terms(&term(
            "x",
            " origin=\"manual\" value=\"x\" externalReference=\"https://example.org/x\"",
            "",
        )),
    );
    for (what, attributes, content) in [
        (
            "<origin xmi:idref=\"manual\"/>",
            " value=\"x\" externalReference=\"https://example.org/x\"",
            "<origin xmi:idref=\"manual\"/>",
        ),
        (
            "<origin href=\"#manual\"/>",
            " value=\"x\" externalReference=\"https://example.org/x\"",
            "<origin href=\"#manual\"/>",
        ),
        (
            "an externalReference element",
            " origin=\"manual\" value=\"x\"",
            "<externalReference>https://example.org/x</externalReference>",
        ),
    ] {
        let doc = read_ok(row, what, &in_terms(&term("x", attributes, content)));
        assert!(
            doc == attribute,
            "{row}: {what}: does not read == to the attributes"
        );
    }
    assert_eq!(
        validation::validate(&attribute),
        [],
        "{row}: a Term with an origin validates with a finding"
    );
}

#[test]
fn sacm23_term_004_a_term_s_attributes_are_written_in_the_pinned_order() {
    let row = TERM_004;
    let what = "a Term with every attribute, given out of order";
    let doc = read_ok(
        row,
        what,
        &in_terms(&term(
            "x",
            " externalReference=\"u\" value=\"v\" origin=\"manual\" category=\"hazards\" \
             abstractForm=\"t1\" citedElement=\"t1\" isAbstract=\"true\" isCitation=\"true\" \
             gid=\"X-1\"",
            "",
        )),
    );
    let text = round_trip(row, what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "x"),
        "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"x\" gid=\"X-1\" isCitation=\"true\" \
         isAbstract=\"true\" citedElement=\"t1\" abstractForm=\"t1\" category=\"hazards\" \
         origin=\"manual\" value=\"v\" externalReference=\"u\">",
        "{row}: {what}: not written in the pinned order\n{text}"
    );
}

// =====================================================================================
// SACM23-TERM-004 item 2: origin
// =====================================================================================

#[test]
fn sacm23_term_004_two_origins_are_repeated_value() {
    let row = TERM_004;
    let what = "origin=\"a b\"";
    let (input, marks) = marked(
        row,
        &in_terms(&term(
            "x",
            &format!(" origin=\"{MARK}manual {MARK}hazards\" value=\"x\""),
            "",
        )),
    );
    let error = refused(row, what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "x" && property == "origin"
        ),
        "{row}: {what}: not RepeatedValue {{ x, origin }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations()),
        (Code::MULTIPLICITY_VIOLATION, marks.as_slice()),
        "{row}: {what}: not SACM-MULT-001 at each id"
    );
    // An attribute and an element.
    assert_repeated(
        row,
        "origin= and an origin element",
        &in_terms(&term(
            "x",
            &format!(" {MARK}origin=\"manual\" value=\"x\""),
            &format!("{MARK}<origin xmi:idref=\"hazards\"/>"),
        )),
        "x",
        "origin",
    );
}

#[test]
fn sacm23_term_004_validate_types_origin_as_a_model_element_with_clause_10_11() {
    let row = TERM_004;
    let with = |target: &str| {
        read_ok(
            row,
            target,
            &case(
                &term("x", &format!(" origin=\"{target}\" value=\"x\""), ""),
                &category("remote", "", ""),
            ),
        )
    };
    assert_eq!(
        findings(&with("n1")),
        [expected(
            Code::REF_WRONG_TYPE,
            Severity::Error,
            "SACM23-VAL-004",
            "10.11",
            &["x", "n1"]
        )],
        "{row}: an origin naming a Note"
    );
    for target in [
        "manual", "c1", "hazards", "remote", "terms", "t1", "e0", "case", "p",
    ] {
        assert_eq!(
            validation::validate(&with(target)),
            [],
            "{row}: an origin naming {target} is reported"
        );
    }
    assert_eq!(
        findings(&with("ghost")),
        [expected(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003",
            "10.11",
            &["x", "ghost"]
        )],
        "{row}: an origin naming no element"
    );
}

// =====================================================================================
// SACM23-TERM-004 item 3: externalReference
// =====================================================================================

#[test]
fn sacm23_term_004_external_reference_reads_and_writes_verbatim() {
    let row = TERM_004;
    for written in ["", "urn:x", " https://x "] {
        let what = format!("externalReference=\"{written}\"");
        let doc = read_ok(
            row,
            &what,
            &in_terms(&term(
                "x",
                &format!(" value=\"x\" externalReference=\"{written}\""),
                "",
            )),
        );
        let text = round_trip(row, &what, &doc, Mode::Strict);
        assert_eq!(
            line_with(row, &text, "x"),
            format!(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"x\" value=\"x\" \
                 externalReference=\"{written}\">"
            ),
            "{row}: {what}: not written back as read\n{text}"
        );
    }
    // Absent and empty are apart.
    let absent = read_ok(
        row,
        "no externalReference",
        &in_terms(&term("x", " value=\"x\"", "")),
    );
    let empty = read_ok(
        row,
        "an empty externalReference",
        &in_terms(&term("x", " value=\"x\" externalReference=\"\"", "")),
    );
    assert!(
        absent != empty,
        "{row}: an empty externalReference reads as none"
    );
}

#[test]
fn sacm23_term_004_external_reference_returns_it_exactly_as_written() {
    let row = TERM_004;
    for written in ["", "urn:x", " https://x "] {
        let doc = read_ok(
            row,
            written,
            &in_terms(&term(
                "x",
                &format!(" value=\"x\" externalReference=\"{written}\""),
                "",
            )),
        );
        assert_eq!(
            element(row, &doc, "x").external_reference(),
            Some(written),
            "{row}: externalReference=\"{written}\""
        );
    }
}

#[test]
fn sacm23_term_004_external_reference_twice_is_repeated_and_elsewhere_is_preserved() {
    let row = TERM_004;
    assert_repeated(
        row,
        "externalReference= and an element",
        &in_terms(&term(
            "x",
            &format!(" value=\"x\" {MARK}externalReference=\"a\""),
            &format!("{MARK}<externalReference>b</externalReference>"),
        )),
        "x",
        "externalReference",
    );
    for (what, template) in [
        (
            "externalReference= on an Expression",
            in_terms(&expression(
                "x",
                &format!(" value=\"x\" {MARK}externalReference=\"u\""),
                "",
            )),
        ),
        (
            "an externalReference element on an Expression",
            in_terms(&expression(
                "x",
                " value=\"x\"",
                &format!("{MARK}<externalReference>u</externalReference>"),
            )),
        ),
        (
            "externalReference= on a Category",
            in_terms(&category(
                "x",
                &format!(" {MARK}externalReference=\"u\""),
                "",
            )),
        ),
    ] {
        assert_preserved_at(row, what, &template, "x");
    }
}

#[test]
fn sacm23_term_004_set_external_reference_sets_removes_and_refuses_a_kind_without_it() {
    let row = TERM_004;
    let operation = SetExternalReference::new("t1", "https://example.org/t1");
    assert_eq!(
        (
            operation.element().as_str(),
            operation.external_reference(),
            SetExternalReference::remove("t1").external_reference()
        ),
        ("t1", Some("https://example.org/t1"), None),
        "{row}: SetExternalReference holds what it was given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "SetExternalReference",
        "{row}: operation_name()"
    );
    let mut doc = built(row);
    let mutation = applied(row, &mut doc, operation);
    assert_eq!(
        (
            mutation.changed_ids(),
            element(row, &doc, "t1").external_reference()
        ),
        (list(&["t1"]).as_slice(), Some("https://example.org/t1")),
        "{row}: set"
    );
    let mutation = applied(
        row,
        &mut doc,
        SetExternalReference::new("t1", "https://example.org/t1"),
    );
    assert_eq!(
        mutation.changed_ids(),
        ([] as [ElementId; 0]).as_slice(),
        "{row}: setting the same value changed the Term"
    );
    let mutation = applied(row, &mut doc, SetExternalReference::remove("t1"));
    assert_eq!(
        (
            mutation.changed_ids(),
            element(row, &doc, "t1").external_reference()
        ),
        (list(&["t1"]).as_slice(), None),
        "{row}: remove"
    );
    let mutation = applied(row, &mut doc, SetExternalReference::remove("t1"));
    assert_eq!(
        mutation.changed_ids(),
        ([] as [ElementId; 0]).as_slice(),
        "{row}: removing what is absent changed the Term"
    );
    let error = refuse(row, &mut doc, SetExternalReference::new("nowhere", "u"));
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
        "{row}: an unknown element: {error:?}"
    );
    for operation in [
        SetExternalReference::new("e0", "u"),
        SetExternalReference::remove("e0"),
    ] {
        let shown = assert_not_applicable(
            row,
            &mut doc,
            operation,
            "e0",
            ElementKind::Expression,
            "externalReference",
        );
        assert_eq!(
            shown, "the Expression \"e0\" has no externalReference",
            "{row}: the Display of externalReference on an Expression"
        );
    }
    assert_not_applicable(
        row,
        &mut doc,
        SetExternalReference::new("hazards", "u"),
        "hazards",
        ElementKind::Category,
        "externalReference",
    );
}

// =====================================================================================
// SACM23-TERM-004 item 4: ReferenceEnd::Origin
// =====================================================================================

#[test]
fn sacm23_term_004_reference_end_origin_applies_only_to_a_term_and_holds_one_id() {
    let row = TERM_004;
    let end = ReferenceEnd::Origin;
    assert_eq!(end.name(), "origin", "{row}: Origin.name()");
    for &kind in ElementKind::ALL {
        let term = kind == ElementKind::Term;
        assert_eq!(
            (end.applies_to(kind), end.bounds(kind)),
            (term, term.then_some((0, Some(1)))),
            "{row}: origin on a {kind}"
        );
    }
    let mut doc = built(row);
    let error = refuse(
        row,
        &mut doc,
        SetReferences::new("t1", ReferenceEnd::Origin, ["manual", "c1"]),
    );
    assert!(
        matches!(&error, EditError::TooManyReferences { end, given, allowed, .. }
            if *end == "origin" && *given == 2 && *allowed == 1),
        "{row}: two origins: not TooManyReferences: {error:?}"
    );
    applied(
        row,
        &mut doc,
        SetReferences::new("t1", ReferenceEnd::Origin, ["manual"]),
    );
    let t1 = element(row, &doc, "t1");
    assert_eq!(
        (t1.origin(), t1.references(ReferenceEnd::Origin).first()),
        (
            Some(&ElementId::from("manual")),
            Some(&ElementId::from("manual"))
        ),
        "{row}: origin() is not references(Origin).first()"
    );
    let error = refuse(
        row,
        &mut doc,
        SetReferences::new("t1", ReferenceEnd::Origin, ["n1"]),
    );
    assert!(
        matches!(&error, EditError::WrongReferenceType { end, id, expected, .. }
            if *end == "origin" && id.as_str() == "n1" && *expected == "ModelElement"),
        "{row}: a Note as the origin: {error:?}"
    );
    assert_not_applicable(
        row,
        &mut doc,
        SetReferences::new("e0", ReferenceEnd::Origin, ["manual"]),
        "e0",
        ElementKind::Expression,
        "origin",
    );
}

// =====================================================================================
// SACM23-TERM-004 item 5: CreateTerm
// =====================================================================================

#[test]
fn sacm23_term_004_create_term_reports_what_it_was_given_and_creates_it() {
    let row = TERM_004;
    let operation = CreateTerm::new("terms", named("Blade"), "blade");
    assert_eq!(
        (
            operation.parent().as_str(),
            operation.id(),
            operation.name(),
            operation.value(),
            operation.categories(),
            operation.origin(),
            operation.external_reference()
        ),
        (
            "terms",
            None,
            &named("Blade"),
            "blade",
            [].as_slice(),
            None,
            None
        ),
        "{row}: the defaults"
    );
    let operation = operation
        .with_id("blade")
        .with_categories(["hazards"])
        .with_origin("c1")
        .with_external_reference("x")
        .with_origin("manual")
        .with_external_reference("https://example.org/terms/blade");
    assert_eq!(
        (
            operation.id().map(ElementId::as_str),
            strs(operation.categories()),
            operation.origin().map(ElementId::as_str),
            operation.external_reference()
        ),
        (
            Some("blade"),
            vec!["hazards"],
            Some("manual"),
            Some("https://example.org/terms/blade")
        ),
        "{row}: the builders replace what they were given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateTerm",
        "{row}: operation_name()"
    );
    let mut doc = built(row);
    create(row, &mut doc, operation);
    let blade = element(row, &doc, "blade");
    assert_eq!(
        (
            blade.kind(),
            blade.value(),
            strs(blade.categories()),
            blade.origin().map(ElementId::as_str),
            blade.external_reference()
        ),
        (
            ElementKind::Term,
            Some("blade"),
            vec!["hazards"],
            Some("manual"),
            Some("https://example.org/terms/blade")
        ),
        "{row}: the created Term"
    );
    let text = write_in(row, "the created Term", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "blade"),
        "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" category=\"hazards\" \
         origin=\"manual\" value=\"blade\" externalReference=\"https://example.org/terms/blade\">",
        "{row}: the created Term's start tag\n{text}"
    );
}

#[test]
fn sacm23_term_004_create_term_legal_parents_refusals_and_generated_id() {
    let row = TERM_004;
    let make: Make = |parent, id| {
        let create = CreateTerm::new(parent.unwrap_or("terms"), named("N"), "n");
        match id {
            Some(id) => create.with_id(id).into(),
            None => create.into(),
        }
    };
    assert_create_rules(
        row,
        ElementKind::Term,
        make,
        "term",
        &["case", "p", "hazards", "t1", "e0", "c1", "manual"],
    );
    let term_with = |categories: &[&str], origin: &str| -> Operation {
        CreateTerm::new("terms", named("N"), "n")
            .with_categories(categories.iter().copied())
            .with_origin(origin)
            .into()
    };
    assert_refusals(
        row,
        vec![
            (
                "the parent before the ends",
                CreateTerm::new("nowhere", named("N"), "n")
                    .with_origin("a b")
                    .into(),
                ("ParentNotFound", vec!["nowhere"]),
            ),
            (
                "an illegal parent before the ends",
                CreateTerm::new("hazards", named("N"), "n")
                    .with_categories(["a b"])
                    .into(),
                ("InvalidParent", vec!["hazards", "Category", "Term"]),
            ),
            (
                "white space in an origin before a missing category",
                term_with(&["nope"], "a b"),
                ("InvalidReference", vec!["origin", "a b"]),
            ),
            (
                "white space in a category before one in the origin",
                term_with(&["a b"], "c d"),
                ("InvalidReference", vec!["category", "a b"]),
            ),
            (
                "a missing origin before a wrongly typed category",
                term_with(&["t1"], "nope"),
                ("ReferenceNotFound", vec!["origin", "nope"]),
            ),
            (
                "a missing category before a missing origin",
                term_with(&["hazards", "nope"], "nope2"),
                ("ReferenceNotFound", vec!["category", "nope"]),
            ),
            (
                "a wrongly typed category before a wrongly typed origin",
                term_with(&["t1"], "n1"),
                ("WrongReferenceType", vec!["category", "t1", "Category"]),
            ),
            (
                "a Note as the origin",
                term_with(&["hazards"], "n1"),
                ("WrongReferenceType", vec!["origin", "n1", "ModelElement"]),
            ),
        ],
    );
}

// =====================================================================================
// SACM23-TERM-004 item 6: Difference::Origin and Difference::ExternalReference
// =====================================================================================

#[test]
fn sacm23_term_004_an_origin_difference_displays_both_origins() {
    let row = TERM_004;
    let left = fixture_doc(row);
    let right = read_ok(
        row,
        "the fixture without blade's origin",
        &fixture_with(row, " origin=\"manual\"", ""),
    );
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{row}: not one difference: {found:?}");
    };
    assert_eq!(
        difference.to_string(),
        "element \"blade\" differs in origin: \"manual\" on the left, no origin on the right",
        "{row}: the Display of the origin difference"
    );
}

#[test]
fn sacm23_term_004_origin_value_and_external_reference_differences_come_in_order() {
    let row = TERM_004;
    let left = read_ok(
        row,
        "left",
        &in_terms(&term(
            "x",
            " category=\"hazards\" origin=\"manual\" value=\"a\" externalReference=\"u\"",
            "",
        )),
    );
    let right = read_ok(row, "right", &in_terms(&term("x", " value=\"b\"", "")));
    let id = || ElementId::from("x");
    assert_eq!(
        semantic_compare(&left, &right),
        [
            Difference::Categories {
                id: id(),
                left: list(&["hazards"]),
                right: Vec::new(),
            },
            Difference::Origin {
                id: id(),
                left: Some(ElementId::from("manual")),
                right: None,
            },
            Difference::Value {
                id: id(),
                left: Some("a".to_owned()),
                right: Some("b".to_owned()),
            },
            Difference::ExternalReference {
                id: id(),
                left: Some("u".to_owned()),
                right: None,
            },
        ],
        "{row}: not Categories, Origin, Value and ExternalReference in that order"
    );
}

#[test]
fn sacm23_term_004_an_external_reference_difference_displays_both_references() {
    let row = TERM_004;
    let with = read_ok(
        row,
        "a Term with an externalReference",
        &in_terms(&term("x", " value=\"a\" externalReference=\"urn:x\"", "")),
    );
    let without = read_ok(
        row,
        "the Term without it",
        &in_terms(&term("x", " value=\"a\"", "")),
    );
    for (left, right, english) in [
        (
            &with,
            &without,
            "element \"x\" differs in externalReference: \"urn:x\" on the left, no externalReference on the right",
        ),
        (
            &without,
            &with,
            "element \"x\" differs in externalReference: no externalReference on the left, \"urn:x\" on the right",
        ),
    ] {
        let found = semantic_compare(left, right);
        let [difference] = found.as_slice() else {
            panic!("{row}: not one difference: {found:?}");
        };
        assert!(
            matches!(difference, Difference::ExternalReference { .. }),
            "{row}: not an ExternalReference difference: {difference:?}"
        );
        assert_eq!(
            (difference.to_string(), difference.element_id()),
            (english.to_owned(), Some(&ElementId::from("x"))),
            "{row}: the Display of the externalReference difference names externalReference"
        );
    }
}

// =====================================================================================
// SACM23-TERM-004 item 7: deleting an origin, and a Term named by an ArtifactReference
// =====================================================================================

#[test]
fn sacm23_term_004_deleting_an_origin_is_referenced_and_a_scrub_clears_it_and_keeps_the_term() {
    let row = TERM_004;
    let input = in_terms(&term("x", " origin=\"hazards\" value=\"x\"", ""));
    let mut doc = read_ok(row, "a Term whose origin is hazards", &input);
    let error = refuse(row, &mut doc, DeleteElement::new("hazards"));
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end, id, .. }
                if holder.as_str() == "x" && *end == "origin" && id.as_str() == "hazards"
        ),
        "{row}: deleting the origin: not Referenced {{ x, origin, hazards }}: {error:?}"
    );
    assert_eq!(
        referrers(&doc, "hazards"),
        [("x".to_owned(), "origin", 0)],
        "{row}: Document::referrers does not list the origin"
    );
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok(row, "a Term whose origin is hazards", &input);
        let mutation = applied(
            row,
            &mut doc,
            DeleteElement::new("hazards").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.deleted_ids(),
                mutation.changed_ids(),
                doc.contains("x"),
                written_end(row, &doc, "x", "origin"),
                validation::validate(&doc)
            ),
            (
                list(&["hazards"]).as_slice(),
                list(&["x"]).as_slice(),
                true,
                None,
                Vec::new()
            ),
            "{row}: {policy:?}: the origin is not cleared with the Term kept"
        );
    }
}

#[test]
fn sacm23_term_004_a_term_named_by_an_artifact_reference_validates_and_is_cross_package() {
    let row = TERM_004;
    // The fixture's ArtifactReference blade_ref names the Term blade: a Term
    // is an ArtifactElement (§10.2), so no finding.
    let doc = fixture_doc(row);
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: an ArtifactReference naming a Term is reported"
    );
    let input = in_terms("").replace(
        "<name content=\"C1\"/>",
        "<name content=\"C1\"/></argumentElement><argumentElement \
         xmi:type=\"sacm:ArtifactReference\" xmi:id=\"ar\" referencedArtifactElement=\"t1\">\
         <name content=\"AR\"/>",
    );
    let mut doc = read_ok(row, "an ArtifactReference naming t1", &input);
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: an ArtifactReference naming t1 is reported"
    );
    let error = refuse(row, &mut doc, DeleteElement::new("t1"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end, id, .. }
                if holder.as_str() == "ar"
                    && *end == "referencedArtifactElement"
                    && id.as_str() == "t1"
        ),
        "{row}: deleting t1: not CrossPackageReference {{ ar, referencedArtifactElement, t1 }}: \
         {error:?}"
    );
}

// =====================================================================================
// SACM23-TERM-004 item 8: the existing edits on a Term
// =====================================================================================

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_term_004_the_existing_edits_apply_to_a_term() {
    let row = TERM_004;
    let mut doc = read_ok(row, "the case", &in_terms(&term("t2", " value=\"t2\"", "")));
    applied(row, &mut doc, SetName::new("t1", named("Renamed")));
    applied(row, &mut doc, SetGid::new("t1", "T-1"));
    applied(row, &mut doc, SetFlag::new("t1", Flag::IsAbstract, true));
    applied(
        row,
        &mut doc,
        SetReferences::new("t1", ReferenceEnd::CitedElement, ["t2"]),
    );
    applied(
        row,
        &mut doc,
        ReorderElements::new("terms", ["t2", "e0", "t1", "hazards"]),
    );
    let t1 = element(row, &doc, "t1");
    assert_eq!(
        (
            t1.name(),
            t1.gid(),
            t1.is_abstract(),
            t1.cited_element().map(ElementId::as_str),
            children(row, &doc, "terms")
        ),
        (
            Some(&named("Renamed")),
            Some("T-1"),
            true,
            Some("t2"),
            ids(&["t2", "e0", "t1", "hazards"])
        ),
        "{row}: the edits on a Term"
    );
    assert_not_applicable(
        row,
        &mut doc,
        SetFlag::new("t1", Flag::IsCounter, true),
        "t1",
        ElementKind::Term,
        "isCounter",
    );
    // MoveElement: a TerminologyPackage or an ArtifactPackage.
    for (parent, role) in [
        ("evidence", "artifactElement"),
        ("other", "terminologyElement"),
        ("terms", "terminologyElement"),
    ] {
        let mutation = applied(row, &mut doc, MoveElement::new("t2", parent));
        assert_eq!(
            (mutation.changed_ids(), children(row, &doc, parent).last()),
            (list(&["t2"]).as_slice(), Some(&"t2".to_owned())),
            "{row}: moving t2 into {parent}"
        );
        let text = round_trip(row, &format!("t2 in {parent}"), &doc, Mode::Strict);
        assert!(
            line_with(row, &text, "t2").starts_with(&format!("<{role} xmi:type=\"sacm:Term\"")),
            "{row}: t2 in {parent} is not written typed in {role}\n{text}"
        );
    }
    for (parent, parent_kind) in [
        ("p", ElementKind::ArgumentPackage),
        ("case", ElementKind::AssuranceCasePackage),
        ("hazards", ElementKind::Category),
        ("e0", ElementKind::Expression),
    ] {
        let error = refuse(row, &mut doc, MoveElement::new("t2", parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::Term
            ),
            "{row}: moving a Term into the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    // CopyElements: a copy with a new id, its value kept.
    let copy = match CopyElements::new(&doc, ["t2"], "other") {
        Ok(copy) => copy,
        Err(error) => panic!("{row}: CopyElements::new refused t2: {error:?}"),
    };
    let mutation = applied(row, &mut doc, copy);
    let [copied] = mutation.created_ids() else {
        panic!("{row}: the copy created {:?}", mutation.created_ids());
    };
    let copied = copied.as_str().to_owned();
    let text = write_in(row, "the copy", &doc, Mode::Strict);
    assert_eq!(
        (
            element(row, &doc, &copied).kind(),
            element(row, &doc, &copied)
                .parent()
                .map(|parent| parent.id().as_str().to_owned()),
            line_with(row, &text, &copied).ends_with(" value=\"t2\">")
        ),
        (ElementKind::Term, Some("other".to_owned()), true),
        "{row}: the copy of a Term\n{text}"
    );
    // A Term is no ArtifactAsset: no Property under it.
    let error = refuse(row, &mut doc, CreateProperty::new("t2", named("P")));
    assert!(
        matches!(
            &error,
            EditError::InvalidParent { parent, parent_kind, kind, .. }
                if parent.as_str() == "t2" && *parent_kind == ElementKind::Term
                    && *kind == ElementKind::Property
        ),
        "{row}: CreateProperty under a Term: not InvalidParent: {error:?}"
    );
    // DeleteElement, once t1 no longer cites t2.
    applied(
        row,
        &mut doc,
        SetReferences::clear("t1", ReferenceEnd::CitedElement),
    );
    let mutation = applied(row, &mut doc, DeleteElement::new("t2"));
    assert_eq!(
        (mutation.deleted_ids(), doc.contains("t2")),
        (list(&["t2"]).as_slice(), false),
        "{row}: deleting a Term"
    );
}

// =====================================================================================
// SACM23-TERM-005 item 1: the fixture's Expression and element's forms
// =====================================================================================

#[test]
fn sacm23_term_005_the_fixture_s_expression_holds_its_elements() {
    let row = TERM_005;
    let doc = fixture_doc(row);
    let guarded = element(row, &doc, "blade_guarded");
    assert_eq!(
        (
            guarded.kind(),
            strs(guarded.expression_elements()),
            guarded.expression_elements(),
            guarded.value()
        ),
        (
            ElementKind::Expression,
            vec!["blade", "lid"],
            guarded.references(ReferenceEnd::Element),
            Some("$Blade$ is covered by the $Lid$")
        ),
        "{row}: the Expression blade_guarded"
    );
    for id in ["blade", "hazards", "reviewed", "terms"] {
        assert!(
            element(row, &doc, id).expression_elements().is_empty(),
            "{row}: expression_elements() of {id} is not empty"
        );
    }
}

#[test]
fn sacm23_term_005_element_reads_from_the_attribute_and_both_element_forms_alike() {
    let row = TERM_005;
    let attribute = read_ok(
        row,
        "element=\"t1 e0\"",
        &in_terms(&expression("x", " element=\"t1 e0\" value=\"x\"", "")),
    );
    let doc = read_ok(
        row,
        "two element elements",
        &in_terms(&expression(
            "x",
            " value=\"x\"",
            "<element xmi:idref=\"t1\"/><element href=\"#e0\"/>",
        )),
    );
    assert!(
        doc == attribute,
        "{row}: the element forms do not read == to the attribute"
    );
    let text = round_trip(row, "element=\"t1 e0\"", &attribute, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "x"),
        "<terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"x\" element=\"t1 e0\" \
         value=\"x\">",
        "{row}: element is not written before value\n{text}"
    );
    assert_repeated(
        row,
        "element= and an element element",
        &in_terms(&expression(
            "x",
            &format!(" {MARK}element=\"t1\" value=\"x\""),
            &format!("{MARK}<element xmi:idref=\"e0\"/>"),
        )),
        "x",
        "element",
    );
    assert_preserved_at(
        row,
        "element=\"\"",
        &in_terms(&expression(
            "x",
            &format!(" value=\"x\" {MARK}element=\"\""),
            "",
        )),
        "x",
    );
    // On a Term, element is no feature: preserved.
    assert_preserved_at(
        row,
        "element= on a Term",
        &in_terms(&term(
            "x",
            &format!(" value=\"x\" {MARK}element=\"t1\""),
            "",
        )),
        "x",
    );
}

#[test]
fn sacm23_term_005_the_fixture_writes_element_after_category_and_before_value() {
    let row = TERM_005;
    let doc = fixture_doc(row);
    let text = write_in(row, "the fixture", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "blade_guarded"),
        "<terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"blade_guarded\" \
         element=\"blade lid\" value=\"$Blade$ is covered by the $Lid$\">",
        "{row}: the fixture's Expression is not written as the golden file has it\n{text}"
    );
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{row}: cannot read {GOLDEN}: {error}"));
    assert_eq!(
        text, golden,
        "{row}: the fixture is not written as the golden file"
    );
    let doc = read_ok(
        row,
        "an Expression with a category and elements",
        &in_terms(&expression(
            "x",
            " value=\"x\" element=\"t1\" category=\"hazards\"",
            "",
        )),
    );
    let text = write_in(row, "category and element", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "x"),
        "<terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"x\" category=\"hazards\" \
         element=\"t1\" value=\"x\">",
        "{row}: element is not written after category\n{text}"
    );
}

// =====================================================================================
// SACM23-TERM-005 item 2: ReferenceEnd::Element
// =====================================================================================

#[test]
fn sacm23_term_005_reference_end_element_applies_only_to_an_expression() {
    let row = TERM_005;
    let end = ReferenceEnd::Element;
    assert_eq!(end.name(), "element", "{row}: Element.name()");
    for &kind in ElementKind::ALL {
        let expression = kind == ElementKind::Expression;
        assert_eq!(
            (end.applies_to(kind), end.bounds(kind)),
            (expression, expression.then_some((0, None))),
            "{row}: element on a {kind}"
        );
    }
}

// =====================================================================================
// SACM23-TERM-005 item 3: validating element
// =====================================================================================

#[test]
fn sacm23_term_005_validate_types_element_as_an_expression_element_with_clause_10_10() {
    let row = TERM_005;
    let with = |body: &str| read_ok(row, body, &in_terms(body));
    for (what, target, kind_code) in [
        ("a Category", "hazards", Code::REF_WRONG_TYPE),
        ("a Claim", "c1", Code::REF_WRONG_TYPE),
        ("no element", "ghost", Code::REF_DANGLING),
    ] {
        let doc = with(&expression(
            "x",
            &format!(" element=\"{target}\" value=\"x\""),
            "",
        ));
        let requirement = if kind_code == Code::REF_DANGLING {
            "SACM23-VAL-003"
        } else {
            "SACM23-VAL-004"
        };
        assert_eq!(
            findings(&doc),
            [expected(
                kind_code,
                Severity::Error,
                requirement,
                "10.10",
                &["x", target]
            )],
            "{row}: element naming {what}"
        );
    }
    let doc = with(&expression("x", " element=\"t1 t1\" value=\"x\"", ""));
    assert_eq!(
        findings(&doc),
        [expected(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022",
            "10.10",
            &["x", "t1"]
        )],
        "{row}: element=\"t1 t1\""
    );
    // Itself, another Expression and a cycle: no finding; nor a value that
    // does not mention the elements' names (D-EXPR-NAMES).
    for (what, body) in [
        (
            "an Expression naming itself",
            expression("x", " element=\"x\" value=\"x\"", ""),
        ),
        (
            "an Expression naming another",
            expression("x", " element=\"e0 t1\" value=\"x\"", ""),
        ),
        (
            "two Expressions naming each other",
            format!(
                "{}{}",
                expression("x", " element=\"y\" value=\"x\"", ""),
                expression("y", " element=\"x\" value=\"y\"", "")
            ),
        ),
        (
            "a value that names none of its elements",
            expression("x", " element=\"t1\" value=\"nothing here\"", ""),
        ),
    ] {
        assert_eq!(
            validation::validate(&with(&body)),
            [],
            "{row}: {what}: validate reports a finding"
        );
    }
}

// =====================================================================================
// SACM23-TERM-005 item 4: CreateExpression
// =====================================================================================

#[test]
fn sacm23_term_005_create_expression_reports_what_it_was_given_and_creates_it() {
    let row = TERM_005;
    let operation = CreateExpression::new("terms", named("Guarded"), "$T1$ guarded");
    assert_eq!(
        (
            operation.parent().as_str(),
            operation.id(),
            operation.name(),
            operation.value(),
            operation.categories(),
            operation.expression_elements()
        ),
        (
            "terms",
            None,
            &named("Guarded"),
            "$T1$ guarded",
            [].as_slice(),
            [].as_slice()
        ),
        "{row}: the defaults"
    );
    let operation = operation
        .with_id("guarded")
        .with_categories(["hazards"])
        .with_expression_elements(["e0"])
        .with_expression_elements(["t1", "e0"]);
    assert_eq!(
        (
            operation.id().map(ElementId::as_str),
            strs(operation.categories()),
            strs(operation.expression_elements())
        ),
        (Some("guarded"), vec!["hazards"], vec!["t1", "e0"]),
        "{row}: the builders replace what they were given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateExpression",
        "{row}: operation_name()"
    );
    let mut doc = built(row);
    create(row, &mut doc, operation);
    let guarded = element(row, &doc, "guarded");
    assert_eq!(
        (
            guarded.kind(),
            guarded.value(),
            strs(guarded.categories()),
            strs(guarded.expression_elements())
        ),
        (
            ElementKind::Expression,
            Some("$T1$ guarded"),
            vec!["hazards"],
            vec!["t1", "e0"]
        ),
        "{row}: the created Expression"
    );
    let text = write_in(row, "the created Expression", &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "guarded"),
        "<terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"guarded\" \
         category=\"hazards\" element=\"t1 e0\" value=\"$T1$ guarded\">",
        "{row}: the created Expression's start tag\n{text}"
    );
}

#[test]
fn sacm23_term_005_create_expression_legal_parents_refusals_and_generated_id() {
    let row = TERM_005;
    let make: Make = |parent, id| {
        let create = CreateExpression::new(parent.unwrap_or("terms"), named("N"), "n");
        match id {
            Some(id) => create.with_id(id).into(),
            None => create.into(),
        }
    };
    assert_create_rules(
        row,
        ElementKind::Expression,
        make,
        "expression",
        &["case", "p", "hazards", "t1", "e0", "c1", "manual"],
    );
    let expression_with = |categories: &[&str], elements: &[&str]| -> Operation {
        CreateExpression::new("terms", named("N"), "n")
            .with_categories(categories.iter().copied())
            .with_expression_elements(elements.iter().copied())
            .into()
    };
    assert_refusals(
        row,
        vec![
            (
                "the id before the ends",
                CreateExpression::new("terms", named("N"), "n")
                    .with_id("1x")
                    .with_expression_elements(["a b"])
                    .into(),
                ("InvalidId", vec!["1x"]),
            ),
            (
                "white space in an element before a missing category",
                expression_with(&["nope"], &["t1", "a b"]),
                ("InvalidReference", vec!["element", "a b"]),
            ),
            (
                "a missing element before a wrongly typed category",
                expression_with(&["t1"], &["nope"]),
                ("ReferenceNotFound", vec!["element", "nope"]),
            ),
            (
                "a missing category before a missing element",
                expression_with(&["nope"], &["nope2"]),
                ("ReferenceNotFound", vec!["category", "nope"]),
            ),
            (
                "a wrongly typed category before a wrongly typed element",
                expression_with(&["e0"], &["hazards"]),
                ("WrongReferenceType", vec!["category", "e0", "Category"]),
            ),
            (
                "a Category as an element",
                expression_with(&[], &["t1", "hazards"]),
                (
                    "WrongReferenceType",
                    vec!["element", "hazards", "ExpressionElement"],
                ),
            ),
            (
                "a Claim as an element",
                expression_with(&[], &["c1"]),
                (
                    "WrongReferenceType",
                    vec!["element", "c1", "ExpressionElement"],
                ),
            ),
        ],
    );
}

// =====================================================================================
// SACM23-TERM-005 item 5: Difference::ExpressionElements, and element on a Term
// =====================================================================================

#[test]
fn sacm23_term_005_an_expression_elements_difference_displays_its_end_and_both_lists() {
    let row = TERM_005;
    let left = fixture_doc(row);
    let right = read_ok(
        row,
        "the fixture without lid in blade_guarded",
        &fixture_with(row, "      <element href=\"#lid\"/>\n", ""),
    );
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{row}: not one difference: {found:?}");
    };
    assert_eq!(
        difference.to_string(),
        "element \"blade_guarded\" differs in element: [\"blade\", \"lid\"] on the left, \
         [\"blade\"] on the right",
        "{row}: the Display of the element difference"
    );
}

#[test]
fn sacm23_term_005_expression_elements_are_reported_after_categories() {
    let row = TERM_005;
    let left = read_ok(
        row,
        "left",
        &in_terms(&expression(
            "x",
            " category=\"hazards\" element=\"t1\" value=\"x\"",
            "",
        )),
    );
    let right = read_ok(
        row,
        "right",
        &in_terms(&expression("x", " element=\"t1 e0\" value=\"x\"", "")),
    );
    assert_eq!(
        semantic_compare(&left, &right),
        [
            Difference::Categories {
                id: ElementId::from("x"),
                left: list(&["hazards"]),
                right: Vec::new(),
            },
            Difference::ExpressionElements {
                id: ElementId::from("x"),
                left: list(&["t1"]),
                right: list(&["t1", "e0"]),
            },
        ],
        "{row}: not Categories, then ExpressionElements"
    );
}

#[test]
fn sacm23_term_005_the_element_edits_work_on_an_expression_and_not_on_a_term() {
    let row = TERM_005;
    let mut doc = built(row);
    applied(
        row,
        &mut doc,
        SetReferences::new("e0", ReferenceEnd::Element, ["t1", "e0"]),
    );
    applied(
        row,
        &mut doc,
        AddReference::new("e0", ReferenceEnd::Element, "t1"),
    );
    applied(
        row,
        &mut doc,
        RemoveReference::new("e0", ReferenceEnd::Element, "e0"),
    );
    assert_eq!(
        strs(element(row, &doc, "e0").expression_elements()),
        ["t1", "t1"],
        "{row}: the element edits"
    );
    let error = refuse(
        row,
        &mut doc,
        AddReference::new("e0", ReferenceEnd::Element, "hazards"),
    );
    assert!(
        matches!(&error, EditError::WrongReferenceType { end, id, expected, .. }
            if *end == "element" && id.as_str() == "hazards" && *expected == "ExpressionElement"),
        "{row}: a Category as an element: {error:?}"
    );
    let doc_with_blade = read_ok(
        row,
        "blade",
        &in_terms(&term("blade", " value=\"blade\"", "")),
    );
    let mut doc = doc_with_blade;
    let shown = assert_not_applicable(
        row,
        &mut doc,
        SetReferences::new("blade", ReferenceEnd::Element, ["t1"]),
        "blade",
        ElementKind::Term,
        "element",
    );
    assert_eq!(
        shown, "the Term \"blade\" has no element",
        "{row}: the Display of element on a Term"
    );
}

// =====================================================================================
// SACM23-TERM-005 item 6: deleting an element of an Expression
// =====================================================================================

#[test]
fn sacm23_term_005_deleting_an_element_of_an_expression_is_referenced_and_a_scrub_keeps_it() {
    let row = TERM_005;
    let input = in_terms(&expression("x", " element=\"t1 e0 t1\" value=\"x\"", ""));
    let mut doc = read_ok(row, "an Expression over t1 and e0", &input);
    let error = refuse(row, &mut doc, DeleteElement::new("t1"));
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end, id, .. }
                if holder.as_str() == "x" && *end == "element" && id.as_str() == "t1"
        ),
        "{row}: deleting t1: not Referenced {{ x, element, t1 }}: {error:?}"
    );
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok(row, "an Expression over t1 and e0", &input);
        let mutation = applied(
            row,
            &mut doc,
            DeleteElement::new("t1").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.deleted_ids(),
                mutation.changed_ids(),
                doc.contains("x"),
                written_end(row, &doc, "x", "element")
            ),
            (
                list(&["t1"]).as_slice(),
                list(&["x"]).as_slice(),
                true,
                Some("e0".to_owned())
            ),
            "{row}: {policy:?}: the Expression is not kept with t1 removed"
        );
        // Its last element: the Expression stays, with no end.
        let mutation = applied(
            row,
            &mut doc,
            DeleteElement::new("e0").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.changed_ids(),
                doc.contains("x"),
                written_end(row, &doc, "x", "element")
            ),
            (list(&["x"]).as_slice(), true, None),
            "{row}: {policy:?}: the Expression does not survive losing its last element"
        );
    }
}

// =====================================================================================
// The usage example of the design brief
// =====================================================================================

#[test]
fn sacm23_term_005_the_usage_example_builds_an_expression_over_two_terms() {
    use sacm::edit::CreateTerminologyPackage;
    let row = TERM_005;
    let en = |text: &str| LangString::new(text).with_lang("en");
    let mut doc = Document::new();
    applied(
        row,
        &mut doc,
        CreateTerminologyPackage::root(en("Blender terms")).with_id("terms"),
    );
    applied(
        row,
        &mut doc,
        CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"),
    );
    applied(
        row,
        &mut doc,
        CreateTerm::new("terms", en("Lid"), "lid").with_id("lid"),
    );
    let guarded = CreateExpression::new(
        "terms",
        en("Blade guarded"),
        "$Blade$ is covered by the $Lid$",
    )
    .with_expression_elements(["blade", "lid"]);
    applied(row, &mut doc, guarded.with_id("guarded"));
    let parts: Vec<&str> = element(row, &doc, "guarded")
        .expression_elements()
        .iter()
        .map(ElementId::as_str)
        .collect();
    assert_eq!(parts, ["blade", "lid"], "{row}: the usage example");
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the usage example validates with a finding"
    );
    round_trip(row, "the usage example", &doc, Mode::Strict);
}
