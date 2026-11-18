//! Tests for `ExpressionLangString` (SACM23-BASE-009; formal/23-05-08 §8.4,
//! ptc/22-03-13 `Base::ExpressionLangString`; ADR 0038, ADR 0030 amendment 4)
//! and its exclusivity rule (SACM23-VAL-009; §8.4 "If expression is not
//! empty, then +content should be empty.", D-EXPR-CONTENT).
//!
//! - An `ExpressionLangString` is a form of `LangString`, with an optional
//!   `expression`: the id of the `ExpressionElement` it links to.
//! - `xmi::read` reads it in a name and in each value of a `content`, `key`
//!   or `location`, typed `sacm:ExpressionLangString`, with its `expression`
//!   as an IDREF attribute or one reference element; `xmi::write` writes it
//!   typed, with the attribute.
//! - `ReferenceEnd::Expression` is derived from the name and the texts:
//!   `referrers`, delete, copy, compare and validation see it, and the
//!   reference edits refuse it as `DerivedEnd`.
//! - `validate` reports a missing expression (SACM-MULT-001) and an
//!   expression beside a content that is not empty (SACM-EXPR-001).
//!
//! The fixtures are `xmi/expression-lang-string-valid.sacm.xmi`, its golden
//! `xmi/golden/expression-lang-string.golden.sacm.xmi`,
//! `validation/invalid/expression-lang-string-findings.sacm.xmi` and
//! `validation/warning/expression-lang-string-with-content.sacm.xmi`.

use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{self, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CrossPackageReferencePolicy, DeleteElement, EditError, Mutation, Operation,
    ReferenceDeletePolicy, SetName,
};
use sacm::edit::{
    AddReference, CopyElements, CreateArgumentPackage, CreateArtifactPackage,
    CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim, CreateExpression, CreateNote,
    CreateResource, CreateTaggedValue, CreateTerm, CreateTerminologyPackage, MoveElement,
    RemoveReference, ReorderElements, SetContent, SetKey, SetLocation, SetReferences,
};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::model::{ElementKind, MultiLangString, ReferenceEnd};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const BASE_009: &str = "SACM23-BASE-009";
const VAL_009: &str = "SACM23-VAL-009";

/// The namespace declarations of a root: `sacm` and `xmi`, as in
/// `message_templates.rs`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// SACM-EXPR-001 by its text: `Code::EXPRESSION_WITH_CONTENT`, which
/// `sacm23_val_009_the_code_and_its_catalog_row` checks.
const EXPR_001: Code = Code::new("SACM-EXPR-001");

/// The type attribute of an `ExpressionLangString`.
const ELS: &str = "xmi:type=\"sacm:ExpressionLangString\"";

/// The fixture directory.
const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23");
const VALID: &str = "xmi/expression-lang-string-valid.sacm.xmi";
const GOLDEN: &str = "xmi/golden/expression-lang-string.golden.sacm.xmi";
const FINDINGS: &str = "validation/invalid/expression-lang-string-findings.sacm.xmi";
const WITH_CONTENT: &str = "validation/warning/expression-lang-string-with-content.sacm.xmi";

/// The catalog, relative to the crate.
const CATALOG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/sacm/diagnostics.md"
);

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The text of the fixture `name`, relative to [`DATA`].
fn fixture(row: &str, name: &str) -> String {
    let path = Path::new(DATA).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: setup: cannot read {}: {error}", path.display()))
}

/// The catalog's text.
fn catalog(row: &str) -> String {
    std::fs::read_to_string(CATALOG)
        .unwrap_or_else(|error| panic!("{row}: setup: cannot read {CATALOG}: {error}"))
}

/// The meaning column of the catalog row of `code`.
fn catalog_row(row: &str, code: &str) -> String {
    let text = catalog(row);
    let prefix = format!("| {code} |");
    let Some(line) = text.lines().find(|line| line.starts_with(&prefix)) else {
        panic!("{row}: the catalog has no row for {code}");
    };
    line.to_owned()
}

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, each with no diagnostic, to equal
/// documents; returns the tolerant one.
fn read_clean(row: &str, what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert_eq!(
                    output.diagnostics(),
                    [],
                    "{row}: {what}: a {mode:?} read reports diagnostics"
                );
                documents.push(output.into_document());
            }
            Err(error) => panic!("{row}: {what}: a {mode:?} read refuses it: {error}"),
        }
    }
    let tolerant = documents
        .pop()
        .unwrap_or_else(|| panic!("{row}: two reads"));
    let strict = documents
        .pop()
        .unwrap_or_else(|| panic!("{row}: two reads"));
    assert!(
        strict == tolerant,
        "{row}: {what}: the strict and the tolerant read differ: {:?}",
        semantic_compare(&strict, &tolerant)
    );
    tolerant
}

/// The output of a read of `input` in `mode`, which must succeed.
fn read_ok(row: &str, what: &str, mode: Mode, input: &str) -> ReadOutput {
    read(mode, input).unwrap_or_else(|error| panic!("{row}: {what}: a {mode:?} read: {error}"))
}

/// The refusal of a strict read of `input`.
fn strict_refusal(row: &str, what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{row}: {what}: a strict read accepts it, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

fn write(doc: &Document, mode: Mode) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(mode))
}

fn written(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    write(doc, mode).unwrap_or_else(|error| panic!("{row}: {what}: a {mode:?} write: {error}"))
}

fn element<'a>(row: &str, doc: &'a Document, id: &str) -> SacmElement<'a> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{row}: setup: no element {id:?}"))
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().map(|&value| ElementId::from(value)).collect()
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

/// Applies `operation`, which must be accepted.
fn apply(row: &str, what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    edit::apply(doc, operation.clone())
        .unwrap_or_else(|error| panic!("{row}: {what}: {operation:?} was refused: {error}"))
}

/// Applies `operation`, which must be refused, to a clone of `doc`, checks
/// that the clone is unchanged, and returns the refusal.
fn refused(row: &str, what: &str, doc: &Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let mut copy = doc.clone();
    let error = match edit::apply(&mut copy, operation.clone()) {
        Ok(mutation) => panic!("{row}: {what}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        copy == *doc,
        "{row}: {what}: the refused {operation:?} changed the document"
    );
    error
}

/// The location of the `n`th (0-based) `needle` in `text`, whose lines end
/// at LF.
fn nth_position(row: &str, text: &str, needle: &str, n: usize) -> SourceLocation {
    let Some((offset, _)) = text.match_indices(needle).nth(n) else {
        panic!(
            "{row}: setup: {needle:?} is not {} times in the input",
            n + 1
        );
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        panic!("{row}: setup: the input is too large");
    };
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{row}: setup: no location {line}:{column}"))
}

fn position_of(row: &str, text: &str, needle: &str) -> SourceLocation {
    nth_position(row, text, needle, 0)
}

/// The `Display` of each diagnostic.
fn lines(found: &[Diagnostic]) -> Vec<String> {
    found.iter().map(ToString::to_string).collect()
}

/// `(code, severity, requirement, clause, location, affected)` of each
/// diagnostic.
type Shape = (
    Code,
    Severity,
    String,
    Option<String>,
    Option<SourceLocation>,
    Vec<ElementId>,
);

fn shapes(found: &[Diagnostic]) -> Vec<Shape> {
    found
        .iter()
        .map(|one| {
            (
                one.code(),
                one.severity(),
                one.requirement_id().to_owned(),
                one.clause().map(str::to_owned),
                one.location(),
                one.affected().to_vec(),
            )
        })
        .collect()
}

fn shape(
    code: Code,
    severity: Severity,
    requirement: &str,
    clause: &str,
    affected: &[&str],
) -> Shape {
    (
        code,
        severity,
        requirement.to_owned(),
        Some(clause.to_owned()),
        None,
        ids(affected),
    )
}

/// A root ArgumentPackage `p` holding `name` and nothing else, as in
/// `message_templates.rs`: its `name` starts at line 1, column 134.
fn named_package(name: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">{name}</sacm:ArgumentPackage>")
}

/// A root AssuranceCasePackage `case` holding the TerminologyPackage `terms`
/// (the Terms `t1` and `t2`), the ArgumentPackage `p` holding `arguments`,
/// and the ArtifactPackage `a` holding `artifacts`.
fn case(arguments: &str, artifacts: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n\
         <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/>\n\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"t1\"><name content=\"T1\"/></terminologyElement>\n\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t2\" value=\"t2\"><name content=\"T2\"/></terminologyElement>\n\
         </terminologyPackage>\n\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>\n{arguments}\n</argumentPackage>\n\
         <artifactPackage xmi:id=\"a\"><name content=\"A\"/>\n{artifacts}\n</artifactPackage>\n\
         </sacm:AssuranceCasePackage>\n"
    )
}

/// A Claim `id` with the name `name` (an element), holding `body`.
fn claim(id: &str, name: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\">{name}{body}</argumentElement>"
    )
}

/// A Note `id` whose `content` holds `values`.
fn note(id: &str, values: &str) -> String {
    format!("<note xmi:id=\"{id}\"><content>{values}</content></note>")
}

/// A `TaggedValue` `id` whose `key` holds `values`, with the content `v`.
fn tagged_value(id: &str, values: &str) -> String {
    format!(
        "<taggedValue xmi:id=\"{id}\"><content><value content=\"v\"/></content><key>{values}</key></taggedValue>"
    )
}

/// A Resource `id` with the name `name`, whose `location` holds `values`.
fn resource(id: &str, name: &str, values: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:Resource\" xmi:id=\"{id}\">{name}<location>{values}</location></artifactElement>"
    )
}

/// A plain name `content`.
fn plain_name(content: &str) -> String {
    format!("<name content=\"{content}\"/>")
}

/// The `RepeatedValue` Display of `property` of `element`, given at
/// `locations`.
fn repeated_display(property: &str, element: &str, locations: &[SourceLocation]) -> String {
    let at: Vec<String> = locations.iter().map(ToString::to_string).collect();
    format!(
        "the single-valued property {property:?} of the element {element:?} is given {} times, at {}",
        locations.len(),
        at.join("; ")
    )
}

