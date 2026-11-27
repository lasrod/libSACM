//! The additive API that docs reviews and Assurance Forge asked for
//! (LIB-API-013; `lasrod/libSACM#113` A1, A2, A4, A5, A7-A9; slice 0082).
//!
//! - A9: `Diagnostic::specification()` names the document a clause is in:
//!   `Specification::Sacm23` or `Specification::Xmi251`, `Some` exactly when
//!   `clause()` is, for every diagnostic the crate makes; it is the last
//!   order key, `Display` does not show it, and serde writes it right after
//!   `clause` (`diagnostics.json`). A record read without it has `None`.
//! - A7: `Document::is_id_in_use` covers the whole id space: an element's
//!   id, and the `xmi:id` of a name, a content, a key, a location, a value,
//!   preserved content and XMI metadata. It agrees with a create's
//!   `DuplicateId`, and an id only a reference names is not in use.
//! - A1, A2: `MultiLangString::index_for_lang` (same language ignoring ASCII
//!   case, then the primary subtag, then the first value with text),
//!   `values_mut`, and `From<&str>` and `From<String>`.
//! - A8: `ElementId` compares with `str` both ways, exactly.
//! - A2: `SacmElement::tagged_value(key)` is the first tagged value one of
//!   whose key's values has the content `key`, byte for byte.
//! - A4, A5: `Mutation::created_id` and `BatchMutation::is_empty`.
//! - `WriteError::UnrepresentableCharacter::position`: the index of the value
//!   a bad character is in, with the template
//!   `SACM-XML-003/unrepresentable-character-in-value`; `None`, and the
//!   0.4.0 template and `Display`, outside a value.
//! - SACM23-XMI-020: a strict read refusal names an element without an
//!   `xmi:id` by the id a tolerant read gives it (no new field).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity, Specification};
use sacm::edit::{
    self, BatchError, BatchMutation, CopyElements, CreateArgumentPackage, CreateArtifactPackage,
    CreateAssuranceCasePackage, CreateClaim, CreateNote, CreateResource, CreateTaggedValue,
    DeleteElement, EditError, Mutation, Operation, SetContent, SetName,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, MultiLangString, SacmElement};
use sacm::validation;
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

const ROW: &str = "LIB-API-013";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The test data directory.
const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

/// The directory of the golden JSON files.
#[cfg(feature = "serde")]
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

// --- Helpers -----------------------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// The output of a read of `input` in `mode`, which must not refuse it.
fn read_ok(what: &str, mode: Mode, input: &str) -> ReadOutput {
    read(mode, input)
        .unwrap_or_else(|error| panic!("{ROW}: setup: {what}: a {mode:?} read refuses: {error}"))
}

/// The document of a read of `input` in `mode`.
fn read_doc(what: &str, mode: Mode, input: &str) -> Document {
    read_ok(what, mode, input).into_document()
}

/// The refusal of a strict read of `input`.
fn strict_refusal(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: setup: {what}: a strict read accepts it, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    edit::apply(doc, operation)
        .unwrap_or_else(|error| panic!("{ROW}: setup: {what}: refused: {error}"))
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: setup: the document has no element {id:?}"))
}

fn ids(list: &[ElementId]) -> Vec<&str> {
    list.iter().map(ElementId::as_str).collect()
}

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// The one diagnostic of `found` with `code`.
fn the_one(what: &str, found: &[Diagnostic], code: Code) -> Diagnostic {
    let of_code: Vec<&Diagnostic> = found.iter().filter(|d| d.code() == code).collect();
    match of_code.as_slice() {
        [one] => (*one).clone(),
        _ => panic!("{ROW}: setup: {what}: not one {code} in {found:?}"),
    }
}

// =========================================================================================
// A9: the specification of a clause
// =========================================================================================

/// A Claim that cites `ghost`, which names no element.
const CITED: &str = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" \
                     citedElement=\"ghost\"><name content=\"C1\"/></argumentElement>";

/// An `AssuranceCasePackageInterface` holding an `ArgumentPackage`, which §9.3
/// does not allow there (SACM23-VAL-015).
fn interface_holding_a_package() -> String {
    format!(
        "<sacm:AssuranceCasePackageInterface{NS} xmi:id=\"ai\" implements=\"ai\">\
         <name content=\"ai\"/><argumentPackage xmi:id=\"ap\"><name content=\"ap\"/>\
         </argumentPackage></sacm:AssuranceCasePackageInterface>"
    )
}

/// A Claim without an `xmi:id`.
const ID_LESS: &str =
    "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>";

/// A Claim whose name is given twice (XMI 2.5.1 §9.5.2 rule 2h).
const NAME_TWICE: &str = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                          <name content=\"A\"/><name content=\"B\"/></argumentElement>";

#[test]
fn lib_api_013_a_sacm_clause_names_sacm_2_3() {
    for mode in MODES {
        let doc = read_doc("a dangling citedElement", mode, &package(CITED));
        let found = validation::validate(&doc);
        let dangling = the_one("a dangling citedElement", &found, Code::REF_DANGLING);
        assert_eq!(
            (
                dangling.requirement_id(),
                dangling.clause(),
                dangling.specification()
            ),
            ("SACM23-VAL-003", Some("8.2"), Some(Specification::Sacm23)),
            "{ROW}: ({mode:?}) the SACM-REF-001 of a dangling citedElement is not SACM 2.3 §8.2"
        );

        let doc = read_doc(
            "an interface holding a package",
            mode,
            &interface_holding_a_package(),
        );
        let found = validation::validate(&doc);
        let val_015: Vec<&Diagnostic> = found
            .iter()
            .filter(|d| d.requirement_id() == "SACM23-VAL-015")
            .collect();
        let [one] = val_015.as_slice() else {
            panic!("{ROW}: setup: ({mode:?}) not one SACM23-VAL-015 finding in {found:?}");
        };
        assert_eq!(
            (one.code(), one.clause(), one.specification()),
            (
                Code::PACKAGE_CONTENT_INVALID,
                Some("9.3"),
                Some(Specification::Sacm23)
            ),
            "{ROW}: ({mode:?}) the SACM23-VAL-015 finding is not SACM 2.3 §9.3: {one:?}"
        );
    }
}

#[test]
fn lib_api_013_an_xmi_clause_names_xmi_2_5_1() {
    for mode in MODES {
        let output = read_ok("an id-less Claim", mode, &package(ID_LESS));
        let missing = the_one(
            "an id-less Claim",
            output.diagnostics(),
            Code::XMI_MISSING_ID,
        );
        assert_eq!(
            (missing.clause(), missing.specification()),
            (Some("7.6.1"), Some(Specification::Xmi251)),
            "{ROW}: ({mode:?}) the SACM-XMI-004 of an id-less Claim is not XMI 2.5.1 §7.6.1"
        );
    }
    let output = read_ok("a name given twice", Mode::Tolerant, &package(NAME_TWICE));
    let repeat = the_one(
        "a name given twice",
        output.diagnostics(),
        Code::MULTIPLICITY_VIOLATION,
    );
    assert_eq!(
        (repeat.clause(), repeat.specification()),
        (Some("9.5.2"), Some(Specification::Xmi251)),
        "{ROW}: the tolerant SACM-MULT-001 of a name given twice is not XMI 2.5.1 §9.5.2"
    );
}

/// An `argumentElement` with no type: its role's declared type is the
/// abstract `ArgumentationElement`, so no class can be inferred (XMI 2.5.1
/// §7.6.3; SACM-XMI-005, kept with a Warning by a tolerant read).
const UNTYPED: &str = "<argumentElement xmi:id=\"u1\"><name content=\"U\"/></argumentElement>";

