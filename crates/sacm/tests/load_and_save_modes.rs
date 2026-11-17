//! Tests for the load and save modes (LIB-COMPAT-001, LIB-COMPAT-007; ADR 0022;
//! D-STRICT, D-SEV).
//!
//! - LIB-COMPAT-001: `ReadOptions` carries an `xmi::Mode`, tolerant by
//!   default, and `ReadOutput::mode()` reports it. Both modes read the same
//!   documents from the inputs both read, and refuse alike a DOCTYPE,
//!   malformed XML, `InvalidRoot`, `NoPackage`, `DuplicateId` and the
//!   limits; what else a strict read refuses, a tolerant read keeps as
//!   preserved content with a Warning (ADR 0035 D1; `tolerant_read.rs`).
//!   `xsi:type` is SACM-XMI-010 (XMI 2.5.1 §9.5.2 rule 2g), an
//!   Error in strict mode and a Warning in tolerant mode; `xmi:version` on the
//!   `xmi:XMI` element or a root is SACM-XMI-011 (§7.5.3), Info in both.
//! - LIB-COMPAT-007: `WriteOptions` carries a mode, strict by default. A strict
//!   write writes only schema-valid output (ADR 0035 D4): it refuses `NoRoot`,
//!   then preserved content, then `WriteError::Invalid` for an `xmi:id` that
//!   is not an `NCName` (SACM-ID-002) or a reference naming no written
//!   `xmi:id` (SACM-REF-001), then an unrepresentable character. It never
//!   validates, so a content Error never refuses it. A tolerant write writes
//!   the document as held.
//!
//! A read diagnostic is compared as a [`Finding`]: its code, severity,
//! requirement id, clause, affected ids and location. The message is not
//! pinned. Locations are counted here: lines end at LF, and columns count
//! characters. An attribute is located at the first character of its name,
//! its prefix included.
//!
//! "The same in both modes" is checked on a representative table, [`table`]:
//! each readable shape, each ADR 0021 non-conforming input (preserved in
//! both modes), the encoding limits, a DOCTYPE, malformed XML,
//! `InvalidRoot`, `NoPackage` and `DuplicateId`. Each entry states what it is
//! expected to give in tolerant mode, so the table cannot drift into testing
//! nothing. What a strict read refuses and a tolerant read keeps is
//! `tolerant_read.rs`'s table (ADR 0035).
//!
//! Not checked here:
//! - `Mode` has no `Default` and is `#[non_exhaustive]`, and
//!   `WriteError::Invalid` is `#[non_exhaustive]`: a test cannot show that an
//!   impl is missing or that an exhaustive pattern fails to compile. The
//!   patterns here use `..`, as a user must.
//! - LIB-COMPAT-007's semver stance: documentation.

use std::fmt::Debug;
use std::hash::Hash;
use std::mem;
use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, Operation};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{
    self, Mode, ReadError, ReadOptions, ReadOutput, WriteAttribute, WriteError, WriteOptions,
};

const COMPAT_001: &str = "LIB-COMPAT-001";
const COMPAT_007: &str = "LIB-COMPAT-007";

/// The pinned SACM 2.3 namespace (D-NS-PIN).
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The XML Schema instance namespace, of `xsi:type`.
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// [`NS`] and `xsi`.
const NS_XSI: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The XMI fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi");
/// The golden files, under [`FIXTURES`].
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi/golden");
/// The golden files the reader reads in this release.
const READABLE_GOLDEN: [&str; 12] = [
    // The golden files of slice 0057 (SACM23-ART-010, SACM23-XMI-009).
    "artifact-asset-relationship.golden.sacm.xmi",
    "artifact-assets.golden.sacm.xmi",
    "artifact-package.golden.sacm.xmi",
    "artifact-relationships.golden.sacm.xmi",
    "asserted-relationships.golden.sacm.xmi",
    "model-spellings.golden.sacm.xmi",
    "named.golden.sacm.xmi",
    "needs-support-claim.golden.sacm.xmi",
    "several-roots.golden.sacm.xmi",
    "single-root.golden.sacm.xmi",
    // The golden files of slice 0062 (SACM23-TERM-001, SACM23-XMI-028).
    "terminology-element-spelling.golden.sacm.xmi",
    "terminology.golden.sacm.xmi",
];
/// The golden file the reader refuses, as `Unsupported`, in this release: it
/// holds an `ArgumentPackageInterface` (SACM23-ARG-014), refused first, and an
/// ArtifactPackage holding an Artifact, which holds a Property, both read since
/// slice 0055 (SACM23-ART-004); its Note and Description are read
/// (SACM23-BASE-004).
const UNREADABLE_GOLDEN: &str = "nested.golden.sacm.xmi";

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// The read options of `mode`.
fn read_options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

/// Reads `input` in `mode`.
fn read_in(mode: Mode, input: &[u8]) -> Result<ReadOutput, ReadError> {
    xmi::read(input, &read_options(mode))
}

/// Reads `input` in `mode`, which the test expects to be read.
fn read_ok(row: &str, what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read_in(mode, input.as_bytes()) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {what}: read in {mode:?} mode refused {input:?}: {error}"),
    }
}

/// Reads `input` in `mode`, which the test expects to be refused.
fn read_err(row: &str, what: &str, mode: Mode, input: &str) -> ReadError {
    match read_in(mode, input.as_bytes()) {
        Ok(output) => panic!(
            "{row}: {what}: read in {mode:?} mode returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The location of every occurrence of `needle` in `text`, in order. `text`
/// must end its lines with LF only.
fn positions_of(text: &str, needle: &str) -> Vec<SourceLocation> {
    assert!(
        !text.contains('\r'),
        "setup: positions_of counts LF line ends only"
    );
    text.match_indices(needle)
        .map(|(offset, _)| {
            let before = &text[..offset];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
            let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
                panic!("setup: the test input is too large to locate");
            };
            loc(line, column)
        })
        .collect()
}

/// The location of the one occurrence of `needle` in `text`.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    let found = positions_of(text, needle);
    assert_eq!(
        found.len(),
        1,
        "setup: {needle:?} occurs {} times in {text:?}, not once",
        found.len()
    );
    found[0]
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// A diagnostic without its message.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Finding {
    code: Code,
    severity: Severity,
    requirement: String,
    clause: Option<String>,
    affected: Vec<String>,
    location: Option<SourceLocation>,
}

fn finding(diagnostic: &Diagnostic) -> Finding {
    Finding {
        code: diagnostic.code(),
        severity: diagnostic.severity(),
        requirement: diagnostic.requirement_id().to_owned(),
        clause: diagnostic.clause().map(str::to_owned),
        affected: diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        location: diagnostic.location(),
    }
}

fn findings(diagnostics: &[Diagnostic]) -> Vec<Finding> {
    diagnostics.iter().map(finding).collect()
}

/// The SACM-XMI-010 of an `xsi:type` at `at`, read in `mode`, affecting
/// `element`.
fn xsi_type(mode: Mode, element: &str, at: SourceLocation) -> Finding {
    Finding {
        code: Code::XMI_XSI_TYPE,
        severity: if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        },
        requirement: COMPAT_001.to_owned(),
        clause: Some("9.5.2".to_owned()),
        affected: vec![element.to_owned()],
        location: Some(at),
    }
}

/// The SACM-XMI-011 of an `xmi:version` at `at`, on the root `root`, or on
/// the `xmi:XMI` element when `None`.
fn xmi_version(root: Option<&str>, at: SourceLocation) -> Finding {
    Finding {
        code: Code::XMI_VERSION_IGNORED,
        severity: Severity::Info,
        requirement: COMPAT_001.to_owned(),
        clause: Some("7.5.3".to_owned()),
        affected: root.map(str::to_owned).into_iter().collect(),
        location: Some(at),
    }
}

/// A diagnostic as its code, severity, affected ids and location.
type Projected = (Code, Severity, Vec<String>, Option<SourceLocation>);

/// `diagnostics` without their requirement, clause and message.
fn without_clauses(diagnostics: &[Diagnostic]) -> Vec<Projected> {
    findings(diagnostics)
        .into_iter()
        .map(|found| (found.code, found.severity, found.affected, found.location))
        .collect()
}

/// The SACM-XMI-003 of content kept as preserved content (LIB-COMPAT-003,
/// ADR 0033) at `at`, read in `mode`, affecting `element`: a Warning in
/// tolerant mode and an Error in strict mode. Its clause is not pinned here.
fn unknown_content(mode: Mode, element: &str, at: SourceLocation) -> Projected {
    (
        Code::XMI_UNKNOWN_CONTENT,
        if mode == Mode::Strict {
            Severity::Error
        } else {
            Severity::Warning
        },
        vec![element.to_owned()],
        Some(at),
    )
}

/// Reads `input` in `mode` and asserts that it gives exactly `expected`, in
/// that order, and the same document as `plain`, the input without the
/// attributes that are reported. Returns the output.
fn assert_reads_with(
    what: &str,
    mode: Mode,
    input: &str,
    plain: &str,
    expected: &[Finding],
) -> ReadOutput {
    let output = read_ok(COMPAT_001, what, mode, input);
    assert_eq!(
        findings(output.diagnostics()),
        expected,
        "{COMPAT_001}: {what}: read in {mode:?} mode, the diagnostics are not the expected ones \
         for {input:?}: {:?}",
        output.diagnostics()
    );
    let reference = read_ok(COMPAT_001, &format!("{what}: the plain input"), mode, plain);
    assert_eq!(
        reference.diagnostics(),
        [],
        "{COMPAT_001}: {what}: setup: the plain input {plain:?} has diagnostics"
    );
    assert_eq!(
        semantic_compare(output.document(), reference.document()),
        [],
        "{COMPAT_001}: {what}: read in {mode:?} mode, the document is not the one without the \
         reported attributes"
    );
    output
}

// ---------------------------------------------------------------------------
// Building and writing
// ---------------------------------------------------------------------------

/// A non-empty English name.
fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies a create operation and returns the id it created, or fails the
/// test as a setup error.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => match mutation.created_ids().first() {
            Some(created) => created.clone(),
            None => panic!("{row}: setup: {operation:?} created no id"),
        },
        Err(error) => panic!("{row}: setup could not apply {operation:?}: {error}"),
    }
}

