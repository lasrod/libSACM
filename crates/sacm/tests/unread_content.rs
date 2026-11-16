//! Tests that a tolerant read keeps conforming content this release does
//! not read yet, and content outside every element, as preserved content
//! with a Warning (LIB-API-011, LIB-COMPAT-003; ADR 0035 D1-D3; ADR 0033
//! amendment 1; the maintainer, 2026-11-16, Option B).
//!
//! A strict read refuses each input here as `Unsupported`
//! (`interim_refusals.rs`, unchanged). A tolerant read keeps it at its
//! construct, with one Warning at the strict refusal's location:
//! SACM-XMI-014 for conforming content a later row reads, its message naming
//! that row; SACM-XMI-007 for a cross-document `href`; SACM-XMI-003 for
//! content on `xmi:XMI` and for a preserved attribute whose `sacm` or `xmi`
//! prefix is bound elsewhere. Content on or under `xmi:XMI`, and a root of an
//! unread kind, is held by the document outside its elements:
//! `Document::has_preserved_content` and
//! `Difference::DocumentPreservedContent` expose it.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{self, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateClaim, DeleteElement, EditError, ReorderElements,
};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const API_011: &str = "LIB-API-011";
const COMPAT_003: &str = "LIB-COMPAT-003";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm`, `xmi`, `xsi` and a vendor
/// `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmlns:v=\"urn:v\"",
);

/// The declaration of `xsi` alone.
const XSI: &str = "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

// --- Inputs ------------------------------------------------------------------------

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// A root ArtifactPackage `a` holding `body`.
fn artifact_package(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"a\"><name content=\"a\"/>{body}</sacm:ArtifactPackage>"
    )
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// A root of the class `kind`, `x`, named.
fn root_of(kind: &str) -> String {
    format!("<sacm:{kind}{NS} xmi:id=\"x\"><name content=\"x\"/></sacm:{kind}>")
}

/// The `xmi:XMI` wrapper with `attributes`, holding `body` and then the root
/// ArgumentPackage `p`.
fn wrapper(attributes: &str, body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:v=\"urn:v\"{attributes}>\
         {body}<sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>\
         </xmi:XMI>"
    )
}

// --- Reading and writing -------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

fn tolerant(row: &str, what: &str, input: &str) -> ReadOutput {
    match read(Mode::Tolerant, input) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?} ({error})"),
    }
}

fn strict_refusal(row: &str, what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{row}: {what}: setup: a strict read returned a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

fn write(doc: &Document, mode: Mode) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(mode))
}

fn write_tolerant(row: &str, what: &str, doc: &Document) -> String {
    match write(doc, Mode::Tolerant) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a tolerant write refused: {error:?} ({error})"),
    }
}

/// Writes `doc` tolerantly, reads the output back and writes that again:
/// equal, and byte-identical. Returns the output.
fn assert_round_trips(row: &str, what: &str, doc: &Document) -> String {
    let text = write_tolerant(row, what, doc);
    let back = tolerant(row, &format!("{what}, written"), &text).into_document();
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the tolerant output does not read back equal\n{text}"
    );
    assert!(
        back == *doc,
        "{row}: {what}: the tolerant output reads back !=\n{text}"
    );
    assert_eq!(
        write_tolerant(row, what, &back),
        text,
        "{row}: {what}: writing the read-back again is not byte-identical"
    );
    text
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().map(|&value| ElementId::from(value)).collect()
}

fn with_code(output: &ReadOutput, code: Code) -> Vec<&Diagnostic> {
    output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == code)
        .collect()
}

/// The ids of the elements that hold preserved content.
fn holders(doc: &Document) -> Vec<String> {
    doc.elements()
        .filter(|element| element.has_preserved_content())
        .map(|element| element.id().as_str().to_owned())
        .collect()
}

