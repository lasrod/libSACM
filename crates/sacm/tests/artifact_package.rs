//! Tests for reading, writing and deleting an `ArtifactPackage`
//! (SACM23-ART-001; formal/23-05-08 §12.2 `artifactElement:Base::ArtifactElement[0..*]
//! (composition)`, "`ArtifactPackages` can also be nested"; ptc/22-03-13
//! `Artifact::ArtifactPackage` (`artifactElement`, composite; `interface`);
//! clause 2; ADR 0014, ADR 0033, ADR 0034).
//!
//! An `ArtifactPackage` is read as a root (bare or inside `xmi:XMI`), in an
//! AssuranceCasePackage's `artifactPackage` role (untyped or typed), and
//! nested in another ArtifactPackage's `artifactElement` role with a type.
//! It holds every `ArtifactElement` kind this release reads in
//! `artifactElement`, in document order. An untyped `artifactElement` child is
//! SACM-XMI-005; one in reference form is a proxy: of a root of the same
//! document, moved into place, or kept with SACM-XMI-003 (SACM23-XMI-030,
//! since slice 0073a), or of an element of another document (SACM23-XMI-029).
//! Every kind it can hold that
//! this release does not read yet is refused as `Interim(row)`, its
//! `interface` end is read (SACM23-ART-012, slice 0066), and the model's spelling
//! `ArtifactAssertedRelationship` as `Tolerance(SACM23-XMI-009)`, each at an
//! exact location in both modes. An `artifactElement` attribute on a package
//! is an attribute the class lacks: preserved content (ADR 0033).
//!
//! The type `ArtifactAssertedRelationship` is refused only where the role
//! admits an `ArtifactAssetRelationship`; elsewhere, and any unknown, abstract,
//! foreign or miscased type in `artifactElement`, is preserved content.
//!
//! `CreateArtifactPackage` is tested in `artifact_edits.rs`, with the other
//! creates of slice 0052.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::edit::{self, DeleteElement, EditError, PackageDeletePolicy};
use sacm::model::{Document, ElementId, ElementKind, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-ART-001";

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

/// The worked fixture of slice 0052.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-package-valid.sacm.xmi"
);

/// The exact output of writing [`FIXTURE`], in either mode.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/artifact-package.golden.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// A function that puts start-tag attributes and a body into the
/// ArtifactPackage `a`: the package under test.
type Wrap = fn(&str, &str) -> String;

/// `a` as a bare root: the attributes on its start tag, the body on line 3.
fn root_a(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"a\"{attributes}>\n  <name content=\"A\"/>\n  {body}\n\
         </sacm:ArtifactPackage>"
    )
}

/// `a` in the `artifactPackage` role of the AssuranceCasePackage root `case`.
fn in_case_a(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
         <artifactPackage xmi:id=\"a\"{attributes}><name content=\"A\"/>\n    {body}\n  \
         </artifactPackage>\n</sacm:AssuranceCasePackage>"
    )
}

/// `a` typed in the `artifactElement` role of the ArtifactPackage root
/// `outer`.
fn nested_a(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"outer\">\n  <name content=\"Outer\"/>\n  \
         <artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"a\"{attributes}>\
         <name content=\"A\"/>\n    {body}\n  </artifactElement>\n</sacm:ArtifactPackage>"
    )
}

/// The three placements of the package `a`: what it is, how to build it, and
/// the parent of `a`.
fn contexts() -> [(&'static str, Wrap, Option<&'static str>); 3] {
    [
        ("an ArtifactPackage root", root_a, None),
        (
            "an ArtifactPackage in artifactPackage of an AssuranceCasePackage",
            in_case_a,
            Some("case"),
        ),
        (
            "an ArtifactPackage nested in artifactElement",
            nested_a,
            Some("outer"),
        ),
    ]
}

/// A typed `artifactElement` child `id` of class `class`, named `name`.
fn typed(class: &str, id: &str, name: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"><name content=\"{name}\"/>\
         </artifactElement>"
    )
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
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
/// documents reporting only SACM-XMI-010 (an `xsi:type`), at least once, an
/// Error in strict mode and a Warning in tolerant mode; returns the strict
/// one.
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
/// returns the refusal. A tolerant read keeps what a strict read refuses
/// here (LIB-COMPAT-001, ADR 0035).
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
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
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
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