/// A name `ExpressionLangString` with `lang` `en`, content `x` and no
/// expression, read from a file.
fn without_expression(row: &str) -> LangString {
    let input = named_package(&format!("<name {ELS} lang=\"en\" content=\"x\"/>"));
    let doc = read_clean(
        row,
        "a name typed ExpressionLangString with no expression",
        &input,
    );
    element(row, &doc, "p")
        .name()
        .cloned()
        .unwrap_or_else(|| panic!("{row}: the name with no expression is not read"))
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 1: the value type
// ---------------------------------------------------------------------------

fn hash_of(value: &LangString) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn sacm23_base_009_lang_string_reads_sets_and_orders_its_expression() {
    let value = LangString::new("x")
        .with_lang("en")
        .with_expression("blade");
    assert_eq!(
        (
            value.expression(),
            value.is_expression_lang_string(),
            value.lang(),
            value.content(),
        ),
        (Some(&id("blade")), true, Some("en"), Some("x")),
        "{BASE_009}: with_expression keeps lang and content and sets the expression"
    );
    let replaced = value.clone().with_expression("lid");
    assert_eq!(
        replaced.expression(),
        Some(&id("lid")),
        "{BASE_009}: a second with_expression replaces the expression"
    );
    assert_eq!(
        value.clone().without_expression(),
        LangString::new("x").with_lang("en"),
        "{BASE_009}: without_expression gives the plain value"
    );
    let plain = LangString::new("x").with_lang("en");
    assert_eq!(
        (plain.expression(), plain.is_expression_lang_string()),
        (None, false),
        "{BASE_009}: a plain value has no expression"
    );
    let no_expression = without_expression(BASE_009);
    assert_eq!(
        (
            no_expression.expression(),
            no_expression.is_expression_lang_string()
        ),
        (None, true),
        "{BASE_009}: an ExpressionLangString read without an expression"
    );
    // With equal lang and content: plain < no expression < a < b.
    let ordered = [
        plain.clone(),
        no_expression.clone(),
        plain.clone().with_expression("a"),
        plain.clone().with_expression("b"),
    ];
    for (i, left) in ordered.iter().enumerate() {
        for (j, right) in ordered.iter().enumerate() {
            assert_eq!(
                left.cmp(right),
                i.cmp(&j),
                "{BASE_009}: the order of {left:?} and {right:?}"
            );
            assert_eq!(
                left == right,
                i == j,
                "{BASE_009}: the equality of {left:?} and {right:?}"
            );
        }
    }
    // The form is compared last: lang, then content, decide first.
    for (left, right) in [
        (
            LangString::new("a").with_expression("z"),
            LangString::new("b"),
        ),
        (
            LangString::default().with_lang("a").with_expression("z"),
            LangString::default().with_lang("b"),
        ),
        (
            LangString::new("a").with_lang("en").with_expression("z"),
            LangString::new("b").with_lang("en"),
        ),
    ] {
        assert!(
            left < right,
            "{BASE_009}: {left:?} is not before {right:?}: the form is compared after lang \
             and content"
        );
    }
    assert_eq!(
        hash_of(&plain.clone().with_expression("a")),
        hash_of(&LangString::new("x").with_lang("en").with_expression("a")),
        "{BASE_009}: equal ExpressionLangStrings hash equally"
    );
    assert_eq!(
        hash_of(&plain),
        hash_of(&LangString::new("x").with_lang("en")),
        "{BASE_009}: equal plain values hash equally"
    );
    assert_eq!(
        format!("{plain:?}"),
        "LangString { lang: Some(\"en\"), content: Some(\"x\") }",
        "{BASE_009}: the Debug of a plain value is unchanged"
    );
    let debug = format!("{value:?}");
    assert!(
        debug.starts_with("ExpressionLangString {") && debug.contains("blade"),
        "{BASE_009}: the Debug of an ExpressionLangString: {debug}"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 2 and 3: the valid fixture and its golden
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_the_valid_fixture_reads_in_both_modes_with_every_expression() {
    let text = fixture(BASE_009, VALID);
    let doc = read_clean(BASE_009, VALID, &text);
    assert_eq!(
        validation::validate(&doc),
        [],
        "{BASE_009}: the valid fixture has findings"
    );
    assert_eq!(
        element(BASE_009, &doc, "c2").name_id(),
        Some(&id("c2_name")),
        "{BASE_009}: c2's name keeps its xmi:id"
    );
    {
        for found in doc.elements() {
            let expected: &[&str] = match found.id().as_str() {
                "c1" => &["blade_guarded"],
                "n1" | "tv1" | "manual_pdf" => &["blade"],
                "c2" => &["lid"],
                _ => &[],
            };
            assert_eq!(
                found.expressions(),
                ids(expected).as_slice(),
                "{BASE_009}: the expressions of {:?}",
                found.id()
            );
        }
        let content: MultiLangString = [
            LangString::new("Kontrollerad mot manualen").with_lang("sv"),
            LangString::new("").with_lang("en").with_expression("blade"),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            element(BASE_009, &doc, "n1").content(),
            Some(&content),
            "{BASE_009}: n1's content"
        );
    }
}

#[test]
fn sacm23_base_009_both_writes_give_the_golden_and_the_golden_reads_back_equal() {
    let text = fixture(BASE_009, VALID);
    let golden = fixture(BASE_009, GOLDEN);
    let doc = read_clean(BASE_009, VALID, &text);
    for mode in MODES {
        assert_eq!(
            written(BASE_009, VALID, &doc, mode),
            golden,
            "{BASE_009}: a {mode:?} write of the valid fixture is not the golden"
        );
    }
    let reread = read_clean(BASE_009, GOLDEN, &golden);
    assert_eq!(
        semantic_compare(&doc, &reread),
        [],
        "{BASE_009}: the golden reads back to another document"
    );
}

#[test]
fn sacm23_base_009_the_reference_and_value_element_forms_read_equal() {
    let with_name = |name: &str| case(&claim("c1", name, ""), "");
    let attribute = with_name(&format!(
        "<name {ELS} lang=\"en\" content=\"x\" expression=\"t1\"/>"
    ));
    let reference = read_clean(BASE_009, "the attribute form", &attribute);
    for (what, name) in [
        (
            "an xmi:idref element",
            format!("<name {ELS} lang=\"en\" content=\"x\"><expression xmi:idref=\"t1\"/></name>"),
        ),
        (
            "an href element",
            format!("<name {ELS} lang=\"en\" content=\"x\"><expression href=\"#t1\"/></name>"),
        ),
        (
            "the lang and content elements beside an expression element",
            format!(
                "<name {ELS}><lang>en</lang><content>x</content><expression xmi:idref=\"t1\"/></name>"
            ),
        ),
    ] {
        let doc = read_clean(BASE_009, what, &with_name(&name));
        assert_eq!(
            semantic_compare(&reference, &doc),
            [],
            "{BASE_009}: {what} does not read as the attribute form"
        );
    }
    assert_eq!(
        element(BASE_009, &reference, "c1").name(),
        Some(&LangString::new("x").with_lang("en").with_expression("t1")),
        "{BASE_009}: the attribute form's name"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 4: xsi:type, no expression, another prefix
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_xsi_type_an_absent_expression_and_another_prefix_read_as_stated() {
    const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
    let in_case =
        |name: &str| case(&claim("c1", name, ""), "").replacen(NS, &format!("{NS}{XSI}"), 1);
    let typed = in_case(&format!("<name {ELS} lang=\"en\" expression=\"t1\"/>"));
    let reference = read_clean(BASE_009, "xmi:type", &typed);
    // xsi:type: the same diagnostics as on a LangString name, and the same
    // document as xmi:type.
    let xsi =
        in_case("<name xsi:type=\"sacm:ExpressionLangString\" lang=\"en\" expression=\"t1\"/>");
    let lang_string = in_case("<name xsi:type=\"sacm:LangString\" lang=\"en\"/>");
    for mode in MODES {
        let output = read_ok(BASE_009, "xsi:type ExpressionLangString", mode, &xsi);
        let control = read_ok(BASE_009, "xsi:type LangString", mode, &lang_string);
        assert_eq!(
            control
                .diagnostics()
                .iter()
                .map(Diagnostic::code)
                .collect::<Vec<_>>(),
            [Code::XMI_XSI_TYPE],
            "{BASE_009}: setup: a LangString name typed with xsi:type ({mode:?})"
        );
        assert_eq!(
            shapes(output.diagnostics()),
            shapes(control.diagnostics()),
            "{BASE_009}: xsi:type on an ExpressionLangString name ({mode:?}) does not report \
             as on a LangString name"
        );
        assert_eq!(
            semantic_compare(&reference, output.document()),
            [],
            "{BASE_009}: xsi:type ({mode:?}) does not read as xmi:type"
        );
    }
    // No expression: read, and written back typed with no expression.
    let absent = in_case(&format!("<name {ELS} lang=\"en\" content=\"x\"/>"));
    let doc = read_clean(BASE_009, "no expression", &absent);
    {
        let name = element(BASE_009, &doc, "c1").name();
        assert_eq!(
            name.map(|name| (name.expression(), name.is_expression_lang_string())),
            Some((None, true)),
            "{BASE_009}: a typed name with no expression"
        );
    }
    for mode in MODES {
        let text = written(BASE_009, "no expression", &doc, mode);
        assert!(
            text.contains(
                "<name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"x\" />"
            ) && !text.contains("expression="),
            "{BASE_009}: a typed name with no expression is not written back typed, with no \
             expression ({mode:?}):\n{text}"
        );
    }
    // Another prefix for the pinned namespace.
    let other_prefix = in_case(
        "<name xmlns:s=\"http://www.omg.org/spec/SACM/20220301\" xmi:type=\"s:ExpressionLangString\" lang=\"en\" expression=\"t1\"/>",
    );
    let doc = read_clean(BASE_009, "another prefix", &other_prefix);
    assert_eq!(
        semantic_compare(&reference, &doc),
        [],
        "{BASE_009}: a type under another prefix of the pinned URI does not read the same"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 5: two expressions
// ---------------------------------------------------------------------------

/// One input with a repeated expression: the owner, its kind of text, the
/// property, the input, and the locations of the occurrences.
struct Repeat {
    what: &'static str,
    owner: &'static str,
    property: &'static str,
    input: String,
    locations: Vec<SourceLocation>,
}

fn repeats(row: &str) -> Vec<Repeat> {
    let mut cases = Vec::new();
    // In a name, in named_package: line 1, the name at column 134.
    let two_ids = named_package(&format!("<name {ELS} expression=\"a b\"/>"));
    let both = named_package(&format!(
        "<name {ELS} expression=\"t1\"><expression xmi:idref=\"t1\"/></name>"
    ));
    let two_elements = named_package(&format!(
        "<name {ELS}><expression xmi:idref=\"t1\"/><expression xmi:idref=\"t1\"/></name>"
    ));
    cases.push(Repeat {
        what: "two ids in a name's attribute",
        owner: "p",
        property: "name.expression",
        locations: vec![
            SourceLocation::new(1, 189).unwrap_or_else(|| panic!("{row}: 1:189")),
            SourceLocation::new(1, 191).unwrap_or_else(|| panic!("{row}: 1:191")),
        ],
        input: two_ids,
    });
    cases.push(Repeat {
        what: "an attribute and an element in a name",
        owner: "p",
        property: "name.expression",
        locations: vec![
            SourceLocation::new(1, 177).unwrap_or_else(|| panic!("{row}: 1:177")),
            SourceLocation::new(1, 193).unwrap_or_else(|| panic!("{row}: 1:193")),
        ],
        input: both,
    });
    cases.push(Repeat {
        what: "two elements in a name",
        owner: "p",
        property: "name.expression",
        locations: vec![
            nth_position(row, &two_elements, "<expression", 0),
            nth_position(row, &two_elements, "<expression", 1),
        ],
        input: two_elements,
    });
    // In a value of a content, a key and a location.
    let value = format!("<value {ELS} expression=\"a b\"/>");
    for (what, property, owner, input) in [
        (
            "two ids in a content value",
            "content.value.expression",
            "n1",
            case(&claim("c1", &plain_name("C1"), &note("n1", &value)), ""),
        ),
        (
            "two ids in a key value",
            "key.value.expression",
            "tv1",
            case(
                &claim("c1", &plain_name("C1"), &tagged_value("tv1", &value)),
                "",
            ),
        ),
        (
            "two ids in a location value",
            "location.value.expression",
            "r1",
            case("", &resource("r1", &plain_name("R1"), &value)),
        ),
    ] {
        let first = position_of(row, &input, "a b\"");
        let second = SourceLocation::new(first.line(), first.column() + 2)
            .unwrap_or_else(|| panic!("{row}: the second id"));
        cases.push(Repeat {
            what,
            owner,
            property,
            locations: vec![first, second],
            input,
        });
    }
    let both_in_value =
        format!("<value {ELS} expression=\"t1\"><expression xmi:idref=\"t1\"/></value>");
    let input = case(
        &claim("c1", &plain_name("C1"), &note("n1", &both_in_value)),
        "",
    );
    cases.push(Repeat {
        what: "an attribute and an element in a content value",
        owner: "n1",
        property: "content.value.expression",
        locations: vec![
            position_of(row, &input, "expression=\"t1\""),
            position_of(row, &input, "<expression"),
        ],
        input,
    });
    cases
}

#[test]
fn sacm23_base_009_two_expressions_are_repeated_values_in_strict_mode() {
    let cases = repeats(BASE_009);
    // The two Displays the row pins.
    assert_eq!(
        repeated_display("name.expression", "p", &cases[0].locations),
        "the single-valued property \"name.expression\" of the element \"p\" is given 2 times, \
         at line 1, column 189; line 1, column 191",
        "{BASE_009}: setup: the pinned Display of two ids"
    );
    for case in &cases {
        let error = strict_refusal(BASE_009, case.what, &case.input);
        match &error {
            ReadError::RepeatedValue {
                element, property, ..
            } => assert_eq!(
                (element.as_str(), property.as_str()),
                (case.owner, case.property),
                "{BASE_009}: {}: the element and the property",
                case.what
            ),
            other => panic!("{BASE_009}: {}: not RepeatedValue: {other:?}", case.what),
        }
        assert_eq!(
            error.code(),
            Code::MULTIPLICITY_VIOLATION,
            "{BASE_009}: {}: the code",
            case.what
        );
        assert_eq!(
            error.to_string(),
            repeated_display(case.property, case.owner, &case.locations),
            "{BASE_009}: {}: the Display",
            case.what
        );
    }
}

#[test]
fn sacm23_base_009_two_expressions_keep_the_whole_part_in_tolerant_mode() {
    for case in repeats(BASE_009) {
        let what = case.what;
        let output = read_ok(BASE_009, what, Mode::Tolerant, &case.input);
        let second = case.locations.get(1).copied();
        // The row the repeat is reported under is the reader's (LIB-COMPAT-001);
        // this row fixes the code, severity, location and affected ids.
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|one| {
                (
                    one.code(),
                    one.severity(),
                    one.location(),
                    one.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                second,
                ids(&[case.owner]),
            )],
            "{BASE_009}: {what}: not one SACM-MULT-001 Warning at the second occurrence, \
             affecting {:?}",
            case.owner
        );
        let doc = output.into_document();
        let owner = element(BASE_009, &doc, case.owner);
        let text_kept = match case.property {
            "name.expression" => owner.name().is_none(),
            "content.value.expression" => owner.content().is_none(),
            "key.value.expression" => owner.key().is_none(),
            _ => owner.location().is_none(),
        };
        assert!(
            owner.has_preserved_content() && text_kept,
            "{BASE_009}: {what}: the whole part is not kept as preserved content of {:?}",
            case.owner
        );
    }
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 6: what does not conform
// ---------------------------------------------------------------------------

/// Which text of the owner is kept whole.
#[derive(Clone, Copy, Debug)]
enum Part {
    Name,
    Content,
    Key,
    Location,
}

/// The inputs that keep a whole part with SACM-XMI-003: what, the input,
/// the owner, the part, and the needle of the part's start tag.
#[expect(
    clippy::too_many_lines,
    reason = "one flat table of inputs is easier to review than several"
)]
fn nonconforming() -> Vec<(&'static str, String, &'static str, Part, &'static str)> {
    const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
    let in_name = |name: &str| case(&claim("c1", name, ""), "");
    let mut cases = vec![
        (
            "expression on an untyped name",
            in_name("<name content=\"x\" expression=\"t1\"/>"),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "expression on a name typed sacm:LangString",
            in_name("<name xmi:type=\"sacm:LangString\" content=\"x\" expression=\"t1\"/>"),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an empty expression",
            in_name(&format!("<name {ELS} content=\"x\" expression=\"\"/>")),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression of white space",
            in_name(&format!("<name {ELS} content=\"x\" expression=\" \"/>")),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "sacm:expression",
            in_name(&format!(
                "<name {ELS} content=\"x\" sacm:expression=\"t1\"/>"
            )),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression element with no link attribute",
            in_name(&format!("<name {ELS} content=\"x\"><expression/></name>")),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression element with both link attributes",
            in_name(&format!(
                "<name {ELS} content=\"x\"><expression xmi:idref=\"t1\" href=\"#t1\"/></name>"
            )),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression element holding text",
            in_name(&format!(
                "<name {ELS} content=\"x\"><expression xmi:idref=\"t1\">t1</expression></name>"
            )),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "xmi:type naming ExpressionLangString beside xsi:type naming LangString",
            in_name(&format!(
                "<name {ELS} xsi:type=\"sacm:LangString\" content=\"x\" expression=\"t1\"/>"
            ))
            .replacen(NS, &format!("{NS}{XSI}"), 1),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an ExpressionLangString value inside a name",
            in_name(&format!("<name><value {ELS} expression=\"t1\"/></name>")),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "xmi:type ExpressionLangString on a content",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &format!(
                        "<note xmi:id=\"n1\"><content {ELS}><value content=\"x\"/></content></note>"
                    ),
                ),
                "",
            ),
            "n1",
            Part::Content,
            "<content",
        ),
    ];
    // The same defects in a value keep the whole content, key or location.
    cases.extend([
        (
            "expression on an untyped content value",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &note("n1", "<value content=\"x\" expression=\"t1\"/>"),
                ),
                "",
            ),
            "n1",
            Part::Content,
            "<content",
        ),
        (
            "an empty expression in a key value",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &tagged_value("tv1", &format!("<value {ELS} expression=\"\"/>")),
                ),
                "",
            ),
            "tv1",
            Part::Key,
            "<key",
        ),
        (
            "an expression element with no link attribute in a location value",
            case(
                "",
                &resource(
                    "r1",
                    &plain_name("R1"),
                    &format!("<value {ELS}><expression/></value>"),
                ),
            ),
            "r1",
            Part::Location,
            "<location",
        ),
    ]);
    // An `expression` reference element in a plain name or value, and a
    // `value` inside an ExpressionLangString name.
    let in_name = |name: &str| case(&claim("c1", name, ""), "");
    cases.extend([
        (
            "an expression element in an untyped name",
            in_name("<name content=\"x\"><expression xmi:idref=\"t1\"/></name>"),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression element in a name typed sacm:LangString",
            in_name(
                "<name xmi:type=\"sacm:LangString\" content=\"x\"><expression xmi:idref=\"t1\"/></name>",
            ),
            "c1",
            Part::Name,
            "<name",
        ),
        (
            "an expression element in an untyped content value",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &note(
                        "n1",
                        "<value content=\"x\"><expression xmi:idref=\"t1\"/></value>",
                    ),
                ),
                "",
            ),
            "n1",
            Part::Content,
            "<content",
        ),
        (
            "a value inside an ExpressionLangString name",
            in_name(&format!(
                "<name {ELS} expression=\"t1\"><value content=\"x\"/></name>"
            )),
            "c1",
            Part::Name,
            "<name",
        ),
    ]);
    cases
}