/// The location of the first `needle` in `text`, whose lines end at LF.
fn position_of(row: &str, text: &str, needle: &str) -> SourceLocation {
    let Some(offset) = text.find(needle) else {
        panic!("{row}: setup: {needle:?} is not in the input");
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

/// The byte offset of `needle` in `text`.
fn at(row: &str, text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{row}: {needle:?} is not written\n{text}"))
}

/// Asserts that `needles` are written in this order.
fn assert_written_in_order(row: &str, what: &str, text: &str, needles: &[&str]) {
    let offsets: Vec<usize> = needles.iter().map(|needle| at(row, text, needle)).collect();
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{row}: {what}: not written in the order {needles:?}\n{text}"
    );
}

// =====================================================================================
// LIB-API-011: every kept entry, in tolerant mode
// =====================================================================================

/// One input of an `Unread` entry a tolerant read keeps.
struct Kept {
    entry: &'static str,
    what: &'static str,
    input: String,
    /// The Warning's code.
    code: Code,
    /// For SACM-XMI-014, the row its message names.
    row: Option<&'static str>,
    /// The element that holds the content, or `None` for the document.
    holder: Option<&'static str>,
}

fn kept(
    entry: &'static str,
    what: &'static str,
    input: String,
    row: &'static str,
    holder: Option<&'static str>,
) -> Kept {
    Kept {
        entry,
        what,
        input,
        code: Code::XMI_UNREAD_CONTENT,
        row: Some(row),
        holder,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn kept_entries() -> Vec<Kept> {
    let mut cases = Vec::new();
    for (kind, row) in [
        ("ArtifactPackageInterface", "SACM23-ART-012"),
        ("ArtifactPackageBinding", "SACM23-ART-013"),
        ("TerminologyPackage", "SACM23-TERM-001"),
        ("TerminologyPackageInterface", "SACM23-TERM-007"),
        ("TerminologyPackageBinding", "SACM23-TERM-008"),
        ("AssuranceCasePackageInterface", "SACM23-AC-002"),
        ("AssuranceCasePackageBinding", "SACM23-AC-003"),
        ("ArgumentPackageInterface", "SACM23-ARG-014"),
        ("ArgumentPackageBinding", "SACM23-ARG-015"),
    ] {
        cases.push(kept(kind, "as the root", root_of(kind), row, None));
    }
    cases.push(kept(
        "ArgumentPackageInterface",
        "in an ArgumentPackage",
        package(
            "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\">\
             <name content=\"i\"/></argumentElement>",
        ),
        "SACM23-ARG-014",
        Some("p"),
    ));
    cases.push(kept(
        "TerminologyPackage",
        "in an AssuranceCasePackage",
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
             <terminologyPackage xmi:id=\"f\"><name content=\"f\"/></terminologyPackage>\
             </sacm:AssuranceCasePackage>"
        ),
        "SACM23-TERM-001",
        Some("case"),
    ));
    for (kind, row) in [
        ("Artifact", "SACM23-ART-004"),
        ("Resource", "SACM23-ART-005"),
        ("Activity", "SACM23-ART-006"),
        ("Event", "SACM23-ART-007"),
        ("ArtifactAssetRelationship", "SACM23-ART-010"),
        ("Category", "SACM23-TERM-002"),
        ("Term", "SACM23-TERM-004"),
        ("Expression", "SACM23-TERM-005"),
        ("TerminologyGroup", "SACM23-TERM-006"),
    ] {
        cases.push(kept(
            kind,
            "in an ArtifactPackage",
            artifact_package(&format!(
                "<artifactElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"><name content=\"x\"/>\
                 </artifactElement>"
            )),
            row,
            Some("a"),
        ));
    }
    // A package's interface end (SACM23-AC-002, SACM23-ARG-014,
    // SACM23-ART-012).
    cases.push(kept(
        "interface",
        "the attribute on an ArgumentPackage",
        package("").replace("xmi:id=\"p\">", "xmi:id=\"p\" interface=\"i\">"),
        "SACM23-ARG-014",
        Some("p"),
    ));
    cases.push(kept(
        "interface",
        "the element on an ArgumentPackage",
        package("<interface xmi:idref=\"i\"/>"),
        "SACM23-ARG-014",
        Some("p"),
    ));
    cases.push(kept(
        "interface",
        "the attribute on an AssuranceCasePackage",
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\" interface=\"i\">\
             <name content=\"case\"/></sacm:AssuranceCasePackage>"
        ),
        "SACM23-AC-002",
        Some("case"),
    ));
    cases.push(kept(
        "interface",
        "the attribute on an ArtifactPackage",
        artifact_package("").replace("xmi:id=\"a\">", "xmi:id=\"a\" interface=\"i\">"),
        "SACM23-ART-012",
        Some("a"),
    ));
    // XMI document metadata (SACM23-XMI-019).
    let metadata = "SACM23-XMI-019";
    for (what, input, holder) in [
        (
            "xmi:uuid on an object",
            package(&claim("c1", " xmi:uuid=\"u1\"", "")),
            Some("c1"),
        ),
        (
            "xmi:label on an object",
            package(&claim("c1", " xmi:label=\"l\"", "")),
            Some("c1"),
        ),
        (
            "href on an object (the root)",
            package(&claim("c2", "", "")).replace("xmi:id=\"p\">", "xmi:id=\"p\" href=\"#c2\">"),
            Some("p"),
        ),
        (
            "xmi:Extension in an object",
            package(&claim("c1", "", "<xmi:Extension extender=\"v\"/>")),
            Some("c1"),
        ),
        (
            "xmi:Documentation in the wrapper",
            wrapper("", "<xmi:Documentation/>"),
            None,
        ),
        ("xmi:id on the wrapper", wrapper(" xmi:id=\"w\"", ""), None),
        (
            "xmi:type on the wrapper",
            wrapper(" xmi:type=\"xmi:XMI\"", ""),
            None,
        ),
        (
            "xmi:uuid on the wrapper",
            wrapper(" xmi:uuid=\"u\"", ""),
            None,
        ),
        ("href on the wrapper", wrapper(" href=\"#p\"", ""), None),
        (
            "xsi:schemaLocation on the wrapper",
            wrapper(
                &format!(" {XSI} xsi:schemaLocation=\"{SACM_NS} SACM.xsd\""),
                "",
            ),
            None,
        ),
        (
            "xmi:id on a reference element",
            package(&format!(
                "{}{}",
                claim("c1", "", "<metaClaim xmi:idref=\"c2\" xmi:id=\"r\"/>"),
                claim("c2", "", "")
            )),
            Some("c1"),
        ),
        (
            "xsi:schemaLocation on the root",
            package("").replace(
                "xmi:id=\"p\">",
                &format!("xmi:id=\"p\" xsi:schemaLocation=\"{SACM_NS} SACM.xsd\">"),
            ),
            Some("p"),
        ),
    ] {
        cases.push(kept("XmiMetadata", what, input, metadata, holder));
    }
    // ExpressionLangString (SACM23-BASE-009): the whole part is kept.
    cases.push(kept(
        "ExpressionLangString",
        "as a name",
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
             <name xmi:type=\"sacm:ExpressionLangString\" content=\"p\"/></sacm:ArgumentPackage>"
        ),
        "SACM23-BASE-009",
        Some("p"),
    ));
    cases.push(kept(
        "ExpressionLangString",
        "as a value of a content",
        package(&claim(
            "c1",
            "",
            "<note xmi:id=\"n\"><content><value xmi:type=\"sacm:ExpressionLangString\" \
             content=\"v\"/></content></note>",
        )),
        "SACM23-BASE-009",
        Some("n"),
    ));
    // The spellings SACM23-XMI-009 is to read: SACM-XMI-014 until it is
    // built (lead correction, 2026-11-16).
    cases.push(kept(
        "TextSpelling",
        "argumentationElement in an ArgumentPackage",
        package(
            "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
             </argumentationElement>",
        ),
        "SACM23-XMI-009",
        Some("p"),
    ));
    cases.push(kept(
        "TextSpelling",
        "argumentationElement in an ArgumentGroup",
        package(
            "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\"><name content=\"g\"/>\
             <argumentationElement xmi:idref=\"c1\"/></argumentElement>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
             </argumentElement>",
        ),
        "SACM23-XMI-009",
        Some("g"),
    ));
    cases.push(kept(
        "ModelSpelling",
        "ArtifactAssertedRelationship in an ArtifactPackage",
        artifact_package(
            "<artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r\">\
             <name content=\"r\"/></artifactElement>",
        ),
        "SACM23-XMI-009",
        Some("a"),
    ));
    // A cross-document href: SACM-XMI-007 (SACM23-XMI-027).
    cases.push(Kept {
        entry: "CrossDocumentHref",
        what: "href to another document",
        input: package(&claim("c1", "", "<metaClaim href=\"other.sacm.xmi#c\"/>")),
        code: Code::XMI_EXTERNAL_HREF,
        row: None,
        holder: Some("c1"),
    });
    // Defects with nowhere to be kept before: SACM-XMI-003.
    for (what, input) in [
        (
            "an unqualified attribute on the wrapper",
            wrapper(" foo=\"bar\"", ""),
        ),
        (
            "a vendor attribute on the wrapper",
            wrapper(" v:w=\"1\"", ""),
        ),
        ("text before the root", wrapper("", "stray")),
    ] {
        cases.push(Kept {
            entry: "WrapperContent",
            what,
            input,
            code: Code::XMI_UNKNOWN_CONTENT,
            row: None,
            holder: None,
        });
    }
    cases.push(Kept {
        entry: "UnwritablePrefix",
        what: "an attribute whose prefix sacm is bound to another URI",
        input: package(&format!(
            "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
             xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/>\
             </argumentElement>"
        )),
        code: Code::XMI_UNKNOWN_CONTENT,
        row: None,
        holder: Some("c1"),
    });
    cases
}