// =====================================================================================
// Item 1: the three placements, both modes, document order
// =====================================================================================

#[test]
fn sacm23_art_001_a_bare_root_and_roots_in_an_xmi_wrapper_read_in_both_modes() {
    let what = "a bare ArtifactPackage root";
    let doc = read_ok(what, &root_a("", ""));
    assert_eq!(
        (
            doc.roots()
                .map(|root| root.id().as_str().to_owned())
                .collect::<Vec<_>>(),
            placed(&doc, "a"),
            doc.element_count()
        ),
        (
            ids(&["a"]),
            (ElementKind::ArtifactPackage, None, Some("A".to_owned())),
            1
        ),
        "{ROW}: {what}: not one ArtifactPackage root a named A"
    );
    // A redundant xmi:type on the root changes nothing.
    let what = "a root with a redundant xmi:type";
    let typed_root = read_ok(what, &root_a(" xmi:type=\"sacm:ArtifactPackage\"", ""));
    assert!(
        typed_root == doc,
        "{ROW}: {what}: does not read == to the bare root"
    );
    // Inside xmi:XMI, with another root kind between, in document order.
    let what = "three roots inside xmi:XMI";
    let doc = read_ok(
        what,
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
         xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n  \
         <sacm:ArtifactPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArtifactPackage>\n  \
         <sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"Case\"/></sacm:AssuranceCasePackage>\n  \
         <sacm:ArtifactPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArtifactPackage>\n</xmi:XMI>",
    );
    assert_eq!(
        doc.roots()
            .map(|root| (root.id().as_str().to_owned(), root.kind()))
            .collect::<Vec<_>>(),
        [
            ("a".to_owned(), ElementKind::ArtifactPackage),
            ("case".to_owned(), ElementKind::AssuranceCasePackage),
            ("b".to_owned(), ElementKind::ArtifactPackage),
        ],
        "{ROW}: {what}: not the roots a, case and b in document order"
    );
}

#[test]
fn sacm23_art_001_an_artifact_package_reads_in_artifact_package_untyped_or_typed_and_nested_typed()
{
    for (what, input, parent) in [
        (
            "untyped in artifactPackage of an AssuranceCasePackage",
            in_case_a("", ""),
            "case",
        ),
        (
            "typed in artifactPackage of an AssuranceCasePackage",
            in_case_a(" xmi:type=\"sacm:ArtifactPackage\"", ""),
            "case",
        ),
        (
            "typed in artifactElement of an ArtifactPackage",
            nested_a("", ""),
            "outer",
        ),
    ] {
        let doc = read_ok(what, &input);
        assert_eq!(
            placed(&doc, "a"),
            (
                ElementKind::ArtifactPackage,
                Some(parent.to_owned()),
                Some("A".to_owned())
            ),
            "{ROW}: {what}: not an ArtifactPackage a named A in {parent}"
        );
        assert_eq!(
            children(&doc, parent),
            ids(&["a"]),
            "{ROW}: {what}: a is not the one child of {parent}"
        );
    }
    // Typed by xsi:type: read, with SACM-XMI-010 as on every other child.
    for (what, input, parent) in [
        (
            "xsi:type in artifactPackage",
            in_case_a(" xsi:type=\"sacm:ArtifactPackage\"", ""),
            "case",
        ),
        (
            "xsi:type in artifactElement",
            format!(
                "<sacm:ArtifactPackage{NS} xmi:id=\"outer\"><name content=\"Outer\"/>\
                 <artifactElement xsi:type=\"sacm:ArtifactPackage\" xmi:id=\"a\">\
                 <name content=\"A\"/></artifactElement></sacm:ArtifactPackage>"
            ),
            "outer",
        ),
    ] {
        let doc = read_xsi_typed(what, &input);
        assert_eq!(
            placed(&doc, "a"),
            (
                ElementKind::ArtifactPackage,
                Some(parent.to_owned()),
                Some("A".to_owned())
            ),
            "{ROW}: {what}: not an ArtifactPackage a named A in {parent}"
        );
    }
}