/// Inserts one element through the `test-support` seam, or fails the test
/// as a setup error.
fn insert(
    row: &str,
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
    name: Option<LangString>,
) {
    let parent = parent.map(ElementId::from);
    if let Err(error) = doc.insert(parent.as_ref(), kind, ElementId::from(id), name) {
        panic!("{row}: setup could not insert {kind} {id:?}: {error}");
    }
}

/// Documents built with `edit::apply` only, so every element has a name.
fn built_with_edit(row: &str) -> Vec<(&'static str, Document)> {
    let mut one = Document::new();
    create(row, &mut one, CreateArgumentPackage::root(en("Arguments")));

    let mut deep = Document::new();
    let case = create(
        row,
        &mut deep,
        CreateAssuranceCasePackage::root(en("Case")).with_id("case"),
    );
    let sub = create(
        row,
        &mut deep,
        CreateAssuranceCasePackage::new(&case, LangString::default()),
    );
    let arguments = create(
        row,
        &mut deep,
        CreateArgumentPackage::new(&sub, en("Arguments")),
    );
    create(row, &mut deep, CreateClaim::new(&arguments, en("Top")));
    create(
        row,
        &mut deep,
        CreateClaim::new(&arguments, LangString::new("Säkerhet")).with_id("c_two"),
    );
    let more = create(row, &mut deep, CreateArgumentPackage::root(en("More")));
    create(row, &mut deep, CreateClaim::new(&more, en("Other")));

    vec![("one root", one), ("three levels and two roots", deep)]
}

/// The write options of `mode`.
fn write_options(mode: Mode) -> WriteOptions {
    WriteOptions::default().with_mode(mode)
}

/// Writes `doc` in `mode`.
fn write_in(mode: Mode, doc: &Document) -> Result<String, WriteError> {
    xmi::write(doc, &write_options(mode))
}

/// Writes `doc` in `mode`, which the test expects to be written.
fn write_ok(row: &str, what: &str, mode: Mode, doc: &Document) -> String {
    match write_in(mode, doc) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: write in {mode:?} mode refused: {error} ({error:?})"),
    }
}

/// Writes `doc` in `mode`, which the test expects to be refused.
fn write_err(row: &str, what: &str, mode: Mode, doc: &Document) -> WriteError {
    match write_in(mode, doc) {
        Ok(text) => panic!("{row}: {what}: write in {mode:?} mode returned:\n{text}"),
        Err(error) => error,
    }
}

/// The Errors of `validate(doc)`, in its order.
fn validation_errors(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.severity() == Severity::Error)
        .collect()
}

/// Reads the golden file `name`.
fn golden(row: &str, name: &str) -> String {
    let path = Path::new(GOLDEN_DIR).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()))
}

/// Every file name in the golden directory, sorted.
fn golden_names(row: &str) -> Vec<String> {
    let entries = std::fs::read_dir(GOLDEN_DIR)
        .unwrap_or_else(|error| panic!("{row}: cannot list {GOLDEN_DIR}: {error}"));
    let mut names: Vec<String> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("{row}: cannot list {GOLDEN_DIR}: {error}"))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

// ===========================================================================
// LIB-COMPAT-001 1. The load mode
// ===========================================================================

fn assert_mode_traits<T: Clone + Copy + Debug + PartialEq + Eq + Hash + Send + Sync + 'static>() {}

#[test]
fn lib_compat_001_read_options_default_to_tolerant_and_with_mode_sets_the_mode() {
    assert_mode_traits::<Mode>();
    assert_eq!(
        ReadOptions::default().mode(),
        Mode::Tolerant,
        "{COMPAT_001}: ReadOptions::default() is not tolerant"
    );
    assert_eq!(
        ReadOptions::default().with_mode(Mode::Strict).mode(),
        Mode::Strict,
        "{COMPAT_001}: with_mode(Mode::Strict) does not set strict"
    );
    assert_eq!(
        ReadOptions::default().with_mode(Mode::Tolerant).mode(),
        Mode::Tolerant,
        "{COMPAT_001}: with_mode(Mode::Tolerant) does not set tolerant"
    );
    assert_eq!(
        ReadOptions::default()
            .with_mode(Mode::Strict)
            .with_mode(Mode::Tolerant)
            .mode(),
        Mode::Tolerant,
        "{COMPAT_001}: a later with_mode does not replace an earlier one"
    );
    assert_ne!(
        ReadOptions::default().with_mode(Mode::Strict),
        ReadOptions::default(),
        "{COMPAT_001}: strict read options compare equal to the default ones"
    );
    assert_eq!(
        ReadOptions::default().with_mode(Mode::Tolerant),
        ReadOptions::default(),
        "{COMPAT_001}: tolerant read options are not the default ones"
    );
    assert_ne!(
        Mode::Strict,
        Mode::Tolerant,
        "{COMPAT_001}: the two modes compare equal"
    );
}

#[test]
fn lib_compat_001_read_output_reports_the_mode_it_was_read_in() {
    let plain = format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>");
    let typed =
        format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>");
    for mode in MODES {
        let output = read_ok(COMPAT_001, "an input with no diagnostic", mode, &plain);
        assert_eq!(
            output.diagnostics(),
            [],
            "{COMPAT_001}: setup: the plain input has diagnostics in {mode:?} mode"
        );
        assert_eq!(
            output.mode(),
            mode,
            "{COMPAT_001}: an input with no diagnostic read in {mode:?} mode reports another mode"
        );
        let output = read_ok(COMPAT_001, "an input with xsi:type", mode, &typed);
        assert_eq!(
            output.mode(),
            mode,
            "{COMPAT_001}: an input with xsi:type read in {mode:?} mode reports another mode"
        );
    }
    let default = match xmi::read(plain.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{COMPAT_001}: the plain input is refused: {error}"),
    };
    assert_eq!(
        default.mode(),
        Mode::Tolerant,
        "{COMPAT_001}: a read with the default options does not report tolerant"
    );
}

// ===========================================================================
// LIB-COMPAT-001 2. Both modes read alike what both read, and refuse alike
// what both refuse
// ===========================================================================

/// What an input of [`table`] gives in tolerant mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    Reads,
    /// Read, with one construct kept as preserved content: one SACM-XMI-003
    /// (LIB-COMPAT-003, ADR 0033).
    Preserved,
    Unsupported,
    Doctype,
    Malformed,
    InvalidRoot,
    NoPackage,
    DuplicateId,
}

/// The [`Expect`] that `result` is, or `None` for a refusal this file does
/// not know.
fn category(result: &Result<ReadOutput, ReadError>) -> Option<Expect> {
    match result {
        Ok(output)
            if output
                .diagnostics()
                .iter()
                .any(|found| found.code() == Code::XMI_UNKNOWN_CONTENT) =>
        {
            Some(Expect::Preserved)
        }
        Ok(_) => Some(Expect::Reads),
        Err(ReadError::Unsupported { .. }) => Some(Expect::Unsupported),
        Err(ReadError::Doctype { .. }) => Some(Expect::Doctype),
        Err(ReadError::Malformed { .. }) => Some(Expect::Malformed),
        Err(ReadError::InvalidRoot { .. }) => Some(Expect::InvalidRoot),
        Err(ReadError::NoPackage { .. }) => Some(Expect::NoPackage),
        Err(ReadError::DuplicateId { .. }) => Some(Expect::DuplicateId),
        Err(_) => None,
    }
}

/// `text` as UTF-16LE with a byte order mark.
fn utf16_le(text: &str) -> Vec<u8> {
    "\u{FEFF}"
        .encode_utf16()
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// A bare ArgumentPackage `p` holding `content`.
fn ap(content: &str) -> String {
    format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"p\">\n  {content}\n</sacm:ArgumentPackage>")
}

/// A bare AssuranceCasePackage `p` holding `content`.
fn acp(content: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS_XSI} xmi:id=\"p\">\n  {content}\n</sacm:AssuranceCasePackage>"
    )
}

