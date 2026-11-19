//! Every read and write refusal has a catalog code (LIB-API-017, acceptance
//! 1, 2 and 9; ADR 0030 amendment 3).
//!
//! - One input per entry of the crate's refusal list (`xmi/refusal.rs`),
//!   read strictly, has the code amendment 3 states for its entry. For each
//!   entry a tolerant read keeps, the tolerant read gives exactly one
//!   diagnostic, a Warning at the strict refusal's location, and its code is
//!   the strict refusal's. The entries a tolerant read refuses too are
//!   `Unsupported` with the same code, and the same refusal, in both modes.
//!   `Capacity` and `Unheld` have no input of a test's size; a unit test of
//!   the list checks their code (`Unread::code()`, design brief).
//! - Each `WriteError` variant has its code.
//! - Acceptance 2's `xmi:Extension` in an `AssuranceCasePackage` has its
//!   code, template, arguments and `Display` (a `TerminologyPackageInterface`
//!   there until slice 0067 read it).
//! - `docs/sacm/diagnostics.md` has live rows for the six codes the slice
//!   adds, and names the refusal uses of SACM-XMI-001, -003, -007 and -014;
//!   45 codes are live: 41 once slice 0063 adds its six, SACM-EXPR-001 of
//!   slice 0065 (SACM23-VAL-009), SACM-PKG-002 of slice 0068, and
//!   SACM-SEC-003 and SACM-XMI-008 of slice 0072 (LIB-SEC-003,
//!   LIB-COMPAT-004).
//! - The namespace entries of the list (LIB-COMPAT-004, acceptance 10;
//!   ADR 0041 D4, D6): a root in an older SACM revision's namespace and one
//!   in a SACM-like namespace are `Tolerated("LIB-COMPAT-004")`, refused
//!   strictly and read tolerantly with a Warning of the strict code at the
//!   strict location; a SACM 2.4 root is a `Limit`, refused in both modes.

use std::fs;
use std::path::Path;

const ROW: &str = "LIB-API-017";

/// The catalog, relative to the repository root.
const CATALOG: &str = "docs/sacm/diagnostics.md";

/// The number of live codes once slice 0063 adds its six (acceptance 9),
/// slice 0065 SACM-EXPR-001 (SACM23-VAL-009), slice 0068 SACM-PKG-002
/// (SACM23-VAL-023), and slice 0072 SACM-SEC-003 (LIB-SEC-003) and
/// SACM-XMI-008 (LIB-COMPAT-004).
const LIVE_CODES: usize = 45;

/// The codes slice 0063 makes live, with the constant each row names.
const NEW_LIVE: [(&str, &str); 6] = [
    ("SACM-XMI-002", "XMI_NAMESPACE_NOT_PINNED"),
    ("SACM-XMI-015", "XMI_WRAPPER_NAMESPACE"),
    ("SACM-XMI-016", "XMI_STRICT_SAVE_INVALID"),
    ("SACM-XML-002", "XML_ENCODING_NOT_READ"),
    ("SACM-XML-003", "XML_UNREPRESENTABLE_CHARACTER"),
    ("SACM-SEC-002", "PARSER_LIMIT"),
];

/// The live codes whose rows gain the refusal that carries them.
const REFUSAL_USES: [(&str, &str); 4] = [
    ("SACM-XMI-001", "`WriteError::NoRoot`"),
    ("SACM-XMI-003", "`ReadError::Unsupported`"),
    ("SACM-XMI-007", "`ReadError::Unsupported`"),
    ("SACM-XMI-014", "`ReadError::Unsupported`"),
];

fn catalog() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(CATALOG);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: read {}: {error}", path.display()))
}

/// The code rows of the catalog, `| SACM-<AREA>-<NNN> | severity | meaning |`,
/// as `(code, meaning)`.
fn code_rows(catalog: &str) -> Vec<(String, String)> {
    catalog
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().strip_prefix('|')?.split('|').collect();
            let code = cells.first()?.trim();
            let is_code = code.starts_with("SACM-")
                && code.len() > "SACM-".len()
                && code.rsplit('-').next().is_some_and(|digits| {
                    digits.len() == 3 && digits.bytes().all(|b| b.is_ascii_digit())
                });
            (is_code && cells.len() >= 3).then(|| (code.to_owned(), cells[2].trim().to_owned()))
        })
        .collect()
}