#[test]
fn sacm23_art_001_children_and_packages_keep_document_order_at_every_depth() {
    let what = "mixed package roles and nested ArtifactPackages";
    let doc = read_ok(
        what,
        &format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
             <artifactPackage xmi:id=\"a2\"><name content=\"A2\"/></artifactPackage>\n  \
             <argumentPackage xmi:id=\"p\"><name content=\"P\"/></argumentPackage>\n  \
             <artifactPackage xmi:id=\"a1\"><name content=\"A1\"/>\n    {}\n    {}\n    {}\n    \
             <artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"n1\"><name content=\"N1\"/>\
             {}{}</artifactElement>\n    {}\n  </artifactPackage>\n\
             </sacm:AssuranceCasePackage>",
            typed("Participant", "pa1", "Pa1"),
            typed("Technique", "t1", "T1"),
            typed("ArtifactGroup", "g1", "G1"),
            typed("Property", "pr1", "Pr1"),
            typed("ArtifactPackage", "n2", "N2"),
            typed("Claim", "c1", "C1"),
        ),
    );
    assert_eq!(
        (
            children(&doc, "case"),
            children(&doc, "a1"),
            children(&doc, "n1")
        ),
        (
            ids(&["a2", "p", "a1"]),
            ids(&["pa1", "t1", "g1", "n1", "c1"]),
            ids(&["pr1", "n2"])
        ),
        "{ROW}: {what}: not in document order"
    );
    assert_eq!(
        doc.elements()
            .map(|found| found.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ids(&[
            "case", "a2", "p", "a1", "pa1", "t1", "g1", "n1", "pr1", "n2", "c1"
        ]),
        "{ROW}: {what}: elements() is not the document's pre-order"
    );
}

// =====================================================================================
// Item 2: an untyped artifactElement child, and the reference form
// =====================================================================================