/// The representative inputs of the existing read tests, each with what it
/// gives in tolerant mode.
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn table() -> Vec<(&'static str, Vec<u8>, Expect)> {
    use Expect::{
        Doctype, DuplicateId, InvalidRoot, Malformed, NoPackage, Preserved, Reads, Unsupported,
    };
    let text =
        |what: &'static str, input: String, expect: Expect| (what, input.into_bytes(), expect);
    let wrapper = |content: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:xsi=\"{XSI_NS}\">\n\
             {content}\n</xmi:XMI>"
        )
    };
    let third_party =
        std::fs::read(Path::new(FIXTURES).join("name-third-party-form-valid.sacm.xmi"))
            .unwrap_or_else(|error| {
                panic!("{COMPAT_001}: cannot read the third-party fixture: {error}")
            });
    vec![
        // --- Each readable shape (SACM23-XMI-001, -002, -012, -016, -017, -018, -024).
        text(
            "a bare AssuranceCasePackage",
            format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"acp_1\"/>"),
            Reads,
        ),
        text(
            "a bare ArgumentPackage",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"arg_1\"/>"),
            Reads,
        ),
        text(
            "a wrapper holding AP, ACP, AP",
            wrapper(
                "<sacm:ArgumentPackage xmi:id=\"a1\"/><sacm:AssuranceCasePackage xmi:id=\"n1\"/>\
                 <sacm:ArgumentPackage xmi:id=\"a2\"/>",
            ),
            Reads,
        ),
        text(
            "named packages and Claims at depth 3, with declarations",
            acp("<name lang=\"en\" content=\"Case\"/>\n  \
                 <assuranceCasePackage xmi:id=\"sub\"><name/></assuranceCasePackage>\n  \
                 <argumentPackage xmi:id=\"ap\"><name content=\"Arguments\"/>\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"needsSupport\">\
                 <name lang=\"sv\" content=\"Säkert\"/></argumentElement>\
                 <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\"/>\
                 </argumentPackage>"),
            Reads,
        ),
        text(
            "children typed with xsi:type, and with both type attributes",
            ap("<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"assumed\"/>\n  \
                <argumentElement xmi:type=\"sacm:Claim\" xsi:type=\"sacm:Claim\" xmi:id=\"c2\"/>\n  \
                <argumentElement xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner\"/>"),
            Reads,
        ),
        text(
            "a redundant xsi:type on a concrete role",
            acp("<argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\"/>"),
            Reads,
        ),
        text(
            "xsi:type naming the root's own class",
            format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>"),
            Reads,
        ),
        text(
            "xsi:type on a name",
            ap("<name xsi:type=\"sacm:LangString\" lang=\"en\" content=\"P\"/>"),
            Reads,
        ),
        text(
            "xmi:version on the wrapper and the roots",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\
                 <sacm:ArgumentPackage xmi:id=\"a\" xmi:version=\"2.1\"/>\
                 <sacm:AssuranceCasePackage xmi:version=\"\" xmi:id=\"n\"/></xmi:XMI>"
            ),
            Reads,
        ),
        text(
            "other prefixes and a default namespace",
            format!(
                "<x:XMI xmlns:x=\"{XMI_NS}\" xmlns=\"{SACM_NS}\" xmlns:i=\"{XSI_NS}\">\
                 <ArgumentPackage x:id=\"a\" i:type=\"ArgumentPackage\"/></x:XMI>"
            ),
            Reads,
        ),
        text(
            "a declaration, comments, PIs and white space",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- c -->\n<?tool a?>\n\
                 <sacm:ArgumentPackage{NS} xmi:id=\"a\">\n  <!-- c --><?tool b?>\n\
                 </sacm:ArgumentPackage>\n<!-- c -->\n"
            ),
            Reads,
        ),
        (
            "UTF-16LE with a byte order mark",
            utf16_le(&format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>")),
            Reads,
        ),
        ("the third-party fixture", third_party, Reads),
        // SACM23-BASE-004/005/006: utility children and `gid`, which ended
        // interim refusals of this table.
        text(
            "a gid on a root",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" gid=\"g\"/>"),
            Reads,
        ),
        text(
            "a description, a note and a tagged value",
            ap("<description xmi:id=\"d\"><content><value lang=\"en\" content=\"x\"/></content></description>\n  \
                <note xmi:id=\"n\"/>\n  \
                <taggedValue xmi:id=\"t\"><key><value content=\"k\"/></key><content/></taggedValue>"),
            Reads,
        ),
        // SACM23-BASE-007/008, SACM23-XMI-005/015: the SACMElement flags and
        // references, which ended two interim refusals of this table.
        text(
            "an isAbstract on a root",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" isAbstract=\"true\"/>"),
            Reads,
        ),
        text(
            "a Claim's isCitation and citedElement, in both forms",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"false\" \
                citedElement=\"c2\"/>\n  \
                <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\
                <isCitation>1</isCitation><abstractForm xmi:idref=\"nope\"/></argumentElement>"),
            Reads,
        ),
        // --- The limits a tolerant read refuses too (ADR 0035 D1). Conforming
        // content this release does not read yet, which a strict read refuses
        // and a tolerant read keeps, is `tolerant_read.rs`'s table.
        (
            "UTF-32 with a byte order mark",
            vec![0xFF, 0xFE, 0x00, 0x00, b'<', 0, 0, 0, b'r', 0, 0, 0, b'/', 0, 0, 0, b'>', 0, 0, 0],
            Unsupported,
        ),
        text(
            "a declared ISO-8859-1 encoding",
            format!("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><sacm:ArgumentPackage{NS} xmi:id=\"a\"/>"),
            Unsupported,
        ),
        // --- ADR 0021's non-conforming inputs (ADR 0022's table): kept as
        // preserved content in both modes (ADR 0033).
        text(
            "a type that does not conform to the role",
            acp("<argumentPackage xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a type naming an abstract class",
            ap("<argumentElement xmi:type=\"sacm:Assertion\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a type naming an unknown class",
            ap("<argumentElement xmi:type=\"sacm:Nothing\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a type naming a class case-insensitively",
            ap("<argumentElement xmi:type=\"sacm:claim\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "two type attributes that disagree",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a role the parent's class lacks",
            acp("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a plural role name",
            ap("<argumentElements xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a namespace-qualified role",
            ap("<sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"),
            Preserved,
        ),
        text(
            "a namespace-qualified name",
            ap("<sacm:name content=\"P\"/>"),
            Preserved,
        ),
        text(
            "assertionDeclaration on a package",
            ap("<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\" assertionDeclaration=\"asserted\"/>"),
            Preserved,
        ),
        text(
            "sacm:assertionDeclaration",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" sacm:assertionDeclaration=\"assumed\"/>"),
            Preserved,
        ),
        text(
            "an attribute the schema does not allow",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" foo=\"1\"/>"),
            Preserved,
        ),
        text(
            "text in a root",
            ap("text"),
            Preserved,
        ),
        // --- A duplicate id (SACM-ID-001), refused in both modes. An untyped
        // child under an abstract role, an empty id, a value that is not a
        // literal and a second name are refused by a strict read and kept
        // by a tolerant one: `tolerant_read.rs`'s table (ADR 0035).
        text(
            "a duplicate xmi:id",
            wrapper("<sacm:ArgumentPackage xmi:id=\"p\"/><sacm:ArgumentPackage xmi:id=\"p\"/>"),
            DuplicateId,
        ),
        text(
            "a duplicate child xmi:id",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"p\"/>"),
            DuplicateId,
        ),
        // --- Ids the reader now reads (SACM23-XMI-020, -022, -025), with
        // their Info in both modes.
        text(
            "a root and a child without an xmi:id",
            format!(
                "<sacm:ArgumentPackage{NS_XSI}>\n  <name content=\"P\"/>\n  \
                 <argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>\n\
                 </sacm:ArgumentPackage>"
            ),
            Reads,
        ),
        text(
            "a padded xmi:id on a root and on a child",
            format!(
                "<sacm:ArgumentPackage{NS_XSI} xmi:id=\" p \">\n  <name content=\"P\"/>\n  \
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1&#9;\"/>\n\
                 </sacm:ArgumentPackage>"
            ),
            Reads,
        ),
        text(
            "an xmi:id on a name",
            ap("<name xmi:id=\"n1\" content=\"P\"/>"),
            Reads,
        ),
        text(
            "a root's type naming another class",
            format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\" xsi:type=\"sacm:Claim\"/>"),
            Preserved,
        ),
        text(
            "xmi:version on a child",
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" xmi:version=\"2.0\"/>"),
            Preserved,
        ),
        text(
            "an older XMI wrapper",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmlns:x=\"{XMI_NS}\" x:id=\"a\"/></xmi:XMI>"
            ),
            Unsupported,
        ),
        // --- A DOCTYPE (SACM-SEC-001).
        text(
            "a DOCTYPE",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE r>\n<r/>".to_owned(),
            Doctype,
        ),
        text(
            "an external entity",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE r [ <!ENTITY xxe SYSTEM \"file:///etc/passwd\"> ]>\n<r>&xxe;</r>"
                .to_owned(),
            Doctype,
        ),
        // --- Malformed XML (SACM-XML-001).
        text("mismatched tags", "<r></s>".to_owned(), Malformed),
        text(
            "a truncated root",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\""),
            Malformed,
        ),
        ("invalid UTF-8", b"<r>\xFF</r>".to_vec(), Malformed),
        text(
            "a duplicate attribute under two prefixes",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:a=\"{XMI_NS}\" \
                 xmlns:b=\"{XMI_NS}\" a:id=\"p\" b:id=\"p\"/>"
            ),
            Malformed,
        ),
        // --- InvalidRoot and NoPackage (SACM-XMI-001).
        text("a root that is not SACM", "<r/>".to_owned(), InvalidRoot),
        text(
            "a Claim root",
            format!("<sacm:Claim{NS} xmi:id=\"c1\"/>"),
            InvalidRoot,
        ),
        text(
            "a wrapper holding no package",
            format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"),
            NoPackage,
        ),
    ]
}

/// A failure unless a [`Expect::Preserved`] input's tolerant findings hold
/// exactly one SACM-XMI-003, a Warning, and any other input's none.
fn unknown_content_failure(what: &str, expect: Expect, tolerant: &[Finding]) -> Option<String> {
    let unknown: Vec<Severity> = tolerant
        .iter()
        .filter(|found| found.code == Code::XMI_UNKNOWN_CONTENT)
        .map(|found| found.severity)
        .collect();
    let expected: &[Severity] = if expect == Expect::Preserved {
        &[Severity::Warning]
    } else {
        &[]
    };
    (unknown != expected)
        .then(|| format!("{what}: SACM-XMI-003 in tolerant mode is {unknown:?}, not {expected:?}"))
}