/// Each entry `Unread::tolerant()` keeps reads tolerantly with one Warning
/// of its code at the strict refusal's location, affecting its holder (none
/// for the document), naming its row; the content is kept, writes back and
/// makes a strict write `PreservedContent`.
#[test]
fn lib_api_011_each_kept_entry_reads_tolerantly_with_its_code_and_row() {
    assert_eq!(
        (
            Code::XMI_UNREAD_CONTENT.as_str(),
            Code::XMI_EXTERNAL_HREF.as_str()
        ),
        ("SACM-XMI-014", "SACM-XMI-007"),
        "{API_011}: the new codes"
    );
    let mut failures = Vec::new();
    for case in kept_entries() {
        let what = format!("{}: {}", case.entry, case.what);
        let strict = strict_refusal(API_011, &what, &case.input);
        assert!(
            matches!(strict, ReadError::Unsupported { .. }),
            "{API_011}: {what}: setup: the strict refusal is not Unsupported: {strict:?}"
        );
        let output = match read(Mode::Tolerant, &case.input) {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("{what}: a tolerant read refused it: {error:?}"));
                continue;
            }
        };
        let affected: Vec<ElementId> = case.holder.map(|id| ids(&[id])).unwrap_or_default();
        match with_code(&output, case.code).as_slice() {
            [one]
                if one.severity() == Severity::Warning
                    && one.location() == strict.location()
                    && one.affected() == affected.as_slice() =>
            {
                if let Some(row) = case.row {
                    let message = one.message();
                    if !message.contains(&format!("; {row} reads it"))
                        || message.contains("not SACM 2.3")
                    {
                        failures.push(format!(
                            "{what}: the message does not end naming {row}, or calls it \
                             defective: {message:?}"
                        ));
                    }
                }
            }
            other => failures.push(format!(
                "{what}: not one {} Warning at {:?} affecting {affected:?}: {other:?}",
                case.code,
                strict.location()
            )),
        }
        if case.code != Code::XMI_UNKNOWN_CONTENT
            && !with_code(&output, Code::XMI_UNKNOWN_CONTENT).is_empty()
        {
            failures.push(format!("{what}: conforming content is called SACM-XMI-003"));
        }
        if output.diagnostics().len() != 1 || diagnostic::has_errors(output.diagnostics()) {
            failures.push(format!(
                "{what}: not the one Warning: {:?}",
                output.diagnostics()
            ));
        }
        let doc = output.into_document();
        let expected_holders: Vec<String> = case.holder.into_iter().map(str::to_owned).collect();
        if !doc.has_preserved_content() || holders(&doc) != expected_holders {
            failures.push(format!(
                "{what}: the document's preserved content is not held by {:?}: {:?}",
                case.holder,
                holders(&doc)
            ));
        }
        assert_round_trips(API_011, &what, &doc);
        match write(&doc, Mode::Strict) {
            Err(WriteError::PreservedContent { elements, .. }) if elements == affected => {}
            other => failures.push(format!(
                "{what}: the strict write is not PreservedContent {affected:?}: {other:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{API_011}: {} kept entries are not kept as stated:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A cross-document `href`, and the reference form of a composite, are kept
/// with SACM-XMI-007, never SACM-XMI-003; the reference end stays empty.
#[test]
fn lib_api_011_a_cross_document_href_is_xmi_007_and_never_xmi_003() {
    let cases = [
        (
            "metaClaim href to another document",
            package(&claim("c1", "", "<metaClaim href=\"other.sacm.xmi#c\"/>")),
            "c1",
        ),
        (
            "XLink attributes on a metaClaim element",
            package(&format!(
                "{}{}",
                claim(
                    "c1",
                    "",
                    "<metaClaim href=\"#c2\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
                     xlink:type=\"simple\"/>"
                ),
                claim("c2", "", "")
            )),
            "c1",
        ),
        (
            "an untyped role element with an href to another document",
            package("<argumentElement href=\"other.sacm.xmi#c9\"/>"),
            "p",
        ),
        (
            "a typed role element with an href",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" href=\"#c2\"/>{}",
                claim("c2", "", "")
            )),
            "p",
        ),
    ];
    for (what, input, holder) in &cases {
        let strict = strict_refusal(API_011, what, input);
        let output = tolerant(API_011, what, input);
        let found: Vec<(Code, Severity, Vec<ElementId>, Option<SourceLocation>)> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.affected().to_vec(),
                    found.location(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_EXTERNAL_HREF,
                Severity::Warning,
                ids(&[holder]),
                strict.location()
            )],
            "{API_011}: {what}: not one SACM-XMI-007 Warning affecting {holder} (and no \
             SACM-XMI-003)"
        );
        let doc = output.into_document();
        let held = doc.get(*holder);
        assert!(
            held.is_some_and(SacmElement::has_preserved_content),
            "{API_011}: {what}: {holder} does not hold the href"
        );
        assert!(
            doc.get("c1").is_none_or(|c1| c1.meta_claims().is_empty()),
            "{API_011}: {what}: a metaClaim is read from it"
        );
        let text = assert_round_trips(API_011, what, &doc);
        assert!(
            text.contains("href=\""),
            "{API_011}: {what}: the href is not written back\n{text}"
        );
    }
}