#[test]
fn sacm23_art_001_an_untyped_artifact_element_child_is_missing_type_at_its_start_tag() {
    for (context, wrap, _) in contexts() {
        for (form, child) in [
            (
                "an untyped artifactElement",
                "<artifactElement xmi:id=\"x\"/>",
            ),
            (
                "an untyped artifactElement with a name",
                "<artifactElement xmi:id=\"x\"><name content=\"X\"/></artifactElement>",
            ),
            (
                "an untyped artifactElement without an xmi:id",
                "<artifactElement><name content=\"X\"/></artifactElement>",
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
        }
    }
}

#[test]
fn sacm23_art_001_a_reference_form_artifact_element_under_a_package_is_a_same_file_proxy() {
    // Under an ArtifactPackage, `artifactElement` is the composite role, so a
    // role element in reference form is a proxy (XMI 2.5.1 §7.10.1), not a
    // group member and not SACM-XMI-005. Since slice 0073a, `xmi:idref` and
    // `href="#t1"` make a same-file proxy (SACM23-XMI-030): `t1` is a nested
    // Technique, no root, so each is kept with `SACM-XMI-003/proxy-of-no-root`
    // at its start tag, typed or untyped, in both modes. Until then a strict
    // read refused it as not read yet, naming SACM23-XMI-019 for `xmi:idref`
    // and SACM23-XMI-029 for `href`. A proxy of a root moves the root into
    // place (`same_file_proxies.rs`).
    let sibling = typed("Technique", "t1", "T1");
    for (context, wrap, _) in contexts() {
        for (form, child) in [
            (
                "an untyped xmi:idref",
                format!("{MARK}<artifactElement xmi:idref=\"t1\"/>"),
            ),
            (
                "an untyped href=\"#t1\"",
                format!("{MARK}<artifactElement href=\"#t1\"/>"),
            ),
            (
                "a typed xmi:idref",
                format!("{MARK}<artifactElement xmi:type=\"sacm:Technique\" xmi:idref=\"t1\"/>"),
            ),
            (
                "a typed href=\"#t1\"",
                format!("{MARK}<artifactElement xmi:type=\"sacm:Technique\" href=\"#t1\"/>"),
            ),
        ] {
            let what = format!("{form} artifactElement in {context}");
            let template = wrap("", &format!("{sibling}{child}"));
            let doc = assert_preserved_at(&what, &template, "a");
            assert_eq!(
                doc.get("t1")
                    .and_then(SacmElement::parent)
                    .map(|parent| parent.id().as_str().to_owned()),
                Some("a".to_owned()),
                "{ROW}: {what}: t1 moved"
            );
            let (input, _) = one_mark(&what, &template);
            match read(Mode::Tolerant, &input) {
                Ok(output) => assert!(
                    output
                        .diagnostics()
                        .iter()
                        .all(|found| found.template() == "SACM-XMI-003/proxy-of-no-root"),
                    "{ROW}: {what}: not kept as a proxy of no root: {:?}",
                    output.diagnostics()
                ),
                Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
            }
        }
    }
}

// =====================================================================================
// Item 3: what an ArtifactPackage holds, and what it does not read yet
// =====================================================================================

/// Every kind this release reads typed in `artifactElement` (ADR 0014,
/// ADR 0034 and its amendments 1 to 3): the two readable packages of other
/// kinds, the nine argumentation kinds, the five artifact kinds of slice
/// 0052, the four `ArtifactAsset` kinds of slice 0055, the
/// `ArtifactAssetRelationship` of slice 0057, the five terminology kinds
/// of slice 0062, the four Argumentation and Artifact interface and binding
/// kinds of slice 0066 (SACM23-ARG-014, -015, SACM23-ART-012, -013) and the
/// four Terminology and Assurance Case ones of slice 0067 (SACM23-TERM-007,
/// -008, SACM23-AC-002, -003): every `ArtifactElement` kind.
const READABLE: [ElementKind; 34] = [
    ElementKind::AssuranceCasePackage,
    ElementKind::ArgumentPackage,
    ElementKind::Claim,
    ElementKind::ArgumentReasoning,
    ElementKind::ArtifactReference,
    ElementKind::ArgumentGroup,
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
    ElementKind::ArtifactPackage,
    ElementKind::Property,
    ElementKind::Technique,
    ElementKind::Participant,
    ElementKind::ArtifactGroup,
    ElementKind::Artifact,
    ElementKind::Resource,
    ElementKind::Activity,
    ElementKind::Event,
    ElementKind::ArtifactAssetRelationship,
    ElementKind::TerminologyPackage,
    ElementKind::Category,
    ElementKind::Term,
    ElementKind::Expression,
    ElementKind::TerminologyGroup,
    ElementKind::ArgumentPackageInterface,
    ElementKind::ArgumentPackageBinding,
    ElementKind::ArtifactPackageInterface,
    ElementKind::ArtifactPackageBinding,
    ElementKind::TerminologyPackageInterface,
    ElementKind::TerminologyPackageBinding,
    ElementKind::AssuranceCasePackageInterface,
    ElementKind::AssuranceCasePackageBinding,
];

#[test]
fn sacm23_art_001_every_readable_kind_is_read_typed_inside_an_artifact_package() {
    // The setup covers every ArtifactElement kind: since slice 0067 none is
    // left unread (this check was the unread-kinds test's until then).
    let artifact_elements = ElementKind::ALL
        .iter()
        .filter(|kind| kind.is_a(sacm::model::AbstractClass::ArtifactElement))
        .count();
    assert_eq!(
        READABLE.len(),
        artifact_elements,
        "{ROW}: setup: {} kinds listed, but {artifact_elements} are ArtifactElements",
        READABLE.len()
    );
    for (context, wrap, _) in contexts() {
        for kind in READABLE {
            let what = format!("the {kind} x in artifactElement of {context}");
            let doc = read_ok(&what, &wrap("", &typed(kind.name(), "x", "X")));
            assert_eq!(
                placed(&doc, "x"),
                (kind, Some("a".to_owned()), Some("X".to_owned())),
                "{ROW}: {what}: not a {kind} x named X in a"
            );
        }
        // All of them in one package, in document order.
        let what = format!("every readable kind in {context}");
        let body: String = READABLE
            .iter()
            .enumerate()
            .map(|(index, kind)| typed(kind.name(), &format!("x{index}"), "X"))
            .collect();
        let doc = read_ok(&what, &wrap("", &body));
        let expected: Vec<String> = (0..READABLE.len())
            .map(|index| format!("x{index}"))
            .collect();
        assert_eq!(
            children(&doc, "a"),
            expected,
            "{ROW}: {what}: not every kind, in document order"
        );
    }
    // A readable kind keeps its own content inside an ArtifactPackage: an
    // ArgumentPackage holding a Claim, and the utility roles of the package.
    let what = "an ArgumentPackage holding a Claim, and a description and a note on a";
    let doc = read_ok(
        what,
        &root_a(
            "",
            &format!(
                "<description xmi:id=\"d\"><content><value content=\"D\"/></content></description>\
                 <note xmi:id=\"n\"><content><value content=\"N\"/></content></note>\
                 <artifactElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p\">\
                 <name content=\"P\"/>{}</artifactElement>",
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/>\
                 </argumentElement>"
            ),
        ),
    );
    assert_eq!(
        (
            placed(&doc, "c1").0,
            children(&doc, "p"),
            children(&doc, "a"),
            element(&doc, "a").descriptions().count(),
            element(&doc, "a").notes().count()
        ),
        (
            ElementKind::Claim,
            ids(&["c1"]),
            ids(&["d", "n", "p"]),
            1,
            1
        ),
        "{ROW}: {what}: not read"
    );
}

#[test]
fn sacm23_art_012_the_interface_end_of_an_artifact_package_is_read_in_every_context() {
    // ptc/22-03-13 gives ArtifactPackage `interface : ArtifactPackageInterface
    // [0..*]`. This test refused it as `Interim(SACM23-ART-012)` until slice
    // 0066 read it: the attribute and the element forms now read equal with
    // no diagnostic, and a dangling id is held as read (SACM23-VAL-003
    // reports it).
    for (context, wrap, _) in contexts() {
        let attribute = format!("interface=\"x\" on {context}");
        let read_attribute = read_ok(&attribute, &wrap(" interface=\"x\"", ""));
        let element_form = format!("an interface element in {context}");
        let read_element = read_ok(&element_form, &wrap("", "<interface xmi:idref=\"x\"/>"));
        assert!(
            read_attribute == read_element,
            "SACM23-ART-012: {element_form} does not read == to {attribute}"
        );
        assert_eq!(
            element(&read_attribute, "a").interfaces(),
            [sacm::model::ElementId::from("x")],
            "SACM23-ART-012: {attribute}: interfaces()"
        );
        // In another namespace it is not that end: preserved.
        let what = format!("v:interface=\"x\" on {context}");
        assert_preserved_at(&what, &wrap(&format!(" {MARK}v:interface=\"x\""), ""), "a");
        let what = format!("a v:interface element in {context}");
        assert_preserved_at(
            &what,
            &wrap("", &format!("{MARK}<v:interface xmi:idref=\"x\"/>")),
            "a",
        );
    }
}

#[test]
fn sacm23_art_001_the_type_artifact_asserted_relationship_is_a_spelling_of_sacm23_xmi_009() {
    // ptc/22-03-13 spells ArtifactAssetRelationship
    // `ArtifactAssertedRelationship` (D-TEXT-MODEL). Since slice 0057
    // (SACM23-XMI-009, ADR 0034 amendment 2) a strict read keeps the child
    // whole as preserved content of the package, with one SACM-XMI-013 Error
    // at its start tag, where it was refused; a tolerant read reads it as an
    // ArtifactAssetRelationship, with one SACM-XMI-013 Warning there
    // affecting the child.
    for (context, wrap, _) in contexts() {
        let what = format!("an ArtifactAssertedRelationship in artifactElement of {context}");
        let (input, at) = one_mark(
            &what,
            &wrap(
                "",
                &format!(
                    "{MARK}<artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" \
                     xmi:id=\"r\"><name content=\"R\"/></artifactElement>"
                ),
            ),
        );
        for (mode, severity, affected, kind) in [
            (Mode::Strict, Severity::Error, "a", None),
            (
                Mode::Tolerant,
                Severity::Warning,
                "r",
                Some(ElementKind::ArtifactAssetRelationship),
            ),
        ] {
            let output = match read(mode, &input) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?}"),
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
                    Code::XMI_SPELLING_VARIANT,
                    severity,
                    Some(at),
                    vec![ElementId::from(affected)]
                )],
                "{ROW}: {what}: not one SACM-XMI-013 at the start tag affecting {affected} in \
                 {mode:?} mode"
            );
            let doc = output.into_document();
            assert_eq!(
                (
                    doc.get("r").map(SacmElement::kind),
                    element(&doc, "a").has_preserved_content()
                ),
                (kind, kind.is_none()),
                "{ROW}: {what}: read as the kind, or kept, in {mode:?} mode"
            );
        }
    }
}

