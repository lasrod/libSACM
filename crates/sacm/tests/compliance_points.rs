//! The unit documents of the four model compliance points of SACM 2.3
//! clause 2 (SACM23-CP-001 to SACM23-CP-004; formal/23-05-08 §2.2 to §2.5;
//! ADR 0043).
//!
//! formal/23-05-08 §2.2: "Software that conforms to the SACM specification at
//! the Argumentation Model compliance point shall be able to import and
//! export XMI documents that conform with the SACM XML Schema … The top
//! object of the Argumentation package as a unit of interchange shall be the
//! `Argumentation::ArgumentPackage` element of the SACM." §2.3, §2.4 and §2.5
//! say the same of `Artifact::ArtifactPackage`, `SACM::AssuranceCasePackage`
//! and `Terminology::TerminologyPackage`.
//!
//! Each point has one unit document under `tests/data/sacm23/cp/`, the
//! strict output of a source that uses every class and property of the
//! point's packages (`xtask/tests/compliance.rs` checks that). Here each one
//! reads strictly with no diagnostic and nothing preserved, validates with no
//! diagnostic, is written back byte for byte by a default (strict) write,
//! reads tolerantly to an equal document, and has the unit of interchange as
//! its root.

use std::fs;

use sacm::compare::semantic_compare;
use sacm::model::ElementKind;
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

/// The unit documents' directory.
const CP_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/cp");

/// Checks the unit document `file` of the compliance point `row`, whose root
/// must be a `root`: a strict read gives no diagnostic and preserves
/// nothing, `validate` gives none, a default write gives the file's bytes, a
/// tolerant read gives a `semantic_compare`-equal document, and the one root
/// is of kind `root`.
fn check_unit_document(row: &str, file: &str, root: ElementKind) {
    let path = format!("{CP_DIR}/{file}");
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{row}: setup: read {path}: {e}"));

    // A strict read: no diagnostic, nothing preserved.
    let strict = xmi::read(&bytes, &ReadOptions::default().with_mode(Mode::Strict))
        .unwrap_or_else(|e| panic!("{row}: a strict read of {file} refuses it: {e}"));
    let read_diagnostics: Vec<String> = strict
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        read_diagnostics.is_empty(),
        "{row}: a strict read of {file} gives diagnostics: {read_diagnostics:#?}"
    );
    let document = strict.document();
    assert!(
        !document.has_preserved_content(),
        "{row}: a strict read of {file} holds preserved content"
    );

    // `validate`: no diagnostic.
    let found: Vec<String> = validation::validate(document)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found.is_empty(),
        "{row}: validate of {file} gives diagnostics: {found:#?}"
    );

    // A default write is the file, byte for byte.
    let written = xmi::write(document, &WriteOptions::default())
        .unwrap_or_else(|e| panic!("{row}: a strict write of {file} refuses it: {e}"));
    let text = std::str::from_utf8(&bytes)
        .unwrap_or_else(|e| panic!("{row}: setup: {file} is not UTF-8: {e}"));
    if written != text {
        let first = written
            .lines()
            .zip(text.lines())
            .position(|(now, then)| now != then)
            .map_or_else(
                || "a line count or trailing difference".to_owned(),
                |n| format!("line {}", n + 1),
            );
        panic!(
            "{row}: a strict write of {file} differs from the file ({} bytes written, {} in \
             the file), first at {first}",
            written.len(),
            bytes.len()
        );
    }

    // A tolerant read: an equal document.
    let tolerant = xmi::read(&bytes, &ReadOptions::default().with_mode(Mode::Tolerant))
        .unwrap_or_else(|e| panic!("{row}: a tolerant read of {file} refuses it: {e}"));
    let differences = semantic_compare(document, tolerant.document());
    assert!(
        differences.is_empty(),
        "{row}: the tolerant read of {file} differs from the strict read: {differences:#?}"
    );

    // The root is the unit of interchange.
    let roots: Vec<(String, ElementKind)> = document
        .roots()
        .map(|element| (element.id().to_string(), element.kind()))
        .collect();
    assert!(
        matches!(roots.as_slice(), [(_, kind)] if *kind == root),
        "{row}: {file} has the roots {roots:?}, not one {root:?}"
    );
}

#[test]
fn sacm23_cp_001_the_assurance_case_unit_document_reads_validates_and_is_its_own_strict_output() {
    check_unit_document(
        "SACM23-CP-001",
        "assurance-case-unit.sacm.xmi",
        ElementKind::AssuranceCasePackage,
    );
}

#[test]
fn sacm23_cp_002_the_argumentation_unit_document_reads_validates_and_is_its_own_strict_output() {
    check_unit_document(
        "SACM23-CP-002",
        "argumentation-unit.sacm.xmi",
        ElementKind::ArgumentPackage,
    );
}

#[test]
fn sacm23_cp_003_the_artifact_unit_document_reads_validates_and_is_its_own_strict_output() {
    check_unit_document(
        "SACM23-CP-003",
        "artifact-unit.sacm.xmi",
        ElementKind::ArtifactPackage,
    );
}

#[test]
fn sacm23_cp_004_the_terminology_unit_document_reads_validates_and_is_its_own_strict_output() {
    check_unit_document(
        "SACM23-CP-004",
        "terminology-unit.sacm.xmi",
        ElementKind::TerminologyPackage,
    );
}