// =====================================================================================
// LIB-COMPAT-003: content outside every element
// =====================================================================================

/// Attributes, XMI-namespace children and text of `xmi:XMI` are held by the
/// document, each with its Warning at its construct, and written back in
/// place among the roots; the writer wraps the output in `xmi:XMI` for it.
#[test]
fn lib_compat_003_wrapper_content_is_document_level_preserved_content_in_place() {
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" {XSI} xmlns:v=\"urn:v\" \
         v:w=\"1\" xsi:schemaLocation=\"{SACM_NS} SACM.xsd\">\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage>\n\
         \x20 <xmi:Documentation exporter=\"tool\"/>\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage>\n\
         </xmi:XMI>\n"
    );
    let what = "a wrapper with a vendor attribute, xsi:schemaLocation and xmi:Documentation";
    let output = tolerant(COMPAT_003, what, &input);
    let mut found: Vec<(Code, Severity, Vec<ElementId>, Option<SourceLocation>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.affected().to_vec(),
                found.location(),
            )
        })
        .collect();
    found.sort_by_key(|entry| entry.3);
    assert_eq!(
        found,
        [
            (
                Code::XMI_UNKNOWN_CONTENT,
                Severity::Warning,
                vec![],
                Some(position_of(COMPAT_003, &input, "v:w="))
            ),
            (
                Code::XMI_UNREAD_CONTENT,
                Severity::Warning,
                vec![],
                Some(position_of(COMPAT_003, &input, "xsi:schemaLocation="))
            ),
            (
                Code::XMI_UNREAD_CONTENT,
                Severity::Warning,
                vec![],
                Some(position_of(COMPAT_003, &input, "<xmi:Documentation"))
            ),
        ],
        "{COMPAT_003}: {what}: not one Warning at each construct, affecting no element"
    );
    let doc = output.into_document();
    let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(roots, ["a", "b"], "{COMPAT_003}: {what}: the roots");
    assert!(
        doc.has_preserved_content() && holders(&doc).is_empty(),
        "{COMPAT_003}: {what}: the content is not held by the document alone"
    );
    let text = assert_round_trips(COMPAT_003, what, &doc);
    let Some(start) = text.lines().nth(1) else {
        panic!("{COMPAT_003}: {what}: the output has no second line\n{text}");
    };
    assert!(
        start.starts_with("<xmi:XMI ")
            && start.contains("v:w=\"1\"")
            && start.contains("xsi:schemaLocation="),
        "{COMPAT_003}: {what}: the wrapper does not carry its attributes: {start}"
    );
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &["xmi:id=\"a\"", "<xmi:Documentation", "xmi:id=\"b\""],
    );
    assert!(
        matches!(
            write(&doc, Mode::Strict),
            Err(WriteError::PreservedContent { elements, .. }) if elements.is_empty()
        ),
        "{COMPAT_003}: {what}: the strict write is not PreservedContent with no element"
    );

    // Text, and one root: the writer still emits xmi:XMI.
    let what = "text before the one root";
    let doc = tolerant(COMPAT_003, what, &wrapper("", "stray")).into_document();
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert!(
        text.contains("<xmi:XMI ")
            && at(COMPAT_003, &text, "stray") < at(COMPAT_003, &text, "xmi:id=\"p\""),
        "{COMPAT_003}: {what}: the text is not written in the wrapper before the root\n{text}"
    );
}