#[test]
fn sacm23_art_001_an_unknown_abstract_or_foreign_type_in_artifact_element_is_preserved() {
    // Only the SACM-namespace class `ArtifactAssertedRelationship`, spelled
    // exactly so, is the model's spelling refused as a tolerance. A type
    // naming an abstract class, no class, a class in another namespace, or
    // the spelling in another case names no concrete class the role admits:
    // the child is kept whole as preserved content of the package, with one
    // SACM-XMI-003 at its start tag (ADR 0033).
    for (context, wrap, _) in contexts() {
        for type_value in [
            "sacm:ArtifactAsset",
            "sacm:Bogus",
            "v:ArtifactAssertedRelationship",
            "sacm:artifactAssertedRelationship",
        ] {
            let what = format!("an artifactElement typed {type_value} in {context}");
            let doc = assert_preserved_at(
                &what,
                &wrap(
                    "",
                    &format!(
                        "{}{MARK}<artifactElement xmi:type=\"{type_value}\" xmi:id=\"r\">\
                         <name content=\"R\"/></artifactElement>",
                        typed("Technique", "t1", "T1")
                    ),
                ),
                "a",
            );
            assert_eq!(
                (children(&doc, "a"), doc.get("r").is_none()),
                (ids(&["t1"]), true),
                "{ROW}: {what}: the preserved child was read as an element"
            );
        }
    }
}