#[test]
fn lib_compat_001_both_modes_give_the_same_document_or_the_same_refusal_for_each_input() {
    let cases = table();
    for expect in [
        Expect::Reads,
        Expect::Preserved,
        Expect::Unsupported,
        Expect::Doctype,
        Expect::Malformed,
        Expect::InvalidRoot,
        Expect::NoPackage,
        Expect::DuplicateId,
    ] {
        assert!(
            cases.iter().any(|&(_, _, wanted)| wanted == expect),
            "{COMPAT_001}: setup: the table has no input that gives {expect:?}"
        );
    }
    let mut failures = Vec::new();
    for (what, input, expect) in &cases {
        let tolerant = read_in(Mode::Tolerant, input);
        let strict = read_in(Mode::Strict, input);
        if category(&tolerant) != Some(*expect) {
            failures.push(format!(
                "{what}: setup: tolerant mode gives {:?}, not {expect:?}: {:?}",
                category(&tolerant),
                tolerant.as_ref().err()
            ));
            continue;
        }
        match (&strict, &tolerant) {
            (Ok(strict), Ok(tolerant)) => {
                let differences = semantic_compare(strict.document(), tolerant.document());
                if !differences.is_empty() || strict.document() != tolerant.document() {
                    failures.push(format!("{what}: the documents differ: {differences:?}"));
                }
                if (strict.mode(), tolerant.mode()) != (Mode::Strict, Mode::Tolerant) {
                    failures.push(format!(
                        "{what}: the outputs report the modes {:?} and {:?}",
                        strict.mode(),
                        tolerant.mode()
                    ));
                }
                // The same findings, apart from the severity of SACM-XMI-010
                // and of SACM-XMI-003 (ADR 0033).
                let strict_findings = findings(strict.diagnostics());
                let tolerant_findings = findings(tolerant.diagnostics());
                let as_tolerant: Vec<Finding> = strict_findings
                    .iter()
                    .cloned()
                    .map(|mut found| {
                        if (found.code == Code::XMI_XSI_TYPE
                            || found.code == Code::XMI_UNKNOWN_CONTENT)
                            && found.severity == Severity::Error
                        {
                            found.severity = Severity::Warning;
                        }
                        found
                    })
                    .collect();
                if as_tolerant != tolerant_findings {
                    failures.push(format!(
                        "{what}: the diagnostics differ by more than SACM-XMI-010's severity:\n\
                         strict {strict_findings:?}\ntolerant {tolerant_findings:?}"
                    ));
                }
                failures.extend(unknown_content_failure(what, *expect, &tolerant_findings));
            }
            (Err(strict), Err(tolerant)) => {
                if mem::discriminant(strict) != mem::discriminant(tolerant)
                    || strict.code() != tolerant.code()
                    || strict.location() != tolerant.location()
                    || strict.locations() != tolerant.locations()
                {
                    failures.push(format!(
                        "{what}: the refusals differ:\nstrict {strict:?}\ntolerant {tolerant:?}"
                    ));
                }
            }
            (strict, tolerant) => failures.push(format!(
                "{what}: one mode reads and the other refuses:\nstrict {:?}\ntolerant {:?}",
                strict.as_ref().err(),
                tolerant.as_ref().err()
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{COMPAT_001}: {} of {} inputs are not read or refused the same in both modes:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// LIB-COMPAT-001 3. Golden files and written output have no read diagnostic
// ===========================================================================

#[test]
fn lib_compat_001_every_golden_file_reads_with_no_diagnostic_in_either_mode() {
    let mut expected: Vec<String> = READABLE_GOLDEN
        .iter()
        .chain([&UNREADABLE_GOLDEN])
        .map(|&name| name.to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        golden_names(COMPAT_001),
        expected,
        "{COMPAT_001}: setup: the golden files are not the ones this test knows"
    );
    for name in READABLE_GOLDEN {
        let text = golden(COMPAT_001, name);
        for mode in MODES {
            let output = read_ok(COMPAT_001, name, mode, &text);
            assert_eq!(
                output.diagnostics(),
                [],
                "{COMPAT_001}: the golden file {name} read in {mode:?} mode has diagnostics"
            );
        }
    }
    // The nested golden file holds kinds this release does not read: a
    // strict read refuses it, and a tolerant read keeps them (ADR 0035).
    let text = golden(COMPAT_001, UNREADABLE_GOLDEN);
    let strict = read_err(COMPAT_001, UNREADABLE_GOLDEN, Mode::Strict, &text);
    assert!(
        matches!(strict, ReadError::Unsupported { .. }),
        "{COMPAT_001}: {UNREADABLE_GOLDEN} is not refused as Unsupported in strict mode: {strict:?}"
    );
}

#[test]
fn lib_compat_001_written_output_of_documents_built_with_edit_reads_with_no_diagnostic() {
    for (what, doc) in built_with_edit(COMPAT_001) {
        let text = match xmi::write(&doc, &WriteOptions::default()) {
            Ok(text) => text,
            Err(error) => panic!("{COMPAT_001}: {what}: does not write: {error} ({error:?})"),
        };
        for mode in MODES {
            let output = read_ok(COMPAT_001, what, mode, &text);
            assert_eq!(
                output.diagnostics(),
                [],
                "{COMPAT_001}: {what}: the written output read in {mode:?} mode has diagnostics\n{text}"
            );
            assert_eq!(
                semantic_compare(&doc, output.document()),
                [],
                "{COMPAT_001}: {what}: the written output does not read back equal\n{text}"
            );
        }
    }
}

// ===========================================================================
// LIB-COMPAT-001 4. xsi:type is SACM-XMI-010
// ===========================================================================

#[test]
fn lib_compat_001_xsi_type_on_a_root_naming_its_class_is_one_sacm_xmi_010_at_the_attribute() {
    for kind in ["ArgumentPackage", "AssuranceCasePackage"] {
        let input = format!("<sacm:{kind}{NS_XSI} xmi:id=\"a\"\n  xsi:type=\"sacm:{kind}\"/>");
        let plain = format!("<sacm:{kind}{NS} xmi:id=\"a\"/>");
        for mode in MODES {
            assert_reads_with(
                &format!("xsi:type on a bare {kind}"),
                mode,
                &input,
                &plain,
                &[xsi_type(mode, "a", loc(2, 3))],
            );
        }
    }
    // A wrapped root.
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:xsi=\"{XSI_NS}\">\n\
         <sacm:ArgumentPackage xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>\n</xmi:XMI>"
    );
    let plain = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\n\
         <sacm:ArgumentPackage xmi:id=\"a\"/>\n</xmi:XMI>"
    );
    for mode in MODES {
        assert_reads_with(
            "xsi:type on a wrapped root",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "a", position_of(&input, "xsi:type="))],
        );
    }
}

#[test]
fn lib_compat_001_xsi_type_on_an_argument_element_claim_is_one_sacm_xmi_010_affecting_the_claim() {
    let input = ap("<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"/>");
    let plain = ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>");
    for mode in MODES {
        let output = assert_reads_with(
            "xsi:type on a Claim",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "c1", position_of(&input, "xsi:type="))],
        );
        assert_eq!(
            output.document().get("c1").map(SacmElement::kind),
            Some(ElementKind::Claim),
            "{COMPAT_001}: the Claim typed with xsi:type is not read in {mode:?} mode"
        );
    }
    // A redundant xsi:type on a concrete role, too.
    let input = acp("<argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\"/>");
    let plain = acp("<argumentPackage xmi:id=\"ap\"/>");
    for mode in MODES {
        assert_reads_with(
            "a redundant xsi:type on argumentPackage",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "ap", position_of(&input, "xsi:type="))],
        );
    }
}

#[test]
fn lib_compat_001_xsi_type_on_a_name_is_one_sacm_xmi_010_affecting_the_owner() {
    let input = ap("<name xsi:type=\"sacm:LangString\" lang=\"en\" content=\"P\"/>");
    let plain = ap("<name lang=\"en\" content=\"P\"/>");
    for mode in MODES {
        assert_reads_with(
            "xsi:type on a root's name",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "p", position_of(&input, "xsi:type="))],
        );
    }
    let input = ap(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    \
         <name xsi:type=\"sacm:LangString\" content=\"C\"/>\n  </argumentElement>",
    );
    let plain = ap(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    \
         <name content=\"C\"/>\n  </argumentElement>",
    );
    for mode in MODES {
        assert_reads_with(
            "xsi:type on a Claim's name",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "c1", position_of(&input, "xsi:type="))],
        );
    }
}

#[test]
fn lib_compat_001_an_element_with_both_xmi_type_and_xsi_type_gives_one_sacm_xmi_010() {
    let cases = [
        (
            "a root",
            format!(
                "<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\" xmi:type=\"sacm:ArgumentPackage\" \
                 xsi:type=\"sacm:ArgumentPackage\"/>"
            ),
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>"),
            "a",
        ),
        (
            "a Claim, xsi:type first",
            ap("<argumentElement xsi:type=\"sacm:Claim\" xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>"),
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>"),
            "c1",
        ),
        (
            "a name",
            ap("<name xmi:type=\"sacm:LangString\" xsi:type=\"sacm:LangString\" content=\"P\"/>"),
            ap("<name content=\"P\"/>"),
            "p",
        ),
    ];
    for (what, input, plain, element) in &cases {
        for mode in MODES {
            assert_reads_with(
                &format!("both type attributes on {what}"),
                mode,
                input,
                plain,
                &[xsi_type(mode, element, position_of(input, "xsi:type="))],
            );
        }
    }
}

#[test]
fn lib_compat_001_xsi_type_under_any_prefix_bound_to_xsi_is_sacm_xmi_010() {
    let plain = format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>");
    for prefix in ["i", "XSI", "ns1", "ü"] {
        let input = format!(
            "<sacm:ArgumentPackage{NS} xmlns:{prefix}=\"{XSI_NS}\" xmi:id=\"a\"\n  \
             {prefix}:type=\"sacm:ArgumentPackage\"/>"
        );
        for mode in MODES {
            assert_reads_with(
                &format!("the XSI prefix {prefix:?}"),
                mode,
                &input,
                &plain,
                &[xsi_type(mode, "a", loc(2, 3))],
            );
        }
    }
    // The prefix `xmi` bound to XSI makes `xmi:type` an XSI type; `x` is XMI.
    let input = format!(
        "<s:ArgumentPackage xmlns:s=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\" xmlns:xmi=\"{XSI_NS}\" \
         x:id=\"a\"\n  xmi:type=\"s:ArgumentPackage\"/>"
    );
    for mode in MODES {
        assert_reads_with(
            "xmi bound to XSI",
            mode,
            &input,
            &plain,
            &[xsi_type(mode, "a", loc(2, 3))],
        );
    }
    // The prefix `xsi` bound elsewhere is no XSI type: no SACM-XMI-010, but
    // an attribute the class lacks, kept as preserved content of the root in
    // both modes (SACM23-XMI-002, ADR 0033).
    let foreign = format!(
        "<sacm:ArgumentPackage{NS} xmlns:xsi=\"urn:not-xsi\" xmi:id=\"a\"\n  \
         xsi:type=\"sacm:ArgumentPackage\"/>"
    );
    for mode in MODES {
        let output = read_ok(COMPAT_001, "xsi bound to urn:not-xsi", mode, &foreign);
        assert_eq!(
            without_clauses(output.diagnostics()),
            [unknown_content(mode, "a", loc(2, 3))],
            "{COMPAT_001}: xsi bound to urn:not-xsi is not one SACM-XMI-003 at the attribute in \
             {mode:?} mode"
        );
    }
}