/// A root of a kind not read yet loads as a document with no elements, its
/// ids in the id space; a tolerant write wraps it, and a strict write is
/// `PreservedContent`, not `NoRoot`.
#[test]
fn lib_compat_003_a_bare_unread_root_loads_empty_and_writes_wrapped() {
    let what = "a bare TerminologyPackage root";
    let input = format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"t\"><name content=\"T\"/></sacm:TerminologyPackage>"
    );
    let strict = strict_refusal(COMPAT_003, what, &input);
    let output = tolerant(COMPAT_003, what, &input);
    let found: Vec<(Code, Option<SourceLocation>)> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.location()))
        .collect();
    assert_eq!(
        found,
        [(Code::XMI_UNREAD_CONTENT, strict.location())],
        "{COMPAT_003}: {what}: not one SACM-XMI-014 at the root"
    );
    let mut doc = output.into_document();
    assert!(
        doc.element_count() == 0 && doc.roots().next().is_none() && doc.has_preserved_content(),
        "{COMPAT_003}: {what}: not an empty document holding the root"
    );
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert!(
        text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xmi:XMI ")
            && text.contains("<sacm:TerminologyPackage"),
        "{COMPAT_003}: {what}: the output is not wrapped\n{text}"
    );
    assert!(
        matches!(
            write(&doc, Mode::Strict),
            Err(WriteError::PreservedContent { elements, .. }) if elements.is_empty()
        ),
        "{COMPAT_003}: {what}: the strict write is not PreservedContent with no element"
    );
    // Its id is in the id space.
    let answer = edit::apply(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("t"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "t"),
        "{COMPAT_003}: {what}: with_id(\"t\") is not DuplicateId: {answer:?}"
    );
    // A created root is written after it.
    edit::apply(
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    )
    .unwrap_or_else(|error| panic!("{COMPAT_003}: {what}: CreateArgumentPackage: {error:?}"));
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &["<sacm:TerminologyPackage", "xmi:id=\"p\""],
    );
}

