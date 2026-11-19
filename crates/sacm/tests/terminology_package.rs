//! Tests for reading, writing, creating and editing a `TerminologyPackage`
//! (SACM23-TERM-001; formal/23-05-08 §10.4 "The TerminologyPackage is the
//! container element for SACM terminology assets.",
//! `TerminologyElement:TerminologyElement[0..*] (composition)`, "`TerminologyPackages`
//! can be nested."; §10.2 "`TerminologyElement` extends `Base::ArtifactElement`";
//! §2.5 "The top object of the Terminology package as a unit of interchange
//! shall be the `Terminology::TerminologyPackage` element."; §9.2
//! `terminologyPackage: TerminologyPackage [0..*] (composition)`; ptc/22-03-13
//! `Terminology::TerminologyPackage`; spec defect D9; ADR 0014, ADR 0034
//! amendment 3).
//!
//! A `TerminologyPackage` is read as a root (bare or inside `xmi:XMI`), in an
//! AssuranceCasePackage's `terminologyPackage` role (untyped or typed), nested
//! typed in another TerminologyPackage's `terminologyElement`, and typed in an
//! ArtifactPackage's `artifactElement`. It holds the five Terminology kinds in
//! `terminologyElement`, in document order. An untyped child is SACM-XMI-005;
//! one in reference form is a proxy, not read yet; the interface and binding
//! kinds and the package's `interface` end are `Interim(SACM23-TERM-007)` or
//! `Interim(SACM23-TERM-008)`; a kind the role does not admit is preserved.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{
    self, DeleteElement, EditError, MoveElement, Mutation, Operation, PackageDeletePolicy,
    ReorderElements, SetFlag, SetGid, SetName,
};
use sacm::model::{Document, ElementId, ElementKind, Flag, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

use sacm::edit::{
    CreateArgumentPackage, CreateArtifactPackage, CreateAssuranceCasePackage, CreateClaim,
    CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage,
};

const ROW: &str = "SACM23-TERM-001";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi`, `xsi` and a
/// foreign `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmlns:v=\"urn:v\"",
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

/// A function that puts start-tag attributes and a body into the
/// TerminologyPackage `t`: the package under test.
type Wrap = fn(&str, &str) -> String;

/// `t` as a bare root: the attributes on its start tag, the body on line 3.
fn root_t(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"t\"{attributes}>\n  <name content=\"T\"/>\n  \
         {body}\n</sacm:TerminologyPackage>"
    )
}

/// `t` in the `terminologyPackage` role of the AssuranceCasePackage root
/// `case`.
fn in_case_t(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
         <terminologyPackage xmi:id=\"t\"{attributes}><name content=\"T\"/>\n    {body}\n  \
         </terminologyPackage>\n</sacm:AssuranceCasePackage>"
    )
}

/// `t` typed in the `terminologyElement` role of the TerminologyPackage root
/// `outer`.
fn nested_t(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"outer\">\n  <name content=\"Outer\"/>\n  \
         <terminologyElement xmi:type=\"sacm:TerminologyPackage\" xmi:id=\"t\"{attributes}>\
         <name content=\"T\"/>\n    {body}\n  </terminologyElement>\n</sacm:TerminologyPackage>"
    )
}

/// `t` typed in the `artifactElement` role of the ArtifactPackage root
/// `outer`.
fn in_artifacts_t(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"outer\">\n  <name content=\"Outer\"/>\n  \
         <artifactElement xmi:type=\"sacm:TerminologyPackage\" xmi:id=\"t\"{attributes}>\
         <name content=\"T\"/>\n    {body}\n  </artifactElement>\n</sacm:ArtifactPackage>"
    )
}

/// The four placements of the package `t`: what it is, how to build it, and
/// the parent of `t`.
fn contexts() -> [(&'static str, Wrap, Option<&'static str>); 4] {
    [
        ("a TerminologyPackage root", root_t, None),
        (
            "a TerminologyPackage in terminologyPackage of an AssuranceCasePackage",
            in_case_t,
            Some("case"),
        ),
        (
            "a TerminologyPackage nested in terminologyElement",
            nested_t,
            Some("outer"),
        ),
        (
            "a TerminologyPackage in artifactElement of an ArtifactPackage",
            in_artifacts_t,
            Some("outer"),
        ),
    ]
}