#[test]
fn lib_compat_001_a_document_with_neither_attribute_gets_no_read_diagnostic() {
    let cases = [
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>"),
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" xmi:type=\"sacm:ArgumentPackage\"/>"),
        // An XSI declaration, unused.
        format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\"/>"),
        ap("<name lang=\"en\" content=\"P\"/>\n  \
            <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"defeated\"/>"),
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
             <sacm:ArgumentPackage xmi:id=\"a\"/><sacm:AssuranceCasePackage xmi:id=\"n\"/></xmi:XMI>"
        ),
        // `version` in no namespace is not `xmi:version`, and the XML
        // declaration's version is not an attribute.
        format!("<?xml version=\"1.0\"?>\n<sacm:ArgumentPackage{NS} xmi:id=\"a\"/>"),
    ];
    for input in &cases {
        for mode in MODES {
            let output = read_ok(COMPAT_001, "an input with neither attribute", mode, input);
            assert_eq!(
                output.diagnostics(),
                [],
                "{COMPAT_001}: {input:?} read in {mode:?} mode has diagnostics"
            );
        }
    }
}

// ===========================================================================
// LIB-COMPAT-001 5. xmi:version is SACM-XMI-011
// ===========================================================================

#[test]
fn lib_compat_001_xmi_version_on_the_wrapper_and_each_of_three_roots_is_one_sacm_xmi_011_each() {
    for version in ["2.0", "2.1", ""] {
        let v = format!("xmi:version=\"{version}\"");
        let input = format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"\n  {v}>\n  \
             <sacm:ArgumentPackage xmi:id=\"a1\"\n    {v}/>\n  \
             <sacm:AssuranceCasePackage {v} xmi:id=\"n1\"/>\n  \
             <sacm:ArgumentPackage xmi:id=\"a2\" {v}/>\n</xmi:XMI>"
        );
        let plain = input.replace(&format!(" {v}"), "");
        assert!(
            !plain.contains("version"),
            "setup: the plain input still has xmi:version: {plain:?}"
        );
        let at = positions_of(&input, "xmi:version=");
        assert_eq!(at.len(), 4, "setup: the input has not four xmi:version");
        for mode in MODES {
            assert_reads_with(
                &format!("xmi:version={version:?} on the wrapper and three roots"),
                mode,
                &input,
                &plain,
                &[
                    xmi_version(None, at[0]),
                    xmi_version(Some("a1"), at[1]),
                    xmi_version(Some("n1"), at[2]),
                    xmi_version(Some("a2"), at[3]),
                ],
            );
        }
        // A bare root.
        let bare = format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"\n  {v}/>");
        let bare_plain = format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"/>");
        for mode in MODES {
            assert_reads_with(
                &format!("xmi:version={version:?} on a bare root"),
                mode,
                &bare,
                &bare_plain,
                &[xmi_version(Some("case"), loc(2, 3))],
            );
        }
    }
}

#[test]
fn lib_compat_001_xmi_version_under_another_xmi_prefix_is_sacm_xmi_011() {
    let input = format!(
        "<x:XMI xmlns:x=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\" x:version=\"2.0\">\
         <s:ArgumentPackage x:id=\"a\" x:version=\"2.0\"/></x:XMI>"
    );
    let plain = format!(
        "<x:XMI xmlns:x=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\"><s:ArgumentPackage x:id=\"a\"/></x:XMI>"
    );
    let at = positions_of(&input, "x:version=");
    for mode in MODES {
        assert_reads_with(
            "x:version",
            mode,
            &input,
            &plain,
            &[xmi_version(None, at[0]), xmi_version(Some("a"), at[1])],
        );
    }
}

#[test]
fn lib_compat_001_xmi_version_on_a_child_is_preserved_in_both_modes() {
    // `xmi:version` belongs on a root or the wrapper only: on a child it does
    // not conform, and is kept as preserved content of the child, with one
    // SACM-XMI-003 and no SACM-XMI-011 (ADR 0033).
    let children = [
        (
            acp("<argumentPackage xmi:id=\"ap\" xmi:version=\"2.0\"/>"),
            "ap",
        ),
        (
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:version=\"2.1\" xmi:id=\"c1\"/>"),
            "c1",
        ),
        (
            ap("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" xmi:version=\"\"/>"),
            "c1",
        ),
    ];
    for (input, holder) in &children {
        let at = position_of(input, "xmi:version=");
        let mut documents = Vec::new();
        for mode in MODES {
            let output = read_ok(COMPAT_001, "xmi:version on a child", mode, input);
            assert_eq!(
                without_clauses(output.diagnostics()),
                [unknown_content(mode, holder, at)],
                "{COMPAT_001}: xmi:version on a child is not one SACM-XMI-003 at its name in \
                 {mode:?} mode"
            );
            assert!(
                output
                    .document()
                    .get(*holder)
                    .is_some_and(SacmElement::has_preserved_content),
                "{COMPAT_001}: {holder:?} holds no preserved content in {mode:?} mode"
            );
            documents.push(output.into_document());
        }
        assert_eq!(
            documents[0], documents[1],
            "{COMPAT_001}: xmi:version on a child reads differently in the two modes"
        );
    }
}

// ===========================================================================
// LIB-COMPAT-001 6. Both attributes: the document is unchanged
// ===========================================================================

/// An input with `xmi:version` on the wrapper and the roots and `xsi:type` on
/// a root, a child, a Claim and a name, every element named; and the same
/// input without them, with `xmi:type` where the role needs a type.
fn with_both_attributes() -> (String, String) {
    let with = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:xsi=\"{XSI_NS}\" xmi:version=\"2.0\">\n\
         \x20 <sacm:AssuranceCasePackage xmi:id=\"case\" xmi:version=\"2.0\" xsi:type=\"sacm:AssuranceCasePackage\">\n\
         \x20   <name xsi:type=\"sacm:LangString\" lang=\"en\" content=\"Case\"/>\n\
         \x20   <argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"arguments\">\n\
         \x20     <name lang=\"en\" content=\"Arguments\"/>\n\
         \x20     <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"assumed\">\n\
         \x20       <name lang=\"en\" content=\"Top\"/>\n\
         \x20     </argumentElement>\n\
         \x20   </argumentPackage>\n\
         \x20 </sacm:AssuranceCasePackage>\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"more\" xmi:version=\"2.1\">\n\
         \x20   <name lang=\"en\" content=\"More\"/>\n\
         \x20 </sacm:ArgumentPackage>\n\
         </xmi:XMI>\n"
    );
    let without = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\n\
         \x20 <sacm:AssuranceCasePackage xmi:id=\"case\">\n\
         \x20   <name lang=\"en\" content=\"Case\"/>\n\
         \x20   <argumentPackage xmi:id=\"arguments\">\n\
         \x20     <name lang=\"en\" content=\"Arguments\"/>\n\
         \x20     <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"assumed\">\n\
         \x20       <name lang=\"en\" content=\"Top\"/>\n\
         \x20     </argumentElement>\n\
         \x20   </argumentPackage>\n\
         \x20 </sacm:AssuranceCasePackage>\n\
         \x20 <sacm:ArgumentPackage xmi:id=\"more\">\n\
         \x20   <name lang=\"en\" content=\"More\"/>\n\
         \x20 </sacm:ArgumentPackage>\n\
         </xmi:XMI>\n"
    );
    (with, without)
}

#[test]
fn lib_compat_001_an_input_with_both_attributes_reads_equal_and_writes_the_same_bytes_without_them()
{
    let (with, without) = with_both_attributes();
    let at_version = positions_of(&with, "xmi:version=");
    let at_type = positions_of(&with, "xsi:type=");
    for mode in MODES {
        let mut expected = vec![
            xmi_version(None, at_version[0]),
            xmi_version(Some("case"), at_version[1]),
            xsi_type(mode, "case", at_type[0]),
            xsi_type(mode, "case", at_type[1]),
            xsi_type(mode, "arguments", at_type[2]),
            xsi_type(mode, "c1", at_type[3]),
            xmi_version(Some("more"), at_version[2]),
        ];
        expected.sort_by_key(|found| found.location);
        let output = assert_reads_with("both attributes", mode, &with, &without, &expected);
        let plain = read_ok(COMPAT_001, "the input without them", mode, &without);
        assert!(
            output.document() == plain.document(),
            "{COMPAT_001}: read in {mode:?} mode, the document with both attributes is not == \
             the one without them"
        );

        let text = write_ok(
            COMPAT_001,
            "with both attributes",
            Mode::Strict,
            output.document(),
        );
        let reference = write_ok(COMPAT_001, "without them", Mode::Strict, plain.document());
        assert_eq!(
            text, reference,
            "{COMPAT_001}: read in {mode:?} mode, the input with both attributes writes other bytes"
        );
        assert!(
            text.contains("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\""),
            "{COMPAT_001}: the Claim is not written with xmi:type\n{text}"
        );
        assert!(
            !text.contains("version=\"2") && !text.contains(":version") && !text.contains("xsi"),
            "{COMPAT_001}: the output carries xmi:version or xsi\n{text}"
        );
        let back = read_ok(COMPAT_001, "the output", Mode::Strict, &text);
        assert_eq!(
            back.diagnostics(),
            [],
            "{COMPAT_001}: the output read strictly has diagnostics\n{text}"
        );
        assert_eq!(
            semantic_compare(back.document(), output.document()),
            [],
            "{COMPAT_001}: the output does not read back equal\n{text}"
        );
    }
}

// ===========================================================================
// LIB-COMPAT-001 7. Order, determinism and has_errors
// ===========================================================================

#[test]
fn lib_compat_001_read_diagnostics_are_sorted_and_identical_on_every_run() {
    let (with, _) = with_both_attributes();
    for mode in MODES {
        let first = read_ok(COMPAT_001, "both attributes", mode, &with);
        let mut sorted = first.diagnostics().to_vec();
        sorted.sort();
        assert_eq!(
            first.diagnostics(),
            sorted.as_slice(),
            "{COMPAT_001}: the diagnostics read in {mode:?} mode are not in Diagnostic order"
        );
        assert_eq!(
            first.diagnostics().len(),
            7,
            "{COMPAT_001}: setup: the input does not give seven diagnostics in {mode:?} mode"
        );
        for _ in 0..3 {
            let again = read_ok(COMPAT_001, "both attributes", mode, &with);
            assert_eq!(
                again.diagnostics(),
                first.diagnostics(),
                "{COMPAT_001}: reading again in {mode:?} mode gives other diagnostics"
            );
            assert_eq!(
                again
                    .diagnostics()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                first
                    .diagnostics()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                "{COMPAT_001}: reading again in {mode:?} mode displays other diagnostics"
            );
        }
    }
}