/// The meaning of `code`'s row, which must be the only one.
fn meaning<'a>(rows: &'a [(String, String)], code: &str) -> &'a str {
    let found: Vec<&str> = rows
        .iter()
        .filter(|(row_code, _)| row_code == code)
        .map(|(_, meaning)| meaning.as_str())
        .collect();
    match found.as_slice() {
        [one] => one,
        _ => panic!(
            "{ROW}: {CATALOG} has {} rows for {code}, not one",
            found.len()
        ),
    }
}

/// Whether `meaning` names a `Code::` constant: a live row (LIB-DIAG-002).
fn is_live(meaning: &str) -> bool {
    meaning.contains("`Code::")
}

/// The details of the `Malformed` refusal of `input`.
fn malformed_details(input: &[u8]) -> sacm::diagnostic::Details {
    match sacm::xmi::read(input, &sacm::xmi::ReadOptions::default()) {
        Err(sacm::xmi::ReadError::Malformed { details, .. }) => details,
        other => panic!("{ROW}: setup: {input:?} is not Malformed: {other:?}"),
    }
}

/// The `DefaultHasher` hash of `value`.
fn hash_of(value: &impl std::hash::Hash) -> u64 {
    use std::hash::{DefaultHasher, Hasher};

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// `Details` compare and hash by their template and arguments, and debug as
/// both.
#[test]
fn lib_api_017_details_compare_hash_and_debug_by_template_and_arguments() {
    let first = malformed_details(b"<r></s>");
    let again = malformed_details(b"<r></s>");
    let other = malformed_details(b"<a></b>");
    assert!(
        first == again,
        "{ROW}: the same refusal's details are not equal"
    );
    assert_eq!(
        hash_of(&first),
        hash_of(&again),
        "{ROW}: equal details hash apart"
    );
    assert!(first != other, "{ROW}: other arguments' details are equal");
    assert_ne!(
        hash_of(&first),
        hash_of(&other),
        "{ROW}: other details hash alike"
    );
    assert_eq!(
        format!("{first:?}"),
        r#"Details { template: "SACM-XML-001/end-tag-mismatch", arguments: [Argument { name: "end_tag", value: Text("s") }, Argument { name: "start_tag", value: Text("r") }] }"#,
        "{ROW}: the Debug of Details"
    );
}

/// An argument value says what it is, and displays as its text or count.
#[test]
fn lib_api_017_an_argument_value_is_text_or_a_count() {
    use sacm::diagnostic::ArgumentValue;

    let count = ArgumentValue::Count(2);
    assert_eq!(
        (count.as_count(), count.as_text(), count.to_string()),
        (Some(2), None, "2".to_owned()),
        "{ROW}: a Count"
    );
    let text = ArgumentValue::Text("x".to_owned());
    assert_eq!(
        (text.as_count(), text.as_text(), text.to_string()),
        (None, Some("x"), "x".to_owned()),
        "{ROW}: a Text"
    );
}

#[test]
fn lib_api_017_the_catalog_has_live_rows_for_the_refusal_codes_and_names_their_refusals() {
    let catalog = catalog();
    let rows = code_rows(&catalog);
    for (code, constant) in NEW_LIVE {
        let meaning = meaning(&rows, code);
        assert!(
            meaning.contains(&format!("`Code::{constant}`")),
            "{ROW}: the {CATALOG} row of {code} does not name `Code::{constant}`: {meaning:?}"
        );
    }
    for (code, refusal) in REFUSAL_USES {
        let meaning = meaning(&rows, code);
        assert!(
            meaning.contains(refusal) && meaning.contains(ROW),
            "{ROW}: the {CATALOG} row of {code} does not name {refusal} with {ROW}: {meaning:?}"
        );
    }
    let live = rows.iter().filter(|(_, meaning)| is_live(meaning)).count();
    assert_eq!(
        live, LIVE_CODES,
        "{ROW}: {CATALOG} has {live} live codes, not {LIVE_CODES}"
    );
}

mod api {
    use super::ROW;
    use sacm::diagnostic::{ArgumentValue, Code, Severity};
    use sacm::edit::{self, CreateArgumentPackage};
    use sacm::model::{Document, LangString};
    use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

    const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
    const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

    const NS: &str = concat!(
        " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    );

    fn strict() -> ReadOptions {
        ReadOptions::default().with_mode(Mode::Strict)
    }

    fn package(body: &str) -> String {
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
        )
    }

    fn claim(id: &str, attributes: &str, body: &str) -> String {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
        )
    }

    fn wrapper(attributes: &str) -> String {
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>\
             <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
        )
    }

    /// The strict refusal of `input`, which must be `Unsupported`.
    fn strict_unsupported(entry: &str, input: &[u8]) -> ReadError {
        match xmi::read(input, &strict()) {
            Ok(output) => panic!(
                "{ROW}: {entry}: a strict read accepts it, with {:?}",
                output.diagnostics()
            ),
            Err(error @ ReadError::Unsupported { .. }) => error,
            Err(error) => panic!("{ROW}: {entry}: a strict read refuses it as {error:?}"),
        }
    }

    /// One input per entry a tolerant read keeps, with its code.
    fn kept_entries() -> Vec<(&'static str, String, Code)> {
        // No element kind is refused as not read yet since slice 0067, which
        // reads the last four interface and binding kinds (ADR 0035
        // amendment 1).
        vec![
            (
                "XmiMetadata",
                package(&claim("c1", " xmi:uuid=\"u1\"", "")),
                Code::XMI_UNREAD_CONTENT,
            ),
            // A cross-document link not read yet (SACM23-XMI-029): an XPointer,
            // since slice 0068 reads an href with a document part
            // (SACM23-XMI-027).
            (
                "CrossDocumentHref",
                package(&claim(
                    "c1",
                    "",
                    "<metaClaim href=\"other.sacm.xmi#xpointer(id('c'))\"/>",
                )),
                Code::XMI_EXTERNAL_HREF,
            ),
            // XMI metadata in a name: the sample ExpressionLangString gave
            // until slice 0065 read it (SACM23-BASE-009).
            (
                "XmiMetadata in a name",
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name xmi:uuid=\"u\" \
                     content=\"p\"/></sacm:ArgumentPackage>"
                ),
                Code::XMI_UNREAD_CONTENT,
            ),
            (
                "WrapperContent",
                wrapper(" foo=\"bar\""),
                Code::XMI_UNKNOWN_CONTENT,
            ),
            (
                "UnwritablePrefix",
                package(&format!(
                    "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
                     xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
                )),
                Code::XMI_UNKNOWN_CONTENT,
            ),
        ]
    }

    #[test]
    fn lib_api_017_each_kept_refusal_list_entry_has_the_code_of_its_tolerant_warning() {
        for (entry, input, code) in kept_entries() {
            let error = strict_unsupported(entry, input.as_bytes());
            assert_eq!(error.code(), code, "{ROW}: {entry}: the strict code()");
            let output = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {entry}: a tolerant read refuses it: {error}"),
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|found| (found.code(), found.severity(), found.location()))
                .collect();
            assert_eq!(
                found,
                [(error.code(), Severity::Warning, error.location())],
                "{ROW}: {entry}: the tolerant read does not give one Warning with the strict \
                 refusal's code at its location"
            );
        }
    }

    /// A root ArgumentPackage `p` in the namespace `uri`, bound to `sacm`.
    fn root_in(uri: &str) -> String {
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{uri}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\">\
             <name content=\"p\"/></sacm:ArgumentPackage>"
        )
    }

    #[test]
    fn lib_api_017_the_entries_refused_in_both_modes_have_their_code_in_both() {
        // Until slice 0072 `OtherSacmNamespace` was refused in both modes;
        // a tolerant read reads it since (LIB-COMPAT-004). `Sacm24` is
        // refused in both modes, with SACM-XMI-008, and ISO-8859-1 is read
        // since (LIB-COMPAT-016), so `Encoding` takes ISO-8859-2. The codes
        // are compared by text, as `Code::XMI_OTHER_REVISION` is new.
        let cases: Vec<(&str, Vec<u8>, &str)> = vec![
            (
                "OtherXmiNamespace",
                format!(
                    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                     <sacm:ArgumentPackage x:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
                )
                .into_bytes(),
                "SACM-XMI-015",
            ),
            (
                "Encoding",
                b"<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?><r/>".to_vec(),
                "SACM-XML-002",
            ),
            (
                "Sacm24",
                root_in("http://www.omg.org/spec/SACM/20260504").into_bytes(),
                "SACM-XMI-008",
            ),
            (
                "XmiMetadata as the document element",
                format!("<xmi:Documentation xmlns:xmi=\"{XMI_NS}\"/>").into_bytes(),
                "SACM-XMI-014",
            ),
        ];
        for (entry, input, code) in cases {
            let strict = strict_unsupported(entry, &input);
            let tolerant = match xmi::read(&input, &ReadOptions::default()) {
                Ok(output) => panic!(
                    "{ROW}: {entry}: a tolerant read accepts it, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => error,
            };
            assert_eq!(
                (strict.code().as_str(), tolerant.code().as_str()),
                (code, code),
                "{ROW}: {entry}: code() in strict and tolerant mode"
            );
            assert_eq!(
                tolerant, strict,
                "{ROW}: {entry}: the modes refuse differently"
            );
        }
        assert_eq!(
            [
                Code::XMI_NAMESPACE_NOT_PINNED.as_str(),
                Code::XMI_WRAPPER_NAMESPACE.as_str(),
                Code::XML_ENCODING_NOT_READ.as_str(),
            ],
            ["SACM-XMI-002", "SACM-XMI-015", "SACM-XML-002"],
            "{ROW}: the codes' texts"
        );
    }

    /// The `row` argument of `error`, if it has one.
    fn row_argument(error: &ReadError) -> Option<String> {
        error
            .arguments()
            .iter()
            .find(|argument| argument.name() == "row")
            .and_then(|argument| argument.value().as_text().map(str::to_owned))
    }

    #[test]
    fn lib_compat_004_the_refusal_list_reads_the_namespace_tolerances() {
        const ROW_004: &str = "LIB-COMPAT-004";
        const TOLERATED: &str =
            "(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)";
        // `OtherRevision` and `OtherSacmNamespace`: Tolerated("LIB-COMPAT-004").
        // A strict read refuses each with its code, the row as its last
        // argument and the Tolerated label; a tolerant read reads it with one
        // Warning of the same code at the strict refusal's location.
        let tolerated = [
            (
                "OtherRevision",
                root_in("http://www.omg.org/spec/SACM/2.2/Argumentation"),
                "SACM-XMI-008",
                "SACM-XMI-008/other-revision-root",
                "SACM-XMI-008/other-revision",
            ),
            (
                "OtherSacmNamespace",
                root_in("urn:example:sacm"),
                "SACM-XMI-002",
                "SACM-XMI-002/other-namespace",
                "SACM-XMI-002/sacm-like",
            ),
        ];
        for (entry, input, code, refusal, warning) in tolerated {
            let error = strict_unsupported(entry, input.as_bytes());
            assert_eq!(
                (error.code().as_str(), error.template()),
                (code, refusal),
                "{ROW_004}: {entry}: the strict code() and template()"
            );
            assert_eq!(
                row_argument(&error).as_deref(),
                Some(ROW_004),
                "{ROW_004}: {entry}: the strict refusal does not name the row"
            );
            assert!(
                error.to_string().ends_with(TOLERATED),
                "{ROW_004}: {entry}: the strict Display does not end with {TOLERATED:?}: {error}"
            );
            let output = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
                Ok(output) => output,
                Err(error) => panic!("{ROW_004}: {entry}: a tolerant read refuses it: {error}"),
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|found| {
                    (
                        found.code().as_str(),
                        found.severity(),
                        found.location(),
                        found.template(),
                        found.requirement_id(),
                    )
                })
                .collect();
            assert_eq!(
                found,
                [(
                    error.code().as_str(),
                    Severity::Warning,
                    error.location(),
                    warning,
                    ROW_004
                )],
                "{ROW_004}: {entry}: the tolerant read does not give one Warning with the strict \
                 refusal's code at its location"
            );
        }
        // `Sacm24`: a Limit, refused in both modes alike with SACM-XMI-008,
        // with no row and the label of a limit.
        let input = root_in("http://www.omg.org/spec/SACM/2.4");
        let strict = strict_unsupported("Sacm24", input.as_bytes());
        let tolerant = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
            Ok(output) => panic!(
                "{ROW_004}: Sacm24: a tolerant read accepts it, with {:?}",
                output.diagnostics()
            ),
            Err(error) => error,
        };
        assert_eq!(
            tolerant, strict,
            "{ROW_004}: Sacm24: the modes refuse differently"
        );
        assert_eq!(
            (strict.code().as_str(), strict.template()),
            ("SACM-XMI-008", "SACM-XMI-008/sacm-2-4"),
            "{ROW_004}: Sacm24: code() and template()"
        );
        assert_eq!(
            row_argument(&strict),
            None,
            "{ROW_004}: Sacm24: a Limit names no row"
        );
        assert!(
            strict.to_string().ends_with(" (a limit of this version)"),
            "{ROW_004}: Sacm24: the Display does not end with the label of a limit: {strict}"
        );
    }

    /// The strict write refusal of `document`.
    fn write_refusal(what: &str, document: &Document) -> WriteError {
        match xmi::write(document, &WriteOptions::default()) {
            Ok(text) => panic!("{ROW}: {what}: a strict write writes it: {text}"),
            Err(error) => error,
        }
    }

    fn tolerant_document(input: &str) -> Document {
        match xmi::read(input.as_bytes(), &ReadOptions::default()) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("{ROW}: setup: a tolerant read refuses {input}: {error}"),
        }
    }

    #[test]
    fn lib_api_017_each_write_error_variant_has_its_code() {
        let mut unrepresentable = Document::new();
        if let Err(error) = edit::apply(
            &mut unrepresentable,
            CreateArgumentPackage::root(LangString::new("bad\u{1}")).with_id("arguments"),
        ) {
            panic!("{ROW}: setup: {error}");
        }
        let invalid = tolerant_document(&package(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" \
             citedElement=\"nope\"><name content=\"c1\"/></argumentElement>",
        ));
        let preserved = tolerant_document(&format!(
            "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
             <name content=\"p\"/></sacm:ArgumentPackage>"
        ));
        let cases = [
            ("NoRoot", Document::new(), Code::XMI_INVALID_ROOT),
            (
                "UnrepresentableCharacter",
                unrepresentable,
                Code::XML_UNREPRESENTABLE_CHARACTER,
            ),
            ("Invalid", invalid, Code::XMI_STRICT_SAVE_INVALID),
            ("PreservedContent", preserved, Code::XMI_STRICT_SAVE_REFUSED),
        ];
        for (variant, document, code) in cases {
            let error = write_refusal(variant, &document);
            let is_variant = match variant {
                "NoRoot" => matches!(error, WriteError::NoRoot),
                "UnrepresentableCharacter" => {
                    matches!(error, WriteError::UnrepresentableCharacter { .. })
                }
                "Invalid" => matches!(error, WriteError::Invalid { .. }),
                _ => matches!(error, WriteError::PreservedContent { .. }),
            };
            assert!(
                is_variant,
                "{ROW}: setup: {variant}: the write refuses it as {error:?}"
            );
            assert_eq!(error.code(), code, "{ROW}: {variant}: code()");
        }
        assert_eq!(
            [
                Code::XML_UNREPRESENTABLE_CHARACTER.as_str(),
                Code::XMI_STRICT_SAVE_INVALID.as_str(),
                Code::PARSER_LIMIT.as_str(),
            ],
            ["SACM-XML-003", "SACM-XMI-016", "SACM-SEC-002"],
            "{ROW}: the codes' texts"
        );
    }

    #[test]
    fn lib_api_017_an_xmi_extension_in_a_case_is_unsupported_with_its_template_and_arguments() {
        // Until slice 0067 this sampled a TerminologyPackageInterface in the
        // case, which that slice reads (SACM23-TERM-007).
        let input = format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"case\"/>\n  \
             <xmi:Extension/>\n</sacm:AssuranceCasePackage>"
        );
        let error = strict_unsupported("xmi:Extension", input.as_bytes());
        assert_eq!(
            error.location().map(|at| (at.line(), at.column())),
            Some((3, 3)),
            "{ROW}: location()"
        );
        assert_eq!(error.code(), Code::XMI_UNREAD_CONTENT, "{ROW}: code()");
        assert_eq!(
            error.template(),
            "SACM-XMI-014/xmi-child-of-root",
            "{ROW}: template()"
        );
        let arguments: Vec<(&str, ArgumentValue)> = error
            .arguments()
            .iter()
            .map(|argument| (argument.name(), argument.value().clone()))
            .collect();
        let text = |value: &str| ArgumentValue::Text(value.to_owned());
        assert_eq!(
            arguments,
            [
                ("element", text("xmi:Extension")),
                ("container", text("sacm:AssuranceCasePackage")),
                ("row", text("SACM23-XMI-019")),
            ],
            "{ROW}: arguments()"
        );
        assert_eq!(
            error.to_string(),
            "the reader cannot represent the input at line 3, column 3: the XMI element \
             \"xmi:Extension\" in the root element \"sacm:AssuranceCasePackage\" is not read in \
             this release (a limit of this version; SACM23-XMI-019 reads it)",
            "{ROW}: Display"
        );
    }
}