/// A Claim whose id holds white space, which is no `NCName` (XMI 2.5.1
/// §7.6.1; SACM23-VAL-020).
const SPACED_ID: &str = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a b\"><name content=\"C\"/></argumentElement>";

/// Every `.xmi` file under `dir`, recursively, sorted by path.
fn xmi_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = std::fs::read_dir(&next)
            .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()))
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "xmi") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The clauses of XMI 2.5.1 the crate's diagnostics cite: the seven of the
/// slice 0082 design, and §7.6.3 (a child under an abstract role with no
/// type, SACM-XMI-005), whose literal `xmi/roots.rs` also holds. Every other
/// clause is SACM 2.3's. The number alone never decides: XMI §9.5.2 and
/// SACM §9.3 share chapter 9.
const XMI_CLAUSES: [&str; 8] = [
    "7.3.3", "7.5.3", "7.6.1", "7.6.3", "7.8.6", "7.10.1", "7.10.2", "9.5.2",
];

#[test]
fn lib_api_013_every_crate_diagnostic_with_a_clause_names_its_specification() {
    let mut inputs: Vec<(String, Vec<u8>)> = xmi_files(Path::new(DATA))
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
            (path.display().to_string(), bytes)
        })
        .collect();
    assert!(
        inputs.len() > 50,
        "{ROW}: setup: only {} .xmi files under {DATA}",
        inputs.len()
    );
    // The inputs of the two tests above, so that both sides are reached
    // whatever the files hold.
    for (what, input) in [
        ("a dangling citedElement", package(CITED)),
        (
            "an interface holding a package",
            interface_holding_a_package(),
        ),
        ("an id-less Claim", package(ID_LESS)),
        ("a name given twice", package(NAME_TWICE)),
        ("an untyped child under an abstract role", package(UNTYPED)),
        ("an element id with white space", package(SPACED_ID)),
    ] {
        inputs.push((what.to_owned(), input.into_bytes()));
    }
    let mut seen = Vec::new();
    // SACM-XMI-005 at XMI 2.5.1 §7.6.3, and SACM23-VAL-020 at §7.6.1.
    let (mut untyped_seen, mut spaced_seen) = (false, false);
    for (what, bytes) in &inputs {
        for mode in MODES {
            let Ok(output) = xmi::read(bytes, &ReadOptions::default().with_mode(mode)) else {
                continue;
            };
            let mut found: Vec<Diagnostic> = output.diagnostics().to_vec();
            found.extend(validation::validate(output.document()));
            if let Err(error) = xmi::write(output.document(), &WriteOptions::default()) {
                found.extend(error.diagnostics().iter().cloned());
            }
            for diagnostic in &found {
                assert_eq!(
                    diagnostic.clause().is_some(),
                    diagnostic.specification().is_some(),
                    "{ROW}: {what} ({mode:?}): a clause without a specification, or the \
                     reverse: {diagnostic:?}"
                );
                if let Some(clause) = diagnostic.clause() {
                    let expected = if XMI_CLAUSES.contains(&clause) {
                        Specification::Xmi251
                    } else {
                        Specification::Sacm23
                    };
                    assert_eq!(
                        diagnostic.specification(),
                        Some(expected),
                        "{ROW}: {what} ({mode:?}): the clause {clause} names another \
                         specification: {diagnostic:?}"
                    );
                    seen.push(expected);
                }
                let xmi = diagnostic.specification() == Some(Specification::Xmi251);
                untyped_seen |= xmi
                    && diagnostic.code().as_str() == "SACM-XMI-005"
                    && diagnostic.clause() == Some("7.6.3");
                spaced_seen |= xmi
                    && diagnostic.requirement_id() == "SACM23-VAL-020"
                    && diagnostic.clause() == Some("7.6.1");
            }
        }
    }
    assert!(
        untyped_seen,
        "{ROW}: no SACM-XMI-005 at clause 7.6.3 of XMI 2.5.1 was seen"
    );
    assert!(
        spaced_seen,
        "{ROW}: no SACM23-VAL-020 at clause 7.6.1 of XMI 2.5.1 was seen"
    );
    assert!(
        seen.contains(&Specification::Sacm23) && seen.contains(&Specification::Xmi251),
        "{ROW}: setup: not both specifications were seen: {seen:?}"
    );
}

#[test]
fn lib_api_013_a_specification_displays_its_name() {
    assert_eq!(
        (
            Specification::Sacm23.as_str(),
            Specification::Xmi251.as_str()
        ),
        ("SACM 2.3", "XMI 2.5.1"),
        "{ROW}: as_str()"
    );
    assert_eq!(
        (
            Specification::Sacm23.to_string(),
            Specification::Xmi251.to_string()
        ),
        ("SACM 2.3".to_owned(), "XMI 2.5.1".to_owned()),
        "{ROW}: Display"
    );
    assert!(
        Specification::Sacm23 < Specification::Xmi251,
        "{ROW}: Sacm23 does not sort before Xmi251"
    );
    assert_eq!(
        Specification::Sacm23.cmp(&Specification::Sacm23),
        Ordering::Equal,
        "{ROW}: a specification is not equal to itself"
    );
}

/// A diagnostic built with `test-support`, with the message `message` and
/// the clause `clause`.
fn built(message: &str, clause: &'static str) -> Diagnostic {
    Diagnostic::new(
        Code::REF_DANGLING,
        Severity::Error,
        "SACM23-VAL-003",
        message,
    )
    .with_clause(clause)
    .with_affected([ElementId::from("c1")])
}

#[test]
fn lib_api_013_a_specification_is_the_last_order_key() {
    let none = built("m", "8.2");
    let sacm = built("m", "8.2").with_specification(Specification::Sacm23);
    let xmi = built("m", "8.2").with_specification(Specification::Xmi251);
    assert_eq!(
        (
            none.specification(),
            sacm.specification(),
            xmi.specification()
        ),
        (
            None,
            Some(Specification::Sacm23),
            Some(Specification::Xmi251)
        ),
        "{ROW}: with_specification does not set the specification"
    );
    let ordered = [none, sacm, xmi];
    for (i, a) in ordered.iter().enumerate() {
        for (j, b) in ordered.iter().enumerate() {
            assert_eq!(
                a.cmp(b),
                i.cmp(&j),
                "{ROW}: None < Some(Sacm23) < Some(Xmi251) does not hold for {i} and {j}"
            );
            assert_eq!(
                a == b,
                i == j,
                "{ROW}: diagnostics that differ only in their specification are == ({i}, {j})"
            );
        }
    }
    let mut shuffled = vec![ordered[2].clone(), ordered[0].clone(), ordered[1].clone()];
    shuffled.sort();
    assert_eq!(shuffled, ordered, "{ROW}: sorting by the specification");
    // Every earlier key decides first: the message, then the clause.
    let earlier_message = built("a", "8.2").with_specification(Specification::Xmi251);
    assert_eq!(
        earlier_message.cmp(&built("b", "8.2")),
        Ordering::Less,
        "{ROW}: the specification is compared before the message"
    );
    let earlier_clause = built("m", "10.1").with_specification(Specification::Xmi251);
    assert_eq!(
        earlier_clause.cmp(&built("m", "8.2")),
        Ordering::Less,
        "{ROW}: the specification is compared before the clause"
    );
}

#[test]
fn lib_api_013_a_diagnostic_display_does_not_name_its_specification() {
    let plain = built("Claim \"c1\" cites \"ghost\"", "8.2");
    for specification in [Specification::Sacm23, Specification::Xmi251] {
        let with = plain.clone().with_specification(specification);
        assert_eq!(
            with.to_string(),
            plain.to_string(),
            "{ROW}: the Display of a diagnostic with {specification} differs"
        );
    }
    let found = validation::validate(&read_doc(
        "a dangling citedElement",
        Mode::Strict,
        &package(CITED),
    ));
    let dangling = the_one("a dangling citedElement", &found, Code::REF_DANGLING);
    let shown = dangling.to_string();
    assert!(
        shown.starts_with("error SACM-REF-001 [SACM23-VAL-003, clause 8.2]: ")
            && !shown.contains("SACM 2.3"),
        "{ROW}: the Display of a real finding names its specification: {shown}"
    );
}