#[test]
fn sacm23_art_001_the_model_spelling_is_preserved_where_the_role_admits_no_artifact_asset_relationship()
 {
    // The type `ArtifactAssertedRelationship` is refused only where the role
    // admits an ArtifactAssetRelationship (an ArtifactPackage's
    // `artifactElement`). Elsewhere it names no class the role admits, as
    // `sacm:ArtifactAssetRelationship` would not either: preserved by the
    // parent, with one SACM-XMI-003 at the child's start tag.
    let child = |role: &str| {
        format!(
            "{MARK}<{role} xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r\">\
             <name content=\"R\"/></{role}>"
        )
    };
    let cases = [
        (
            "in argumentElement of an ArgumentPackage root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {}\n\
                 </sacm:ArgumentPackage>",
                child("argumentElement")
            ),
            "p",
        ),
        (
            "in argumentElement of an ArgumentPackage in an AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  \
                 <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}</argumentPackage>\n\
                 </sacm:AssuranceCasePackage>",
                child("argumentElement")
            ),
            "p",
        ),
        (
            "in artifactPackage of an AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\n  {}\n\
                 </sacm:AssuranceCasePackage>",
                child("artifactPackage")
            ),
            "case",
        ),
        (
            "in property of a Technique",
            root_a(
                "",
                &format!(
                    "<artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"t1\">\
                     <name content=\"T1\"/>{}</artifactElement>",
                    child("property")
                ),
            ),
            "t1",
        ),
    ];
    for (what, template, holder) in &cases {
        let what = format!("an ArtifactAssertedRelationship {what}");
        let doc = assert_preserved_at(&what, template, holder);
        assert!(
            doc.get("r").is_none(),
            "{ROW}: {what}: the preserved child was read as an element"
        );
    }
}

#[test]
fn sacm23_art_001_an_artifact_element_attribute_on_a_package_is_preserved_at_the_attribute() {
    // `artifactElement` is ArtifactPackage's composite role, not a reference
    // end of it: as an attribute it is one the class lacks (ADR 0033), kept,
    // never read as a reference or a child.
    for (context, wrap, _) in contexts() {
        let what = format!("artifactElement=\"t1\" on {context}");
        let doc = assert_preserved_at(
            &what,
            &wrap(
                &format!(" {MARK}artifactElement=\"t1\""),
                &typed("Technique", "t1", "T1"),
            ),
            "a",
        );
        assert_eq!(
            (
                children(&doc, "a"),
                element(&doc, "a").group_members().len()
            ),
            (ids(&["t1"]), 0),
            "{ROW}: {what}: t1 is not the one child of a, or a has members"
        );
    }
}

// =====================================================================================
// Item 4: writing, the fixture and the golden file
// =====================================================================================