/// A typed `terminologyElement` child `id` of class `class`, named `id`.
fn typed(class: &str, id: &str) -> String {
    format!(
        "<terminologyElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"><name content=\"{id}\"/>\
         </terminologyElement>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
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

/// `template` without its one mark, and the mark's location.
fn one_mark(what: &str, template: &str) -> (String, SourceLocation) {
    let (text, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has {} marks, not one", marks.len());
    };
    (text, *at)
}

// --- Reading -----------------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
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

/// Reads `input` in both modes, which the test expects to read to `==`
/// documents reporting only SACM-XMI-010 (an `xsi:type`), an Error in strict
/// mode and a Warning in tolerant mode; returns the strict one.
fn read_xsi_typed(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        match read(mode, input) {
            Ok(output) => {
                let found = output.diagnostics();
                assert!(
                    !found.is_empty()
                        && found.iter().all(|diagnostic| {
                            diagnostic.code() == Code::XMI_XSI_TYPE
                                && diagnostic.severity() == severity
                        }),
                    "{ROW}: {what}: reading in {mode:?} mode did not report only SACM-XMI-010 \
                     {severity:?}s: {found:?}"
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

/// Reads `input` in strict mode, which the test expects to refuse it, and
/// returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// The label of `Interim(row)`.
fn interim(row: &str) -> String {
    format!("(a limit of this version; {row} reads it)")
}

/// Asserts that `template`, without its one mark, is refused by a strict read
/// as `Unsupported` with no code at the mark, its one-line reason ending with
/// `label`; and that a tolerant read keeps it as preserved content of
/// `holder` with exactly one `tolerant_code` Warning at the mark affecting
/// `holder`, whose message names `row` when one is given.
fn assert_unsupported_with(
    what: &str,
    template: &str,
    label: &str,
    tolerant_code: Code,
    row: Option<&str>,
    holder: &str,
) {
    let (input, at) = one_mark(what, template);
    let error = refused(what, &input);
    let ReadError::Unsupported {
        location, reason, ..
    } = &error
    else {
        panic!("{ROW}: {what}: not Unsupported: {error:?} ({error})");
    };
    // The strict refusal has the code of the tolerant Warning (LIB-API-017).
    assert_eq!(
        (*location, error.code()),
        (Some(at), tolerant_code),
        "{ROW}: {what}: not refused with the tolerant read's code at {at}: {error}"
    );
    assert!(
        reason.ends_with(label) && !reason.contains('\n'),
        "{ROW}: {what}: the reason {reason:?} does not end with {label:?}"
    );
    let output = match read(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
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
            tolerant_code,
            Severity::Warning,
            Some(at),
            vec![ElementId::from(holder)]
        )],
        "{ROW}: {what}: a tolerant read does not give one {tolerant_code} Warning at the \
         construct affecting {holder:?}"
    );
    if let Some(row) = row {
        let message = output.diagnostics()[0].message();
        assert!(
            message.contains(&format!("; {row} reads it")),
            "{ROW}: {what}: the tolerant message does not name {row}: {message:?}"
        );
    }
    assert!(
        output
            .document()
            .get(holder)
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: {what}: a tolerant read keeps nothing in {holder:?}"
    );
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, a Warning in tolerant mode
/// and an Error in strict mode, and the same document in both. Returns the
/// strict document.
fn assert_preserved_at(what: &str, template: &str, holder: &str) -> Document {
    let (input, at) = one_mark(what, template);
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?} \
                 ({error})"
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
                Some(at),
                vec![ElementId::from(holder)]
            )],
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// The element `id` of `doc`, which must exist.
fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

/// The ids of the children of `id`, in order.
fn children(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
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

/// `(kind, parent id, name content)` of the element `id`.
fn placed(doc: &Document, id: &str) -> (ElementKind, Option<String>, Option<String>) {
    let found = element(doc, id);
    (
        found.kind(),
        found.parent().map(|parent| parent.id().as_str().to_owned()),
        found
            .name()
            .and_then(|name| name.content().map(str::to_owned)),
    )
}

// --- Writing -----------------------------------------------------------------------

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
/// Returns the first output.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(what, doc, mode);
    let back = read_ok(&format!("{what}, written in {mode:?} mode"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write_in(what, &back, mode),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
    );
    text
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

fn fixture_text() -> String {
    std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"))
}

fn golden_text() -> String {
    std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"))
}

// --- Editing -----------------------------------------------------------------------

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it and `edit::preview` gives the mutation
/// `apply` gives.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    assert_eq!(
        previewed.as_ref(),
        Ok(&mutation),
        "{ROW}: preview of {operation:?} does not give apply's mutation"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` and `preview` refusing it the same, and
/// returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    error
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

// =====================================================================================
// Item 1: the placements, both modes, document order
// =====================================================================================

#[test]
fn sacm23_term_001_a_bare_root_and_a_root_inside_xmi_xmi_read_in_both_modes() {
    let what = "a bare TerminologyPackage root";
    let doc = read_ok(what, &root_t("", ""));
    assert_eq!(
        (
            doc.roots()
                .map(|root| root.id().as_str().to_owned())
                .collect::<Vec<_>>(),
            placed(&doc, "t"),
            doc.element_count()
        ),
        (
            ids(&["t"]),
            (ElementKind::TerminologyPackage, None, Some("T".to_owned())),
            1
        ),
        "{ROW}: {what}: not one TerminologyPackage root t named T"
    );
    let what = "a root with a redundant xmi:type";
    let typed_root = read_ok(what, &root_t(" xmi:type=\"sacm:TerminologyPackage\"", ""));
    assert!(
        typed_root == doc,
        "{ROW}: {what}: does not read == to the bare root"
    );
    // Inside xmi:XMI, after an ArgumentPackage, in document order (§2.5: the
    // Terminology unit of interchange).
    let what = "an ArgumentPackage and a TerminologyPackage inside xmi:XMI";
    let doc = read_ok(
        what,
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
         xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>\n  \
         <sacm:TerminologyPackage xmi:id=\"t\"><name content=\"T\"/>\
         <terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"k\"><name content=\"K\"/>\
         </terminologyElement></sacm:TerminologyPackage>\n</xmi:XMI>",
    );
    assert_eq!(
        (
            doc.roots()
                .map(|root| (root.id().as_str().to_owned(), root.kind()))
                .collect::<Vec<_>>(),
            placed(&doc, "k")
        ),
        (
            vec![
                ("p".to_owned(), ElementKind::ArgumentPackage),
                ("t".to_owned(), ElementKind::TerminologyPackage),
            ],
            (
                ElementKind::Category,
                Some("t".to_owned()),
                Some("K".to_owned())
            )
        ),
        "{ROW}: {what}: not the roots p and t in document order, with the Category k in t"
    );
}

#[test]
fn sacm23_term_001_a_terminology_package_reads_in_each_role_untyped_or_typed() {
    for (what, input, parent) in [
        (
            "untyped in terminologyPackage of an AssuranceCasePackage",
            in_case_t("", ""),
            "case",
        ),
        (
            "typed in terminologyPackage of an AssuranceCasePackage",
            in_case_t(" xmi:type=\"sacm:TerminologyPackage\"", ""),
            "case",
        ),
        (
            "typed in terminologyElement of a TerminologyPackage",
            nested_t("", ""),
            "outer",
        ),
        (
            "typed in artifactElement of an ArtifactPackage",
            in_artifacts_t("", ""),
            "outer",
        ),
    ] {
        let doc = read_ok(what, &input);
        assert_eq!(
            placed(&doc, "t"),
            (
                ElementKind::TerminologyPackage,
                Some(parent.to_owned()),
                Some("T".to_owned())
            ),
            "{ROW}: {what}: not a TerminologyPackage t named T in {parent}"
        );
        assert_eq!(
            children(&doc, parent),
            ids(&["t"]),
            "{ROW}: {what}: t is not the one child of {parent}"
        );
    }
    // Typed by xsi:type: read, with SACM-XMI-010 as on every other child.
    for (what, input, parent) in [
        (
            "xsi:type in terminologyPackage",
            in_case_t(" xsi:type=\"sacm:TerminologyPackage\"", ""),
            "case",
        ),
        (
            "xsi:type in terminologyElement",
            format!(
                "<sacm:TerminologyPackage{NS} xmi:id=\"outer\"><name content=\"Outer\"/>\
                 <terminologyElement xsi:type=\"sacm:TerminologyPackage\" xmi:id=\"t\">\
                 <name content=\"T\"/></terminologyElement></sacm:TerminologyPackage>"
            ),
            "outer",
        ),
        (
            "xsi:type on a Term in terminologyElement",
            root_t(
                "",
                "<terminologyElement xsi:type=\"sacm:Term\" xmi:id=\"x\" value=\"x\">\
                 <name content=\"X\"/></terminologyElement>",
            ),
            "t",
        ),
    ] {
        let doc = read_xsi_typed(what, &input);
        let child = if parent == "t" { "x" } else { "t" };
        assert_eq!(
            placed(&doc, child).1,
            Some(parent.to_owned()),
            "{ROW}: {what}: {child} is not in {parent}"
        );
    }
}

#[test]
fn sacm23_term_001_children_and_packages_keep_document_order_at_every_depth() {
    let what = "mixed package roles and nested TerminologyPackages";
    let doc = read_ok(
        what,
        &format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
             <terminologyPackage xmi:id=\"t2\"><name content=\"T2\"/></terminologyPackage>\n  \
             <argumentPackage xmi:id=\"p\"><name content=\"P\"/></argumentPackage>\n  \
             <terminologyPackage xmi:id=\"t1\"><name content=\"T1\"/>\n    {}\n    {}\n    {}\n    \
             <terminologyElement xmi:type=\"sacm:TerminologyPackage\" xmi:id=\"n1\">\
             <name content=\"N1\"/>{}{}</terminologyElement>\n    {}\n    {}\n  \
             </terminologyPackage>\n</sacm:AssuranceCasePackage>",
            typed("Term", "x1"),
            typed("Category", "k1"),
            typed("TerminologyGroup", "g1"),
            typed("Expression", "e1"),
            typed("TerminologyPackage", "n2"),
            typed("Term", "x2"),
            typed("Expression", "e2"),
        ),
    );
    assert_eq!(
        (
            children(&doc, "case"),
            children(&doc, "t1"),
            children(&doc, "n1")
        ),
        (
            ids(&["t2", "p", "t1"]),
            ids(&["x1", "k1", "g1", "n1", "x2", "e2"]),
            ids(&["e1", "n2"])
        ),
        "{ROW}: {what}: not in document order"
    );
    assert_eq!(
        doc.elements()
            .map(|found| found.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ids(&[
            "case", "t2", "p", "t1", "x1", "k1", "g1", "n1", "e1", "n2", "x2", "e2"
        ]),
        "{ROW}: {what}: elements() is not the document's pre-order"
    );
    assert_eq!(
        (
            placed(&doc, "x1").0,
            placed(&doc, "k1").0,
            placed(&doc, "g1").0,
            placed(&doc, "e1").0,
            placed(&doc, "n2").0
        ),
        (
            ElementKind::Term,
            ElementKind::Category,
            ElementKind::TerminologyGroup,
            ElementKind::Expression,
            ElementKind::TerminologyPackage
        ),
        "{ROW}: {what}: not read with their kinds"
    );
}

// =====================================================================================
// Item 2: an untyped terminologyElement child, and the reference form
// =====================================================================================

#[test]
fn sacm23_term_001_an_untyped_terminology_element_child_is_missing_type_or_kept_with_a_warning() {
    for (context, wrap, _) in contexts() {
        for (form, child) in [
            (
                "an untyped terminologyElement",
                "<terminologyElement xmi:id=\"x\"/>",
            ),
            (
                "an untyped terminologyElement with a name",
                "<terminologyElement xmi:id=\"x\"><name content=\"X\"/></terminologyElement>",
            ),
        ] {
            let what = format!("{form} in {context}");
            let (input, at) = one_mark(&what, &wrap("", &format!("{MARK}{child}")));
            let error = refused(&what, &input);
            assert!(
                !matches!(error, ReadError::Unsupported { .. }),
                "{ROW}: {what}: Unsupported, not SACM-XMI-005: {error:?} ({error})"
            );
            assert_eq!(
                (error.code(), error.location(), error.locations()),
                (Code::XMI_MISSING_TYPE, Some(at), [at].as_slice()),
                "{ROW}: {what}: not SACM-XMI-005 at the start tag: {error:?} ({error})"
            );
            // A tolerant read keeps it whole, as preserved content of t.
            let output = match read(Mode::Tolerant, &input) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
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
                    Code::XMI_MISSING_TYPE,
                    Severity::Warning,
                    Some(at),
                    list(&["t"])
                )],
                "{ROW}: {what}: a tolerant read does not give one SACM-XMI-005 Warning on t"
            );
            let doc = output.into_document();
            assert!(
                !doc.contains("x") && element(&doc, "t").has_preserved_content(),
                "{ROW}: {what}: a tolerant read does not keep it whole in t"
            );
        }
    }
}

#[test]
fn sacm23_term_001_a_reference_form_terminology_element_under_a_package_is_a_proxy_not_read_yet() {
    // Under a TerminologyPackage, `terminologyElement` is the composite role,
    // so a role element in reference form is a proxy (XMI 2.5.1 §7.10.1), not
    // a group member and not SACM-XMI-005.
    let sibling = typed("Category", "k");
    for (context, wrap, _) in contexts() {
        for (form, child, row, tolerant_code) in [
            (
                "an untyped xmi:idref",
                format!("{MARK}<terminologyElement xmi:idref=\"k\"/>"),
                "SACM23-XMI-019",
                Code::XMI_UNREAD_CONTENT,
            ),
            (
                "an untyped href=\"#k\"",
                format!("{MARK}<terminologyElement href=\"#k\"/>"),
                "SACM23-XMI-029",
                Code::XMI_EXTERNAL_HREF,
            ),
        ] {
            let what = format!("{form} terminologyElement in {context}");
            assert_unsupported_with(
                &what,
                &wrap("", &format!("{sibling}{child}")),
                &interim(row),
                tolerant_code,
                None,
                "t",
            );
        }
    }
}

// =====================================================================================
// Item 3: what a TerminologyPackage holds, and what it does not read yet
// =====================================================================================

/// The kinds a TerminologyPackage reads typed in `terminologyElement`.
const READABLE: [ElementKind; 5] = [
    ElementKind::TerminologyPackage,
    ElementKind::Category,
    ElementKind::Term,
    ElementKind::Expression,
    ElementKind::TerminologyGroup,
];

#[test]
fn sacm23_term_001_every_terminology_kind_is_read_typed_inside_a_terminology_package() {
    for (context, wrap, _) in contexts() {
        for kind in READABLE {
            let what = format!("the {kind} x in terminologyElement of {context}");
            let doc = read_ok(&what, &wrap("", &typed(kind.name(), "x")));
            assert_eq!(
                placed(&doc, "x"),
                (kind, Some("t".to_owned()), Some("x".to_owned())),
                "{ROW}: {what}: not a {kind} x named x in t"
            );
        }
        let what = format!("every terminology kind in {context}");
        let body: String = READABLE
            .iter()
            .enumerate()
            .map(|(index, kind)| typed(kind.name(), &format!("x{index}")))
            .collect();
        let doc = read_ok(&what, &wrap("", &body));
        assert_eq!(
            children(&doc, "t"),
            ids(&["x0", "x1", "x2", "x3", "x4"]),
            "{ROW}: {what}: not every kind, in document order"
        );
    }
}

#[test]
fn sacm23_term_001_the_interface_and_binding_kinds_and_the_interface_end_are_read() {
    // Until slice 0067 these were Interim (SACM23-TERM-007, -008): refused
    // as Unsupported in strict mode and kept with SACM-XMI-014 in tolerant
    // mode. They are read now, in every context of t.
    for (context, wrap, _) in contexts() {
        for kind in [
            ElementKind::TerminologyPackageInterface,
            ElementKind::TerminologyPackageBinding,
        ] {
            let what = format!("the {kind} x in terminologyElement of {context}");
            let child = format!("{}{}", typed("Category", "k"), typed(kind.name(), "x"));
            let doc = read_ok(&what, &wrap("", &child));
            assert_eq!(
                (placed(&doc, "x"), children(&doc, "t")),
                (
                    (kind, Some("t".to_owned()), Some("x".to_owned())),
                    ids(&["k", "x"])
                ),
                "{ROW}: {what}: not a {kind} x named x in t, after k"
            );
        }
        // ptc/22-03-13 gives TerminologyPackage `interface :
        // TerminologyPackageInterface [0..*]`, read as an attribute and as a
        // reference element.
        for (form, attributes, body) in [
            ("interface=\"i\"", " interface=\"i\"", String::new()),
            (
                "an interface element",
                "",
                "<interface xmi:idref=\"i\"/>".to_owned(),
            ),
        ] {
            let what = format!("{form} on {context}");
            let doc = read_ok(&what, &wrap(attributes, &body));
            assert_eq!(
                element(&doc, "t").interfaces(),
                list(&["i"]).as_slice(),
                "{ROW}: {what}: t's interface end"
            );
        }
        // In another namespace it is not that end: preserved.
        assert_preserved_at(
            &format!("v:interface=\"i\" on {context}"),
            &wrap(&format!(" {MARK}v:interface=\"i\""), ""),
            "t",
        );
    }
}

#[test]
fn sacm23_term_001_a_kind_or_role_a_terminology_package_does_not_admit_is_preserved() {
    for (context, wrap, _) in contexts() {
        for (what, child) in [
            (
                "a Claim typed in terminologyElement",
                format!(
                    "{MARK}<terminologyElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                     <name content=\"X\"/></terminologyElement>"
                ),
            ),
            (
                "an Artifact typed in terminologyElement",
                format!(
                    "{MARK}<terminologyElement xmi:type=\"sacm:Artifact\" xmi:id=\"x\">\
                     <name content=\"X\"/></terminologyElement>"
                ),
            ),
            (
                "a Note typed in terminologyElement",
                format!(
                    "{MARK}<terminologyElement xmi:type=\"sacm:Note\" xmi:id=\"x\">\
                     <content><value content=\"N\"/></content></terminologyElement>"
                ),
            ),
            (
                "an argumentElement child",
                format!(
                    "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                     <name content=\"X\"/></argumentElement>"
                ),
            ),
            (
                "an artifactElement child",
                format!(
                    "{MARK}<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"x\">\
                     <name content=\"X\"/></artifactElement>"
                ),
            ),
        ] {
            let what = format!("{what} of {context}");
            let doc = assert_preserved_at(&what, &wrap("", &child), "t");
            assert!(
                !doc.contains("x") && children(&doc, "t").is_empty(),
                "{ROW}: {what}: the content is read as a child"
            );
            // A tolerant write writes it back where it was.
            let text = write_in(&what, &doc, Mode::Tolerant);
            assert!(
                text.contains("xmi:id=\"x\""),
                "{ROW}: {what}: the preserved content is not written back\n{text}"
            );
        }
    }
}

// =====================================================================================
// Item 4: writing, and the fixture
// =====================================================================================

#[test]
fn sacm23_term_001_written_as_a_root_an_untyped_terminology_package_and_typed_children() {
    // A root: written as sacm:TerminologyPackage, with its children typed.
    let what = "a TerminologyPackage root with two children";
    let doc = read_ok(
        what,
        &root_t(
            "",
            &format!(
                "{}{}",
                typed("Category", "k"),
                typed("TerminologyPackage", "n")
            ),
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        (
            line_with(&text, "t"),
            line_with(&text, "k"),
            line_with(&text, "n")
        ),
        (
            "<sacm:TerminologyPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
             xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"t\">",
            "<terminologyElement xmi:type=\"sacm:Category\" xmi:id=\"k\">",
            "<terminologyElement xmi:type=\"sacm:TerminologyPackage\" xmi:id=\"n\">"
        ),
        "{ROW}: {what}: not written as a root with typed terminologyElement children\n{text}"
    );
    // In an AssuranceCasePackage: an untyped terminologyPackage, even when
    // read typed.
    for attributes in ["", " xmi:type=\"sacm:TerminologyPackage\""] {
        let what = format!("a terminologyPackage read with {attributes:?}");
        let doc = read_ok(&what, &in_case_t(attributes, ""));
        let text = round_trip(&what, &doc, Mode::Strict);
        assert_eq!(
            line_with(&text, "t"),
            "<terminologyPackage xmi:id=\"t\">",
            "{ROW}: {what}: not written untyped\n{text}"
        );
    }
    // In an ArtifactPackage: typed in artifactElement.
    let what = "a TerminologyPackage in artifactElement";
    let doc = read_ok(what, &in_artifacts_t("", ""));
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "t"),
        "<artifactElement xmi:type=\"sacm:TerminologyPackage\" xmi:id=\"t\">",
        "{ROW}: {what}: not written typed in artifactElement\n{text}"
    );
}

#[test]
fn sacm23_term_001_the_fixture_reads_in_both_modes_with_no_diagnostic_and_validates() {
    let doc = read_ok("the fixture", &fixture_text());
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture does not validate cleanly"
    );
    assert_eq!(
        (
            children(&doc, "case"),
            children(&doc, "terms"),
            children(&doc, "iec"),
            children(&doc, "evidence"),
            element(&doc, "terms").gid()
        ),
        (
            ids(&["terms", "arguments", "evidence"]),
            ids(&[
                "hazards",
                "mechanical",
                "blade",
                "lid",
                "guard",
                "blade_guarded",
                "iec",
                "reviewed"
            ]),
            ids(&["appliance"]),
            ids(&["manual", "jar"]),
            Some("TP-1")
        ),
        "{ROW}: the fixture's packages do not hold their elements in document order"
    );
    assert_eq!(
        (
            placed(&doc, "terms").0,
            placed(&doc, "iec").0,
            placed(&doc, "jar").0,
            placed(&doc, "reviewed").0,
            element(&doc, "guard").is_abstract()
        ),
        (
            ElementKind::TerminologyPackage,
            ElementKind::TerminologyPackage,
            ElementKind::Term,
            ElementKind::TerminologyGroup,
            true
        ),
        "{ROW}: the fixture's kinds"
    );
}

#[test]
fn sacm23_term_001_the_fixture_writes_exactly_the_golden_file_which_reads_back_equal() {
    let doc = read_ok("the fixture", &fixture_text());
    let golden = golden_text();
    let text = write_in("the fixture", &doc, Mode::Strict);
    assert_eq!(
        text, golden,
        "{ROW}: a strict write of the fixture is not the golden file"
    );
    let back = read_ok("the golden file", &golden);
    assert!(
        back == doc,
        "{ROW}: the golden file does not read back == to the fixture"
    );
    assert_eq!(
        write_in("the golden file", &back, Mode::Strict),
        golden,
        "{ROW}: writing the golden file again gives other bytes"
    );
    assert_eq!(
        write_in("the fixture", &doc, Mode::Tolerant),
        golden,
        "{ROW}: a tolerant write of the fixture is not the golden file"
    );
}

// =====================================================================================
// Item 5: CreateTerminologyPackage
// =====================================================================================

/// Applies a create the test expects to succeed, and returns the one id it
/// created, with nothing changed or deleted.
fn create(doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let mutation = applied(doc, operation);
    let [only] = mutation.created_ids() else {
        panic!(
            "{ROW}: created_ids() is {:?}, not the one new element",
            mutation.created_ids()
        );
    };
    assert_eq!(
        (mutation.changed_ids(), mutation.deleted_ids()),
        ([].as_slice(), [].as_slice()),
        "{ROW}: a create changed or deleted elements"
    );
    only.clone()
}

/// A document built by edits: the AssuranceCasePackage `case` holding the
/// ArgumentPackage `p` (the Claim `c1`), the ArtifactPackage `evidence` and
/// the TerminologyPackage `terms` (the Term `t1` and the `TerminologyGroup`
/// `g1`).
fn built() -> Document {
    let mut doc = Document::new();
    for operation in [
        Operation::from(CreateAssuranceCasePackage::root(named("Case")).with_id("case")),
        CreateArgumentPackage::new("case", named("P"))
            .with_id("p")
            .into(),
        CreateClaim::new("p", named("C1")).with_id("c1").into(),
        CreateArtifactPackage::new("case", named("Evidence"))
            .with_id("evidence")
            .into(),
        CreateTerminologyPackage::new("case", named("Terms"))
            .with_id("terms")
            .into(),
        CreateTerm::new("terms", named("T1"), "t1")
            .with_id("t1")
            .into(),
        CreateTerminologyGroup::new("terms", named("G1"))
            .with_id("g1")
            .into(),
    ] {
        create(&mut doc, operation);
    }
    doc
}

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
        _ => Vec::new(),
    };
    (variant, named)
}