#[cfg(feature = "serde")]
mod serde_specification {
    use super::{
        Code, Diagnostic, ElementId, GOLDEN, Mode, ROW, Severity, Specification, read_doc,
        validation,
    };

    /// A Claim with two descriptions: exactly one finding.
    const CLM_132: &str = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" ",
        "xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"><name content=\"P\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"CLM_132\"><name content=\"C\"/>",
        "<description xmi:id=\"d1\"/><description xmi:id=\"d2\"/></argumentElement>\n",
        "</sacm:ArgumentPackage>",
    );

    /// The finding of `CLM_132` as a compact record, `specification` right
    /// after `clause`.
    const TOO_MANY: &str = r#"{"code":"SACM-MULT-001","severity":"warning","requirement_id":"SACM23-VAL-011","clause":"8.6","specification":"SACM 2.3","affected":["CLM_132"],"location":null,"message":"Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])","template":"SACM-MULT-001/too-many","arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}}"#;

    const SPECIFICATION: &str = r#""specification":"SACM 2.3""#;

    fn golden(name: &str) -> String {
        let path = std::path::Path::new(GOLDEN).join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
    }

    fn two_descriptions_finding() -> Diagnostic {
        let document = read_doc("CLM_132", Mode::Strict, CLM_132);
        let mut found = validation::validate(&document);
        assert_eq!(found.len(), 1, "{ROW}: setup: CLM_132 gives {found:?}");
        found.remove(0)
    }

    /// The values of `diagnostics.json`, as `serde.rs` builds them.
    fn samples() -> Vec<Diagnostic> {
        let location = sacm::diagnostic::SourceLocation::new(4, 20)
            .unwrap_or_else(|| panic!("{ROW}: setup: 4:20 is a location"));
        vec![
            Diagnostic::new(
                Code::REF_DANGLING,
                Severity::Error,
                "SACM23-VAL-003",
                "Claim \"c1\" has the metaClaim \"x\", which names no element of the document",
            )
            .with_clause("11.10")
            .with_specification(Specification::Sacm23)
            .with_affected([ElementId::from("c1"), ElementId::from("x")])
            .with_location(location),
            Diagnostic::new(
                Code::XMI_MISSING_ID,
                Severity::Info,
                "SACM23-XMI-020",
                "a Claim has no xmi:id; it is read as claim_1",
            ),
            two_descriptions_finding(),
        ]
    }

    /// The keys of each object of the pretty JSON array `json`, in order.
    fn keys_of_each_value(json: &str) -> Vec<Vec<String>> {
        let mut values: Vec<Vec<String>> = Vec::new();
        for line in json.lines() {
            if line == "  {" {
                values.push(Vec::new());
            } else if let Some(rest) = line.strip_prefix("    \"") {
                if let (Some((key, _)), Some(keys)) = (rest.split_once("\":"), values.last_mut()) {
                    keys.push(key.to_owned());
                }
            }
        }
        values
    }

    #[test]
    fn lib_api_013_a_diagnostic_writes_its_specification_after_its_clause() {
        let samples = samples();
        let expected = golden("diagnostics.json");
        let written = match serde_json::to_string_pretty(&samples) {
            Ok(text) => text + "\n",
            Err(error) => panic!("{ROW}: the samples do not serialize: {error}"),
        };
        assert_eq!(
            written, expected,
            "{ROW}: serializing the samples does not give the bytes of diagnostics.json"
        );
        let keys = keys_of_each_value(&expected);
        assert_eq!(
            keys.len(),
            3,
            "{ROW}: setup: diagnostics.json holds 3 values"
        );
        for (index, keys) in keys.iter().enumerate() {
            let at = keys.iter().position(|key| key == "clause");
            assert!(
                at.is_some_and(|at| keys.get(at + 1).map(String::as_str) == Some("specification")),
                "{ROW}: value {index} of diagnostics.json does not write specification right \
                 after clause: {keys:?}"
            );
        }
        for (index, value) in [
            "\"clause\": \"11.10\",\n    \"specification\": \"SACM 2.3\",",
            "\"clause\": null,\n    \"specification\": null,",
            "\"clause\": \"8.6\",\n    \"specification\": \"SACM 2.3\",",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                expected.contains(value),
                "{ROW}: value {index} of diagnostics.json does not hold {value:?}"
            );
        }
        let read: Vec<Diagnostic> = match serde_json::from_str(&expected) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: diagnostics.json does not read back: {error}"),
        };
        assert_eq!(
            read, samples,
            "{ROW}: diagnostics.json reads back to other values"
        );
        assert_eq!(
            read.iter()
                .map(Diagnostic::specification)
                .collect::<Vec<_>>(),
            [
                Some(Specification::Sacm23),
                None,
                Some(Specification::Sacm23)
            ],
            "{ROW}: the specifications read back"
        );

        // The real finding writes the key after `clause`, and reads back.
        let finding = two_descriptions_finding();
        match serde_json::to_string(&finding) {
            Ok(json) => assert_eq!(json, TOO_MANY, "{ROW}: the compact record of a finding"),
            Err(error) => panic!("{ROW}: the finding does not serialize: {error}"),
        }
        let back: Diagnostic = match serde_json::from_str(TOO_MANY) {
            Ok(back) => back,
            Err(error) => panic!("{ROW}: {TOO_MANY} does not read: {error}"),
        };
        assert_eq!(back, finding, "{ROW}: the record does not read back equal");
        assert_record_forms();
    }

    /// A record without `specification`, or `null`, reads `None`; `"XMI
    /// 2.5.1"` reads `Xmi251`; another name, the variant name or a number is
    /// refused; and a `Specification` alone is its name.
    fn assert_record_forms() {
        // A record as 0.4.0 or 0.1.0 wrote it has no specification.
        let old: Vec<Diagnostic> = match serde_json::from_str(&golden("diagnostics-0.1.0.json")) {
            Ok(old) => old,
            Err(error) => panic!("{ROW}: diagnostics-0.1.0.json does not read: {error}"),
        };
        assert!(
            !old.is_empty() && old.iter().all(|d| d.specification().is_none()),
            "{ROW}: a value of diagnostics-0.1.0.json reads with a specification: {old:?}"
        );
        let without = TOO_MANY.replacen(&format!("{SPECIFICATION},"), "", 1);
        assert_ne!(without, TOO_MANY, "{ROW}: setup: no specification key");
        let read: Diagnostic = match serde_json::from_str(&without) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: a record without specification is refused: {error}"),
        };
        assert_eq!(
            (read.clause(), read.specification()),
            (Some("8.6"), None),
            "{ROW}: a record without specification"
        );
        let null = TOO_MANY.replacen(SPECIFICATION, r#""specification":null"#, 1);
        let read: Diagnostic = match serde_json::from_str(&null) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: a null specification is refused: {error}"),
        };
        assert_eq!(
            read.specification(),
            None,
            "{ROW}: a null specification reads as another value"
        );
        let xmi = TOO_MANY.replacen(SPECIFICATION, r#""specification":"XMI 2.5.1""#, 1);
        let read: Diagnostic = match serde_json::from_str(&xmi) {
            Ok(read) => read,
            Err(error) => panic!("{ROW}: \"XMI 2.5.1\" is refused: {error}"),
        };
        assert_eq!(
            read.specification(),
            Some(Specification::Xmi251),
            "{ROW}: \"XMI 2.5.1\" reads as another value"
        );
        for (what, to) in [
            ("an older SACM", r#""specification":"SACM 2.2""#),
            ("the variant name", r#""specification":"Sacm23""#),
            ("a number", r#""specification":23"#),
        ] {
            let bad = TOO_MANY.replacen(SPECIFICATION, to, 1);
            match serde_json::from_str::<Diagnostic>(&bad) {
                Ok(value) => panic!("{ROW}: {what}: {bad} was read as {value:?}"),
                Err(error) => assert!(
                    error.is_data(),
                    "{ROW}: {what}: refused as malformed JSON, not as data: {error}"
                ),
            }
        }
        // A specification alone is its name.
        for (specification, name) in [
            (Specification::Sacm23, "\"SACM 2.3\""),
            (Specification::Xmi251, "\"XMI 2.5.1\""),
        ] {
            match serde_json::to_string(&specification) {
                Ok(json) => assert_eq!(json, name, "{ROW}: the JSON of {specification:?}"),
                Err(error) => panic!("{ROW}: {specification:?} does not serialize: {error}"),
            }
            match serde_json::from_str::<Specification>(name) {
                Ok(read) => assert_eq!(read, specification, "{ROW}: {name} reads back"),
                Err(error) => panic!("{ROW}: {name} does not read: {error}"),
            }
        }
    }
}

// =========================================================================================
// A7: is_id_in_use
// =========================================================================================

/// An `xmi:id` on an element, a name, a content, a content value, a key, a
/// key value, a location and its value, a preserved vendor child element
/// and an `xmi:Extension`; and `ghost`, which only a `citedElement` names.
const IDS: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:vendor" xmi:id="case">
  <name xmi:id="case_name" content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <xmi:Extension extender="Tool" xmi:id="ext1"><layout x="1"/></xmi:Extension>
    <v:layout xmi:id="vendor1"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
      <name content="C1"/>
      <note xmi:id="n1">
        <content xmi:id="n1_content"><value xmi:id="n1_value" content="N"/></content>
      </note>
      <taggedValue xmi:id="t1">
        <key xmi:id="t1_key"><value xmi:id="t1_key_value" content="K"/></key>
        <content><value content="V"/></content>
      </taggedValue>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" isCitation="true" citedElement="ghost">
      <name content="C2"/>
    </argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="a">
    <name content="A"/>
    <artifactElement xmi:type="sacm:Resource" xmi:id="r1">
      <name content="R1"/>
      <location xmi:id="r1_location"><value xmi:id="r1_location_value" content="u"/></location>
    </artifactElement>
  </artifactPackage>
</sacm:AssuranceCasePackage>"#;

/// The element ids of [`IDS`].
const ELEMENT_IDS: [&str; 8] = ["case", "p", "c1", "n1", "t1", "c2", "a", "r1"];

/// The ids of [`IDS`] that identify no element.
const OTHER_IDS: [&str; 9] = [
    "case_name",
    "n1_content",
    "n1_value",
    "t1_key",
    "t1_key_value",
    "r1_location",
    "r1_location_value",
    "vendor1",
    "ext1",
];

/// [`IDS`], read tolerantly: the vendor element is preserved content.
fn ids_document() -> Document {
    let output = read_ok("IDS", Mode::Tolerant, IDS);
    let codes: Vec<&str> = output
        .diagnostics()
        .iter()
        .map(|found| found.code().as_str())
        .collect();
    assert_eq!(
        codes,
        ["SACM-XMI-003"],
        "{ROW}: setup: IDS reads with more than the preserved vendor element"
    );
    output.into_document()
}

#[test]
fn lib_api_013_is_id_in_use_covers_the_ids_of_names_contents_keys_and_values() {
    let doc = ids_document();
    for id in ELEMENT_IDS {
        assert!(
            doc.is_id_in_use(id),
            "{ROW}: the element id {id:?} is not in use"
        );
        assert!(doc.contains(id), "{ROW}: setup: contains({id:?})");
    }
    for id in OTHER_IDS {
        assert!(
            doc.is_id_in_use(id),
            "{ROW}: the xmi:id {id:?} is not in use"
        );
        assert!(
            !doc.contains(id),
            "{ROW}: contains({id:?}) is true for an id that identifies no element"
        );
    }
    assert!(
        !doc.is_id_in_use("unused"),
        "{ROW}: an id the document never holds is in use"
    );
}

#[test]
fn lib_api_013_is_id_in_use_agrees_with_a_create_given_that_id() {
    let doc = ids_document();
    let mut all: Vec<&str> = ELEMENT_IDS.to_vec();
    all.extend(OTHER_IDS);
    all.extend(["ghost", "unused"]);
    for id in all {
        let create = Operation::from(CreateClaim::new("p", LangString::new("C")).with_id(id));
        let answer = edit::check(&doc, &create);
        assert_eq!(
            doc.is_id_in_use(id),
            matches!(answer, Err(EditError::DuplicateId { .. })),
            "{ROW}: is_id_in_use({id:?}) disagrees with a create given that id: {answer:?}"
        );
    }
}

#[test]
fn lib_api_013_an_id_only_a_reference_names_is_not_in_use() {
    let doc = ids_document();
    assert!(
        !doc.referrers("ghost").is_empty(),
        "{ROW}: setup: no reference names ghost"
    );
    assert!(
        !doc.is_id_in_use("ghost"),
        "{ROW}: an id only a citedElement names is in use"
    );
    assert!(!doc.contains("ghost"), "{ROW}: setup: contains(ghost)");
}

#[test]
fn lib_api_013_is_id_in_use_takes_str_string_and_element_id() {
    let doc = ids_document();
    for id in ["c1", "t1_key", "ghost", "unused"] {
        let from_str = doc.is_id_in_use(id);
        let from_string = doc.is_id_in_use(&String::from(id));
        let from_id = doc.is_id_in_use(&ElementId::from(id));
        assert_eq!(
            (from_string, from_id),
            (from_str, from_str),
            "{ROW}: is_id_in_use({id:?}) answers differently for a str, a String and an ElementId"
        );
    }
}

// =========================================================================================
// A1, A2: MultiLangString
// =========================================================================================

/// A `MultiLangString` of `(lang, content)` values.
fn text(values: &[(Option<&str>, Option<&str>)]) -> MultiLangString {
    values
        .iter()
        .map(|&(lang, content)| {
            let mut value = LangString::default();
            value.set_lang(lang.map(str::to_owned));
            value.set_content(content.map(str::to_owned));
            value
        })
        .collect()
}

/// The `(lang, content)` values of a case.
type Values<'a> = &'a [(Option<&'a str>, Option<&'a str>)];

/// Checks `index_for_lang(lang)` of `values`.
fn assert_index(values: Values<'_>, lang: &str, expected: Option<usize>) {
    assert_eq!(
        text(values).index_for_lang(lang),
        expected,
        "{ROW}: index_for_lang({lang:?}) of {values:?}"
    );
}

#[test]
fn lib_api_013_index_for_lang_prefers_the_same_language_ignoring_ascii_case() {
    assert_index(
        &[
            (Some("en-GB"), Some("Colour")),
            (Some("en"), Some("Color")),
            (Some("sv"), Some("Färg")),
        ],
        "en",
        Some(1),
    );
    // No content: only the language finds them.
    assert_index(&[(Some("EN"), None)], "en", Some(0));
    // The same language ignoring case comes before a primary subtag.
    assert_index(&[(Some("en-US"), None), (Some("EN"), None)], "en", Some(1));
    assert_index(&[(Some("en-GB"), None)], "en-gb", Some(0));
    assert_index(
        &[(Some("sv"), None), (Some("En-Gb"), None)],
        "EN-gb",
        Some(1),
    );
}

#[test]
fn lib_api_013_index_for_lang_falls_back_to_the_primary_subtag() {
    let sv_en_us: Values<'_> = &[(Some("sv"), Some("Hej")), (Some("en-US"), Some("Hi"))];
    assert_index(sv_en_us, "en-GB", Some(1));
    assert_index(sv_en_us, "en", Some(1));
    assert_index(
        &[(Some("sv"), None), (Some("EN-us"), None)],
        "en-GB",
        Some(1),
    );
    // `_` is no separator: `en_US` has the primary subtag `en_US`.
    assert_index(&[(Some("en_US"), None)], "en", None);
    // An empty primary subtag matches nothing.
    assert_index(&[(Some("-x"), None)], "-y", None);
}

#[test]
fn lib_api_013_index_for_lang_falls_back_to_the_first_value_with_text() {
    assert_index(
        &[(None, Some("")), (Some("sv"), Some("Hej"))],
        "ja",
        Some(1),
    );
    assert_index(
        &[(Some("sv"), None), (Some("de"), Some("Hallo"))],
        "ja",
        Some(1),
    );
    // A value with no lang is found only by this step.
    assert_index(&[(None, Some("Hi"))], "en", Some(0));
    assert_index(&[(None, Some("Hi")), (Some("en-US"), None)], "en", Some(1));
    assert_index(&[(None, Some("Hi")), (Some("EN"), None)], "en", Some(1));
}

#[test]
fn lib_api_013_index_for_lang_is_none_without_a_match_or_text() {
    assert_index(&[], "en", None);
    assert_index(&[], "", None);
    assert_index(&[(None, None)], "ja", None);
    assert_index(&[(Some("sv"), Some(""))], "ja", None);
    // `""` matches only a value whose lang is `""`.
    assert_index(&[(Some(""), None)], "", Some(0));
    assert_index(&[(None, None)], "", None);
    assert_index(&[(Some("en"), None)], "", None);
    assert_index(&[(Some("en"), None), (Some(""), None)], "", Some(1));
}

#[test]
fn lib_api_013_index_for_lang_takes_the_first_of_equal_matches() {
    assert_index(
        &[(Some("en"), Some("a")), (Some("EN"), Some("b"))],
        "en",
        Some(0),
    );
    assert_index(
        &[(Some("en-US"), None), (Some("en-AU"), None)],
        "en-GB",
        Some(0),
    );
    assert_index(&[(None, Some("a")), (None, Some("b"))], "ja", Some(0));
}

/// A Note `n1` whose content, of id `nc`, has three values of ids `v0`,
/// `v1` and `v2`.
const NOTE_WITH_IDS: &str = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
     <name content=\"C1\"/><note xmi:id=\"n1\"><content xmi:id=\"nc\">\
     <value xmi:id=\"v0\" lang=\"en\" content=\"zero\"/>\
     <value xmi:id=\"v1\" lang=\"sv\" content=\"ett\"/>\
     <value xmi:id=\"v2\" lang=\"de\" content=\"zwei\"/>\
     </content></note></argumentElement>";

#[test]
fn lib_api_013_values_mut_replaces_one_value_in_place() {
    let first = LangString::new("zero").with_lang("en");
    let second = LangString::new("ett").with_lang("sv");
    let third = LangString::new("zwei").with_lang("de");
    let mut values: MultiLangString = [first.clone(), second, third.clone()].into_iter().collect();
    let replacement = LangString::new("one").with_lang("sv");
    values.values_mut()[1] = replacement.clone();
    assert_eq!(
        values.values(),
        [first, replacement.clone(), third],
        "{ROW}: replacing values_mut()[1] changes another value"
    );
    assert_eq!(values.len(), 3, "{ROW}: the length changed");
    if let Some(value) = values.values_mut().get_mut(0) {
        value.set_content(Some("nil".to_owned()));
    }
    assert_eq!(
        values.values()[0].content(),
        Some("nil"),
        "{ROW}: a value changed through values_mut is not changed"
    );

    // Set back on a note read with value ids, every id is kept.
    let mut doc = read_doc(
        "a note with value ids",
        Mode::Strict,
        &package(NOTE_WITH_IDS),
    );
    let Some(mut content) = element(&doc, "n1").content().cloned() else {
        panic!("{ROW}: setup: n1 has no content");
    };
    content.values_mut()[1] = replacement;
    let set = apply(
        "SetContent",
        &mut doc,
        SetContent::new("n1", content.clone()),
    );
    assert_eq!(
        ids(set.changed_ids()),
        ["n1"],
        "{ROW}: SetContent changes n1"
    );
    let note = element(&doc, "n1");
    assert_eq!(note.content(), Some(&content), "{ROW}: the content set");
    assert_eq!(
        note.content_id().map(ElementId::as_str),
        Some("nc"),
        "{ROW}: the content's id is not kept"
    );
    let value_ids: Vec<Option<&str>> = note
        .content_value_ids()
        .iter()
        .map(|id| id.as_ref().map(ElementId::as_str))
        .collect();
    assert_eq!(
        value_ids,
        [Some("v0"), Some("v1"), Some("v2")],
        "{ROW}: the value ids are not kept by position"
    );
}

#[test]
fn lib_api_013_a_str_converts_into_a_multi_lang_string_of_one_value() {
    assert_eq!(
        MultiLangString::from("D"),
        MultiLangString::from(LangString::new("D")),
        "{ROW}: From<&str>"
    );
    assert_eq!(
        MultiLangString::from(String::from("D")),
        MultiLangString::from(LangString::new("D")),
        "{ROW}: From<String>"
    );
    assert_eq!(
        MultiLangString::from("").values(),
        [LangString::new("")],
        "{ROW}: From<&str> of the empty string"
    );
    let tagged = CreateTaggedValue::new("c1", "ASIL", "D");
    assert_eq!(
        (tagged.key(), tagged.content()),
        (
            &MultiLangString::from(LangString::new("ASIL")),
            &MultiLangString::from(LangString::new("D"))
        ),
        "{ROW}: CreateTaggedValue from two strs"
    );
    let set = SetContent::new("n1", "x");
    assert_eq!(
        set.content(),
        Some(&MultiLangString::from(LangString::new("x"))),
        "{ROW}: SetContent from a str"
    );
    let note = CreateNote::new("c1", String::from("y"));
    assert_eq!(
        note.content(),
        &MultiLangString::from(LangString::new("y")),
        "{ROW}: CreateNote from a String"
    );
}

// =========================================================================================
// A8: ElementId compared with str
// =========================================================================================

#[test]
fn lib_api_013_an_element_id_compares_with_str() {
    let id = ElementId::from("c1");
    assert!(id == "c1", "{ROW}: ElementId == &str");
    assert!(id == *"c1", "{ROW}: ElementId == str");
    assert!("c1" == id, "{ROW}: &str == ElementId");
    assert!(*"c1" == id, "{ROW}: str == ElementId");
    assert!(id != "C1", "{ROW}: the comparison ignores case");
    assert!(id != " c1", "{ROW}: the comparison trims");
    assert!(id != "c1 ", "{ROW}: the comparison trims");
    assert!("C1" != id, "{ROW}: the reverse comparison ignores case");
    let mut doc = Document::new();
    apply(
        "the package",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    );
    apply(
        "c1",
        &mut doc,
        CreateClaim::new("p", LangString::new("C1")).with_id("c1"),
    );
    let claim = element(&doc, "c1");
    let held: &ElementId = claim.id();
    assert!(held == "c1", "{ROW}: &ElementId == &str");
    assert!(claim.id() == "c1", "{ROW}: element.id() == \"c1\"");
    assert!(claim.id() != "p", "{ROW}: element.id() != \"p\"");
}

// =========================================================================================
// A2: a tagged value by its plain key
// =========================================================================================

const ELS: &str = "xmi:type=\"sacm:ExpressionLangString\"";

/// A tagged value `id` with the key values `key` (none: no key element)
/// and the content `V`.
fn tagged(id: &str, key: Option<&str>) -> String {
    let key = key.map_or_else(String::new, |values| format!("<key>{values}</key>"));
    format!(
        "<taggedValue xmi:id=\"{id}\">{key}<content><value content=\"V\"/></content></taggedValue>"
    )
}

/// A Claim `id` holding `body`.
fn claim(id: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// One Claim per key case of the plain-key rule.
fn tagged_values_input() -> String {
    let value = |content: &str| format!("<value content=\"{content}\"/>");
    package(
        &[
            claim(
                "c_abb",
                &[
                    tagged("ta", Some(&value("A"))),
                    tagged("tb1", Some(&value("B"))),
                    tagged("tb2", Some(&value("B"))),
                ]
                .concat(),
            ),
            claim(
                "c_multi",
                &tagged(
                    "tm",
                    Some(
                        "<value lang=\"en\" content=\"Identifier\"/>\
                     <value lang=\"sv\" content=\"Identifierare\"/>",
                    ),
                ),
            ),
            claim("c_exact", &tagged("tx", Some(&value("ASIL")))),
            claim(
                "c_lang",
                &tagged("tl", Some("<value lang=\"en\" content=\"L\"/>")),
            ),
            claim(
                "c_absent",
                &tagged("t_absent", Some("<value lang=\"en\"/>")),
            ),
            claim("c_empty", &tagged("t_empty", Some(&value("")))),
            claim(
                "c_expr",
                &tagged(
                    "t_expr",
                    Some(&format!(
                        "<value {ELS} lang=\"en\" content=\"E\" expression=\"ghost\"/>"
                    )),
                ),
            ),
            claim(
                "c_nokey",
                &[tagged("t_nokey", None), tagged("t_emptykey", Some(""))].concat(),
            ),
            claim(
                "c_none",
                "<note xmi:id=\"n1\"><content><value content=\"N\"/></content></note>",
            ),
        ]
        .concat(),
    )
}

/// The id of `holder.tagged_value(key)`.
fn found(doc: &Document, holder: &str, key: &str) -> Option<String> {
    element(doc, holder)
        .tagged_value(key)
        .map(|tag| tag.id().as_str().to_owned())
}

/// Checks `holder.tagged_value(key)` for each case.
fn assert_found(doc: &Document, cases: &[(&str, &str, Option<&str>)]) {
    for &(holder, key, expected) in cases {
        assert_eq!(
            found(doc, holder, key).as_deref(),
            expected,
            "{ROW}: {holder}.tagged_value({key:?})"
        );
    }
}

fn tagged_values_document() -> Document {
    read_doc("the tagged values", Mode::Strict, &tagged_values_input())
}

#[test]
fn lib_api_013_tagged_value_finds_the_first_tagged_value_with_the_key() {
    let doc = tagged_values_document();
    assert_found(
        &doc,
        &[
            ("c_abb", "A", Some("ta")),
            ("c_abb", "B", Some("tb1")),
            ("c_abb", "C", None),
        ],
    );
    let Some(second) = element(&doc, "c_abb").tagged_value("B") else {
        panic!("{ROW}: c_abb has no tagged value B");
    };
    assert_eq!(
        element(&doc, "c_abb")
            .tagged_values()
            .position(|tag| tag.id() == second.id()),
        Some(1),
        "{ROW}: the tagged value B found is not the second tagged value"
    );
    assert_eq!(
        second.kind(),
        ElementKind::TaggedValue,
        "{ROW}: tagged_value gives another kind"
    );
}

#[test]
fn lib_api_013_tagged_value_matches_any_value_of_the_key() {
    let doc = tagged_values_document();
    assert_found(
        &doc,
        &[
            ("c_multi", "Identifier", Some("tm")),
            ("c_multi", "Identifierare", Some("tm")),
            ("c_multi", "en", None),
            ("c_multi", "sv", None),
        ],
    );
}

#[test]
fn lib_api_013_tagged_value_compares_content_exactly() {
    let doc = tagged_values_document();
    assert_found(
        &doc,
        &[
            ("c_exact", "ASIL", Some("tx")),
            ("c_exact", "asil", None),
            ("c_exact", " ASIL", None),
            ("c_exact", "ASIL ", None),
            ("c_exact", "ASI", None),
            // The lang is ignored, and is never the key.
            ("c_lang", "L", Some("tl")),
            ("c_lang", "en", None),
            // A value with no content is never found, even by "".
            ("c_absent", "", None),
            ("c_absent", "en", None),
            // The content "" is found by "".
            ("c_empty", "", Some("t_empty")),
            ("c_empty", " ", None),
            // An ExpressionLangString value is found by its content.
            ("c_expr", "E", Some("t_expr")),
            ("c_expr", "ghost", None),
            // A tagged value with no key, or an empty one, is never found.
            ("c_nokey", "", None),
            ("c_nokey", "V", None),
        ],
    );
    assert_eq!(
        element(&doc, "t_nokey").key(),
        None,
        "{ROW}: setup: t_nokey has a key"
    );
    assert_eq!(
        element(&doc, "t_emptykey").key().map(MultiLangString::len),
        Some(0),
        "{ROW}: setup: t_emptykey has no empty key"
    );
}

#[test]
fn lib_api_013_tagged_value_is_none_where_there_are_none() {
    let doc = tagged_values_document();
    assert_found(&doc, &[("c_none", "", None), ("c_none", "N", None)]);
    assert_eq!(
        element(&doc, "c_none").tagged_values().count(),
        0,
        "{ROW}: setup: c_none has tagged values"
    );
    let note = element(&doc, "n1");
    assert_eq!(note.kind(), ElementKind::Note, "{ROW}: setup: n1");
    assert!(
        note.tagged_value("N").is_none() && note.tagged_value("").is_none(),
        "{ROW}: a Note has a tagged value"
    );
    // A tagged value holds none itself.
    assert!(
        element(&doc, "tx").tagged_value("ASIL").is_none(),
        "{ROW}: a tagged value finds itself"
    );
}

// =========================================================================================
// A4, A5: created_id, is_empty
// =========================================================================================

/// `case` > `p` > `c1`; `case` > `q` > `cq`.
fn two_packages() -> Document {
    let mut doc = Document::new();
    let name = LangString::new;
    let operations: [Operation; 5] = [
        CreateAssuranceCasePackage::root(name("Case"))
            .with_id("case")
            .into(),
        CreateArgumentPackage::new("case", name("P"))
            .with_id("p")
            .into(),
        CreateClaim::new("p", name("C1")).with_id("c1").into(),
        CreateArgumentPackage::new("case", name("Q"))
            .with_id("q")
            .into(),
        CreateClaim::new("q", name("CQ")).with_id("cq").into(),
    ];
    for operation in operations {
        apply("two_packages", &mut doc, operation);
    }
    doc
}

#[test]
fn lib_api_013_created_id_is_the_one_element_an_operation_created() {
    let mut doc = two_packages();
    let created = apply(
        "CreateClaim",
        &mut doc,
        CreateClaim::new("p", LangString::new("C2")),
    );
    assert_eq!(
        created.created_ids().len(),
        1,
        "{ROW}: setup: a CreateClaim creates one"
    );
    assert_eq!(
        created.created_id(),
        Some(&created.created_ids()[0]),
        "{ROW}: a CreateClaim's created_id"
    );
    let renamed = apply(
        "SetName",
        &mut doc,
        SetName::new("c1", LangString::new("Renamed")),
    );
    assert_eq!(renamed.created_id(), None, "{ROW}: a SetName's created_id");

    let source = two_packages();
    let copy = CopyElements::new(&source, ["c1"], "p")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of c1: {error}"));
    let leaf = apply("CopyElements of a leaf", &mut doc, copy);
    assert_eq!(
        (leaf.created_ids().len(), leaf.created_id()),
        (1, leaf.created_ids().first()),
        "{ROW}: a copy of one leaf"
    );
    assert!(leaf.created_id().is_some(), "{ROW}: a copy of one leaf");
    let copy = CopyElements::new(&source, ["q"], "case")
        .unwrap_or_else(|error| panic!("{ROW}: setup: the copy of q: {error}"));
    let tree = apply("CopyElements of a package", &mut doc, copy);
    assert_eq!(
        (tree.created_ids().len(), tree.created_id()),
        (2, None),
        "{ROW}: a copy of a package with one child creates two, so has no created_id"
    );
    let deleted = apply(
        "DeleteElement",
        &mut doc,
        DeleteElement::new(created.created_ids()[0].clone()),
    );
    assert_eq!(
        (deleted.deleted_ids().len(), deleted.created_id()),
        (1, None),
        "{ROW}: a DeleteElement's created_id"
    );
}

/// Whether `batch` changed nothing net, by its three lists.
fn net_empty(batch: &BatchMutation) -> bool {
    batch.created_ids().is_empty()
        && batch.changed_ids().is_empty()
        && batch.deleted_ids().is_empty()
}

#[test]
fn lib_api_013_a_batch_that_changes_nothing_net_is_empty() {
    let mut doc = two_packages();
    let held = element(&doc, "c1")
        .name()
        .cloned()
        .unwrap_or_else(|| panic!("{ROW}: setup: c1 has no name"));
    let outcome = |what: &str, result: Result<BatchMutation, BatchError>| {
        result.unwrap_or_else(|error| panic!("{ROW}: setup: {what}: {error}"))
    };
    let empty = outcome(
        "an empty closure",
        edit::batch(&mut doc, |_tx| Ok::<(), BatchError>(())),
    );
    let same = outcome(
        "SetName to the name held",
        edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
            tx.apply(SetName::new("c1", held.clone()))?;
            Ok(())
        }),
    );
    let came_and_went = outcome(
        "a create and a delete of tmp",
        edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
            tx.apply(CreateClaim::new("p", LangString::new("T")).with_id("tmp"))?;
            tx.apply(DeleteElement::new("tmp"))?;
            Ok(())
        }),
    );
    let real = outcome(
        "a real SetName",
        edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
            tx.apply(SetName::new("c1", LangString::new("Renamed")))?;
            Ok(())
        }),
    );
    for (what, batch, mutations, is_empty) in [
        ("an empty closure", &empty, 0, true),
        ("SetName to the name held", &same, 1, true),
        ("a create and a delete of tmp", &came_and_went, 2, true),
        ("a real SetName", &real, 1, false),
    ] {
        assert_eq!(
            batch.mutations().len(),
            mutations,
            "{ROW}: setup: {what}: the mutations"
        );
        assert_eq!(batch.is_empty(), is_empty, "{ROW}: {what}: is_empty()");
        assert_eq!(
            batch.is_empty(),
            net_empty(batch),
            "{ROW}: {what}: is_empty() is not whether the three lists are empty"
        );
    }
}