#[test]
fn sacm23_art_001_written_as_a_root_an_untyped_artifact_package_and_typed_artifact_elements() {
    let what = "an AssuranceCasePackage holding an ArtifactPackage with a nested one";
    let doc = read_ok(
        what,
        &in_case_a(
            "",
            &format!(
                "{}<artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"n\">\
                 <name content=\"N\"/>{}</artifactElement>",
                typed("Technique", "t1", "T1"),
                typed("Participant", "pa1", "Pa1")
            ),
        ),
    );
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            (
                line_with(&text, "a"),
                line_with(&text, "t1"),
                line_with(&text, "n"),
                line_with(&text, "pa1")
            ),
            (
                "<artifactPackage xmi:id=\"a\">",
                "<artifactElement xmi:type=\"sacm:Technique\" xmi:id=\"t1\">",
                "<artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"n\">",
                "<artifactElement xmi:type=\"sacm:Participant\" xmi:id=\"pa1\">"
            ),
            "{ROW}: {what}: not written as an untyped artifactPackage and typed artifactElements \
             in {mode:?} mode\n{text}"
        );
    }
    let what = "a bare ArtifactPackage root";
    let doc = read_ok(what, &root_a("", ""));
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            line_with(&text, "a"),
            "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
             xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\">",
            "{ROW}: {what}: not written as sacm:ArtifactPackage in {mode:?} mode\n{text}"
        );
    }
}

/// The pre-order of the fixture's elements.
const FIXTURE_ORDER: [&str; 10] = [
    "case",
    "evidence",
    "fta",
    "fta_q",
    "assessor",
    "analysis",
    "library",
    "calibrated",
    "shared",
    "supplier",
];