/// `CreateTerminologyPackage` under `parent` (a root for `None`), with the
/// caller id `id`.
fn make(parent: Option<&str>, id: Option<&str>) -> Operation {
    let create = match parent {
        Some(parent) => CreateTerminologyPackage::new(parent, named("N")),
        None => CreateTerminologyPackage::root(named("N")),
    };
    match id {
        Some(id) => create.with_id(id).into(),
        None => create.into(),
    }
}

#[test]
fn sacm23_term_001_create_terminology_package_reports_what_it_was_given() {
    let root = CreateTerminologyPackage::root(named("Blender terminology"));
    assert_eq!(
        (root.parent(), root.id(), root.name()),
        (None, None, &named("Blender terminology")),
        "{ROW}: root(name)"
    );
    let child =
        CreateTerminologyPackage::new("case", named("Blender terminology")).with_id("terms");
    assert_eq!(
        (
            child.parent().map(ElementId::as_str),
            child.id().map(ElementId::as_str),
            child.name()
        ),
        (Some("case"), Some("terms"), &named("Blender terminology")),
        "{ROW}: new(parent, name).with_id(id)"
    );
    assert_eq!(
        (
            Operation::from(root).operation_name(),
            Operation::from(child).operation_name()
        ),
        ("CreateTerminologyPackage", "CreateTerminologyPackage"),
        "{ROW}: operation_name()"
    );
}