// =========================================================================================
// The position of a bad value in a write error
// =========================================================================================

fn named(error: &WriteError) -> Vec<(&'static str, ArgumentValue)> {
    error
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.value().clone()))
        .collect()
}

/// The refusal of a write of `doc` in `mode`.
fn write_refusal(what: &str, doc: &Document, mode: Mode) -> WriteError {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => panic!("{ROW}: setup: {what}: a {mode:?} write writes {text}"),
        Err(error) => error,
    }
}

/// A root ArgumentPackage `p` holding a Note `n1` with `content`.
fn note_with(content: MultiLangString) -> Document {
    let mut doc = Document::new();
    apply(
        "p",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    );
    apply("n1", &mut doc, CreateNote::new("p", content).with_id("n1"));
    doc
}

#[test]
fn lib_api_013_a_value_error_names_the_value_s_position() {
    let content: MultiLangString = [LangString::new("ok"), LangString::new("bad\u{4}")]
        .into_iter()
        .collect();
    let doc = note_with(content);
    for mode in MODES {
        let error = write_refusal("U+0004 in the second content value", &doc, mode);
        assert!(
            matches!(
                &error,
                WriteError::UnrepresentableCharacter {
                    element,
                    attribute: WriteAttribute::ContentValueContent,
                    position: Some(1),
                    character: '\u{4}',
                    ..
                } if element.as_str() == "n1"
            ),
            "{ROW}: ({mode:?}) the refusal of U+0004 in content value 1: {error:?}"
        );
        assert_eq!(
            (error.code(), error.template()),
            (
                Code::XML_UNREPRESENTABLE_CHARACTER,
                "SACM-XML-003/unrepresentable-character-in-value"
            ),
            "{ROW}: ({mode:?}) the code and template"
        );
        assert_eq!(
            named(&error),
            [
                ("element", ArgumentValue::Text("n1".to_owned())),
                (
                    "attribute",
                    ArgumentValue::Prose("content value's content".to_owned())
                ),
                ("position", ArgumentValue::Count(1)),
                ("code_point", ArgumentValue::Count(4)),
            ],
            "{ROW}: ({mode:?}) the arguments by name and kind"
        );
        assert_eq!(
            error.to_string(),
            "the element \"n1\" cannot be written: its content value's content at index 1 \
             holds U+0004, which XML 1.0 cannot represent",
            "{ROW}: ({mode:?}) the Display"
        );
    }

    // A bad lang of the first key value.
    let mut doc = note_with(MultiLangString::from("ok"));
    apply(
        "t1",
        &mut doc,
        CreateTaggedValue::new("p", LangString::new("K").with_lang("e\u{4}"), "V").with_id("t1"),
    );
    let error = write_refusal("U+0004 in the lang of key value 0", &doc, Mode::Strict);
    assert!(
        matches!(
            &error,
            WriteError::UnrepresentableCharacter {
                element,
                attribute: WriteAttribute::KeyValueLang,
                position: Some(0),
                ..
            } if element.as_str() == "t1"
        ),
        "{ROW}: the refusal of U+0004 in the lang of key value 0: {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the element \"t1\" cannot be written: its key value's lang at index 0 holds U+0004, \
         which XML 1.0 cannot represent",
        "{ROW}: the Display of a bad key value lang"
    );
    assert_location_value_errors();
}