/// Root create, delete and reorder move document-level content as ADR 0033
/// moves a preserved child: it keeps its index among the roots.
#[test]
fn lib_compat_003_document_level_content_moves_with_root_edits() {
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
         <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage>\
         <xmi:Documentation/>\
         <sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"c\"><name content=\"C\"/></sacm:ArgumentPackage>\
         </xmi:XMI>"
    );
    let read_doc = || tolerant(COMPAT_003, "a, D, b, c", &input).into_document();
    let doc = read_doc();
    let text = write_tolerant(COMPAT_003, "as read", &doc);
    assert_written_in_order(
        COMPAT_003,
        "as read",
        &text,
        &[
            "xmi:id=\"a\"",
            "<xmi:Documentation",
            "xmi:id=\"b\"",
            "xmi:id=\"c\"",
        ],
    );
    let apply = |doc: &mut Document, what: &str, operation: edit::Operation| {
        if let Err(error) = edit::apply(doc, operation) {
            panic!("{COMPAT_003}: {what}: refused: {error:?}");
        }
    };

    let what = "after deleting a, before it";
    let mut doc = read_doc();
    apply(&mut doc, what, DeleteElement::new("a").into());
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &["<xmi:Documentation", "xmi:id=\"b\"", "xmi:id=\"c\""],
    );

    let what = "after deleting b, after it";
    let mut doc = read_doc();
    apply(&mut doc, what, DeleteElement::new("b").into());
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &["xmi:id=\"a\"", "<xmi:Documentation", "xmi:id=\"c\""],
    );

    let what = "after reordering the roots to c, b, a";
    let mut doc = read_doc();
    apply(
        &mut doc,
        what,
        ReorderElements::roots(["c", "b", "a"]).into(),
    );
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &[
            "xmi:id=\"c\"",
            "<xmi:Documentation",
            "xmi:id=\"b\"",
            "xmi:id=\"a\"",
        ],
    );

    let what = "after creating the root d";
    let mut doc = read_doc();
    apply(
        &mut doc,
        what,
        CreateArgumentPackage::root(LangString::new("D"))
            .with_id("d")
            .into(),
    );
    let text = assert_round_trips(COMPAT_003, what, &doc);
    assert_written_in_order(
        COMPAT_003,
        what,
        &text,
        &[
            "xmi:id=\"a\"",
            "<xmi:Documentation",
            "xmi:id=\"b\"",
            "xmi:id=\"c\"",
            "xmi:id=\"d\"",
        ],
    );
}