#[test]
fn sacm23_term_001_create_terminology_package_creates_roots_in_document_order() {
    let mut doc = Document::new();
    let first = create(&mut doc, make(None, None));
    let second = create(&mut doc, make(None, Some("more")));
    let third = create(&mut doc, make(None, None));
    assert_eq!(
        (
            [first, second, third].map(|id| id.as_str().to_owned()),
            doc.roots()
                .map(|root| (root.id().as_str().to_owned(), root.kind()))
                .collect::<Vec<_>>()
        ),
        (
            [
                "terminology_package_1".to_owned(),
                "more".to_owned(),
                "terminology_package_2".to_owned()
            ],
            vec![
                (
                    "terminology_package_1".to_owned(),
                    ElementKind::TerminologyPackage
                ),
                ("more".to_owned(), ElementKind::TerminologyPackage),
                (
                    "terminology_package_2".to_owned(),
                    ElementKind::TerminologyPackage
                )
            ]
        ),
        "{ROW}: the created roots"
    );
    let text = write_in("the created roots", &doc, Mode::Tolerant);
    assert_eq!(
        line_with(&text, "more"),
        "<sacm:TerminologyPackage xmi:id=\"more\">",
        "{ROW}: a created root is not written as sacm:TerminologyPackage\n{text}"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_term_001_create_terminology_package_legal_parents_refusals_and_generated_id() {
    let kind = ElementKind::TerminologyPackage;
    let mut doc = built();
    // Generated ids, under the first legal parent.
    let id1 = create(&mut doc, make(Some("case"), None));
    let id2 = create(&mut doc, make(Some("case"), None));
    assert_eq!(
        (id1.as_str(), id2.as_str()),
        ("terminology_package_1", "terminology_package_2"),
        "{ROW}: the generated ids"
    );
    // Every legal parent, with a caller id; the role it is written in.
    for (index, (parent, role)) in [
        ("case", "terminologyPackage"),
        ("terms", "terminologyElement"),
        ("evidence", "artifactElement"),
    ]
    .iter()
    .enumerate()
    {
        let given = format!("given_{index}");
        create(&mut doc, make(Some(*parent), Some(given.as_str())));
        assert_eq!(
            placed(&doc, &given),
            (kind, Some((*parent).to_owned()), Some("N".to_owned())),
            "{ROW}: a TerminologyPackage with a caller id under {parent}"
        );
        assert_eq!(
            element(&doc, parent)
                .children()
                .last()
                .map(|child| child.id().as_str().to_owned()),
            Some(given.clone()),
            "{ROW}: the TerminologyPackage under {parent} is not its last child"
        );
        let text = write_in("the built document", &doc, Mode::Strict);
        assert!(
            line_with(&text, &given).starts_with(&format!("<{role} ")),
            "{ROW}: the TerminologyPackage under {parent} is not written in {role}\n{text}"
        );
    }
    // A root.
    create(&mut doc, make(None, Some("shared")));
    assert_eq!(
        placed(&doc, "shared").1,
        None,
        "{ROW}: root(name) does not create a root"
    );
    // No finding: the Term t1 has its value.
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the built document does not validate cleanly"
    );
    round_trip("the built document", &doc, Mode::Strict);
    // InvalidParent under each illegal parent.
    for parent in ["c1", "p", "t1", "g1"] {
        let parent_kind = element(&doc, parent).kind();
        let error = refuse(&mut doc, make(Some(parent), None));
        assert_eq!(
            (refusal(&error), error.code()),
            (
                (
                    "InvalidParent".to_owned(),
                    vec![
                        parent.to_owned(),
                        parent_kind.name().to_owned(),
                        kind.name().to_owned()
                    ]
                ),
                Code::CMD_INVALID_PARENT
            ),
            "{ROW}: a TerminologyPackage under the {parent_kind} {parent}"
        );
    }
    // The four checks in order: each later fault is hidden by an earlier.
    for (what, parent, id, expected) in [
        (
            "a missing parent, an invalid id",
            Some("nowhere"),
            Some("1x"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "a missing parent, a duplicate id",
            Some("nowhere"),
            Some("c1"),
            ("ParentNotFound", vec!["nowhere"]),
        ),
        (
            "an illegal parent, an invalid id",
            Some("p"),
            Some("1x"),
            (
                "InvalidParent",
                vec!["p", "ArgumentPackage", "TerminologyPackage"],
            ),
        ),
        (
            "an illegal parent, a duplicate id",
            Some("p"),
            Some("c1"),
            (
                "InvalidParent",
                vec!["p", "ArgumentPackage", "TerminologyPackage"],
            ),
        ),
        (
            "an invalid id",
            Some("case"),
            Some("1x"),
            ("InvalidId", vec!["1x"]),
        ),
        (
            "an invalid root id",
            None,
            Some("a b"),
            ("InvalidId", vec!["a b"]),
        ),
        (
            "a duplicate id",
            Some("case"),
            Some("c1"),
            ("DuplicateId", vec!["c1"]),
        ),
        (
            "a duplicate root id",
            None,
            Some("terms"),
            ("DuplicateId", vec!["terms"]),
        ),
    ] {
        let error = refuse(&mut doc, make(parent, id));
        assert_eq!(
            refusal(&error),
            (
                expected.0.to_owned(),
                expected.1.iter().map(|&s| s.to_owned()).collect::<Vec<_>>()
            ),
            "{ROW}: {what}: the refusal"
        );
    }
    // A refusal does not move the next generated id.
    let next = create(&mut doc, make(Some("case"), None));
    assert_eq!(
        next.as_str(),
        "terminology_package_3",
        "{ROW}: a refusal moved the next generated id"
    );
}

// =====================================================================================
// Item 6: delete, move and the existing edits
// =====================================================================================

fn fixture_doc() -> Document {
    read_ok("the fixture", &fixture_text())
}

#[test]
fn sacm23_term_001_deleting_a_non_empty_terminology_package_is_not_empty_unless_recursive() {
    for (context, wrap, _) in contexts() {
        let what = format!("{context} holding the Term x");
        let mut doc = read_ok(&what, &wrap("", &typed("Term", "x")));
        let error = refuse(&mut doc, DeleteElement::new("t"));
        assert!(
            matches!(
                &error,
                EditError::NotEmpty { element, contained, .. }
                    if element.as_str() == "t" && *contained == 1
            ),
            "{ROW}: {what}: deleting t: not NotEmpty {{ t, 1 }}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_PACKAGE_NOT_EMPTY,
            "{ROW}: {what}: deleting a non-empty TerminologyPackage: not SACM-CMD-006"
        );
        let mutation = applied(
            &mut doc,
            DeleteElement::new("t").with_package_policy(PackageDeletePolicy::DeleteRecursively),
        );
        assert_eq!(
            (mutation.deleted_ids(), doc.contains("t"), doc.contains("x")),
            (list(&["t", "x"]).as_slice(), false, false),
            "{ROW}: {what}: DeleteRecursively does not delete t and x"
        );
    }
    // An empty TerminologyPackage is deleted as it is.
    let mut doc = read_ok("an empty package", &in_case_t("", ""));
    let mutation = applied(&mut doc, DeleteElement::new("t"));
    assert_eq!(
        (mutation.deleted_ids(), children(&doc, "case")),
        (list(&["t"]).as_slice(), Vec::<String>::new()),
        "{ROW}: an empty TerminologyPackage is not deleted"
    );
}

#[test]
fn sacm23_term_001_move_element_takes_a_terminology_package_to_each_legal_parent() {
    let mut doc = fixture_doc();
    for (parent, role) in [
        ("case", "terminologyPackage"),
        ("evidence", "artifactElement"),
        ("terms", "terminologyElement"),
    ] {
        let mutation = applied(&mut doc, MoveElement::new("iec", parent));
        assert_eq!(
            (
                mutation.changed_ids(),
                placed(&doc, "iec").1,
                children(&doc, parent).last().cloned(),
                children(&doc, "iec")
            ),
            (
                list(&["iec"]).as_slice(),
                Some(parent.to_owned()),
                Some("iec".to_owned()),
                ids(&["appliance"])
            ),
            "{ROW}: moving iec into {parent}"
        );
        let text = round_trip(&format!("iec in {parent}"), &doc, Mode::Strict);
        assert!(
            line_with(&text, "iec").starts_with(&format!("<{role} ")),
            "{ROW}: iec moved into {parent} is not written in {role}\n{text}"
        );
    }
    // Into an ArgumentPackage: InvalidParent.
    let error = refuse(&mut doc, MoveElement::new("iec", "arguments"));
    assert!(
        matches!(
            &error,
            EditError::InvalidParent { parent, parent_kind, kind, .. }
                if parent.as_str() == "arguments"
                    && *parent_kind == ElementKind::ArgumentPackage
                    && *kind == ElementKind::TerminologyPackage
        ),
        "{ROW}: moving iec into an ArgumentPackage: not InvalidParent: {error:?}"
    );
}

#[test]
fn sacm23_term_001_set_name_gid_flag_and_reorder_apply_to_a_terminology_package() {
    let mut doc = fixture_doc();
    applied(&mut doc, SetName::new("terms", named("Renamed terms")));
    applied(&mut doc, SetGid::new("iec", "TP-2"));
    applied(&mut doc, SetGid::remove("terms"));
    applied(&mut doc, SetFlag::new("iec", Flag::IsAbstract, true));
    applied(&mut doc, SetFlag::new("iec", Flag::IsCitation, true));
    applied(
        &mut doc,
        ReorderElements::new(
            "terms",
            [
                "reviewed",
                "iec",
                "blade_guarded",
                "guard",
                "lid",
                "blade",
                "mechanical",
                "hazards",
            ],
        ),
    );
    let terms = element(&doc, "terms");
    let iec = element(&doc, "iec");
    assert_eq!(
        (
            terms.name(),
            terms.gid(),
            iec.gid(),
            iec.is_abstract(),
            iec.is_citation(),
            children(&doc, "terms")
        ),
        (
            Some(&named("Renamed terms")),
            None,
            Some("TP-2"),
            true,
            true,
            ids(&[
                "reviewed",
                "iec",
                "blade_guarded",
                "guard",
                "lid",
                "blade",
                "mechanical",
                "hazards"
            ])
        ),
        "{ROW}: the edits on a TerminologyPackage"
    );
    // isCounter is no feature of a TerminologyPackage.
    let error = refuse(&mut doc, SetFlag::new("terms", Flag::IsCounter, true));
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind, feature, .. }
                if element.as_str() == "terms"
                    && *kind == ElementKind::TerminologyPackage
                    && *feature == "isCounter"
        ),
        "{ROW}: isCounter on a TerminologyPackage: not FeatureNotApplicable: {error:?}"
    );
    round_trip("the edited fixture", &doc, Mode::Strict);
}