#[test]
fn lib_compat_001_has_errors_is_true_only_for_xsi_type_read_strictly() {
    let typed =
        format!("<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>");
    let versioned = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\
         <sacm:ArgumentPackage xmi:id=\"a\" xmi:version=\"2.0\"/></xmi:XMI>"
    );
    let strict = read_ok(COMPAT_001, "xsi:type", Mode::Strict, &typed);
    assert!(
        diagnostic::has_errors(strict.diagnostics()),
        "{COMPAT_001}: xsi:type read strictly has no Error: {:?}",
        strict.diagnostics()
    );
    let tolerant = read_ok(COMPAT_001, "xsi:type", Mode::Tolerant, &typed);
    assert!(
        !tolerant.diagnostics().is_empty() && !diagnostic::has_errors(tolerant.diagnostics()),
        "{COMPAT_001}: xsi:type read tolerantly has no diagnostic, or an Error: {:?}",
        tolerant.diagnostics()
    );
    for mode in MODES {
        let output = read_ok(COMPAT_001, "xmi:version only", mode, &versioned);
        assert!(
            !output.diagnostics().is_empty() && !diagnostic::has_errors(output.diagnostics()),
            "{COMPAT_001}: xmi:version only, read in {mode:?} mode, has no diagnostic or an \
             Error: {:?}",
            output.diagnostics()
        );
    }
}

// ===========================================================================
// LIB-API-016. ReadOutput::has_errors
// ===========================================================================

const API_016: &str = "LIB-API-016";

/// A named ArgumentPackage `a` with a Claim typed by `xsi:type`.
fn api_016_xsi_typed() -> String {
    format!(
        "<sacm:ArgumentPackage{NS_XSI} xmi:id=\"a\"><name content=\"A\"/>\
         <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>\
         </argumentElement></sacm:ArgumentPackage>"
    )
}

/// A named ArgumentPackage `a`, clean in both modes.
fn api_016_clean() -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage>")
}

#[test]
fn lib_api_016_has_errors_is_true_only_for_a_strict_read_with_xsi_type() {
    let typed = api_016_xsi_typed();
    let strict = read_ok(API_016, "xsi:type", Mode::Strict, &typed);
    assert!(
        strict.has_errors(),
        "{API_016}: a strict read with xsi:type does not report an Error: {:?}",
        strict.diagnostics()
    );
    let tolerant = read_ok(API_016, "xsi:type", Mode::Tolerant, &typed);
    assert!(
        !tolerant.diagnostics().is_empty(),
        "{API_016}: setup: a tolerant read with xsi:type reports nothing"
    );
    assert!(
        !tolerant.has_errors(),
        "{API_016}: a tolerant read with xsi:type, whose finding is a Warning, reports an \
         Error: {:?}",
        tolerant.diagnostics()
    );
    let clean = api_016_clean();
    for mode in MODES {
        let output = read_ok(API_016, "clean", mode, &clean);
        assert!(
            output.diagnostics().is_empty(),
            "{API_016}: setup: the clean input has read diagnostics in {mode:?} mode: {:?}",
            output.diagnostics()
        );
        assert!(
            !output.has_errors(),
            "{API_016}: a clean read in {mode:?} mode reports an Error"
        );
    }
}

#[test]
fn lib_api_016_has_errors_agrees_with_diagnostic_has_errors() {
    let versioned = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\
         <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage></xmi:XMI>"
    );
    let unknown = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"a\"><name content=\"A\"/><unknownChild/>\
         </sacm:ArgumentPackage>"
    );
    // The SACM-XMI-003 input is an Error read strictly and a Warning read
    // tolerantly, so the comparison below meets both answers.
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = read_ok(API_016, "an element that is not SACM 2.3", mode, &unknown);
        assert!(
            output
                .diagnostics()
                .iter()
                .any(|d| d.code() == Code::XMI_UNKNOWN_CONTENT && d.severity() == severity),
            "{API_016}: setup: <unknownChild/> read in {mode:?} mode is not SACM-XMI-003 as \
             {severity:?}: {:?}",
            output.diagnostics()
        );
    }
    let inputs = [
        ("xsi:type", api_016_xsi_typed()),
        ("xmi:version only", versioned),
        ("an element that is not SACM 2.3", unknown),
        ("clean", api_016_clean()),
    ];
    for (what, input) in &inputs {
        for mode in MODES {
            let output = read_ok(API_016, what, mode, input);
            assert_eq!(
                output.has_errors(),
                diagnostic::has_errors(output.diagnostics()),
                "{API_016}: {what}, read in {mode:?} mode: has_errors disagrees with \
                 diagnostic::has_errors on {:?}",
                output.diagnostics()
            );
        }
    }
}

// ===========================================================================
// LIB-COMPAT-007 1. Content Errors never refuse a strict write
// ===========================================================================

/// A bare ArgumentPackage `p`, named, holding `content`.
fn named_ap(content: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{content}</sacm:ArgumentPackage>"
    )
}

/// A Claim `id`, named after its id, carrying `attributes`.
fn named_claim(id: &str, attributes: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/></argumentElement>"
    )
}

/// Reads `input` tolerantly, which the test expects to read with no read
/// diagnostic, and returns the document.
fn document_of(what: &str, input: &str) -> Document {
    let output = read_ok(COMPAT_007, what, Mode::Tolerant, input);
    assert_eq!(
        output.diagnostics(),
        [],
        "{COMPAT_007}: {what}: setup: the read reports a diagnostic"
    );
    output.into_document()
}

/// Whether `code` is one the strict-save schema check gives: SACM-ID-002 or
/// SACM-REF-001 (ADR 0035 D4).
fn is_schema_code(code: Code) -> bool {
    code == Code::ID_INVALID || code == Code::REF_DANGLING
}

/// A diagnostic as its code, severity and affected ids.
fn coded(found: &Diagnostic) -> (Code, Severity, Vec<String>) {
    (
        found.code(),
        found.severity(),
        found
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    )
}

/// `(code, Error, ids)`, as [`coded`] gives it.
fn error_of(code: Code, ids: &[&str]) -> (Code, Severity, Vec<String>) {
    (
        code,
        Severity::Error,
        ids.iter().map(|&id| id.to_owned()).collect(),
    )
}

/// A strict write is schema-only (ADR 0035 D4; XMI 2.5.1 §7.8.2 "By default,
/// XMI 2 produces schemas that ignore multiplicities also"; SACM §2.2): a
/// document that `validate` reports a model Error for, and no schema error,
/// writes strictly, byte-identical to a tolerant write. So do the golden
/// files, the nameless drafts included, a document whose only findings are
/// Warnings, and one read with `xsi:type` and `xmi:version`.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn lib_compat_007_content_errors_never_refuse_a_strict_write() {
    let cases = [
        (
            "a nameless package",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\"/>"),
            Code::MULTIPLICITY_VIOLATION,
        ),
        (
            "a relationship without a source or a target",
            named_ap(&format!(
                "{}<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\">\
                 <name content=\"I1\"/></argumentElement>",
                named_claim("c1", "")
            )),
            Code::MULTIPLICITY_VIOLATION,
        ),
        (
            "a duplicate gid",
            named_ap(&format!(
                "{}{}",
                named_claim("c1", " gid=\"g\""),
                named_claim("c2", " gid=\"g\"")
            )),
            Code::GID_DUPLICATE,
        ),
        (
            "a metaClaim naming an ArgumentReasoning (SACM-REF-002)",
            named_ap(&format!(
                "{}<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\">\
                 <name content=\"R1\"/></argumentElement>",
                named_claim("c1", " metaClaim=\"r1\"")
            )),
            Code::REF_WRONG_TYPE,
        ),
        (
            "an ArgumentPackage nesting a package beside a Claim (SACM-PKG-001)",
            named_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"q\">\
                 <name content=\"Q\"/></argumentElement>{}",
                named_claim("c1", "")
            )),
            Code::PACKAGE_CONTENT_INVALID,
        ),
        (
            "an implementation constraint on a concrete element (SACM-ABS-001)",
            named_ap(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
                 <implementationConstraint xmi:id=\"k1\"><content><value content=\"k\"/></content>\
                 </implementationConstraint></argumentElement>",
            ),
            Code::ABSTRACT_FORM_INVALID,
        ),
        (
            "a citedElement without isCitation (SACM-CITE-001)",
            named_ap(&format!(
                "{}{}",
                named_claim("c1", " citedElement=\"c2\""),
                named_claim("c2", "")
            )),
            Code::CITATION_INVALID,
        ),
    ];
    for (what, input, code) in &cases {
        let doc = document_of(what, input);
        let errors: Vec<Code> = validation_errors(&doc)
            .iter()
            .map(Diagnostic::code)
            .collect();
        assert!(
            errors.contains(code) && !errors.iter().any(|&found| is_schema_code(found)),
            "{COMPAT_007}: {what}: setup: validate does not give {code} without a schema \
             error: {errors:?}"
        );
        let before = doc.clone();
        let tolerant = write_ok(COMPAT_007, what, Mode::Tolerant, &doc);
        let strict = write_ok(COMPAT_007, what, Mode::Strict, &doc);
        assert_eq!(
            strict, tolerant,
            "{COMPAT_007}: {what}: the strict write is not the tolerant one"
        );
        assert!(
            doc == before,
            "{COMPAT_007}: {what}: writing changed the document"
        );
    }

    // The golden files, the nameless drafts among them: each writes back
    // unchanged in both modes.
    let mut drafts = 0;
    for name in READABLE_GOLDEN {
        let text = golden(COMPAT_007, name);
        let doc = read_ok(COMPAT_007, name, Mode::Tolerant, &text).into_document();
        if !validation_errors(&doc).is_empty() {
            drafts += 1;
        }
        for mode in MODES {
            assert_eq!(
                write_ok(COMPAT_007, name, mode, &doc),
                text,
                "{COMPAT_007}: {name} is not written back unchanged in {mode:?} mode"
            );
        }
    }
    assert!(
        drafts >= 2,
        "{COMPAT_007}: setup: fewer than two golden files have a validation Error ({drafts})"
    );

    // A document whose only findings are Warnings: two descriptions on a
    // Claim (SACM23-VAL-011) and a repeated metaClaim (SACM23-VAL-022).
    let what = "a document whose only findings are Warnings";
    let doc = document_of(
        what,
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name lang=\"en\" content=\"P\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"m1\">\
             <name lang=\"en\" content=\"M\"/></argumentElement>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" metaClaim=\"m1 m1\">\
             <name lang=\"en\" content=\"C\"/>\
             <description xmi:id=\"d1\"><content><value content=\"one\"/></content></description>\
             <description xmi:id=\"d2\"><content><value content=\"two\"/></content></description>\
             </argumentElement></sacm:ArgumentPackage>"
        ),
    );
    let found = validation::validate(&doc);
    assert!(
        !found.is_empty() && !diagnostic::has_errors(&found),
        "{COMPAT_007}: {what}: setup: validate does not give Warnings alone: {found:?}"
    );
    let strict = write_ok(COMPAT_007, what, Mode::Strict, &doc);
    assert_eq!(
        strict,
        write_ok(COMPAT_007, what, Mode::Tolerant, &doc),
        "{COMPAT_007}: {what}: the strict and tolerant writes differ"
    );
    let back = read_ok(COMPAT_007, "the strict output", Mode::Strict, &strict).into_document();
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{COMPAT_007}: {what}: the strict output does not read back equal\n{strict}"
    );

    // A document read tolerantly with xsi:type and xmi:version: their read
    // diagnostics are not schema errors of the output.
    let (with, _) = with_both_attributes();
    let doc = read_ok(COMPAT_007, "both attributes", Mode::Tolerant, &with).into_document();
    let text = write_ok(COMPAT_007, "both attributes", Mode::Strict, &doc);
    assert!(
        text.contains("xmi:type=\"sacm:Claim\"")
            && !text.contains("xmi:version")
            && !text.contains("xsi:"),
        "{COMPAT_007}: both attributes: the strict output is not in the standard form\n{text}"
    );
}