/// The location of the first `start` after `owner`'s id in `input`.
fn start_tag(row: &str, input: &str, owner: &str, start: &str) -> SourceLocation {
    let anchor = format!("xmi:id=\"{owner}\"");
    let Some(from) = input.find(&anchor) else {
        panic!("{row}: setup: no {anchor}");
    };
    let Some(offset) = input[from..].find(start) else {
        panic!("{row}: setup: no {start} after {anchor}");
    };
    position_of(row, input, &input[from + offset..])
}

/// Why `input` is not kept as the row says in `mode`, or `None`.
fn kept_with_xmi_003(
    input: &str,
    mode: Mode,
    owner: &str,
    part: Part,
    at: SourceLocation,
) -> Option<String> {
    let severity = match mode {
        Mode::Strict => Severity::Error,
        _ => Severity::Warning,
    };
    let output = match read(mode, input) {
        Ok(output) => output,
        Err(error) => return Some(format!("a {mode:?} read refuses it: {error}")),
    };
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .filter(|one| one.code() == Code::XMI_UNKNOWN_CONTENT)
        .map(|one| (one.severity(), one.location(), one.affected().to_vec()))
        .collect();
    if found != [(severity, Some(at), ids(&[owner]))] {
        return Some(format!(
            "not one SACM-XMI-003 at the {part:?}'s start tag {at} affecting {owner:?} \
             ({mode:?}): {:?}",
            lines(output.diagnostics())
        ));
    }
    let doc = output.into_document();
    let holder = element(BASE_009, &doc, owner);
    let absent = match part {
        Part::Name => holder.name().is_none(),
        Part::Content => holder.content().is_none(),
        Part::Key => holder.key().is_none(),
        Part::Location => holder.location().is_none(),
    };
    if !holder.has_preserved_content() || !absent {
        return Some(format!(
            "the whole {part:?} is not kept as preserved content of {owner:?} ({mode:?})"
        ));
    }
    match write(&doc, Mode::Strict) {
        Err(error @ WriteError::PreservedContent { .. })
            if error.code() == Code::XMI_STRICT_SAVE_REFUSED =>
        {
            None
        }
        other => Some(format!(
            "a strict write is not refused with SACM-XMI-006 ({mode:?}): {other:?}"
        )),
    }
}