// =====================================================================================
// Item 7: no refusal is left for a TerminologyPackage
// =====================================================================================

#[test]
fn sacm23_term_001_a_strict_read_of_a_bare_terminology_package_root_returns_its_document() {
    let input = root_t(
        "",
        &typed("Term", "x").replace("xmi:id=\"x\"", "xmi:id=\"x\" value=\"x\""),
    );
    let output = match read(Mode::Strict, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: a strict read refused a bare root: {error:?} ({error})"),
    };
    let doc = output.into_document();
    assert_eq!(
        (
            doc.roots()
                .map(|root| (root.id().as_str().to_owned(), root.kind()))
                .collect::<Vec<_>>(),
            children(&doc, "t"),
            doc.has_preserved_content()
        ),
        (
            vec![("t".to_owned(), ElementKind::TerminologyPackage)],
            ids(&["x"]),
            false
        ),
        "{ROW}: the strict read of a bare root"
    );
    // A strict write is no PreservedContent and no NoRoot: the root is read.
    let text = round_trip("the bare root", &doc, Mode::Strict);
    assert!(
        text.contains("<sacm:TerminologyPackage ") && !text.contains("<xmi:XMI"),
        "{ROW}: the bare root is not written as a bare root\n{text}"
    );
}

// =====================================================================================
// The operations of slice 0062 are values like every other operation
// =====================================================================================