/// A bad location value, content and lang, names its position.
fn assert_location_value_errors() {
    for (location, attribute, position) in [
        (
            [LangString::new("u"), LangString::new("x\u{1}")],
            WriteAttribute::LocationValueContent,
            1,
        ),
        (
            [
                LangString::new("u").with_lang("\u{1}"),
                LangString::new("v"),
            ],
            WriteAttribute::LocationValueLang,
            0,
        ),
    ] {
        let mut doc = Document::new();
        apply(
            "a",
            &mut doc,
            CreateArtifactPackage::root(LangString::new("A")).with_id("a"),
        );
        apply(
            "r1",
            &mut doc,
            CreateResource::new("a", LangString::new("R1"))
                .with_location(location.into_iter().collect::<MultiLangString>())
                .with_id("r1"),
        );
        let error = write_refusal("a bad location value", &doc, Mode::Strict);
        assert!(
            matches!(
                &error,
                WriteError::UnrepresentableCharacter {
                    element,
                    attribute: found,
                    position: Some(at),
                    character: '\u{1}',
                    ..
                } if element.as_str() == "r1" && *found == attribute && *at == position
            ),
            "{ROW}: the refusal of U+0001 in the {attribute} of location value {position}: \
             {error:?}"
        );
        assert_eq!(
            error.template(),
            "SACM-XML-003/unrepresentable-character-in-value",
            "{ROW}: the template of a bad location value"
        );
    }
}