#[test]
fn sacm23_base_009_a_nonconforming_expression_keeps_the_whole_part_with_xmi_003() {
    let cases = nonconforming();
    let mut failures = Vec::new();
    for (what, input, owner, part, start) in &cases {
        let at = start_tag(BASE_009, input, owner, start);
        for mode in [Mode::Tolerant, Mode::Strict] {
            if let Some(why) = kept_with_xmi_003(input, mode, owner, *part, at) {
                failures.push(format!("{what}: {why}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{BASE_009}: {} of {} reads do not keep the whole part with SACM-XMI-003:\n{}",
        failures.len(),
        cases.len() * 2,
        failures.join("\n")
    );
}

#[test]
fn sacm23_base_009_two_type_attributes_naming_different_classes_keep_the_whole_name() {
    // With no `expression` beside them, neither type may win: the name is
    // kept whole, whichever order the attributes come in.
    const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
    let mut failures = Vec::new();
    for (what, types) in [
        (
            "xmi:type ExpressionLangString, then xsi:type LangString",
            format!("{ELS} xsi:type=\"sacm:LangString\""),
        ),
        (
            "xmi:type LangString, then xsi:type ExpressionLangString",
            "xmi:type=\"sacm:LangString\" xsi:type=\"sacm:ExpressionLangString\"".to_owned(),
        ),
    ] {
        let input = case(
            &claim(
                "c1",
                &format!("<name {types} lang=\"en\" content=\"x\"/>"),
                "",
            ),
            "",
        )
        .replacen(NS, &format!("{NS}{XSI}"), 1);
        let at = start_tag(BASE_009, &input, "c1", "<name");
        for mode in [Mode::Tolerant, Mode::Strict] {
            if let Some(why) = kept_with_xmi_003(&input, mode, "c1", Part::Name, at) {
                failures.push(format!("{what}: {why}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{BASE_009}: a name with two disagreeing types is read:\n{}",
        failures.join("\n")
    );
}

#[test]
fn sacm23_base_009_an_expression_attribute_on_a_claim_is_a_preserved_attribute() {
    let with = |attribute: &str| {
        case(
            &format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" {attribute}=\"t1\"><name content=\"C1\"/></argumentElement>"
            ),
            "",
        )
    };
    let expression = with("expression");
    let unknown = with("vendorAttribute");
    for mode in MODES {
        let output = read_ok(
            BASE_009,
            "an expression attribute on a Claim",
            mode,
            &expression,
        );
        let control = read_ok(BASE_009, "an unknown attribute on a Claim", mode, &unknown);
        assert!(
            !control.diagnostics().is_empty(),
            "{BASE_009}: setup: an unknown attribute on a Claim reports nothing ({mode:?})"
        );
        assert_eq!(
            shapes(output.diagnostics()),
            shapes(control.diagnostics()),
            "{BASE_009}: an expression attribute on a Claim is not kept as any unknown \
             attribute is ({mode:?})"
        );
        let doc = output.into_document();
        let c1 = element(BASE_009, &doc, "c1");
        assert!(
            c1.has_preserved_content(),
            "{BASE_009}: the attribute is not kept ({mode:?})"
        );
        assert_eq!(
            c1.expressions(),
            [],
            "{BASE_009}: an expression attribute on a Claim is read as an end ({mode:?})"
        );
    }
    // With an ExpressionLangString name too: the derived end holds the
    // name's id once, and a write never writes it as an element beside the
    // preserved attribute.
    let both = case(
        &format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" expression=\"t1\">\
             <name {ELS} expression=\"t1\"/></argumentElement>"
        ),
        "",
    );
    let doc = read_ok(BASE_009, "both", Mode::Tolerant, &both).into_document();
    assert_eq!(
        element(BASE_009, &doc, "c1").expressions(),
        ids(&["t1"]),
        "{BASE_009}: the derived end of a Claim with both"
    );
    let text = written(BASE_009, "both", &doc, Mode::Tolerant);
    assert!(
        !text.contains("<expression"),
        "{BASE_009}: the derived end is written as an element:\n{text}"
    );
    let reread = read_ok(BASE_009, "both written", Mode::Tolerant, &text).into_document();
    assert!(
        reread == doc,
        "{BASE_009}: the write of a Claim with both does not read back equal: {:?}",
        semantic_compare(&doc, &reread)
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 7: what stays Unsupported
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_xmi_metadata_and_a_cross_document_href_on_expression_are_unsupported() {
    let in_name = |inside: &str| {
        case(
            &claim(
                "c1",
                &format!("<name {ELS} lang=\"en\">{inside}</name>"),
                "",
            ),
            "",
        )
    };
    let metadata = in_name("<expression xmi:idref=\"t1\" xmi:id=\"e\"/>");
    let error = strict_refusal(BASE_009, "an xmi:id on an expression element", &metadata);
    assert!(
        matches!(error, ReadError::Unsupported { .. })
            && error
                .to_string()
                .ends_with("(a limit of this version; SACM23-XMI-019 reads it)"),
        "{BASE_009}: an xmi:id on an expression element is not Unsupported naming \
         SACM23-XMI-019: {error}"
    );
    let href = in_name("<expression href=\"other.xmi#t1\"/>");
    let error = strict_refusal(BASE_009, "a cross-document href", &href);
    assert!(
        matches!(error, ReadError::Unsupported { .. })
            && error
                .to_string()
                .ends_with("(a limit of this version; SACM23-XMI-027 reads it)"),
        "{BASE_009}: a cross-document href on an expression element is not Unsupported naming \
         SACM23-XMI-027: {error}"
    );
    let output = read_ok(BASE_009, "a cross-document href", Mode::Tolerant, &href);
    assert_eq!(
        output
            .diagnostics()
            .iter()
            .map(|one| (one.code(), one.severity(), one.location()))
            .collect::<Vec<_>>(),
        [(Code::XMI_EXTERNAL_HREF, Severity::Warning, error.location())],
        "{BASE_009}: a tolerant read does not keep the href with one SACM-XMI-007 Warning"
    );
    assert!(
        output
            .document()
            .elements()
            .any(SacmElement::has_preserved_content),
        "{BASE_009}: the cross-document href is not kept"
    );
}

#[test]
fn sacm23_base_009_no_input_is_unsupported_naming_sacm23_base_009() {
    let mut inputs: Vec<(String, String)> = vec![
        (VALID.to_owned(), fixture(BASE_009, VALID)),
        (GOLDEN.to_owned(), fixture(BASE_009, GOLDEN)),
        (FINDINGS.to_owned(), fixture(BASE_009, FINDINGS)),
        (WITH_CONTENT.to_owned(), fixture(BASE_009, WITH_CONTENT)),
        (
            "the name of 0.1.0's refusal".to_owned(),
            named_package(&format!("<name {ELS} content=\"p\"/>")),
        ),
        (
            "a content value".to_owned(),
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &note("n1", &format!("<value {ELS} content=\"v\"/>")),
                ),
                "",
            ),
        ),
    ];
    inputs.extend(
        repeats(BASE_009)
            .into_iter()
            .map(|case| (case.what.to_owned(), case.input)),
    );
    inputs.extend(
        nonconforming()
            .into_iter()
            .map(|(what, input, _, _, _)| (what.to_owned(), input)),
    );
    let mut failures = Vec::new();
    for (what, input) in &inputs {
        if let Err(error) = read(Mode::Strict, input) {
            if error.to_string().contains("SACM23-BASE-009")
                || error.template() == "SACM-XMI-014/expression-lang-string"
            {
                failures.push(format!("{what}: {error}"));
            }
        }
        if let Ok(output) = read(Mode::Tolerant, input) {
            for one in output.diagnostics() {
                if one.message().contains("SACM23-BASE-009 reads it") {
                    failures.push(format!("{what}: tolerant: {one}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{BASE_009}: {} inputs are refused, or kept, as not read until SACM23-BASE-009:\n{}",
        failures.len(),
        failures.join("\n")
    );
    let text = catalog(BASE_009);
    assert!(
        !text.contains("SACM-XMI-014/expression-lang-string"),
        "{BASE_009}: the catalog still lists the template SACM-XMI-014/expression-lang-string"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 8: the derived end
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_reference_end_expression_is_derived_on_every_kind() {
    let end = ReferenceEnd::Expression;
    assert_eq!(end.name(), "expression", "{BASE_009}: the end's name");
    assert_eq!(
        (ReferenceEnd::ALL.len(), ReferenceEnd::ALL.last()),
        (17, Some(&ReferenceEnd::Expression)),
        "{BASE_009}: Expression is the last of the 17 ends of ALL"
    );
    for &kind in ElementKind::ALL {
        assert_eq!(
            (end.applies_to(kind), end.bounds(kind)),
            (true, Some((0, None))),
            "{BASE_009}: Expression on a {kind}"
        );
    }
    #[cfg(feature = "serde")]
    assert_eq!(
        serde_json::to_string(&end).ok(),
        Some("\"expression\"".to_owned()),
        "{BASE_009}: the end's JSON name"
    );
    let doc = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let mut held = 0;
    for found in doc.elements() {
        assert_eq!(
            found.references(end),
            found.expressions(),
            "{BASE_009}: references(Expression) of {:?} is not expressions()",
            found.id()
        );
        held += found.expressions().len();
    }
    assert_eq!(
        held, 5,
        "{BASE_009}: setup: the valid fixture's five expressions"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 9: referrers and generated ids
// ---------------------------------------------------------------------------

/// The referrers of `target`, as `(holder, end, position)`.
fn referrers(doc: &Document, target: &str) -> Vec<(String, ReferenceEnd, usize)> {
    doc.referrers(target)
        .into_iter()
        .map(|found| {
            (
                found.holder().id().as_str().to_owned(),
                found.end(),
                found.position(),
            )
        })
        .collect()
}

/// Every typed reference of `doc` to `target`, by a walk of every element in
/// pre-order and every end of `ReferenceEnd::ALL`.
fn full_scan(doc: &Document, target: &str) -> Vec<(String, ReferenceEnd, usize)> {
    let mut found = Vec::new();
    for holder in doc.elements() {
        for &end in ReferenceEnd::ALL {
            for (position, held) in holder.references(end).iter().enumerate() {
                if held.as_str() == target {
                    found.push((holder.id().as_str().to_owned(), end, position));
                }
            }
        }
    }
    found
}

fn assert_referrers_are_the_full_scan(what: &str, doc: &Document) {
    let mut targets: Vec<String> = vec!["nowhere".to_owned()];
    for found in doc.elements() {
        targets.push(found.id().as_str().to_owned());
        for &end in ReferenceEnd::ALL {
            targets.extend(
                found
                    .references(end)
                    .iter()
                    .map(|held| held.as_str().to_owned()),
            );
        }
    }
    targets.sort();
    targets.dedup();
    for target in &targets {
        assert_eq!(
            referrers(doc, target),
            full_scan(doc, target),
            "{BASE_009}: {what}: the referrers of {target:?} are not the full scan"
        );
    }
}

#[test]
fn sacm23_base_009_referrers_list_the_expression_end() {
    let doc = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let entry =
        |holder: &str, end: ReferenceEnd, position: usize| (holder.to_owned(), end, position);
    assert_eq!(
        referrers(&doc, "blade"),
        [
            entry("blade_guarded", ReferenceEnd::Element, 0),
            entry("n1", ReferenceEnd::Expression, 0),
            entry("tv1", ReferenceEnd::Expression, 0),
            entry("manual_pdf", ReferenceEnd::Expression, 0),
        ],
        "{BASE_009}: the referrers of blade"
    );
    assert_eq!(
        referrers(&doc, "lid"),
        [
            entry("blade_guarded", ReferenceEnd::Element, 1),
            entry("c2", ReferenceEnd::Expression, 0),
        ],
        "{BASE_009}: the referrers of lid"
    );
    assert_eq!(
        referrers(&doc, "blade_guarded"),
        [entry("c1", ReferenceEnd::Expression, 0)],
        "{BASE_009}: the referrers of blade_guarded"
    );
    assert_referrers_are_the_full_scan(VALID, &doc);
}

#[test]
fn sacm23_base_009_referrers_equal_a_full_scan_after_each_operation() {
    let start = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let lid = LangString::default().with_lang("en").with_expression("lid");
    let scrub = |target: &str| {
        DeleteElement::new(target)
            .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
    };
    let copy = CopyElements::new(&start, ["terms", "arguments"], "case")
        .unwrap_or_else(|error| panic!("{BASE_009}: setup: the copy: {error}"));
    let operations: Vec<(&str, Operation)> = vec![
        (
            "SetName to plain",
            SetName::new("c1", LangString::default().with_lang("en")).into(),
        ),
        ("SetName to lid", SetName::new("c1", lid.clone()).into()),
        (
            "SetContent",
            SetContent::new(
                "n1",
                [
                    LangString::new("a").with_lang("sv").with_expression("lid"),
                    LangString::default()
                        .with_lang("en")
                        .with_expression("blade"),
                ]
                .into_iter()
                .collect::<MultiLangString>(),
            )
            .into(),
        ),
        ("SetKey", SetKey::new("tv1", lid.clone()).into()),
        (
            "SetLocation",
            SetLocation::new("manual_pdf", lid.clone()).into(),
        ),
        ("SetContent::remove", SetContent::remove("n1").into()),
        (
            "SetLocation::remove",
            SetLocation::remove("manual_pdf").into(),
        ),
        (
            "CreateResource::with_location",
            CreateResource::new("evidence", LangString::new("R"))
                .with_location(LangString::default().with_expression("blade"))
                .into(),
        ),
        (
            "CreateArtifact",
            edit::CreateArtifact::new("evidence", lid.clone()).into(),
        ),
        (
            "CreateClaim",
            CreateClaim::new("arguments", lid.clone())
                .with_id("c3")
                .into(),
        ),
        ("MoveElement", MoveElement::new("n1", "c2").into()),
        (
            "ReorderElements",
            ReorderElements::new("arguments", ["c3", "c2", "c1"]).into(),
        ),
        ("CopyElements", copy.into()),
        ("DeleteElement lid", scrub("lid").into()),
        ("DeleteElement blade_guarded", scrub("blade_guarded").into()),
    ];
    let mut doc = start;
    assert_referrers_are_the_full_scan("as read", &doc);
    assert_expressions_are_the_texts("as read", &doc);
    for (what, operation) in operations {
        apply(BASE_009, what, &mut doc, operation);
        assert_referrers_are_the_full_scan(what, &doc);
        assert_expressions_are_the_texts(what, &doc);
    }
}

/// An oracle independent of the derived end: for each element, the
/// `expression()` of its name, then of each value of its `content`, `key`
/// and `location`, in order, is `expressions()`.
fn assert_expressions_are_the_texts(what: &str, doc: &Document) {
    for found in doc.elements() {
        let texts = [found.content(), found.key(), found.location()];
        let expected: Vec<ElementId> = found
            .name()
            .into_iter()
            .chain(
                texts
                    .into_iter()
                    .flatten()
                    .flat_map(MultiLangString::values),
            )
            .filter_map(LangString::expression)
            .cloned()
            .collect();
        assert_eq!(
            found.expressions(),
            expected.as_slice(),
            "{BASE_009}: {what}: the expressions of {:?} are not those of its name and texts",
            found.id()
        );
    }
}

#[test]
fn sacm23_base_009_a_dangling_name_expression_is_not_generated_again() {
    let with_name = |name: &str| case(&claim("c0", name, ""), "");
    // Without the expression, the first generated Claim id is claim_1.
    let mut control = read_clean(BASE_009, "a plain name", &with_name("<name lang=\"en\"/>"));
    let created = apply(
        BASE_009,
        "a create in the control",
        &mut control,
        edit::CreateClaim::new("p", LangString::new("C")),
    );
    assert_eq!(
        created.created_ids(),
        ids(&["claim_1"]),
        "{BASE_009}: setup: the generated id of the control"
    );
    let mut doc = read_clean(
        BASE_009,
        "a dangling name expression",
        &with_name(&format!("<name {ELS} lang=\"en\" expression=\"claim_1\"/>")),
    );
    let created = apply(
        BASE_009,
        "a create beside a dangling expression",
        &mut doc,
        edit::CreateClaim::new("p", LangString::new("C")),
    );
    assert!(
        created.created_ids().len() == 1 && created.created_ids()[0].as_str() != "claim_1",
        "{BASE_009}: CreateClaim generates claim_1, which an expression names: {:?}",
        created.created_ids()
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 10: the reference edits refuse the end
// ---------------------------------------------------------------------------

/// The Term `blade`, the Claims `c1` and `c2` with the Note `n1` and the
/// `TaggedValue` `tv1` on `c2`, and the Resource `r1`, built with edits.
fn built() -> Document {
    let en = |text: &str| LangString::new(text).with_lang("en");
    let mut doc = Document::new();
    let operations: Vec<Operation> = vec![
        CreateAssuranceCasePackage::root(en("Case"))
            .with_id("case")
            .into(),
        CreateTerminologyPackage::new("case", en("Terms"))
            .with_id("terms")
            .into(),
        CreateTerm::new("terms", en("Blade"), "blade")
            .with_id("blade")
            .into(),
        CreateTerm::new("terms", en("Lid"), "lid")
            .with_id("lid")
            .into(),
        CreateArgumentPackage::new("case", en("Arguments"))
            .with_id("p")
            .into(),
        CreateClaim::new("p", en("C1")).with_id("c1").into(),
        CreateClaim::new("p", en("C2")).with_id("c2").into(),
        CreateNote::new("c2", en("N1")).with_id("n1").into(),
        CreateTaggedValue::new("c2", en("K"), en("V"))
            .with_id("tv1")
            .into(),
        CreateArtifactPackage::new("case", en("Evidence"))
            .with_id("a")
            .into(),
        CreateResource::new("a", en("R1")).with_id("r1").into(),
    ];
    for operation in operations {
        apply(BASE_009, "setup: built()", &mut doc, operation);
    }
    doc
}

#[test]
fn sacm23_base_009_reference_edits_refuse_the_derived_end() {
    let doc = built();
    let end = ReferenceEnd::Expression;
    let operations: [(&str, Operation); 4] = [
        (
            "SetReferences::new",
            SetReferences::new("c1", end, ["blade"]).into(),
        ),
        (
            "SetReferences::clear",
            SetReferences::clear("c1", end).into(),
        ),
        ("AddReference", AddReference::new("c1", end, "blade").into()),
        (
            "RemoveReference",
            RemoveReference::new("c1", end, "blade").into(),
        ),
    ];
    for (what, operation) in operations {
        let error = refused(BASE_009, what, &doc, operation);
        assert!(
            matches!(
                &error,
                EditError::DerivedEnd { element, kind: ElementKind::Claim, end: "expression", .. }
                    if element.as_str() == "c1"
            ),
            "{BASE_009}: {what}: not DerivedEnd: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "{BASE_009}: {what}: the code"
        );
        assert_eq!(
            error.to_string(),
            "the expression of the Claim \"c1\" is derived from its name and texts: set it with \
             SetName, SetContent, SetKey or SetLocation",
            "{BASE_009}: {what}: the Display"
        );
    }
    let unknown: [(&str, Operation); 4] = [
        (
            "SetReferences::new",
            SetReferences::new("nowhere", end, ["blade"]).into(),
        ),
        (
            "SetReferences::clear",
            SetReferences::clear("nowhere", end).into(),
        ),
        (
            "AddReference",
            AddReference::new("nowhere", end, "blade").into(),
        ),
        (
            "RemoveReference",
            RemoveReference::new("nowhere", end, "blade").into(),
        ),
    ];
    for (what, operation) in unknown {
        let error = refused(BASE_009, what, &doc, operation);
        assert!(
            matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
            "{BASE_009}: {what} on an unknown element: not ElementNotFound first: {error:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 11: creates and sets check the expression ids
// ---------------------------------------------------------------------------

/// Asserts that `error` is the refusal `expected` names for an expression:
/// `nowhere`, the Claim `c2`, the id `a b`, or no expression.
fn assert_expression_refusal(what: &str, expected: &str, error: &EditError) {
    let ok = match expected {
        "nowhere" => matches!(
            error,
            EditError::ReferenceNotFound { end: "expression", id, .. } if id.as_str() == "nowhere"
        ),
        "c2" => matches!(
            error,
            EditError::WrongReferenceType {
                end: "expression",
                id,
                kind: ElementKind::Claim,
                expected: "ExpressionElement",
                ..
            } if id.as_str() == "c2"
        ),
        "a b" => matches!(
            error,
            EditError::InvalidReference { end: "expression", id, .. } if id.as_str() == "a b"
        ),
        _ => matches!(
            error,
            EditError::TooFewReferences {
                end: "expression",
                given: 0,
                required: 1,
                ..
            }
        ),
    };
    assert!(ok, "{BASE_009}: {what}, expression {expected:?}: {error:?}");
}

/// The four refused expressions: dangling, a Claim, not an `NCName`, and none.
fn refused_expressions(row: &str) -> [(&'static str, LangString); 4] {
    let x = LangString::new("x").with_lang("en");
    [
        ("nowhere", x.clone().with_expression("nowhere")),
        ("c2", x.clone().with_expression("c2")),
        ("a b", x.with_expression("a b")),
        ("none", without_expression(row)),
    ]
}

#[test]
fn sacm23_base_009_create_claim_checks_its_name_expression() {
    let doc = built();
    let blade = LangString::default()
        .with_lang("en")
        .with_expression("blade");
    let mut accepted = doc.clone();
    apply(
        BASE_009,
        "CreateClaim with an expression",
        &mut accepted,
        CreateClaim::new("p", blade.clone()).with_id("c3"),
    );
    assert_eq!(
        element(BASE_009, &accepted, "c3").name(),
        Some(&blade),
        "{BASE_009}: the created name"
    );
    for (expected, name) in refused_expressions(BASE_009) {
        let error = refused(BASE_009, "CreateClaim", &doc, CreateClaim::new("p", name));
        assert_expression_refusal("CreateClaim", expected, &error);
        let display = error.to_string();
        let pinned = match expected {
            "nowhere" => {
                Some("the document has no element \"nowhere\" for the expression to reference")
            }
            "c2" => Some(
                "the expression \"c2\" names a Claim, and the expression must be an ExpressionElement",
            ),
            "none" => Some("the expression is given 0 ids, and SACM requires at least 1"),
            _ => None,
        };
        if let Some(pinned) = pinned {
            assert_eq!(display, pinned, "{BASE_009}: the Display for {expected:?}");
        }
    }
}

#[test]
fn sacm23_base_009_an_expression_lang_string_without_expression_is_too_few_references() {
    let doc = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let name = without_expression(BASE_009);
    for (what, operation) in [
        (
            "CreateClaim",
            Operation::from(edit::CreateClaim::new("arguments", name.clone())),
        ),
        ("SetName", SetName::new("c1", name.clone()).into()),
    ] {
        let error = refused(BASE_009, what, &doc, operation);
        assert!(
            matches!(
                error,
                EditError::TooFewReferences {
                    end: "expression",
                    given: 0,
                    required: 1,
                    ..
                }
            ),
            "{BASE_009}: {what} with no expression is not TooFewReferences: {error:?}"
        );
        assert_eq!(
            error.to_string(),
            "the expression is given 0 ids, and SACM requires at least 1",
            "{BASE_009}: {what}: the Display"
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn sacm23_base_009_an_expression_lang_string_without_expression_from_json_is_too_few_references() {
    let doc = built();
    let name: LangString = serde_json::from_str(
        r#"{"type":"ExpressionLangString","lang":null,"content":null,"expression":null}"#,
    )
    .unwrap_or_else(|error| panic!("{BASE_009}: the JSON does not read: {error}"));
    let error = refused(
        BASE_009,
        "CreateClaim from JSON",
        &doc,
        CreateClaim::new("p", name),
    );
    assert_expression_refusal("CreateClaim from JSON", "none", &error);
}

#[test]
fn sacm23_base_009_expression_ids_are_checked_after_the_operation_s_own_ends() {
    let doc = built();
    let named = |expression: &str| LangString::new("I").with_expression(expression);
    // Each check phase: the source's refusal comes first.
    for (what, source, expression) in [
        ("invalid ids", "a b", "c d"),
        ("dangling ids", "ghost_source", "ghost_name"),
        ("wrongly typed ids", "blade", "c2"),
    ] {
        let error = refused(
            BASE_009,
            what,
            &doc,
            CreateAssertedInference::new("p", named(expression), [source], "c1"),
        );
        let first = match &error {
            EditError::InvalidReference { end, .. }
            | EditError::ReferenceNotFound { end, .. }
            | EditError::WrongReferenceType { end, .. } => *end,
            other => panic!("{BASE_009}: {what}: not a reference refusal: {other:?}"),
        };
        assert_eq!(
            first, "source",
            "{BASE_009}: {what}: the expression is checked before the source: {error:?}"
        );
    }
    // The same on an Expression create: its `element` end comes first.
    for (what, element, expression) in [
        ("invalid ids", "a b", "c d"),
        ("dangling ids", "ghost_e", "ghost_n"),
        ("wrongly typed ids", "c1", "c2"),
    ] {
        let error = refused(
            BASE_009,
            what,
            &doc,
            CreateExpression::new("terms", named(expression), "v")
                .with_expression_elements([element]),
        );
        let first = match &error {
            EditError::InvalidReference { end, .. }
            | EditError::ReferenceNotFound { end, .. }
            | EditError::WrongReferenceType { end, .. } => *end,
            other => panic!("{BASE_009}: {what}: not a reference refusal: {other:?}"),
        };
        let phase = match what {
            "invalid ids" => matches!(error, EditError::InvalidReference { .. }),
            "dangling ids" => matches!(error, EditError::ReferenceNotFound { .. }),
            _ => matches!(error, EditError::WrongReferenceType { .. }),
        };
        assert!(
            first == "element" && phase,
            "{BASE_009}: {what}: an Expression create does not refuse its element first: \
             {error:?}"
        );
    }
    // The multiplicity: no source before no expression.
    let error = refused(
        BASE_009,
        "no source and no expression",
        &doc,
        CreateAssertedInference::new(
            "p",
            without_expression(BASE_009),
            Vec::<ElementId>::new(),
            "c1",
        ),
    );
    assert!(
        matches!(error, EditError::TooFewReferences { end: "source", .. }),
        "{BASE_009}: no source and no expression: {error:?}"
    );
}

#[test]
fn sacm23_base_009_creates_with_ends_check_the_name_expression_in_each_phase() {
    let doc = built();
    // With ends that pass, each create refuses each refused expression.
    for (expected, name) in refused_expressions(BASE_009) {
        let operations: [(&str, Operation); 4] = [
            (
                "CreateAssertedInference",
                CreateAssertedInference::new("p", name.clone(), ["c2"], "c1").into(),
            ),
            (
                "CreateArgumentReasoning",
                edit::CreateArgumentReasoning::new("p", name.clone()).into(),
            ),
            (
                "CreateArtifactGroup",
                edit::CreateArtifactGroup::new("a", name.clone()).into(),
            ),
            (
                "CreateTerm",
                CreateTerm::new("terms", name.clone(), "v").into(),
            ),
        ];
        for (what, operation) in operations {
            let error = refused(BASE_009, what, &doc, operation);
            assert_expression_refusal(what, expected, &error);
        }
    }
    // In each phase the expression comes after the ends, but an earlier
    // phase comes first: an id with white space before a dangling source,
    // and a dangling expression before a wrongly typed source.
    let named = |expression: &str| LangString::new("I").with_expression(expression);
    let error = refused(
        BASE_009,
        "an invalid expression and a dangling source",
        &doc,
        CreateAssertedInference::new("p", named("c d"), ["ghost"], "c1"),
    );
    assert!(
        matches!(
            &error,
            EditError::InvalidReference {
                end: "expression",
                ..
            }
        ),
        "{BASE_009}: an invalid expression is not refused before a dangling source: {error:?}"
    );
    let error = refused(
        BASE_009,
        "a dangling expression and a wrongly typed source",
        &doc,
        CreateAssertedInference::new("p", named("ghost"), ["blade"], "c1"),
    );
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound {
                end: "expression",
                ..
            }
        ),
        "{BASE_009}: a dangling expression is not refused before a wrongly typed source: \
         {error:?}"
    );
}

#[test]
fn sacm23_base_009_every_set_and_create_of_a_text_checks_its_expression_ids() {
    type Build<'a> = Box<dyn Fn(LangString) -> Operation + 'a>;
    let doc = built();
    let en = |text: &str| LangString::new(text).with_lang("en");
    let blade = LangString::new("y")
        .with_lang("sv")
        .with_expression("blade");
    // Value 2 of each text is the ExpressionLangString.
    let text =
        |second: LangString| -> MultiLangString { [en("first"), second].into_iter().collect() };
    let operations: Vec<(&str, Build<'_>)> = vec![
        ("SetName", Box::new(|name| SetName::new("c1", name).into())),
        (
            "SetContent",
            Box::new(move |value| SetContent::new("n1", text(value)).into()),
        ),
        (
            "SetKey",
            Box::new(move |value| SetKey::new("tv1", text(value)).into()),
        ),
        (
            "SetLocation",
            Box::new(move |value| SetLocation::new("r1", text(value)).into()),
        ),
        (
            "CreateNote",
            Box::new(move |value| CreateNote::new("c1", text(value)).into()),
        ),
        (
            "CreateTaggedValue key",
            Box::new(move |value| CreateTaggedValue::new("c1", text(value), en("v")).into()),
        ),
        (
            "CreateTaggedValue content",
            Box::new(move |value| CreateTaggedValue::new("c1", en("k"), text(value)).into()),
        ),
        (
            "CreateResource::with_location",
            Box::new(move |value| {
                CreateResource::new("a", en("R2"))
                    .with_location(text(value))
                    .into()
            }),
        ),
        (
            "CreateExpression name",
            Box::new(|name| CreateExpression::new("terms", name, "e").into()),
        ),
        // The asset creates check their name (SACM23-BASE-009).
        (
            "CreateArtifact name",
            Box::new(|name| edit::CreateArtifact::new("a", name).into()),
        ),
        (
            "CreateResource name",
            Box::new(|name| CreateResource::new("a", name).into()),
        ),
        (
            "CreateActivity name",
            Box::new(|name| edit::CreateActivity::new("a", name).into()),
        ),
        (
            "CreateEvent name",
            Box::new(|name| edit::CreateEvent::new("a", name).into()),
        ),
    ];
    for (what, build) in &operations {
        let mut accepted = doc.clone();
        apply(BASE_009, what, &mut accepted, build(blade.clone()));
        for (expected, value) in refused_expressions(BASE_009) {
            let error = refused(BASE_009, what, &doc, build(value));
            assert_expression_refusal(what, expected, &error);
        }
    }
}

#[test]
fn sacm23_base_009_a_set_keeps_a_held_expression_and_changes_only_on_change() {
    // r1's name names ghost, which no element has: a set keeps an id the
    // element's Expression end already holds, from any of its places.
    let input = case(
        &claim(
            "c1",
            &format!("<name {ELS} lang=\"en\" expression=\"ghost\"/>"),
            "",
        ),
        &resource(
            "r1",
            &format!("<name {ELS} expression=\"ghost\"/>"),
            "<value content=\"u\"/>",
        ),
    );
    let mut doc = read_clean(BASE_009, "held dangling expressions", &input);
    let ghost = |content: &str| LangString::new(content).with_expression("ghost");
    let renamed = apply(
        BASE_009,
        "SetName keeping ghost",
        &mut doc,
        SetName::new("c1", ghost("y")),
    );
    assert_eq!(
        renamed.changed_ids(),
        ids(&["c1"]),
        "{BASE_009}: SetName changes c1"
    );
    apply(
        BASE_009,
        "SetLocation keeping the name's ghost",
        &mut doc,
        SetLocation::new("r1", ghost("u")),
    );
    let error = refused(
        BASE_009,
        "SetName to another dangling id",
        &doc,
        SetName::new("c1", ghost("y").with_expression("phantom")),
    );
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound { end: "expression", id, .. } if id.as_str() == "phantom"
        ),
        "{BASE_009}: SetName to an id the end does not hold: {error:?}"
    );
    // changed_ids: the same name changes nothing; another expression, or
    // the form alone, changes the element.
    let again = apply(
        BASE_009,
        "the same name",
        &mut doc,
        SetName::new("c1", ghost("y")),
    );
    assert_eq!(
        again.changed_ids(),
        [],
        "{BASE_009}: the same name changes nothing"
    );
    let to_t1 = apply(
        BASE_009,
        "another expression",
        &mut doc,
        SetName::new("c1", LangString::new("y").with_expression("t1")),
    );
    assert_eq!(
        to_t1.changed_ids(),
        ids(&["c1"]),
        "{BASE_009}: another expression"
    );
    let plain = apply(
        BASE_009,
        "the plain form",
        &mut doc,
        SetName::new("c1", LangString::new("y")),
    );
    assert_eq!(
        plain.changed_ids(),
        ids(&["c1"]),
        "{BASE_009}: the form alone"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 12: copy
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_copy_remaps_and_checks_expression_ids() {
    let source = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    // c1 together with blade_guarded: their packages, into the same case.
    let mut doc = source.clone();
    let copy = CopyElements::new(&source, ["terms", "arguments"], "case")
        .unwrap_or_else(|error| panic!("{BASE_009}: setup: the copy: {error}"));
    let mutation = apply(BASE_009, "copy terms and arguments", &mut doc, copy.clone());
    let copied = |from: &str| -> ElementId {
        copy.source_ids()
            .iter()
            .zip(mutation.created_ids())
            .find(|(source_id, _)| source_id.as_str() == from)
            .map_or_else(
                || panic!("{BASE_009}: {from} is not copied"),
                |(_, created)| created.clone(),
            )
    };
    let c1_copy = copied("c1");
    let blade_guarded_copy = copied("blade_guarded");
    assert_ne!(
        blade_guarded_copy.as_str(),
        "blade_guarded",
        "{BASE_009}: setup: the copy of blade_guarded keeps its id"
    );
    assert_eq!(
        element(BASE_009, &doc, c1_copy.as_str()).expressions(),
        std::slice::from_ref(&blade_guarded_copy),
        "{BASE_009}: the copy's name does not name the copied Expression"
    );
    assert_eq!(
        element(BASE_009, &doc, c1_copy.as_str()).name(),
        Some(
            &LangString::default()
                .with_lang("en")
                .with_expression(blade_guarded_copy)
        ),
        "{BASE_009}: the copy's name"
    );
    // c1 alone, into a document without blade_guarded.
    let mut target = Document::new();
    apply(
        BASE_009,
        "setup: the target",
        &mut target,
        CreateArgumentPackage::root(LangString::new("P")).with_id("q"),
    );
    let alone = CopyElements::new(&source, ["c1"], "q")
        .unwrap_or_else(|error| panic!("{BASE_009}: setup: the copy of c1: {error}"));
    let error = refused(BASE_009, "copy c1 alone", &target, alone);
    assert!(
        matches!(
            error,
            EditError::ReferenceNotFound {
                end: "expression",
                ..
            }
        ),
        "{BASE_009}: copying c1 alone is not ReferenceNotFound of its expression: {error:?}"
    );
}

#[test]
fn sacm23_base_009_copy_refuses_an_expression_lang_string_with_no_expression() {
    // A name read from a file with no expression, and a value of a content
    // with none: neither is copied (§8.4 `expression[1]`; ADR 0038 D3).
    for (what, input, top) in [
        (
            "a name",
            case(&claim("c1", &format!("<name {ELS} lang=\"en\"/>"), ""), ""),
            "c1",
        ),
        (
            "a value of a Note's content",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &note("n1", &format!("<value {ELS} lang=\"en\"/>")),
                ),
                "",
            ),
            "c1",
        ),
    ] {
        let source = read_clean(BASE_009, what, &input);
        let copy = edit::CopyElements::new(&source, [top], "p")
            .unwrap_or_else(|error| panic!("{BASE_009}: setup: {what}: the copy: {error}"));
        let error = refused(BASE_009, what, &source, copy);
        assert!(
            matches!(
                error,
                EditError::TooFewReferences {
                    end: "expression",
                    given: 0,
                    required: 1,
                    ..
                }
            ),
            "{BASE_009}: copying {what} with no expression is not TooFewReferences: {error:?}"
        );
    }
}

#[test]
fn sacm23_base_009_expressions_follow_write_order_and_a_scrub_keeps_the_others() {
    // r1's name links t1 and its location value t2; tv1's content value
    // links t2 and its key value t1: the end follows the name, then the
    // content, key and location, whatever the order in the file.
    let input = case(
        &claim(
            "c1",
            &plain_name("C1"),
            &format!(
                "<taggedValue xmi:id=\"tv1\"><key><value {ELS} expression=\"t1\"/></key>\
                 <content><value {ELS} expression=\"t2\"/></content></taggedValue>"
            ),
        ),
        &resource(
            "r1",
            &format!("<name {ELS} lang=\"en\" expression=\"t1\"/>"),
            &format!("<value {ELS} expression=\"t2\"/>"),
        ),
    );
    let doc = read_clean(BASE_009, "two expressions per holder", &input);
    let expressions = |doc: &Document, holder: &str| -> Vec<String> {
        element(BASE_009, doc, holder)
            .expressions()
            .iter()
            .map(|found| found.as_str().to_owned())
            .collect()
    };
    assert_eq!(
        (expressions(&doc, "r1"), expressions(&doc, "tv1")),
        (
            vec!["t1".to_owned(), "t2".to_owned()],
            vec!["t2".to_owned(), "t1".to_owned()]
        ),
        "{BASE_009}: the expressions are not the name's, then the content's, key's and \
         location's"
    );
    // Deleting t2 makes plain only the texts that named it.
    let mut after = doc.clone();
    let mutation = apply(
        BASE_009,
        "delete t2",
        &mut after,
        delete_with(
            "t2",
            ReferenceDeletePolicy::ScrubReferences,
            CrossPackageReferencePolicy::ApplyReferencePolicy,
        ),
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["tv1", "r1"]),
        "{BASE_009}: the holders of t2 change"
    );
    let r1 = element(BASE_009, &after, "r1");
    assert_eq!(
        (
            r1.name(),
            r1.location().map(|location| location.values().to_vec())
        ),
        (
            Some(&LangString::default().with_lang("en").with_expression("t1")),
            Some(vec![LangString::default()])
        ),
        "{BASE_009}: r1 keeps its name's t1 and its location value is made plain"
    );
    assert_eq!(
        (expressions(&after, "r1"), expressions(&after, "tv1")),
        (vec!["t1".to_owned()], vec!["t1".to_owned()]),
        "{BASE_009}: the expressions after the delete"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 13: delete
// ---------------------------------------------------------------------------

fn delete_with(
    target: &str,
    reference: ReferenceDeletePolicy,
    cross: CrossPackageReferencePolicy,
) -> DeleteElement {
    DeleteElement::new(target)
        .with_reference_policy(reference)
        .with_cross_package_policy(cross)
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "acceptance 13 is one sequence of refusals and scrubs on one fixture"
)]
fn sacm23_base_009_delete_refuses_or_scrubs_an_expression() {
    let doc = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let error = refused(
        BASE_009,
        "delete blade_guarded",
        &doc,
        DeleteElement::new("blade_guarded"),
    );
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end: "expression", id, .. }
                if holder.as_str() == "c1" && id.as_str() == "blade_guarded"
        ),
        "{BASE_009}: deleting blade_guarded is not CrossPackageReference of c1's expression: \
         {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the expression of \"c1\" names \"blade_guarded\" in another package, and the delete \
         would leave it dangling",
        "{BASE_009}: the Display"
    );
    let error = refused(
        BASE_009,
        "delete blade_guarded across packages",
        &doc,
        DeleteElement::new("blade_guarded")
            .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
    );
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end: "expression", .. } if holder.as_str() == "c1"
        ),
        "{BASE_009}: deleting blade_guarded with ApplyReferencePolicy is not Referenced: {error:?}"
    );
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let operation = delete_with(
            "blade_guarded",
            policy,
            CrossPackageReferencePolicy::ApplyReferencePolicy,
        );
        let preview = edit::preview(&doc, &operation.clone().into())
            .unwrap_or_else(|error| panic!("{BASE_009}: {policy:?}: the preview: {error}"));
        let dangling: Vec<(&str, &str, &str)> = preview
            .dangling_references()
            .iter()
            .map(|found| {
                (
                    found.holder().as_str(),
                    found.end().name(),
                    found.id().as_str(),
                )
            })
            .collect();
        assert_eq!(
            dangling,
            [("c1", "expression", "blade_guarded")],
            "{BASE_009}: {policy:?}: the preview's dangling references"
        );
        assert_eq!(
            preview
                .dangling_references()
                .first()
                .map(sacm::edit::DanglingReference::end),
            Some(ReferenceEnd::Expression),
            "{BASE_009}: {policy:?}: the dangling reference's end"
        );
        let mut after = doc.clone();
        let mutation = apply(BASE_009, "delete blade_guarded", &mut after, operation);
        assert_eq!(
            (mutation.deleted_ids(), mutation.changed_ids()),
            (ids(&["blade_guarded"]).as_slice(), ids(&["c1"]).as_slice()),
            "{BASE_009}: {policy:?}: the deleted and changed ids"
        );
        assert_eq!(
            element(BASE_009, &after, "c1").name(),
            Some(&LangString::default().with_lang("en")),
            "{BASE_009}: {policy:?}: c1's name is not made plain"
        );
        assert!(
            element(BASE_009, &after, "c1")
                .name()
                .is_some_and(|name| !name.is_expression_lang_string()),
            "{BASE_009}: {policy:?}: c1's name is still an ExpressionLangString"
        );
        assert_eq!(
            validation::validate(&after),
            [],
            "{BASE_009}: {policy:?}: findings after the delete"
        );
    }
    // Deleting blade scrubs n1, tv1 and manual_pdf, and blade_guarded's
    // element.
    let mut after = doc.clone();
    let mutation = apply(
        BASE_009,
        "delete blade",
        &mut after,
        delete_with(
            "blade",
            ReferenceDeletePolicy::ScrubReferences,
            CrossPackageReferencePolicy::ApplyReferencePolicy,
        ),
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["blade_guarded", "n1", "tv1", "manual_pdf"]),
        "{BASE_009}: the changed ids of deleting blade"
    );
    let n1 = element(BASE_009, &after, "n1");
    assert_eq!(
        n1.content().map(|content| content.values().to_vec()),
        Some(vec![
            LangString::new("Kontrollerad mot manualen").with_lang("sv"),
            LangString::new("").with_lang("en"),
        ]),
        "{BASE_009}: n1's value 2 is not made plain"
    );
    assert_eq!(
        n1.content_value_ids(),
        element(BASE_009, &doc, "n1").content_value_ids(),
        "{BASE_009}: n1's value ids change"
    );
    assert_eq!(
        element(BASE_009, &after, "tv1")
            .key()
            .map(|key| key.values().to_vec()),
        Some(vec![LangString::default().with_lang("en")]),
        "{BASE_009}: tv1's key value is not made plain"
    );
    assert_eq!(
        element(BASE_009, &after, "manual_pdf")
            .location()
            .map(|location| location.values().to_vec()),
        Some(vec![LangString::default()]),
        "{BASE_009}: manual_pdf's location value is not made plain"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 14: compare
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_compare_reports_a_changed_expression_as_a_name_difference() {
    let doc = read_clean(BASE_009, VALID, &fixture(BASE_009, VALID));
    let mut changed = doc.clone();
    apply(
        BASE_009,
        "make c1's name plain",
        &mut changed,
        SetName::new("c1", LangString::default().with_lang("en")),
    );
    let differences = semantic_compare(&doc, &changed);
    let [only] = differences.as_slice() else {
        panic!("{BASE_009}: not exactly one difference: {differences:?}");
    };
    assert!(
        matches!(only, Difference::Name { id, .. } if id.as_str() == "c1"),
        "{BASE_009}: not a Name difference of c1: {only:?}"
    );
    assert_eq!(
        only.to_string(),
        "element \"c1\" differs in name: [lang \"en\", no content, expression \"blade_guarded\"] \
         on the left, [lang \"en\", no content] on the right",
        "{BASE_009}: the Display"
    );
    // No expression.
    let typed = read_clean(
        BASE_009,
        "no expression",
        &named_package(&format!("<name {ELS} lang=\"en\"/>")),
    );
    let plain = read_clean(BASE_009, "plain", &named_package("<name lang=\"en\"/>"));
    let differences = semantic_compare(&typed, &plain);
    let [only] = differences.as_slice() else {
        panic!("{BASE_009}: no expression: not exactly one difference: {differences:?}");
    };
    assert_eq!(
        only.to_string(),
        "element \"p\" differs in name: [lang \"en\", no content, no expression] on the left, \
         [lang \"en\", no content] on the right",
        "{BASE_009}: the Display of no expression"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 15: validation and the strict save
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_the_findings_fixture_gives_its_four_lines() {
    let text = fixture(BASE_009, FINDINGS);
    let doc = read_clean(BASE_009, FINDINGS, &text);
    let found = validation::validate(&doc);
    assert_eq!(
        shapes(&found),
        [
            shape(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                BASE_009,
                "8.4",
                &["c1"]
            ),
            shape(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                BASE_009,
                "8.4",
                &["n1"]
            ),
            shape(
                Code::REF_DANGLING,
                Severity::Error,
                "SACM23-VAL-003",
                "8.4",
                &["c2", "nowhere"]
            ),
            shape(
                Code::REF_WRONG_TYPE,
                Severity::Error,
                "SACM23-VAL-004",
                "8.4",
                &["c3", "c1"]
            ),
        ],
        "{BASE_009}: the findings by code, severity, row, clause and affected ids"
    );
    assert_eq!(
        lines(&found),
        [
            "error SACM-MULT-001 [SACM23-BASE-009, clause 8.4]: the name of Claim \"c1\" is an \
             ExpressionLangString with no expression (expression:ExpressionElement[1])",
            "error SACM-MULT-001 [SACM23-BASE-009, clause 8.4]: value 2 of the content of Note \
             \"n1\" is an ExpressionLangString with no expression (expression:ExpressionElement[1])",
            "error SACM-REF-001 [SACM23-VAL-003, clause 8.4]: Claim \"c2\" has the expression \
             \"nowhere\", which names no element of the document (the end's type is \
             ExpressionElement)",
            "error SACM-REF-002 [SACM23-VAL-004, clause 8.4]: Claim \"c3\" has the expression \
             \"c1\", whose kind Claim does not conform to the end's type ExpressionElement",
        ],
        "{BASE_009}: the fixture's four lines"
    );
    match write(&doc, Mode::Strict) {
        Err(error @ WriteError::Invalid { .. }) => assert_eq!(
            error.to_string(),
            "the document cannot be written in strict mode: 1 schema error, SACM-REF-001 on \"c2\"",
            "{BASE_009}: the strict write's Display"
        ),
        other => panic!("{BASE_009}: the strict write is not Invalid: {other:?}"),
    }
    let tolerant = written(BASE_009, FINDINGS, &doc, Mode::Tolerant);
    assert!(
        tolerant.contains("expression=\"nowhere\"") && tolerant.contains("expression=\"c1\""),
        "{BASE_009}: a tolerant write drops a reference:\n{tolerant}"
    );
}

#[test]
fn sacm23_base_009_a_resource_naming_blade_twice_and_a_repeated_missing_id_give_one_finding() {
    // A Resource whose name and location value both name t1: no
    // SACM23-VAL-022 Warning, and no finding at all.
    let twice = case(
        "",
        &resource(
            "r1",
            &format!("<name {ELS} expression=\"t1\"/>"),
            &format!("<value {ELS} expression=\"t1\"/>"),
        ),
    );
    let doc = read_clean(BASE_009, "t1 named twice", &twice);
    assert_eq!(
        lines(&validation::validate(&doc)),
        Vec::<String>::new(),
        "{BASE_009}: a Resource naming t1 in its name and its location"
    );
    // Two places naming the same missing id: one SACM-REF-001.
    let ghost = case(
        "",
        &resource(
            "r1",
            &format!("<name {ELS} expression=\"ghost\"/>"),
            &format!("<value {ELS} expression=\"ghost\"/>"),
        ),
    );
    let doc = read_clean(BASE_009, "ghost named twice", &ghost);
    assert_eq!(
        shapes(&validation::validate(&doc)),
        [shape(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003",
            "8.4",
            &["r1", "ghost"]
        )],
        "{BASE_009}: two places of r1 naming ghost"
    );
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 16: JSON
// ---------------------------------------------------------------------------

#[cfg(feature = "serde")]
#[test]
fn sacm23_base_009_json_of_an_expression_lang_string() {
    let none = r#"{"type":"ExpressionLangString","lang":null,"content":null,"expression":null}"#;
    let value: LangString = serde_json::from_str(none)
        .unwrap_or_else(|error| panic!("{BASE_009}: {none} does not read: {error}"));
    assert_eq!(
        serde_json::to_string(&value).ok().as_deref(),
        Some(none),
        "{BASE_009}: {none} does not write back the same"
    );
    assert_eq!(
        (value.expression(), value.is_expression_lang_string()),
        (None, true),
        "{BASE_009}: {none} is not an ExpressionLangString with no expression"
    );
    for refused in [
        r#"{"type":"LangString","lang":null,"content":null,"expression":null}"#,
        r#"{"type":"LangString","lang":null,"content":null}"#,
        r#"{"type":"MultiLangString","lang":null,"content":null,"expression":null}"#,
        r#"{"lang":"en","content":"x","expression":"t1"}"#,
        r#"{"type":"ExpressionLangString","lang":"en","content":"x"}"#,
        // A key given twice (R4).
        r#"{"lang":"en","lang":"sv"}"#,
        r#"{"type":"ExpressionLangString","type":"ExpressionLangString","expression":null}"#,
        r#"{"type":"ExpressionLangString","expression":"a","expression":"b"}"#,
    ] {
        match serde_json::from_str::<LangString>(refused) {
            Ok(read) => panic!("{BASE_009}: {refused} is read as {read:?}"),
            Err(error) => assert!(
                error.is_data(),
                "{BASE_009}: {refused} is refused as malformed JSON: {error}"
            ),
        }
    }
    assert_eq!(
        serde_json::from_str::<LangString>("{}").ok(),
        Some(LangString::default()),
        "{BASE_009}: {{}} is not LangString::default()"
    );
    match serde_json::from_str::<LangString>("[]") {
        Ok(read) => panic!("{BASE_009}: [] is read as {read:?}"),
        Err(error) => assert!(
            error
                .to_string()
                .contains("a LangString or an ExpressionLangString"),
            "{BASE_009}: [] is not refused as no LangString: {error}"
        ),
    }
    {
        let sample = LangString::default()
            .with_lang("en")
            .with_expression("blade");
        assert_eq!(
            serde_json::to_string(&sample).ok().as_deref(),
            Some(
                r#"{"type":"ExpressionLangString","lang":"en","content":null,"expression":"blade"}"#
            ),
            "{BASE_009}: the JSON of an ExpressionLangString, keys in order"
        );
        let text: MultiLangString = [
            LangString::new("Kontrollerad mot manualen").with_lang("sv"),
            LangString::new("").with_lang("en").with_expression("blade"),
        ]
        .into_iter()
        .collect();
        let written = serde_json::to_string(&text).unwrap_or_default();
        assert_eq!(
            written,
            r#"[{"lang":"sv","content":"Kontrollerad mot manualen"},{"type":"ExpressionLangString","lang":"en","content":"","expression":"blade"}]"#,
            "{BASE_009}: a MultiLangString holds either form"
        );
        assert_eq!(
            serde_json::from_str::<MultiLangString>(&written).ok(),
            Some(text),
            "{BASE_009}: the MultiLangString reads back"
        );
        let error = refused(
            BASE_009,
            "the DerivedEnd sample",
            &built(),
            SetReferences::new("c1", ReferenceEnd::Expression, ["blade"]),
        );
        let json = r#"{"type":"DerivedEnd","code":"SACM-CMD-008","element":"c1","kind":"Claim","end":"expression"}"#;
        assert_eq!(
            serde_json::to_string(&error).ok().as_deref(),
            Some(json),
            "{BASE_009}: the JSON of DerivedEnd"
        );
        assert_eq!(
            serde_json::from_str::<EditError>(json).ok(),
            Some(error),
            "{BASE_009}: DerivedEnd reads back"
        );
    }
}

// ---------------------------------------------------------------------------
// SACM23-BASE-009 acceptance 17: the catalog
// ---------------------------------------------------------------------------

#[test]
fn sacm23_base_009_the_catalog_states_the_expression_rules() {
    let multiplicity = catalog_row(BASE_009, "SACM-MULT-001");
    assert!(
        multiplicity.contains("ExpressionLangString") && multiplicity.contains("8.4"),
        "{BASE_009}: the SACM-MULT-001 row does not name a missing expression (clause 8.4)"
    );
    for code in ["SACM-REF-001", "SACM-REF-002", "SACM-REF-003"] {
        let meaning = catalog_row(BASE_009, code);
        assert!(
            meaning.contains("8.4") && meaning.contains("`expression`"),
            "{BASE_009}: the {code} row does not give clause 8.4 for `expression`"
        );
    }
    assert!(
        catalog_row(BASE_009, "SACM-CMD-008").contains("DerivedEnd"),
        "{BASE_009}: the SACM-CMD-008 row does not name DerivedEnd"
    );
    assert!(
        !catalog(BASE_009).contains("`SACM-XMI-014/expression-lang-string`"),
        "{BASE_009}: the template table still has SACM-XMI-014/expression-lang-string"
    );
}

// ===========================================================================
// SACM23-VAL-009: an expression beside a content that is not empty
// ===========================================================================

#[test]
fn sacm23_val_009_the_with_content_fixture_gives_its_two_lines() {
    let doc = read_clean(VAL_009, WITH_CONTENT, &fixture(VAL_009, WITH_CONTENT));
    let found = validation::validate(&doc);
    assert_eq!(
        shapes(&found),
        [
            shape(EXPR_001, Severity::Warning, VAL_009, "8.4", &["c1"]),
            shape(EXPR_001, Severity::Warning, VAL_009, "8.4", &["n1"]),
        ],
        "{VAL_009}: the findings by code, severity, row, clause and affected ids"
    );
    assert_eq!(
        lines(&found),
        [
            "warning SACM-EXPR-001 [SACM23-VAL-009, clause 8.4]: the name of Claim \"c1\" has the \
             expression \"blade\" and a content that is not empty, and §8.4 says that if \
             expression is not empty, then content should be empty",
            "warning SACM-EXPR-001 [SACM23-VAL-009, clause 8.4]: value 1 of the content of Note \
             \"n1\" has the expression \"blade\" and a content that is not empty, and §8.4 says \
             that if expression is not empty, then content should be empty",
        ],
        "{VAL_009}: the fixture's two lines"
    );
}

/// The message of SACM-EXPR-001 for `subject`, an expression `t1`.
fn with_content_message(subject: &str) -> String {
    format!(
        "{subject} has the expression \"t1\" and a content that is not empty, and §8.4 says that \
         if expression is not empty, then content should be empty"
    )
}

#[test]
fn sacm23_val_009_each_place_with_expression_and_content_gives_one_warning() {
    let value = format!("<value {ELS} lang=\"sv\" content=\"x\" expression=\"t1\"/>");
    let cases = [
        (
            "a Claim's name",
            case(
                &claim(
                    "c1",
                    &format!("<name {ELS} content=\"x\" expression=\"t1\"/>"),
                    "",
                ),
                "",
            ),
            "c1",
            "the name of Claim \"c1\"",
        ),
        (
            "value 1 of a Note's content",
            case(&claim("c1", &plain_name("C1"), &note("n1", &value)), ""),
            "n1",
            "value 1 of the content of Note \"n1\"",
        ),
        (
            "value 2 of a TaggedValue's key",
            case(
                &claim(
                    "c1",
                    &plain_name("C1"),
                    &tagged_value("tv1", &format!("<value lang=\"en\" content=\"k\"/>{value}")),
                ),
                "",
            ),
            "tv1",
            "value 2 of the key of TaggedValue \"tv1\"",
        ),
        (
            "value 1 of a Resource's location",
            case("", &resource("r1", &plain_name("R1"), &value)),
            "r1",
            "value 1 of the location of Resource \"r1\"",
        ),
    ];
    for (what, input, holder, subject) in cases {
        let doc = read_clean(VAL_009, what, &input);
        let found = validation::validate(&doc);
        assert_eq!(
            shapes(&found),
            [shape(
                EXPR_001,
                Severity::Warning,
                VAL_009,
                "8.4",
                &[holder]
            )],
            "{VAL_009}: {what}: not one Warning affecting {holder:?}: {:?}",
            lines(&found)
        );
        assert_eq!(
            found.first().map(Diagnostic::message),
            Some(with_content_message(subject).as_str()),
            "{VAL_009}: {what}: the message"
        );
    }
}

#[test]
fn sacm23_val_009_absent_or_empty_content_no_expression_and_plain_give_none() {
    let with_name = |name: &str| case(&claim("c1", name, ""), "");
    for (what, name) in [
        (
            "an absent content",
            format!("<name {ELS} expression=\"t1\"/>"),
        ),
        (
            "an empty content",
            format!("<name {ELS} content=\"\" expression=\"t1\"/>"),
        ),
        ("a plain LangString", "<name content=\"x\"/>".to_owned()),
    ] {
        let doc = read_clean(VAL_009, what, &with_name(&name));
        assert_eq!(
            lines(&validation::validate(&doc)),
            Vec::<String>::new(),
            "{VAL_009}: {what} gives a finding"
        );
    }
    // No expression: only SACM-MULT-001.
    let doc = read_clean(
        VAL_009,
        "no expression",
        &with_name(&format!("<name {ELS} content=\"x\"/>")),
    );
    assert_eq!(
        shapes(&validation::validate(&doc)),
        [shape(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Error,
            BASE_009,
            "8.4",
            &["c1"]
        )],
        "{VAL_009}: no expression gives only SACM-MULT-001"
    );
    // White space is a content that is not empty.
    let doc = read_clean(
        VAL_009,
        "a content of white space",
        &with_name(&format!("<name {ELS} content=\" \" expression=\"t1\"/>")),
    );
    assert_eq!(
        shapes(&validation::validate(&doc)),
        [shape(EXPR_001, Severity::Warning, VAL_009, "8.4", &["c1"])],
        "{VAL_009}: a content of white space gives one Warning"
    );
}

#[test]
fn sacm23_val_009_the_warning_never_refuses_a_write_or_an_edit() {
    let doc = read_clean(VAL_009, WITH_CONTENT, &fixture(VAL_009, WITH_CONTENT));
    let found = validation::validate(&doc);
    assert!(
        !found.is_empty() && found.iter().all(|one| one.code() == EXPR_001),
        "{VAL_009}: setup: the fixture's only findings are SACM-EXPR-001: {:?}",
        lines(&found)
    );
    assert!(
        !diagnostic::has_errors(&found),
        "{VAL_009}: has_errors is true for the Warning alone"
    );
    let strict = written(VAL_009, WITH_CONTENT, &doc, Mode::Strict);
    assert_eq!(
        strict,
        written(VAL_009, WITH_CONTENT, &doc, Mode::Tolerant),
        "{VAL_009}: the strict and the tolerant write differ"
    );
    {
        let mut edited = doc.clone();
        apply(
            VAL_009,
            "SetName with an expression and a content",
            &mut edited,
            SetName::new(
                "c3",
                LangString::new("The lid")
                    .with_lang("en")
                    .with_expression("blade"),
            ),
        );
        assert_eq!(
            validation::validate(&edited).len(),
            3,
            "{VAL_009}: the set name gives one more Warning"
        );
    }
}

#[test]
fn sacm23_val_009_the_code_and_its_catalog_row() {
    assert_eq!(
        Code::EXPRESSION_WITH_CONTENT.as_str(),
        "SACM-EXPR-001",
        "{VAL_009}: Code::EXPRESSION_WITH_CONTENT"
    );
    let meaning = catalog_row(VAL_009, "SACM-EXPR-001");
    assert!(
        meaning.contains("`Code::EXPRESSION_WITH_CONTENT`") && meaning.contains(VAL_009),
        "{VAL_009}: the SACM-EXPR-001 row does not name Code::EXPRESSION_WITH_CONTENT and \
         {VAL_009}: {meaning}"
    );
}