/// Document-level preserved content that differs, or is on one side only,
/// is one `Difference::DocumentPreservedContent`, about no element.
#[test]
fn lib_compat_003_document_preserved_content_is_one_difference() {
    let read_doc = |what: &str, input: &str| tolerant(COMPAT_003, what, input).into_document();
    let one = read_doc("v:w=\"1\"", &wrapper(" v:w=\"1\"", ""));
    let two = read_doc("v:w=\"2\"", &wrapper(" v:w=\"2\"", ""));
    let none = read_doc("no wrapper content", &wrapper("", ""));
    assert!(
        !none.has_preserved_content() && one.has_preserved_content(),
        "{COMPAT_003}: has_preserved_content is not false for a clean read and true for one \
         with wrapper content"
    );
    assert_eq!(
        semantic_compare(&one, &one.clone()),
        [],
        "{COMPAT_003}: a document differs from its clone"
    );
    for (what, left, right) in [
        ("values that differ", &one, &two),
        ("content on the left only", &one, &none),
        ("content on the right only", &none, &two),
    ] {
        let differences = semantic_compare(left, right);
        assert_eq!(
            differences,
            [Difference::DocumentPreservedContent],
            "{COMPAT_003}: {what}: not one DocumentPreservedContent"
        );
        assert!(left != right, "{COMPAT_003}: {what}: the documents are ==");
        let difference = &differences[0];
        assert_eq!(
            difference.element_id(),
            None,
            "{COMPAT_003}: {what}: the difference names an element"
        );
        let shown = difference.to_string();
        assert!(
            !shown.is_empty() && !shown.contains('\n'),
            "{COMPAT_003}: {what}: the Display is not one line: {shown:?}"
        );
    }
    // Moving the content among the roots is a difference too.
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
         <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage>\
         <xmi:Documentation/>\
         <sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage>\
         </xmi:XMI>"
    );
    let before = read_doc("a, D, b", &input);
    let moved = read_doc(
        "a, b, D",
        &input.replace(
            "<xmi:Documentation/><sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage>",
            "<sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage><xmi:Documentation/>",
        ),
    );
    assert_eq!(
        semantic_compare(&before, &moved),
        [Difference::DocumentPreservedContent],
        "{COMPAT_003}: content moved among the roots is not one DocumentPreservedContent"
    );
}

/// With every root deleted, a document whose preserved content outside its
/// elements is only XMI metadata holds no SACM package, so both writes are
/// `NoRoot` (an `xmi:XMI` holding no package is `NoPackage` on read). A
/// root of a kind not read yet that the document holds is a root: it is
/// written, wrapped.
#[test]
fn lib_compat_003_deleting_every_root_beside_document_level_content_is_no_root() {
    // XMI metadata, text and an attribute of the wrapper each hold no root.
    for (what, input) in [
        (
            "xmi:Documentation beside the one root p, p deleted",
            wrapper("", "<xmi:Documentation/>"),
        ),
        ("text beside the one root p, p deleted", wrapper("", "note")),
        (
            "a vendor attribute on the wrapper of the one root p, p deleted",
            wrapper(" v:w=\"1\"", ""),
        ),
    ] {
        let mut doc = tolerant(COMPAT_003, what, &input).into_document();
        if let Err(error) = edit::apply(&mut doc, DeleteElement::new("p")) {
            panic!("{COMPAT_003}: {what}: setup: the delete was refused: {error:?}");
        }
        assert!(
            doc.roots().next().is_none() && doc.has_preserved_content(),
            "{COMPAT_003}: {what}: setup: no root is left and the wrapper's content is"
        );
        for mode in [Mode::Strict, Mode::Tolerant] {
            assert_eq!(
                write(&doc, mode),
                Err(WriteError::NoRoot),
                "{COMPAT_003}: {what}: a {mode:?} write is not NoRoot"
            );
        }
    }

    // The control: a bare root of a kind not read yet is written, wrapped.
    let what = "a bare TerminologyPackage root";
    let doc = tolerant(COMPAT_003, what, &root_of("TerminologyPackage")).into_document();
    let text = write_tolerant(COMPAT_003, what, &doc);
    assert!(
        text.contains("<xmi:XMI ") && text.contains("<sacm:TerminologyPackage"),
        "{COMPAT_003}: {what}: not written wrapped\n{text}"
    );
    assert!(
        matches!(
            write(&doc, Mode::Strict),
            Err(WriteError::PreservedContent { .. })
        ),
        "{COMPAT_003}: {what}: a strict write is not PreservedContent"
    );
}