fn assert_operation_value<T: Send + Sync + Clone + std::fmt::Debug + PartialEq + Eq>() {}

#[test]
fn sacm23_term_001_the_terminology_operations_are_send_sync_clone_debug_eq_and_convert_into_operation()
 {
    use sacm::edit::{CreateCategory, CreateExpression, SetExternalReference, SetValue};
    assert_operation_value::<CreateTerminologyPackage>();
    assert_operation_value::<CreateCategory>();
    assert_operation_value::<CreateTerm>();
    assert_operation_value::<CreateExpression>();
    assert_operation_value::<CreateTerminologyGroup>();
    assert_operation_value::<SetValue>();
    assert_operation_value::<SetExternalReference>();
    for (operation, name) in [
        (
            Operation::from(CreateTerminologyPackage::root(named("T"))),
            "CreateTerminologyPackage",
        ),
        (
            Operation::from(CreateCategory::new("t", named("C"))),
            "CreateCategory",
        ),
        (
            Operation::from(CreateTerm::new("t", named("T"), "t")),
            "CreateTerm",
        ),
        (
            Operation::from(CreateExpression::new("t", named("E"), "e")),
            "CreateExpression",
        ),
        (
            Operation::from(CreateTerminologyGroup::new("t", named("G"))),
            "CreateTerminologyGroup",
        ),
        (Operation::from(SetValue::new("t", "v")), "SetValue"),
        (
            Operation::from(SetExternalReference::remove("t")),
            "SetExternalReference",
        ),
    ] {
        assert_eq!(
            operation.operation_name(),
            name,
            "{ROW}: operation_name() of {operation:?}"
        );
    }
}