// ===========================================================================
// LIB-COMPAT-007 2. The schema errors: SACM-ID-002 and SACM-REF-001
// ===========================================================================

/// An `xmi:id` is an `xsd:ID` (XMI.xsd `<xsd:attribute name="id"
/// type="xsd:ID"/>`), whose lexical space is the `NCName`s (XML Schema
/// Part 2 §3.3.8). One SACM-ID-002 per such id, of an element, a name, a
/// `content` or a `value`, affecting the element that owns it.
#[test]
fn lib_compat_007_a_non_ncname_id_is_one_id_002() {
    let cases = [
        (
            "xmi:id=\"1c\" on a Claim",
            named_ap(&named_claim("1c", "")),
            "1c",
        ),
        (
            "name id 1n",
            named_ap(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 <name xmi:id=\"1n\" content=\"C1\"/></argumentElement>",
            ),
            "c1",
        ),
        (
            "content id 1t on a Note",
            named_ap(
                "<note xmi:id=\"n\"><content xmi:id=\"1t\"><value content=\"x\"/></content></note>",
            ),
            "n",
        ),
        (
            "key id 1k on a TaggedValue",
            named_ap(
                "<taggedValue xmi:id=\"t\"><content><value content=\"x\"/></content>\
                 <key xmi:id=\"1k\"><value content=\"k\"/></key></taggedValue>",
            ),
            "t",
        ),
        (
            "value id 1v in a Note's content",
            named_ap(
                "<note xmi:id=\"n\"><content><value xmi:id=\"1v\" content=\"x\"/></content></note>",
            ),
            "n",
        ),
    ];
    for (what, input, owner) in &cases {
        let doc = document_of(what, input);
        let error = write_err(COMPAT_007, what, Mode::Strict, &doc);
        assert!(
            matches!(error, WriteError::Invalid { .. }),
            "{COMPAT_007}: {what}: the strict write is not Invalid: {error:?}"
        );
        assert_eq!(
            error.diagnostics().iter().map(coded).collect::<Vec<_>>(),
            [error_of(Code::ID_INVALID, &[owner])],
            "{COMPAT_007}: {what}: not one SACM-ID-002 Error affecting {owner:?}"
        );
        let text = write_ok(COMPAT_007, what, Mode::Tolerant, &doc);
        assert!(
            text.contains("xmi:id=\"1"),
            "{COMPAT_007}: {what}: the tolerant write does not keep the id\n{text}"
        );
    }
}

/// `WriteOptions` default to strict, and `with_mode` sets the mode; the
/// default refuses output that would not be schema-valid, which tolerant mode
/// writes (ADR 0022, ADR 0035 D4).
#[test]
fn lib_compat_007_write_options_default_to_strict_and_with_mode_sets_the_mode() {
    assert_eq!(
        WriteOptions::default().mode(),
        Mode::Strict,
        "{COMPAT_007}: WriteOptions::default() is not strict"
    );
    assert_eq!(
        WriteOptions::default().with_mode(Mode::Tolerant).mode(),
        Mode::Tolerant,
        "{COMPAT_007}: with_mode(Mode::Tolerant) does not set tolerant"
    );
    assert_eq!(
        WriteOptions::default()
            .with_mode(Mode::Tolerant)
            .with_mode(Mode::Strict)
            .mode(),
        Mode::Strict,
        "{COMPAT_007}: a later with_mode does not replace an earlier one"
    );
    assert_eq!(
        WriteOptions::default().with_mode(Mode::Strict),
        WriteOptions::default(),
        "{COMPAT_007}: strict write options are not the default ones"
    );
    // The default refuses what only tolerant mode writes.
    let what = "citedElement=\"nope\"";
    let dangling = document_of(
        what,
        &named_ap(&named_claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\"",
        )),
    );
    assert!(
        matches!(
            xmi::write(&dangling, &WriteOptions::default()),
            Err(WriteError::Invalid { .. })
        ),
        "{COMPAT_007}: the default write does not refuse a dangling reference"
    );
    write_ok(COMPAT_007, what, Mode::Tolerant, &dangling);
}

/// An IDREF naming no ID is schema-invalid (XML Schema Part 1 §3.3.4,
/// Validation Root Valid (ID/IDREF) clause 1; XMI 2.5.1 §8.2.1 rule 4i): one
/// SACM-REF-001 affecting the element and the id.
#[test]
fn lib_compat_007_a_dangling_reference_is_one_ref_001() {
    let what = "citedElement=\"nope\"";
    let doc = document_of(
        what,
        &named_ap(&named_claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\"",
        )),
    );
    let error = write_err(COMPAT_007, what, Mode::Strict, &doc);
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{COMPAT_007}: {what}: the strict write is not Invalid: {error:?}"
    );
    assert_eq!(
        error.diagnostics().iter().map(coded).collect::<Vec<_>>(),
        [error_of(Code::REF_DANGLING, &["c1", "nope"])],
        "{COMPAT_007}: {what}: not one SACM-REF-001 Error affecting [c1, nope]"
    );
    // A tolerant write writes it as held (ADR 0026).
    let text = write_ok(COMPAT_007, what, Mode::Tolerant, &doc);
    assert!(
        text.contains("citedElement=\"nope\""),
        "{COMPAT_007}: {what}: the tolerant write does not keep the reference\n{text}"
    );
}

/// The `xmi:id` of a name is an ID the output writes, so a reference to it
/// is XSD-valid and a strict write writes it, though `validate` still
/// reports SACM-REF-001 (a name identifies no element; ADR 0024).
#[test]
fn lib_compat_007_a_reference_to_a_name_id_is_written() {
    let what = "a citedElement naming a name's xmi:id";
    let doc = document_of(
        what,
        &named_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name xmi:id=\"n1\" content=\"C1\"/></argumentElement>{}",
            named_claim("c2", " isCitation=\"true\" citedElement=\"n1\"")
        )),
    );
    assert!(
        validation_errors(&doc)
            .iter()
            .any(|found| found.code() == Code::REF_DANGLING),
        "{COMPAT_007}: {what}: setup: validate does not report SACM-REF-001"
    );
    let strict = write_ok(COMPAT_007, what, Mode::Strict, &doc);
    assert!(
        strict.contains("citedElement=\"n1\"") && strict.contains("xmi:id=\"n1\""),
        "{COMPAT_007}: {what}: the strict output does not keep the reference and the id\n{strict}"
    );
    assert_eq!(
        strict,
        write_ok(COMPAT_007, what, Mode::Tolerant, &doc),
        "{COMPAT_007}: {what}: the strict write is not the tolerant one"
    );
}

// ===========================================================================
// LIB-COMPAT-007 3. The order of the refusals
// ===========================================================================