#[test]
fn lib_api_013_a_write_error_outside_a_value_has_no_position() {
    let mut doc = Document::new();
    apply(
        "arguments",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("bad\u{1}")).with_id("arguments"),
    );
    let error = write_refusal("U+0001 in a name", &doc, Mode::Strict);
    assert!(
        matches!(
            &error,
            WriteError::UnrepresentableCharacter {
                attribute: WriteAttribute::NameContent,
                position: None,
                character: '\u{1}',
                ..
            }
        ),
        "{ROW}: the refusal of U+0001 in a name: {error:?}"
    );
    assert_eq!(
        error.template(),
        "SACM-XML-003/unrepresentable-character",
        "{ROW}: the template outside a value"
    );
    assert_eq!(
        named(&error),
        [
            ("element", ArgumentValue::Text("arguments".to_owned())),
            (
                "attribute",
                ArgumentValue::Prose("name's content".to_owned())
            ),
            ("code_point", ArgumentValue::Count(1)),
        ],
        "{ROW}: the arguments outside a value"
    );
    assert_eq!(
        error.to_string(),
        "the element \"arguments\" cannot be written: its name's content holds U+0001, which \
         XML 1.0 cannot represent",
        "{ROW}: the Display outside a value is not 0.4.0's"
    );
    // A gid is no value either.
    let mut doc = Document::new();
    apply(
        "p",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    );
    apply("gid", &mut doc, edit::SetGid::new("p", "g\u{2}"));
    let error = write_refusal("U+0002 in a gid", &doc, Mode::Strict);
    assert!(
        matches!(
            &error,
            WriteError::UnrepresentableCharacter {
                attribute: WriteAttribute::Gid,
                position: None,
                ..
            }
        ),
        "{ROW}: the refusal of U+0002 in a gid: {error:?}"
    );
    #[cfg(feature = "serde")]
    assert_position_records();
}

