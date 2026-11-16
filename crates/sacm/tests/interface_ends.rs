//! Tests that the `interface` reference end of an AssuranceCasePackage and an
//! ArgumentPackage is refused as conforming content this version does not
//! read yet, with its package's row, by a strict read (LIB-API-011;
//! ADR 0033; ptc/22-03-13 `AssuranceCasePackage::interface`,
//! `ArgumentPackage::interface`).
//!
//! The end is conforming SACM, read by SACM23-AC-002 and SACM23-ARG-014, so
//! ADR 0033 refuses it as `Unsupported` with the label
//! `(a limit of this version; ROW reads it)`: an attribute at its name, an
//! element at its start tag, in strict mode; a tolerant read keeps it as
//! preserved content (ADR 0035). An `interface` in another
//! namespace is not that end: it does not conform and is kept as preserved
//! content of the package, with one SACM-XMI-003 at it. The ArtifactPackage's
//! `interface` (SACM23-ART-012) is tested with the ArtifactPackage, in
//! `artifact_package.rs`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading.

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::model::{ElementId, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "LIB-API-011";

/// Marks, in a test input, where the refusal or the preserved content is.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and a foreign
/// `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:v=\"urn:v\"",
);

/// One package holding the `interface` construct: what it is, the row that
/// reads the end, the template (with the construct at `{attr}` in the start
/// tag or `{content}` in the content), and the package's id.
struct Package {
    what: &'static str,
    row: &'static str,
    wrap: fn(&str, &str) -> String,
    id: &'static str,
}

/// The packages: each kind as a root and nested.
fn packages() -> [Package; 5] {
    [
        Package {
            what: "an AssuranceCasePackage root",
            row: "SACM23-AC-002",
            wrap: |attribute, content| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"{attribute}>\n  \
                     <name content=\"Case\"/>{content}\n</sacm:AssuranceCasePackage>"
                )
            },
            id: "case",
        },
        Package {
            what: "an AssuranceCasePackage in assuranceCasePackage",
            row: "SACM23-AC-002",
            wrap: |attribute, content| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
                     <assuranceCasePackage xmi:id=\"sub\"{attribute}><name content=\"Sub\"/>\
                     {content}</assuranceCasePackage>\n</sacm:AssuranceCasePackage>"
                )
            },
            id: "sub",
        },
        Package {
            what: "an ArgumentPackage root",
            row: "SACM23-ARG-014",
            wrap: |attribute, content| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\"{attribute}>\n  \
                     <name content=\"P\"/>{content}\n</sacm:ArgumentPackage>"
                )
            },
            id: "p",
        },
        Package {
            what: "an ArgumentPackage in argumentPackage",
            row: "SACM23-ARG-014",
            wrap: |attribute, content| {
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
                     <argumentPackage xmi:id=\"ap\"{attribute}><name content=\"A\"/>\
                     {content}</argumentPackage>\n</sacm:AssuranceCasePackage>"
                )
            },
            id: "ap",
        },
        Package {
            what: "an ArgumentPackage in argumentElement",
            row: "SACM23-ARG-014",
            wrap: |attribute, content| {
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  \
                     <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\"{attribute}>\
                     <name content=\"I\"/>{content}</argumentElement>\n</sacm:ArgumentPackage>"
                )
            },
            id: "inner",
        },
    ]
}

/// The forms of the end: as an attribute (marked at its name) and as an
/// element (marked at its start tag), each as `(what, attribute, content)`.
fn forms(namespace: &str) -> [(String, String, String); 2] {
    [
        (
            format!("{namespace}interface as an attribute"),
            format!(" {MARK}{namespace}interface=\"x\""),
            String::new(),
        ),
        (
            format!("{namespace}interface as an element"),
            String::new(),
            format!("{MARK}<{namespace}interface xmi:idref=\"x\"/>"),
        ),
    ]
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its one mark, and the mark's location: lines end at
/// LF, and columns count characters.
fn marked(template: &str) -> (String, SourceLocation) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut at = None;
    for c in template.chars() {
        if c == MARK {
            assert!(at.is_none(), "{ROW}: setup: two marks in {template:?}");
            at = Some(loc(line, column));
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
    let Some(at) = at else {
        panic!("{ROW}: setup: no mark in {template:?}");
    };
    (text, at)
}

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// A strict read refuses the end; a tolerant read keeps it as preserved
/// content with SACM-XMI-014 (LIB-COMPAT-001, ADR 0035).
#[test]
fn lib_api_011_the_interface_end_of_a_package_is_unsupported_with_its_row_in_strict_mode() {
    for package in packages() {
        let label = format!("(a limit of this version; {} reads it)", package.row);
        for (form, attribute, content) in forms("") {
            let what = format!("{form} on {}", package.what);
            let (input, at) = marked(&(package.wrap)(&attribute, &content));
            match read(Mode::Strict, &input) {
                Ok(output) => panic!(
                    "{ROW}: {what}: a strict read returned a document, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => {
                    assert!(
                        matches!(
                            &error,
                            ReadError::Unsupported { reason, .. } if reason.ends_with(&label)
                        ),
                        "{ROW}: {what}: not Unsupported labelled {label:?} in strict mode: \
                         {error:?}"
                    );
                    assert_eq!(
                        (error.code(), error.location()),
                        (None, Some(at)),
                        "{ROW}: {what}: not refused with no code at the construct in strict \
                         mode: {error}"
                    );
                }
            }
        }
    }
}

#[test]
fn lib_api_011_an_interface_in_another_namespace_is_preserved_not_refused() {
    for package in packages() {
        for (form, attribute, content) in forms("v:") {
            let what = format!("{form} on {}", package.what);
            let (input, at) = marked(&(package.wrap)(&attribute, &content));
            let mut documents = Vec::new();
            for (mode, severity) in [
                (Mode::Tolerant, Severity::Warning),
                (Mode::Strict, Severity::Error),
            ] {
                let output = match read(mode, &input) {
                    Ok(output) => output,
                    Err(error) => panic!(
                        "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: \
                         {error:?}"
                    ),
                };
                let found: Vec<_> = output
                    .diagnostics()
                    .iter()
                    .filter(|found| found.code() == Code::XMI_UNKNOWN_CONTENT)
                    .map(|found| {
                        (
                            found.severity(),
                            found.location(),
                            found.affected().to_vec(),
                        )
                    })
                    .collect();
                assert_eq!(
                    found,
                    [(severity, Some(at), vec![ElementId::from(package.id)])],
                    "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {:?} in \
                     {mode:?} mode: {:?}",
                    package.id,
                    output.diagnostics()
                );
                let doc = output.into_document();
                assert!(
                    doc.get(package.id)
                        .is_some_and(SacmElement::has_preserved_content),
                    "{ROW}: {what}: {:?} holds no preserved content in {mode:?} mode",
                    package.id
                );
                documents.push(doc);
            }
            assert_eq!(
                documents[0], documents[1],
                "{ROW}: {what}: the modes read different documents"
            );
        }
    }
}