/// A strict write refuses `NoRoot`, then `PreservedContent`, then `Invalid`,
/// then an unrepresentable character (ADR 0035 D4). A tolerant write refuses
/// only the first and the last.
#[test]
fn lib_compat_007_the_refusals_come_in_order() {
    // NoRoot, in both modes.
    for mode in MODES {
        assert_eq!(
            write_err(COMPAT_007, "an empty document", mode, &Document::new()),
            WriteError::NoRoot,
            "{COMPAT_007}: an empty document written in {mode:?} mode is not NoRoot"
        );
    }

    // Preserved content comes before the schema check.
    let what = "preserved content and a dangling reference";
    let doc = read_ok(
        COMPAT_007,
        what,
        Mode::Tolerant,
        &format!(
            "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:v\" xmi:id=\"p\" v:colour=\"red\">\
             <name content=\"P\"/>{}</sacm:ArgumentPackage>",
            named_claim("1c", " isCitation=\"true\" citedElement=\"nope\"")
        ),
    )
    .into_document();
    let error = write_err(COMPAT_007, what, Mode::Strict, &doc);
    assert!(
        matches!(&error, WriteError::PreservedContent { elements, .. }
            if elements.iter().map(ElementId::as_str).eq(["p"])),
        "{COMPAT_007}: {what}: the strict write is not PreservedContent [p]: {error:?}"
    );

    // The schema check comes before an unrepresentable character.
    let what = "a dangling reference and a U+0001 in a name";
    let mut doc = document_of(
        what,
        &named_ap(&named_claim(
            "c1",
            " isCitation=\"true\" citedElement=\"nope\"",
        )),
    );
    if let Err(error) = edit::apply(
        &mut doc,
        edit::SetName::new("p", LangString::new("bad\u{1}")),
    ) {
        panic!("{COMPAT_007}: {what}: setup: the name was not set: {error}");
    }
    let strict = write_err(COMPAT_007, what, Mode::Strict, &doc);
    assert_eq!(
        strict.diagnostics().iter().map(coded).collect::<Vec<_>>(),
        [error_of(Code::REF_DANGLING, &["c1", "nope"])],
        "{COMPAT_007}: {what}: the strict write is not Invalid with the SACM-REF-001: {strict:?}"
    );
    let tolerant = write_err(COMPAT_007, what, Mode::Tolerant, &doc);
    assert!(
        matches!(
            &tolerant,
            WriteError::UnrepresentableCharacter {
                element,
                character: '\u{1}',
                attribute: WriteAttribute::NameContent,
                ..
            } if element.as_str() == "p"
        ),
        "{COMPAT_007}: {what}: the tolerant write does not refuse the U+0001: {tolerant:?}"
    );

    // A content Error is no schema error: with a nameless Claim and a
    // U+0001, the character is what a strict write refuses.
    let mut bad = Document::new();
    insert(
        COMPAT_007,
        &mut bad,
        None,
        ElementKind::ArgumentPackage,
        "p",
        Some(LangString::new("bad\u{1}")),
    );
    insert(
        COMPAT_007,
        &mut bad,
        Some("p"),
        ElementKind::Claim,
        "c1",
        None,
    );
    for mode in MODES {
        let error = write_err(COMPAT_007, "a nameless Claim and a U+0001", mode, &bad);
        assert!(
            matches!(
                &error,
                WriteError::UnrepresentableCharacter {
                    element,
                    attribute: WriteAttribute::NameContent,
                    ..
                } if element.as_str() == "p"
            ),
            "{COMPAT_007}: a nameless Claim and a U+0001 written in {mode:?} mode is not \
             refused for the character: {error:?}"
        );
    }
}

// ===========================================================================
// LIB-COMPAT-007 4. The Invalid refusal's code, element and Display
// ===========================================================================

#[test]
fn lib_compat_007_the_invalid_display_counts_schema_errors() {
    let what = "one dangling target";
    let one = document_of(
        what,
        &named_ap(&format!(
            "{}<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"c1\" \
             target=\"nope\"><name content=\"I1\"/></argumentElement>",
            named_claim("c1", "")
        )),
    );
    let error = write_err(COMPAT_007, what, Mode::Strict, &one);
    assert_eq!(
        (error.code(), error.element()),
        (None, None),
        "{COMPAT_007}: {what}: Invalid has a code or an element"
    );
    assert_eq!(
        error.to_string(),
        "the document cannot be written in strict mode: 1 schema error, SACM-REF-001 on \"i1\"",
        "{COMPAT_007}: {what}: the Display"
    );

    let what = "two non-NCName ids and a dangling reference";
    let three = document_of(
        what,
        &named_ap(&format!(
            "{}{}{}",
            named_claim("c3", " isCitation=\"true\" citedElement=\"nope\""),
            named_claim("2c", ""),
            named_claim("1c", "")
        )),
    );
    let error = write_err(COMPAT_007, what, Mode::Strict, &three);
    assert_eq!(
        error.diagnostics().iter().map(coded).collect::<Vec<_>>(),
        [
            error_of(Code::ID_INVALID, &["1c"]),
            error_of(Code::ID_INVALID, &["2c"]),
            error_of(Code::REF_DANGLING, &["c3", "nope"]),
        ],
        "{COMPAT_007}: {what}: not the three schema errors in Diagnostic order"
    );
    let shown = error.to_string();
    assert_eq!(
        shown,
        "the document cannot be written in strict mode: 3 schema errors, the first is \
         SACM-ID-002 on \"1c\"",
        "{COMPAT_007}: {what}: the Display"
    );
    assert_eq!(
        write_err(COMPAT_007, what, Mode::Strict, &three).to_string(),
        shown,
        "{COMPAT_007}: {what}: a second write displays differently"
    );

    // An id with a quote and a line break is quoted with {:?}, on one line.
    let odd = "a\"b\nc";
    let mut doc = Document::new();
    insert(
        COMPAT_007,
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        odd,
        Some(en("P")),
    );
    let error = write_err(COMPAT_007, "an id with a line break", Mode::Strict, &doc);
    assert_eq!(
        error.to_string(),
        format!(
            "the document cannot be written in strict mode: 1 schema error, SACM-ID-002 on {odd:?}"
        ),
        "{COMPAT_007}: an id with a line break: the Display"
    );
}

// ===========================================================================
// LIB-COMPAT-001 8. A read diagnostic's message names the element it concerns
// ===========================================================================

/// Inputs that each give one SACM-XMI-010 or SACM-XMI-011 about an element
/// with an id: what the input is, the input, the code, and the kind name and
/// the id the message must give.
fn subject_cases() -> [(&'static str, String, Code, &'static str, &'static str); 9] {
    [
        (
            "xsi:type on a bare ArgumentPackage",
            format!(
                "<sacm:ArgumentPackage{NS_XSI} xmi:id=\"pkg_9\" xsi:type=\"sacm:ArgumentPackage\"/>"
            ),
            Code::XMI_XSI_TYPE,
            "ArgumentPackage",
            "pkg_9",
        ),
        (
            "xsi:type on a bare AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS_XSI} xmi:id=\"case_9\" \
                 xsi:type=\"sacm:AssuranceCasePackage\"/>"
            ),
            Code::XMI_XSI_TYPE,
            "AssuranceCasePackage",
            "case_9",
        ),
        (
            "xmi:version on a bare AssuranceCasePackage",
            format!("<sacm:AssuranceCasePackage{NS} xmi:id=\"case_9\" xmi:version=\"2.0\"/>"),
            Code::XMI_VERSION_IGNORED,
            "AssuranceCasePackage",
            "case_9",
        ),
        (
            "xmi:version on a wrapped ArgumentPackage",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmi:id=\"pkg_9\" xmi:version=\"2.1\"/></xmi:XMI>"
            ),
            Code::XMI_VERSION_IGNORED,
            "ArgumentPackage",
            "pkg_9",
        ),
        (
            "xsi:type on an argumentElement Claim",
            ap("<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"claim_9\"/>"),
            Code::XMI_XSI_TYPE,
            "Claim",
            "claim_9",
        ),
        (
            "a redundant xsi:type on argumentPackage",
            acp("<argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"inner_9\"/>"),
            Code::XMI_XSI_TYPE,
            "ArgumentPackage",
            "inner_9",
        ),
        (
            "xsi:type on a root's name",
            ap("<name xsi:type=\"sacm:LangString\" content=\"P\"/>"),
            Code::XMI_XSI_TYPE,
            "ArgumentPackage",
            "p",
        ),
        (
            "xsi:type on a Claim's name",
            ap(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"claim_9\">\
                <name xsi:type=\"sacm:LangString\" content=\"C\"/></argumentElement>",
            ),
            Code::XMI_XSI_TYPE,
            "Claim",
            "claim_9",
        ),
        (
            "xsi:type on a root whose id holds a quote",
            format!(
                "<sacm:ArgumentPackage{NS_XSI} xmi:id=\"q&quot;9\" xsi:type=\"sacm:ArgumentPackage\"/>"
            ),
            Code::XMI_XSI_TYPE,
            "ArgumentPackage",
            "q\"9",
        ),
    ]
}

#[test]
fn lib_compat_001_sacm_xmi_010_and_011_messages_name_the_element_by_its_kind_and_quoted_id() {
    // Only the kind name and the quoted id are pinned: the wording around
    // them is not API.
    for (what, input, code, kind, id) in &subject_cases() {
        let quoted = format!("{id:?}");
        for mode in MODES {
            let output = read_ok(COMPAT_001, what, mode, input);
            let [diagnostic] = output.diagnostics() else {
                panic!(
                    "{COMPAT_001}: {what}: setup: read in {mode:?} mode, there is not one \
                     diagnostic: {:?}",
                    output.diagnostics()
                );
            };
            assert_eq!(
                diagnostic.code(),
                *code,
                "{COMPAT_001}: {what}: setup: read in {mode:?} mode, the diagnostic is not {code}"
            );
            let message = diagnostic.message();
            assert!(
                message.contains(&quoted),
                "{COMPAT_001}: {what}: read in {mode:?} mode, the {code} message does not give \
                 the id {id:?} quoted as {quoted}: {message:?}"
            );
            assert!(
                message.contains(kind),
                "{COMPAT_001}: {what}: read in {mode:?} mode, the {code} message does not give \
                 the kind {kind}: {message:?}"
            );
        }
    }
}

#[test]
fn lib_compat_001_sacm_xmi_011_on_the_xmi_element_names_the_xmi_element_and_not_a_root() {
    // The xmi:XMI element has no id: the message names the XMI element.
    let wrapper = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\
         <sacm:ArgumentPackage xmi:id=\"pkg_9\"/></xmi:XMI>"
    );
    for mode in MODES {
        let output = read_ok(COMPAT_001, "xmi:version on the XMI element", mode, &wrapper);
        let [diagnostic] = output.diagnostics() else {
            panic!(
                "{COMPAT_001}: xmi:version on the XMI element: setup: read in {mode:?} mode, \
                 there is not one diagnostic: {:?}",
                output.diagnostics()
            );
        };
        assert!(
            diagnostic.code() == Code::XMI_VERSION_IGNORED
                && diagnostic.message().contains("XMI element")
                && !diagnostic.message().contains("pkg_9"),
            "{COMPAT_001}: xmi:version on the XMI element, read in {mode:?} mode, is not a \
             SACM-XMI-011 naming the XMI element and not the root: {diagnostic:?}"
        );
    }
}