/// With serde, `position` is written after `attribute`, `null` outside a
/// value, and a record without it reads as `None`.
#[cfg(feature = "serde")]
fn assert_position_records() {
    {
        let mut doc = Document::new();
        apply(
            "arguments",
            &mut doc,
            CreateArgumentPackage::root(LangString::new("bad\u{1}")).with_id("arguments"),
        );
        let error = write_refusal("U+0001 in a name", &doc, Mode::Strict);
        let json = serde_json::to_string(&error)
            .unwrap_or_else(|failure| panic!("{ROW}: the refusal does not serialize: {failure}"));
        assert!(
            json.ends_with(r#""attribute":"NameContent","position":null}"#),
            "{ROW}: the refusal does not write \"position\": null after \"attribute\": {json}"
        );
        // A record as 0.4.0 wrote it, without `position`, reads as None.
        let old = r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","template":"SACM-XML-003/unrepresentable-character","arguments":{"element":"arguments","attribute":"name's content","code_point":1},"element":"arguments","character":"\u0001","attribute":"NameContent"}"#;
        let read: WriteError = serde_json::from_str(old)
            .unwrap_or_else(|failure| panic!("{ROW}: a 0.4.0 record does not read: {failure}"));
        assert!(
            matches!(
                &read,
                WriteError::UnrepresentableCharacter { position: None, .. }
            ),
            "{ROW}: a record without position reads as {read:?}"
        );
        assert_eq!(read, error, "{ROW}: a 0.4.0 record reads as another value");
        // A value refusal writes its position, and reads it back.
        let content: MultiLangString = [LangString::new("ok"), LangString::new("bad\u{4}")]
            .into_iter()
            .collect();
        let error = write_refusal("U+0004 in a value", &note_with(content), Mode::Tolerant);
        let json = serde_json::to_string(&error)
            .unwrap_or_else(|failure| panic!("{ROW}: the refusal does not serialize: {failure}"));
        assert!(
            json.ends_with(r#""attribute":"ContentValueContent","position":1}"#),
            "{ROW}: the value refusal does not write its position: {json}"
        );
        let read: WriteError = serde_json::from_str(&json)
            .unwrap_or_else(|failure| panic!("{ROW}: {json} does not read back: {failure}"));
        assert_eq!(
            read, error,
            "{ROW}: the value refusal does not read back equal"
        );
    }
}

// =========================================================================================
// SACM23-XMI-020: the id a read error names
// =========================================================================================

/// The values of every `xmi:id="…"` of `input`.
fn input_ids(input: &str) -> Vec<String> {
    input
        .split("xmi:id=\"")
        .skip(1)
        .filter_map(|rest| rest.split_once('"').map(|(id, _)| id.to_owned()))
        .collect()
}

/// The element ids a read of `input` gives that are no `xmi:id` of it.
fn generated_ids(doc: &Document, input: &str) -> Vec<String> {
    let given = input_ids(input);
    doc.elements()
        .map(|found| found.id().as_str().to_owned())
        .filter(|id| !given.contains(id))
        .collect()
}

#[test]
fn lib_api_013_a_read_error_names_an_id_less_element_by_the_id_a_tolerant_read_gives_it() {
    // Each id-less element comes before an element that has the first id
    // its kind would be given.
    let later = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"claim_1\"><name content=\"B\"/>\
                 </argumentElement>";
    let later_package = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"argument_package_1\">\
                         <name content=\"D\"/></argumentElement>";
    let cases = [
        (
            "an invalid assertionDeclaration",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" assertionDeclaration=\"Assumed\">\
                 <name content=\"A\"/></argumentElement>{later}"
            )),
            "InvalidLiteral",
        ),
        (
            "a gid as attribute and element",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" gid=\"g\"><name content=\"A\"/>\
                 <gid>h</gid></argumentElement>{later}"
            )),
            "RepeatedValue",
        ),
        (
            "an ArgumentPackage named by an attribute",
            package(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" name=\"x\"/>\
                 {later_package}{later}"
            )),
            "LegacyShape",
        ),
    ];
    for (what, input, variant) in cases {
        let refusal = strict_refusal(what, &input);
        let (refused, kind) = match &refusal {
            ReadError::InvalidLiteral { element, .. } => (element.clone(), "InvalidLiteral"),
            ReadError::RepeatedValue { element, .. } => (element.clone(), "RepeatedValue"),
            ReadError::LegacyShape { element, .. } => (element.clone(), "LegacyShape"),
            other => panic!("{ROW}: setup: {what}: the strict refusal is {other:?}"),
        };
        assert_eq!(kind, variant, "{ROW}: setup: {what}: the variant");
        let output = read_ok(what, Mode::Tolerant, &input);
        let generated = generated_ids(output.document(), &input);
        assert_eq!(
            generated,
            [refused.as_str().to_owned()],
            "{ROW}: {what}: the strict refusal names {refused:?}, not the id the tolerant read \
             gives the id-less element"
        );
        assert!(
            !input_ids(&input).iter().any(|id| *id == refused.as_str()),
            "{ROW}: {what}: the id {refused:?} is an xmi:id of the input"
        );
        assert!(
            refusal.location().is_some(),
            "{ROW}: {what}: the refusal has no location"
        );
        let kept: Vec<&Diagnostic> = output
            .diagnostics()
            .iter()
            .filter(|found| found.code() == refusal.code())
            .collect();
        let [kept] = kept.as_slice() else {
            panic!(
                "{ROW}: {what}: not one tolerant {} in {:?}",
                refusal.code(),
                output.diagnostics()
            );
        };
        assert_eq!(
            kept.affected(),
            std::slice::from_ref(&refused),
            "{ROW}: {what}: the tolerant diagnostic does not affect the generated id"
        );
    }
}