/// A kept `xmi:id` of `xmi:XMI` is in the document's id space: a root that
/// repeats it is `DuplicateId`, and an edit cannot take it. A strict read
/// refuses it as XMI metadata not read yet.
#[test]
fn lib_compat_003_a_wrapper_id_is_in_the_id_space() {
    let what = "xmi:id=\"w\" on the wrapper";
    let input = wrapper(" xmi:id=\"w\"", "");
    let strict = strict_refusal(COMPAT_003, what, &input);
    assert!(
        matches!(strict, ReadError::Unsupported { .. }),
        "{COMPAT_003}: {what}: the strict refusal is not Unsupported: {strict:?}"
    );
    let mut doc = tolerant(COMPAT_003, what, &input).into_document();
    let answer = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("C")).with_id("w"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "w"),
        "{COMPAT_003}: {what}: with_id(\"w\") is not DuplicateId: {answer:?}"
    );

    let what = "a wrapper xmi:id that the root repeats";
    match read(Mode::Tolerant, &wrapper(" xmi:id=\"p\"", "")) {
        Err(error) => assert_eq!(
            error.code(),
            Some(Code::ID_DUPLICATE),
            "{COMPAT_003}: {what}: not DuplicateId: {error:?}"
        ),
        Ok(output) => panic!(
            "{COMPAT_003}: {what}: a tolerant read returned a document, with {:?}",
            output.diagnostics()
        ),
    }
}

/// An empty `xmi:id` on `xmi:XMI` identifies nothing: a tolerant read keeps
/// it as written, outside the id space, with one SACM-ID-002 Warning
/// affecting no element, beside its SACM-XMI-014.
#[test]
fn lib_compat_003_an_empty_wrapper_id_is_id_002() {
    let what = "xmi:id=\"\" on the wrapper";
    let output = tolerant(COMPAT_003, what, &wrapper(" xmi:id=\"\"", ""));
    let found: Vec<(Code, Severity, usize)> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.severity(), found.affected().len()))
        .collect();
    assert_eq!(
        found,
        [
            (Code::ID_INVALID, Severity::Warning, 0),
            (Code::XMI_UNREAD_CONTENT, Severity::Warning, 0),
        ],
        "{COMPAT_003}: {what}: not one SACM-ID-002 and one SACM-XMI-014 affecting no element"
    );
    let doc = output.into_document();
    assert!(
        doc.has_preserved_content(),
        "{COMPAT_003}: {what}: the empty id is not kept"
    );
    assert_round_trips(COMPAT_003, what, &doc);
}

/// The `xmi:id` of document-level content stays in the id space when an
/// element is deleted: the delete frees only what the element owned.
#[test]
fn lib_compat_003_a_document_level_id_stays_in_the_id_space_after_a_delete() {
    let what = "xmi:Documentation xmi:id=\"d\" beside a package holding c1, c1 deleted";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
         <xmi:Documentation xmi:id=\"d\"/>{}</xmi:XMI>",
        package(&claim("c1", "", ""))
    );
    let mut doc = tolerant(COMPAT_003, what, &input).into_document();
    if let Err(error) = edit::apply(&mut doc, DeleteElement::new("c1")) {
        panic!("{COMPAT_003}: {what}: setup: the delete was refused: {error:?}");
    }
    let answer = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("C")).with_id("d"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "d"),
        "{COMPAT_003}: {what}: with_id(\"d\") is not DuplicateId: {answer:?}"
    );
}