#[test]
fn sacm23_art_001_the_fixture_reads_in_both_modes_with_no_diagnostic_and_validates() {
    let doc = read_ok("the fixture", &fixture_text());
    assert_eq!(
        doc.roots()
            .map(|root| root.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ids(&["case", "shared"]),
        "{ROW}: the fixture's roots"
    );
    assert_eq!(
        doc.elements()
            .map(|found| found.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ids(&FIXTURE_ORDER),
        "{ROW}: the fixture's pre-order"
    );
    for (id, kind, parent) in [
        ("case", ElementKind::AssuranceCasePackage, None),
        ("evidence", ElementKind::ArtifactPackage, Some("case")),
        ("fta", ElementKind::Technique, Some("evidence")),
        ("fta_q", ElementKind::Property, Some("fta")),
        ("assessor", ElementKind::Participant, Some("evidence")),
        ("analysis", ElementKind::ArtifactGroup, Some("evidence")),
        ("library", ElementKind::ArtifactPackage, Some("evidence")),
        ("calibrated", ElementKind::Property, Some("library")),
        ("shared", ElementKind::ArtifactPackage, None),
        ("supplier", ElementKind::Participant, Some("shared")),
    ] {
        let found = element(&doc, id);
        assert_eq!(
            (
                found.kind(),
                found.parent().map(|p| p.id().as_str().to_owned())
            ),
            (kind, parent.map(str::to_owned)),
            "{ROW}: the fixture's {id}"
        );
    }
    assert_eq!(
        (
            element(&doc, "analysis")
                .group_members()
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>(),
            children(&doc, "analysis"),
            children(&doc, "fta"),
            children(&doc, "library")
        ),
        (
            ids(&["fta", "assessor"]),
            Vec::new(),
            ids(&["fta_q"]),
            ids(&["calibrated"])
        ),
        "{ROW}: the fixture's group members and children"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture does not validate cleanly"
    );
}

#[test]
fn sacm23_art_001_the_fixture_writes_exactly_the_golden_file_which_reads_back_equal() {
    let doc = read_ok("the fixture", &fixture_text());
    let golden = golden_text();
    for mode in MODES {
        let text = write_in("the fixture", &doc, mode);
        assert_eq!(
            text, golden,
            "{ROW}: the fixture does not write the golden file's bytes in {mode:?} mode"
        );
    }
    let back = read_ok("the golden file", &golden);
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: the golden file does not read back equal to the fixture"
    );
    assert!(
        back == doc,
        "{ROW}: the golden file does not read back == to the fixture"
    );
    for mode in MODES {
        assert_eq!(
            write_in("the golden file", &back, mode),
            golden,
            "{ROW}: writing the golden file again gives other bytes in {mode:?} mode"
        );
    }
}

/// The fixture with `from` replaced by `to`, which must change it.
fn fixture_with(from: &str, to: &str) -> String {
    let text = fixture_text();
    assert!(
        text.contains(from),
        "{ROW}: setup: the fixture has no {from:?}"
    );
    text.replacen(from, to, 1)
}

#[test]
fn sacm23_art_001_fixture_variant_g_an_untyped_artifact_element_is_missing_type() {
    let assessor = "      <artifactElement xmi:type=\"sacm:Participant\" xmi:id=\"assessor\">";
    // Variants (a) to (c) sampled kinds and ends not read yet, the last of
    // which slice 0067 reads (SACM23-TERM-007, -008, SACM23-AC-002): only
    // (g) is left.
    // (g) An untyped artifactElement: SACM-XMI-005.
    let what = "variant (g): an untyped artifactElement in evidence";
    let (input, at) = one_mark(
        what,
        &fixture_with(
            assessor,
            &format!("      {MARK}<artifactElement xmi:id=\"x\"/>\n{assessor}"),
        ),
    );
    let error = refused(what, &input);
    assert_eq!(
        (error.code(), error.location()),
        (Code::XMI_MISSING_TYPE, Some(at)),
        "{ROW}: {what}: not SACM-XMI-005 at the start tag: {error:?}"
    );
}

// =====================================================================================
// Item 6: deleting an ArtifactPackage
// =====================================================================================

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, and returns why.
fn refuse_delete(doc: &mut Document, what: &str, operation: DeleteElement) -> EditError {
    let before = doc.clone();
    let error = match edit::apply(doc, operation) {
        Ok(mutation) => panic!(
            "{ROW}: {what}: accepted, deleting {:?}",
            mutation.deleted_ids()
        ),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: {what}: the refusal changed the document"
    );
    error
}

#[test]
fn sacm23_art_001_deleting_a_non_empty_artifact_package_is_not_empty_unless_recursive() {
    let mut doc = read_ok("the fixture", &fixture_text());
    for (id, contained) in [("library", 1), ("evidence", 6), ("shared", 1)] {
        let what = format!("deleting {id}");
        let error = refuse_delete(&mut doc, &what, DeleteElement::new(id));
        assert!(
            matches!(
                &error,
                EditError::NotEmpty { element, contained: count, .. }
                    if element.as_str() == id && *count == contained
            ),
            "{ROW}: {what}: not NotEmpty {{ {id}, contained: {contained} }}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_PACKAGE_NOT_EMPTY,
            "{ROW}: {what}: not SACM-CMD-006"
        );
    }
    // Recursively: the package and its subtree, in pre-order.
    let recursive = |id: &str| {
        DeleteElement::new(id).with_package_policy(PackageDeletePolicy::DeleteRecursively)
    };
    let mutation = match edit::apply(&mut doc, recursive("library")) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: a recursive delete of library was refused: {error}"),
    };
    assert_eq!(
        (
            mutation.deleted_ids(),
            mutation.created_ids(),
            children(&doc, "evidence")
        ),
        (
            [ElementId::from("library"), ElementId::from("calibrated")].as_slice(),
            [].as_slice(),
            ids(&["fta", "assessor", "analysis"])
        ),
        "{ROW}: a recursive delete of library"
    );
    let mutation = match edit::apply(&mut doc, recursive("evidence")) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: a recursive delete of evidence was refused: {error}"),
    };
    assert_eq!(
        mutation.deleted_ids(),
        ["evidence", "fta", "fta_q", "assessor", "analysis"]
            .map(ElementId::from)
            .as_slice(),
        "{ROW}: a recursive delete of evidence: its subtree in pre-order"
    );
    assert_eq!(
        doc.elements()
            .map(|found| found.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ids(&["case", "shared", "supplier"]),
        "{ROW}: what is left after the recursive deletes"
    );
    // A package holding only utility elements is not refused.
    let what = "an ArtifactPackage holding only a note";
    let mut doc = read_ok(
        what,
        &root_a(
            "",
            "<note xmi:id=\"n\"><content><value content=\"N\"/></content></note>",
        ),
    );
    match edit::apply(&mut doc, DeleteElement::new("a")) {
        Ok(mutation) => assert_eq!(
            (mutation.deleted_ids(), doc.element_count()),
            ([ElementId::from("a"), ElementId::from("n")].as_slice(), 0),
            "{ROW}: {what}: not deleted with its note"
        ),
        Err(error) => panic!("{ROW}: {what}: refused: {error:?}"),
    }
}
