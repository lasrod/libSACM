//! Every diagnostic carries its template and arguments (LIB-API-018; ADR 0030
//! amendment 3, sections Templates, R8, R9, R11, Records and Order).
//!
//! One input per diagnostic template of `docs/sacm/diagnostics.md`'s
//! `## Message templates` table, 113 in all: 38 that `validate` gives and 75
//! that a tolerant or a strict read, or a strict write, gives (slice 0081
//! adds `SACM-XMI-011/emf-version-on-xmi-element` and `/emf-version-on-root`,
//! SACM23-XMI-032; slice 0075b
//! adds `SACM-XMI-003/subclass-type-kept`, which a tolerant read with a
//! subclass declaration gives, LIB-COMPAT-017, ADR 0044 D4; slice 0073b
//! removes the four kept forms of SACM-XMI-007, whose links SACM23-XMI-029
//! reads, and adds `SACM-XMI-010/xsi-type-on-proxy`, ADR 0040 amendment 1;
//! slice 0073a
//! removes the eight kept forms of SACM-XMI-014 and adds eight of XMI
//! metadata and same-file proxies, ADR 0042; slice 0072 adds the three of a
//! root in another SACM namespace, LIB-COMPAT-004; slice 0068 adds the eight of the
//! package interface and binding rules; slice 0067 removes the four kept
//! forms of unread kinds and ends: `SACM-XMI-014/interface-element-kept`,
//! `/unread-kind-kept`, `/unread-kind-in-root-kept` and `/unread-root-kept`). Each input is a document read,
//! written or validated through the public API. For each, the table states
//! the template id, the step that gives the diagnostic, its severity, row and
//! clause, its arguments by name, kind and value, the English pattern (as the
//! catalog lists it, the markers of the design expanded), and the message.
//!
//! - `lib_api_018_each_diagnostic_template_input_gives_its_message` runs on
//!   today's API. The step of each input gives exactly one diagnostic of the
//!   template's code with the message, whose severity, row and clause are the
//!   table's; the message is pinned byte for byte as the code before slice
//!   0064 wrote it. The table is checked against itself: the pattern rendered
//!   with the arguments is the message; every placeholder names an argument
//!   of the right kind; the arguments are exactly those the pattern names, in
//!   the order it first names them, so `row` is declared only where the
//!   pattern names it, and then last; and the inputs are the 106 templates,
//!   once each. [`sites`] adds an input for the other sites of the templates
//!   that more than one site gives, checked the same way.
//! - `lib_api_018_each_diagnostic_template_input_has_its_code_template_and_arguments`
//!   checks that `code()`, `template()` and `arguments()` are the table's.
//!
//! No template is without an input: the design expects none, so there is no
//! `NO_INPUT` list here.

use std::fmt::Write as _;

use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::model::{Document, ElementId};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-API-018";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm`, `xmi`, `xsi` and a vendor
/// namespace `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmlns:v=\"urn:vendor\"",
);

/// The number of diagnostic templates (acceptance 1): 38 from validation and
/// 68 from the reader and the writer, once slice 0067 removes the four kept
/// forms of unread kinds and ends (ADR 0035 amendment 1), slice 0068 adds
/// eight to validation (SACM23-VAL-013, -014, -015, -023), slice 0072 adds
/// three to the reader (LIB-COMPAT-004), and slice 0073a removes the eight
/// kept forms of SACM-XMI-014 and adds eight (SACM23-XMI-019,
/// SACM23-XMI-030; ADR 0042), and slice 0073b removes the four kept forms of
/// SACM-XMI-007 and adds `SACM-XMI-010/xsi-type-on-proxy` (SACM23-XMI-029;
/// ADR 0040 amendment 1), and slice 0075a adds three for the EMF dialect
/// (LIB-COMPAT-002; ADR 0044), and slice 0075b one for a declared subclass
/// (LIB-COMPAT-017; ADR 0044 D4), and slice 0079 one for a proxy of a root
/// that keeps a type attribute (LIB-COMPAT-020; ADR 0042 amendment 1), and
/// slice 0081 two for a `version` in EMF's XMI namespace (SACM23-XMI-032).
const DIAGNOSTIC_TEMPLATES: usize = 113;

/// The number of them that `validate` gives.
const VALIDATION_TEMPLATES: usize = 38;

// --- The markers of the design, expanded --------------------------------------------

/// ‹KE›: kept as preserved content of an element; adds `holder_class` and
/// `holder_id`.
macro_rules! ke {
    () => {
        ", so it is kept as preserved content of the {holder_class} {holder_id:?}, which a \
         tolerant write writes back and a strict write refuses"
    };
}

/// ‹KD›: kept as preserved content of the document; adds no argument.
macro_rules! kd {
    () => {
        ", so it is kept as preserved content of the document, which a tolerant write writes \
         back and a strict write refuses"
    };
}

/// ‹L›: a C++ shape, read as its standard form.
macro_rules! legacy {
    () => {
        ", a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is \
         read as its standard form, which write writes"
    };
}

/// ‹ID›: an id that is no `NCName`.
macro_rules! not_ncname {
    () => {
        ", which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)"
    };
}

/// ‹XSI›: typed with `xsi:type`.
macro_rules! xsi {
    () => {
        " is typed with xsi:type, which XMI 2.5.1 allows only when \
         org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as \
         xmi:type"
    };
}

/// ‹TRIM›: an id read trimmed.
macro_rules! trim {
    () => {
        ": an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write \
         writes it trimmed"
    };
}

/// ‹EMPTY›: an empty id.
macro_rules! empty {
    () => {
        " is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so "
    };
}

/// ‹T:x›: a spelling a tolerant read reads as `x`.
macro_rules! read_as {
    ($x:literal) => {
        concat!("; it is read as ", $x, ", which write writes")
    };
}

/// ‹S:x›: a spelling a strict read keeps.
macro_rules! kept_as {
    ($x:literal) => {
        concat!(
            "; it is kept as preserved content, and a tolerant read reads the name as ",
            $x
        )
    };
}

// --- The table ----------------------------------------------------------------------

/// An argument's expected value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Text(&'static str),
    Count(u64),
}

fn t(text: &'static str) -> Value {
    Value::Text(text)
}

fn n(count: u64) -> Value {
    Value::Count(count)
}

/// The step whose diagnostics hold the case's.
enum Step {
    /// A tolerant read of the input.
    Tolerant(String),
    /// A tolerant read of the input with the subclass declaration of
    /// [`declared`] (LIB-COMPAT-017).
    Declared(String),
    /// A strict read of the input, which reads it.
    Strict(String),
    /// `validate` of the document a tolerant read of the input returns.
    Validate(String),
    /// The `Invalid` refusal of a strict write of the document a tolerant read
    /// of the input returns.
    Write(String),
}

fn tolerant(input: impl Into<String>) -> Step {
    Step::Tolerant(input.into())
}

fn strict(input: impl Into<String>) -> Step {
    Step::Strict(input.into())
}

fn validate(input: impl Into<String>) -> Step {
    Step::Validate(input.into())
}

fn write(input: impl Into<String>) -> Step {
    Step::Write(input.into())
}

/// One diagnostic template and its input.
struct Case {
    /// The template id, `<CODE>/<form>`.
    template: &'static str,
    step: Step,
    severity: Severity,
    row: &'static str,
    clause: Option<&'static str>,
    /// The arguments, in the template's order.
    arguments: Vec<(&'static str, Value)>,
    /// The template's English pattern.
    pattern: &'static str,
    /// The message, byte for byte as the code before slice 0064 wrote it.
    message: &'static str,
}

/// The case of `template`, whose diagnostic `step` gives with `found`'s
/// severity, row and clause.
fn case(
    template: &'static str,
    step: Step,
    found: (Severity, &'static str, Option<&'static str>),
    arguments: &[(&'static str, Value)],
    pattern: &'static str,
    message: &'static str,
) -> Case {
    let (severity, row, clause) = found;
    Case {
        template,
        step,
        severity,
        row,
        clause,
        arguments: arguments.to_vec(),
        pattern,
        message,
    }
}

const ERROR: Severity = Severity::Error;
const WARNING: Severity = Severity::Warning;
const INFO: Severity = Severity::Info;

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// A Claim `id`, as [`claim`], with no name: `body` is all it holds.
fn bare_claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>{body}</argumentElement>"
    )
}

/// An element of `kind` under the role `argumentElement`, with the id `id`,
/// the extra attributes `attributes` and a name.
fn argument(kind: &str, id: &str, attributes: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/></argumentElement>"
    )
}

/// The Claim `c1` holding the Note `n`, whose content element is `content`.
fn note_in_claim(content: &str) -> String {
    package(&claim(
        "c1",
        "",
        &format!("<note xmi:id=\"n\">{content}</note>"),
    ))
}

/// A root AssuranceCasePackage `case` holding the ArgumentPackage `ap`,
/// which holds `body`: an element that is not a root.
fn nested(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <argumentPackage xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"ap\"><name content=\"ap\"/>{body}\
         </argumentPackage></sacm:AssuranceCasePackage>"
    )
}

/// A root TerminologyPackage `terms` holding `body`.
fn terminology(body: &str) -> String {
    format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"terms\"><name content=\"terms\"/>{body}</sacm:TerminologyPackage>"
    )
}

/// A root AssuranceCasePackage `case` holding the TerminologyPackage
/// `tp`, which holds `body`.
fn nested_terminology(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <terminologyPackage xmi:id=\"tp\"><name content=\"tp\"/>{body}\
         </terminologyPackage></sacm:AssuranceCasePackage>"
    )
}

/// A root ArtifactPackage `arts` holding `body`.
fn artifacts(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"arts\"><name content=\"arts\"/>{body}</sacm:ArtifactPackage>"
    )
}

/// A root AssuranceCasePackage `case` holding the ArtifactPackage `ap`,
/// which holds `body`.
fn nested_artifacts(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <artifactPackage xmi:id=\"ap\"><name content=\"ap\"/>{body}\
         </artifactPackage></sacm:AssuranceCasePackage>"
    )
}

/// The `xmi:XMI` wrapper with `attributes` and `body` before its one root.
fn wrapper(attributes: &str, body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>{body}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
    )
}

/// An Event of an ArtifactPackage, with the extra attributes `attributes`.
fn event(attributes: &str) -> String {
    artifacts(&format!(
        "<artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\"{attributes}><name content=\"e1\"/></artifactElement>"
    ))
}

/// An `ArgumentGroup` `g` whose end is written `argumentationElement`, as the
/// attribute, beside the Claim `c1` it names.
fn spelled_group() -> String {
    package(&format!(
        "{}<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\" argumentationElement=\"c1\"><name content=\"g\"/></argumentElement>",
        claim("c1", "", "")
    ))
}

/// The child written `argumentationElement`, a Claim `c1`.
const SPELLED_ROLE: &str = "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentationElement>";

/// The child written `TerminologyElement`, a Term `t1`.
const SPELLED_TERM: &str = "<TerminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"t\"><name content=\"t1\"/></TerminologyElement>";

/// The child typed `ArtifactAssertedRelationship`, between the Artifacts `a1`
/// and `a2`.
const SPELLED_TYPE: &str = "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a1\"><name content=\"a1\"/></artifactElement>\
     <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a2\"><name content=\"a2\"/></artifactElement>\
     <artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r1\" source=\"a1\" target=\"a2\"><name content=\"r1\"/></artifactElement>";

/// A Claim with two descriptions (acceptance 2): exactly one finding.
const CLM_132: &str = "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"><name content=\"P\"/>\n  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"CLM_132\"><name content=\"C\"/><description xmi:id=\"d1\"/><description xmi:id=\"d2\"/></argumentElement>\n</sacm:ArgumentPackage>";

/// The 38 templates `validate` gives, in the design's order, with the eight
/// of slice 0068 beside their codes.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn validation_cases() -> Vec<Case> {
    vec![
        // --- SACM-MULT-001 -------------------------------------------------------------
        case(
            "SACM-MULT-001/missing",
            validate(package(&bare_claim("c1", "", ""))),
            (ERROR, "SACM23-VAL-018", Some("8.6")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("property", t("name")),
                ("type", t("LangString")),
                ("multiplicity", t("1")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "Claim \"c1\" has no name (name:LangString[1])",
        ),
        case(
            "SACM-MULT-001/too-many",
            validate(CLM_132),
            (WARNING, "SACM23-VAL-011", Some("8.6")),
            &[
                ("class", t("Claim")),
                ("id", t("CLM_132")),
                ("count", n(2)),
                ("property", t("description")),
                ("type", t("Description")),
                ("multiplicity", t("0..1")),
            ],
            "{class} {id:?} has {count} {property}s ({property}:{type}[{multiplicity}])",
            "Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])",
        ),
        case(
            "SACM-MULT-001/no-value",
            validate(note_in_claim("<content/>")),
            (ERROR, "SACM23-VAL-010", Some("8.5")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            "the {role} of {class} {id:?} has no value (value:LangString[1..*])",
            "the content of Note \"n\" has no value (value:LangString[1..*])",
        ),
        case(
            "SACM-MULT-001/repeated-lang",
            validate(note_in_claim(
                "<content><value lang=\"en\" content=\"a\"/><value lang=\"en\" content=\"b\"/></content>",
            )),
            (ERROR, "SACM23-VAL-010", Some("8.5")),
            &[
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("count", n(2)),
                ("lang", t("en")),
            ],
            "the {role} of {class} {id:?} has {count} values in the language {lang:?}, and each value's lang must be unique",
            "the content of Note \"n\" has 2 values in the language \"en\", and each value's lang must be unique",
        ),
        case(
            "SACM-MULT-001/repeated-absent-lang",
            validate(note_in_claim(
                "<content><value content=\"a\"/><value content=\"b\"/></content>",
            )),
            (ERROR, "SACM23-VAL-010", Some("8.5")),
            &[
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("count", n(2)),
            ],
            "the {role} of {class} {id:?} has {count} values in no language (lang absent), and each value's lang must be unique",
            "the content of Note \"n\" has 2 values in no language (lang absent), and each value's lang must be unique",
        ),
        case(
            "SACM-MULT-001/repeated-reference",
            validate(package(&format!(
                "{}{}",
                claim("c1", " metaClaim=\"c2 c2\"", ""),
                claim("c2", "", "")
            ))),
            (WARNING, "SACM23-VAL-022", Some("11.10")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("target", t("c2")),
                ("count", n(2)),
                ("end", t("metaClaim")),
            ],
            "{class} {id:?} holds {target:?} {count} times in its {end}, whose ids are unique in SACM 2.3 (the repeat is kept)",
            "Claim \"c1\" holds \"c2\" 2 times in its metaClaim, whose ids are unique in SACM 2.3 (the repeat is kept)",
        ),
        case(
            "SACM-MULT-001/name-without-expression",
            validate(package(&bare_claim(
                "c1",
                "",
                "<name xmi:type=\"sacm:ExpressionLangString\" content=\"c1\"/>",
            ))),
            (ERROR, "SACM23-BASE-009", Some("8.4")),
            &[("class", t("Claim")), ("id", t("c1"))],
            "the name of {class} {id:?} is an ExpressionLangString with no expression (expression:ExpressionElement[1])",
            "the name of Claim \"c1\" is an ExpressionLangString with no expression (expression:ExpressionElement[1])",
        ),
        // The template of slice 0068 (SACM23-VAL-014; D-BIND-PART).
        case(
            "SACM-MULT-001/too-few",
            validate(package(&argument(
                "ArgumentPackageBinding",
                "b",
                " participantPackage=\"p\"",
            ))),
            (ERROR, "SACM23-VAL-014", Some("11.5")),
            &[
                ("class", t("ArgumentPackageBinding")),
                ("id", t("b")),
                ("count", n(1)),
                ("property", t("participantPackage")),
                ("type", t("ArgumentPackage")),
                ("multiplicity", t("2..*")),
            ],
            "{class} {id:?} has {count} distinct {property} ({property}:{type}[{multiplicity}])",
            "ArgumentPackageBinding \"b\" has 1 distinct participantPackage (participantPackage:ArgumentPackage[2..*])",
        ),
        case(
            "SACM-MULT-001/value-without-expression",
            validate(note_in_claim(
                "<content><value content=\"a\"/><value xmi:type=\"sacm:ExpressionLangString\" lang=\"sv\"/></content>",
            )),
            (ERROR, "SACM23-BASE-009", Some("8.4")),
            &[
                ("position", n(2)),
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
            ],
            "value {position} of the {role} of {class} {id:?} is an ExpressionLangString with no expression (expression:ExpressionElement[1])",
            "value 2 of the content of Note \"n\" is an ExpressionLangString with no expression (expression:ExpressionElement[1])",
        ),
        // --- SACM-EXPR-001 -------------------------------------------------------------
        case(
            "SACM-EXPR-001/name-with-content",
            validate(terminology(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\"><name content=\"blade\"/></terminologyElement>\
                 <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t2\" value=\"t2\"><name xmi:type=\"sacm:ExpressionLangString\" expression=\"blade\" content=\"x\"/></terminologyElement>",
            )),
            (WARNING, "SACM23-VAL-009", Some("8.4")),
            &[
                ("class", t("Term")),
                ("id", t("t2")),
                ("expression", t("blade")),
            ],
            "the name of {class} {id:?} has the expression {expression:?} and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty",
            "the name of Term \"t2\" has the expression \"blade\" and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty",
        ),
        case(
            "SACM-EXPR-001/value-with-content",
            validate(terminology(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\"><name content=\"blade\"/>\
                 <note xmi:id=\"n\"><content><value xmi:type=\"sacm:ExpressionLangString\" expression=\"blade\" content=\"x\"/></content></note>\
                 </terminologyElement>",
            )),
            (WARNING, "SACM23-VAL-009", Some("8.4")),
            &[
                ("position", n(1)),
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("expression", t("blade")),
            ],
            "value {position} of the {role} of {class} {id:?} has the expression {expression:?} and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty",
            "value 1 of the content of Note \"n\" has the expression \"blade\" and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty",
        ),
        // --- SACM-ABS-001 --------------------------------------------------------------
        case(
            "SACM-ABS-001/implementation-constraint",
            validate(package(&claim(
                "c1",
                "",
                "<implementationConstraint xmi:id=\"k1\"/>",
            ))),
            (ERROR, "SACM23-VAL-021", Some("8.6")),
            &[("class", t("Claim")), ("id", t("c1"))],
            "{class} {id:?} has an implementationConstraint but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)",
            "Claim \"c1\" has an implementationConstraint but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)",
        ),
        case(
            "SACM-ABS-001/implementation-constraints",
            validate(package(&claim(
                "c1",
                "",
                "<implementationConstraint xmi:id=\"k1\"/><implementationConstraint xmi:id=\"k2\"/>",
            ))),
            (ERROR, "SACM23-VAL-021", Some("8.6")),
            &[("class", t("Claim")), ("id", t("c1")), ("count", n(2))],
            "{class} {id:?} has {count} implementationConstraints but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)",
            "Claim \"c1\" has 2 implementationConstraints but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)",
        ),
        case(
            "SACM-ABS-001/abstract-holder",
            validate(package(&format!(
                "{}{}",
                claim("c1", " isAbstract=\"true\" abstractForm=\"c2\"", ""),
                claim("c2", " isAbstract=\"true\"", "")
            ))),
            (ERROR, "SACM23-VAL-008", Some("8.2")),
            &[("class", t("Claim")), ("id", t("c1")), ("target", t("c2"))],
            "{class} {id:?} has the abstractForm {target:?} but is itself abstract (isAbstract is true); §8.2 gives abstractForm to a concrete element",
            "Claim \"c1\" has the abstractForm \"c2\" but is itself abstract (isAbstract is true); §8.2 gives abstractForm to a concrete element",
        ),
        case(
            "SACM-ABS-001/form-not-abstract",
            validate(package(&format!(
                "{}{}",
                claim("c1", " abstractForm=\"c2\"", ""),
                claim("c2", "", "")
            ))),
            (WARNING, "SACM23-VAL-008", Some("8.2")),
            &[("target", t("c2")), ("class", t("Claim")), ("id", t("c1"))],
            "the abstractForm {target:?} of {class} {id:?} is not abstract: its isAbstract should be true (§8.2)",
            "the abstractForm \"c2\" of Claim \"c1\" is not abstract: its isAbstract should be true (§8.2)",
        ),
        case(
            "SACM-ABS-001/form-of-another-kind",
            validate(package(&format!(
                "{}{}",
                claim("c1", " abstractForm=\"r1\"", ""),
                argument("ArgumentReasoning", "r1", " isAbstract=\"true\"")
            ))),
            (WARNING, "SACM23-VAL-008", Some("8.2")),
            &[
                ("target", t("r1")),
                ("class", t("Claim")),
                ("id", t("c1")),
                ("target_class", t("ArgumentReasoning")),
            ],
            "the abstractForm {target:?} of {class} {id:?} is of the kind {target_class}, not {class}: it should be of the same type (§8.2)",
            "the abstractForm \"r1\" of Claim \"c1\" is of the kind ArgumentReasoning, not Claim: it should be of the same type (§8.2)",
        ),
        case(
            "SACM-ABS-001/abstract-element",
            validate(terminology(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"guard\" isAbstract=\"true\" value=\"g\"><name content=\"guard\"/></terminologyElement>\
                 <terminologyElement xmi:type=\"sacm:Expression\" xmi:id=\"e1\" element=\"guard\" value=\"e\"><name content=\"e1\"/></terminologyElement>",
            )),
            (ERROR, "SACM23-VAL-017", Some("10.10")),
            &[
                ("class", t("Expression")),
                ("id", t("e1")),
                ("target", t("guard")),
                ("target_class", t("Term")),
            ],
            "{class} {id:?} is concrete but has the element {target:?}, an abstract {target_class}: a concrete Expression references only concrete ExpressionElements (§10.10)",
            "Expression \"e1\" is concrete but has the element \"guard\", an abstract Term: a concrete Expression references only concrete ExpressionElements (§10.10)",
        ),
        // --- SACM-ID-003 ---------------------------------------------------------------
        case(
            "SACM-ID-003/duplicate-gid",
            validate(package(&format!(
                "{}{}",
                claim("c1", " gid=\"g\"", ""),
                claim("c2", " gid=\"g\"", "")
            ))),
            (ERROR, "SACM23-VAL-002", Some("8.2")),
            &[
                ("count", n(2)),
                ("gid", t("g")),
                ("elements", t("Claim \"c1\", Claim \"c2\"")),
            ],
            "{count} elements hold the gid {gid:?}, which must be unique within the model instance: {elements}",
            "2 elements hold the gid \"g\", which must be unique within the model instance: Claim \"c1\", Claim \"c2\"",
        ),
        // --- SACM-PKG-001 --------------------------------------------------------------
        case(
            "SACM-PKG-001/mixed-content",
            validate(package(&format!(
                "{}{}",
                argument("ArgumentPackage", "ap", ""),
                claim("c1", "", "")
            ))),
            (ERROR, "SACM23-VAL-012", Some("11.4")),
            &[
                ("class", t("ArgumentPackage")),
                ("id", t("p")),
                ("nested_class", t("ArgumentPackage")),
                ("nested_id", t("ap")),
                ("child_class", t("Claim")),
                ("child_id", t("c1")),
            ],
            "{class} {id:?} nests the {nested_class} {nested_id:?}, so it may contain only ArgumentPackages, but it also contains {child_class} {child_id:?}",
            "ArgumentPackage \"p\" nests the ArgumentPackage \"ap\", so it may contain only ArgumentPackages, but it also contains Claim \"c1\"",
        ),
        // The templates of slice 0068 (SACM23-VAL-013, -014, -015; D-IFACE-CITE,
        // D-BIND-PART).
        case(
            "SACM-PKG-001/binding-participant",
            validate(package(&format!(
                "{}{}{}",
                argument("ArgumentPackage", "q", ""),
                argument("ArgumentPackageBinding", "b", " participantPackage=\"p q\""),
                argument(
                    "ArgumentPackageBinding",
                    "b2",
                    " participantPackage=\"q b\""
                )
            ))),
            (WARNING, "SACM23-VAL-014", Some("11.5")),
            &[
                ("class", t("ArgumentPackageBinding")),
                ("id", t("b2")),
                ("target", t("b")),
                ("target_class", t("ArgumentPackageBinding")),
            ],
            "{class} {id:?} has the participantPackage {target:?}, of the kind {target_class}: a binding binds packages, not bindings",
            "ArgumentPackageBinding \"b2\" has the participantPackage \"b\", of the kind ArgumentPackageBinding: a binding binds packages, not bindings",
        ),
        case(
            "SACM-PKG-001/cited-kind",
            validate(package(&format!(
                "{}<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" implements=\"p\"><name content=\"i\"/>{}</argumentElement>",
                argument("ArgumentPackage", "q", ""),
                claim("ci", " isCitation=\"true\" citedElement=\"q\"", "")
            ))),
            (WARNING, "SACM23-VAL-013", Some("11.6")),
            &[
                ("class", t("ArgumentPackageInterface")),
                ("id", t("i")),
                ("child_class", t("Claim")),
                ("child_id", t("ci")),
                ("target", t("q")),
                ("target_class", t("ArgumentPackage")),
                ("clause", t("11.6")),
                ("expected", t("ArgumentAsset")),
            ],
            "{class} {id:?} contains {child_class} {child_id:?}, which cites {target:?} of the kind {target_class}, but §{clause} has an interface cite only {expected}s",
            "ArgumentPackageInterface \"i\" contains Claim \"ci\", which cites \"q\" of the kind ArgumentPackage, but §11.6 has an interface cite only ArgumentAssets",
        ),
        case(
            "SACM-PKG-001/cited-outside",
            validate(package(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"q\"><name content=\"q\"/>\
                 <argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" implements=\"q\"><name content=\"i\"/>{}</argumentElement>\
                 </argumentElement>\
                 <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"r\"><name content=\"r\"/>{}</argumentElement>",
                claim("ci", " isCitation=\"true\" citedElement=\"d\"", ""),
                claim("d", "", "")
            ))),
            (WARNING, "SACM23-VAL-013", Some("11.6")),
            &[
                ("class", t("ArgumentPackageInterface")),
                ("id", t("i")),
                ("child_class", t("Claim")),
                ("child_id", t("ci")),
                ("target", t("d")),
                ("clause", t("11.6")),
                ("package", t("q")),
            ],
            "{class} {id:?} contains {child_class} {child_id:?}, which cites {target:?}, but §{clause} has an interface cite only elements within the package it implements, {package:?}",
            "ArgumentPackageInterface \"i\" contains Claim \"ci\", which cites \"d\", but §11.6 has an interface cite only elements within the package it implements, \"q\"",
        ),
        case(
            "SACM-PKG-001/not-a-citation",
            validate(package(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\" implements=\"p\"><name content=\"i\"/>{}</argumentElement>",
                claim("c", "", "")
            ))),
            (ERROR, "SACM23-VAL-013", Some("11.6")),
            &[
                ("class", t("ArgumentPackageInterface")),
                ("id", t("i")),
                ("child_class", t("Claim")),
                ("child_id", t("c")),
                ("clause", t("11.6")),
            ],
            "{class} {id:?} contains {child_class} {child_id:?}, which is no citation (isCitation true and a citedElement), but §{clause} allows only citations in it",
            "ArgumentPackageInterface \"i\" contains Claim \"c\", which is no citation (isCitation true and a citedElement), but §11.6 allows only citations in it",
        ),
        case(
            "SACM-PKG-001/not-a-citation-or-relationship",
            validate(package(&format!(
                "{}<argumentElement xmi:type=\"sacm:ArgumentPackageBinding\" xmi:id=\"b\" participantPackage=\"p q\"><name content=\"b\"/>{}</argumentElement>",
                argument("ArgumentPackage", "q", ""),
                claim("c", "", "")
            ))),
            (ERROR, "SACM23-VAL-013", Some("11.5")),
            &[
                ("class", t("ArgumentPackageBinding")),
                ("id", t("b")),
                ("child_class", t("Claim")),
                ("child_id", t("c")),
                ("clause", t("11.5")),
                ("relationship", t("AssertedRelationship")),
            ],
            "{class} {id:?} contains {child_class} {child_id:?}, which is no citation (isCitation true and a citedElement), but §{clause} allows only citations and {relationship}s in it",
            "ArgumentPackageBinding \"b\" contains Claim \"c\", which is no citation (isCitation true and a citedElement), but §11.5 allows only citations and AssertedRelationships in it",
        ),
        case(
            "SACM-PKG-001/not-an-interface",
            validate(format!(
                "<sacm:AssuranceCasePackageInterface{NS} xmi:id=\"ai\" implements=\"ai\"><name content=\"ai\"/>\
                 <argumentPackage xmi:id=\"ap\"><name content=\"ap\"/></argumentPackage>\
                 </sacm:AssuranceCasePackageInterface>"
            )),
            (ERROR, "SACM23-VAL-015", Some("9.3")),
            &[
                ("class", t("AssuranceCasePackageInterface")),
                ("id", t("ai")),
                ("child_class", t("ArgumentPackage")),
                ("child_id", t("ap")),
                ("role", t("argumentPackage")),
                ("expected", t("ArgumentPackageInterface")),
            ],
            "{class} {id:?} contains {child_class} {child_id:?} in its {role}, but §9.3 allows only {expected}s there",
            "AssuranceCasePackageInterface \"ai\" contains ArgumentPackage \"ap\" in its argumentPackage, but §9.3 allows only ArgumentPackageInterfaces there",
        ),
        // --- SACM-PKG-002 --------------------------------------------------------------
        case(
            "SACM-PKG-002/outside-implemented-package",
            validate(package(&format!(
                "{}{}",
                argument("ArgumentPackage", "q", ""),
                argument("ArgumentPackageInterface", "i", " implements=\"q\"")
            ))),
            (WARNING, "SACM23-VAL-023", Some("11.6")),
            &[
                ("class", t("ArgumentPackageInterface")),
                ("id", t("i")),
                ("package_class", t("ArgumentPackage")),
                ("package", t("q")),
                ("clause", t("11.6")),
            ],
            "{class} {id:?} implements the {package_class} {package:?} but does not reside inside it, as §{clause} says an interface does",
            "ArgumentPackageInterface \"i\" implements the ArgumentPackage \"q\" but does not reside inside it, as §11.6 says an interface does",
        ),
        // --- SACM-ID-002 ---------------------------------------------------------------
        case(
            "SACM-ID-002/not-an-ncname",
            validate(package(&claim("1c", "", ""))),
            (ERROR, "SACM23-VAL-020", Some("7.6.1")),
            &[("class", t("Claim")), ("id", t("1c"))],
            "{class} {id:?} has an id that is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
            "Claim \"1c\" has an id that is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
        ),
        case(
            "SACM-ID-002/name-id-not-an-ncname",
            validate(package(&bare_claim(
                "c1",
                "",
                "<name xmi:id=\"1n\" content=\"c1\"/>",
            ))),
            (ERROR, "SACM23-VAL-020", Some("7.6.1")),
            &[("class", t("Claim")), ("id", t("c1")), ("xmi_id", t("1n"))],
            concat!(
                "the name of {class} {id:?} has the xmi:id {xmi_id:?}",
                not_ncname!()
            ),
            "the name of Claim \"c1\" has the xmi:id \"1n\", which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
        ),
        case(
            "SACM-ID-002/text-id-not-an-ncname",
            validate(note_in_claim(
                "<content xmi:id=\"1t\"><value content=\"a\"/></content>",
            )),
            (ERROR, "SACM23-VAL-020", Some("7.6.1")),
            &[
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("xmi_id", t("1t")),
            ],
            concat!(
                "the {role} of {class} {id:?} has the xmi:id {xmi_id:?}",
                not_ncname!()
            ),
            "the content of Note \"n\" has the xmi:id \"1t\", which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
        ),
        case(
            "SACM-ID-002/value-id-not-an-ncname",
            validate(note_in_claim(
                "<content><value xmi:id=\"1v\" content=\"a\"/></content>",
            )),
            (ERROR, "SACM23-VAL-020", Some("7.6.1")),
            &[
                ("position", n(1)),
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("xmi_id", t("1v")),
            ],
            concat!(
                "value {position} of the {role} of {class} {id:?} has the xmi:id {xmi_id:?}",
                not_ncname!()
            ),
            "value 1 of the content of Note \"n\" has the xmi:id \"1v\", which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
        ),
        // --- SACM-REF-001, -002, -003 --------------------------------------------------
        case(
            "SACM-REF-001/dangling",
            validate(package(&claim(
                "c1",
                " isCitation=\"true\" citedElement=\"nope\"",
                "",
            ))),
            (ERROR, "SACM23-VAL-003", Some("8.2")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("end", t("citedElement")),
                ("target", t("nope")),
                ("type", t("SACMElement")),
            ],
            "{class} {id:?} has the {end} {target:?}, which names no element of the document (the end's type is {type})",
            "Claim \"c1\" has the citedElement \"nope\", which names no element of the document (the end's type is SACMElement)",
        ),
        case(
            "SACM-REF-002/wrong-type",
            validate(package(&format!(
                "{}{}",
                claim("c1", " metaClaim=\"r1\"", ""),
                argument("ArgumentReasoning", "r1", "")
            ))),
            (ERROR, "SACM23-VAL-004", Some("11.10")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("end", t("metaClaim")),
                ("target", t("r1")),
                ("target_class", t("ArgumentReasoning")),
                ("type", t("Claim")),
            ],
            "{class} {id:?} has the {end} {target:?}, whose kind {target_class} does not conform to the end's type {type}",
            "Claim \"c1\" has the metaClaim \"r1\", whose kind ArgumentReasoning does not conform to the end's type Claim",
        ),
        case(
            "SACM-REF-002/not-an-artifact-reference",
            validate(package(&format!(
                "{}{}{}",
                claim("c1", "", ""),
                claim("c2", "", ""),
                argument("AssertedEvidence", "e1", " source=\"c1\" target=\"c2\"")
            ))),
            (ERROR, "SACM23-VAL-016", Some("11.15")),
            &[
                ("class", t("AssertedEvidence")),
                ("id", t("e1")),
                ("end", t("source")),
                ("target", t("c1")),
                ("target_class", t("Claim")),
                ("clause", t("11.15")),
            ],
            "{class} {id:?} has the {end} {target:?}, of the kind {target_class}, but §{clause} requires each {end} of the {class} to be an ArtifactReference",
            "AssertedEvidence \"e1\" has the source \"c1\", of the kind Claim, but §11.15 requires each source of the AssertedEvidence to be an ArtifactReference",
        ),
        case(
            "SACM-REF-003/preserved-target",
            validate(package(&format!(
                "<v:thing xmi:id=\"x\"/>{}",
                claim("c1", " isCitation=\"true\" citedElement=\"x\"", "")
            ))),
            (WARNING, "LIB-COMPAT-003", Some("8.2")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("end", t("citedElement")),
                ("target", t("x")),
                ("type", t("SACMElement")),
            ],
            "{class} {id:?} has the {end} {target:?}, which names an element the document holds only as preserved content, so its type is not checked (the end's type is {type})",
            "Claim \"c1\" has the citedElement \"x\", which names an element the document holds only as preserved content, so its type is not checked (the end's type is SACMElement)",
        ),
        // --- SACM-CITE-001, -002 -------------------------------------------------------
        case(
            "SACM-CITE-001/not-a-citation",
            validate(package(&format!(
                "{}{}",
                claim("c1", " citedElement=\"c2\"", ""),
                claim("c2", "", "")
            ))),
            (ERROR, "SACM23-VAL-007", Some("8.2")),
            &[("class", t("Claim")), ("id", t("c1")), ("target", t("c2"))],
            "{class} {id:?} has the citedElement {target:?} but isCitation is not true (§8.2: citedElement <> null implies isCitation = true)",
            "Claim \"c1\" has the citedElement \"c2\" but isCitation is not true (§8.2: citedElement <> null implies isCitation = true)",
        ),
        case(
            "SACM-CITE-002/not-a-citation",
            validate(package(&format!(
                "{}{}",
                claim(
                    "c1",
                    " assertionDeclaration=\"asCited\" citedElement=\"c2\"",
                    ""
                ),
                claim("c2", "", "")
            ))),
            (WARNING, "SACM23-VAL-019", Some("11.8")),
            &[("class", t("Claim")), ("id", t("c1"))],
            "{class} {id:?} is declared asCited but isCitation is not true: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
            "Claim \"c1\" is declared asCited but isCitation is not true: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
        ),
        case(
            "SACM-CITE-002/no-cited-element",
            validate(package(&claim(
                "c1",
                " assertionDeclaration=\"asCited\" isCitation=\"true\"",
                "",
            ))),
            (WARNING, "SACM23-VAL-019", Some("11.8")),
            &[("class", t("Claim")), ("id", t("c1"))],
            "{class} {id:?} is declared asCited but has no citedElement: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
            "Claim \"c1\" is declared asCited but has no citedElement: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
        ),
        case(
            "SACM-CITE-002/cited-not-an-assertion",
            validate(package(&format!(
                "{}{}",
                claim(
                    "c1",
                    " assertionDeclaration=\"asCited\" isCitation=\"true\" citedElement=\"r1\"",
                    ""
                ),
                argument("ArgumentReasoning", "r1", "")
            ))),
            (WARNING, "SACM23-VAL-019", Some("11.8")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("target", t("r1")),
                ("target_class", t("ArgumentReasoning")),
            ],
            "{class} {id:?} is declared asCited but its citedElement {target:?} is of the kind {target_class}, which is no Assertion: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
            "Claim \"c1\" is declared asCited but its citedElement \"r1\" is of the kind ArgumentReasoning, which is no Assertion: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)",
        ),
    ]
}

/// The 72 templates a read, or a strict write's check of XMI metadata, gives,
/// in the design's order.
fn reader_cases() -> Vec<Case> {
    let mut cases = plain_reader_cases();
    cases.extend(preserved_cases());
    cases.extend(legacy_cases());
    cases.extend(spelling_cases());
    cases.extend(kept_cases());
    cases.extend(namespace_cases());
    cases.extend(metadata_cases());
    cases.extend(emf_cases());
    cases.extend(subclass_cases());
    cases
}

/// The options of a [`Step::Declared`] read: tolerant, with the class
/// `Proposition` of `urn:example:profile` declared a subclass of Claim.
fn declared() -> ReadOptions {
    ReadOptions::default()
        .with_mode(Mode::Tolerant)
        .with_subclass(
            "urn:example:profile",
            "Proposition",
            sacm::model::ElementKind::Claim,
        )
}

/// The template of slice 0075b (LIB-COMPAT-017; ADR 0044 D3, D4): an element
/// typed with a class the application declared a subclass of its kind.
fn subclass_cases() -> Vec<Case> {
    vec![case(
        "SACM-XMI-003/subclass-type-kept",
        Step::Declared(format!(
            "<sacm:ArgumentPackage{NS} xmlns:p=\"urn:example:profile\" xmi:id=\"p\">\
             <name content=\"p\"/>\
             <argumentElement xmi:type=\"p:Proposition\" xmi:id=\"c1\"><name content=\"c1\"/>\
             </argumentElement></sacm:ArgumentPackage>"
        )),
        (WARNING, "LIB-COMPAT-017", Some("7.8.6")),
        &[
            ("type", t("p:Proposition")),
            ("class", t("Claim")),
            ("id", t("c1")),
        ],
        "the type {type:?} of the {class} {id:?} is not SACM 2.3, but the application declared it a subclass of {class}, so the element is read as {class} and its type is kept as preserved content, which a tolerant write writes back and a strict write refuses",
        "the type \"p:Proposition\" of the Claim \"c1\" is not SACM 2.3, but the application declared it a subclass of Claim, so the element is read as Claim and its type is kept as preserved content, which a tolerant write writes back and a strict write refuses",
    )]
}

/// The three templates of slice 0075a (LIB-COMPAT-002; ADR 0044 D1, D2): the
/// XMI namespace EMF writes, read as XMI 2.5.1's, and its fragment paths,
/// all read or some kept; and the two of slice 0081 (SACM23-XMI-032): a
/// `version` in that namespace on the `xmi:XMI` element or a root.
fn emf_cases() -> Vec<Case> {
    let emf = |body: &str| emf_root("", body);
    let paths = |source: &str| {
        package(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\
             <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"{source}\" \
             target=\"c0\"/>"
        ))
    };
    vec![
        case(
            "SACM-XMI-015/emf-xmi-namespace-read",
            tolerant(emf("")),
            (WARNING, "LIB-COMPAT-002", Some("7.5.3")),
            &[("name", t("xmi:id"))],
            "the name {name:?} is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\", so the XMI names of the document in it are read as XMI 2.5.1's, which write writes",
            "the name \"xmi:id\" is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\", so the XMI names of the document in it are read as XMI 2.5.1's, which write writes",
        ),
        case(
            "SACM-XMI-017/paths-read",
            tolerant(paths("//@argumentElement.0")),
            (WARNING, "LIB-COMPAT-002", Some("7.10.2")),
            &[("count", n(1)), ("first", t("//@argumentElement.0"))],
            "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define ({count} in all, the first {first:?}); each names an element of the document, so it is read as that element's id, which write writes",
            "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define (1 in all, the first \"//@argumentElement.0\"); each names an element of the document, so it is read as that element's id, which write writes",
        ),
        case(
            "SACM-XMI-017/paths-partly-read",
            tolerant(paths("//@argumentElement.0 //@argumentElement.7")),
            (WARNING, "LIB-COMPAT-002", Some("7.10.2")),
            &[
                ("count", n(2)),
                ("first", t("//@argumentElement.0")),
                ("kept", n(1)),
            ],
            "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define ({count} in all, the first {first:?}); each that names an element of the document is read as that element's id, which write writes, and those that name none ({kept}) are kept as written",
            "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define (2 in all, the first \"//@argumentElement.0\"); each that names an element of the document is read as that element's id, which write writes, and those that name none (1) are kept as written",
        ),
        // Slice 0081 (SACM23-XMI-032): a `version` in EMF's XMI namespace,
        // named with the prefix the file uses.
        case(
            "SACM-XMI-011/emf-version-on-xmi-element",
            tolerant(format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:sacm=\"{SACM_NS}\" \
                 xmi:version=\"2.0\"><sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/>\
                 </sacm:ArgumentPackage></xmi:XMI>"
            )),
            (INFO, "LIB-COMPAT-001", Some("7.5.3")),
            &[("attribute", t("xmi:version")), ("value", t("2.0"))],
            "the {attribute} {value:?} of the XMI element is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
            "the xmi:version \"2.0\" of the XMI element is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
        ),
        case(
            "SACM-XMI-011/emf-version-on-root",
            tolerant(emf_root(" xmi:version=\"2.0\"", "")),
            (INFO, "LIB-COMPAT-001", Some("7.5.3")),
            &[
                ("attribute", t("xmi:version")),
                ("value", t("2.0")),
                ("class", t("ArgumentPackage")),
                ("id", t("p")),
            ],
            "the {attribute} {value:?} of the {class} {id:?} is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
            "the xmi:version \"2.0\" of the ArgumentPackage \"p\" is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
        ),
    ]
}

/// A root ArgumentPackage `p` with the prefix `xmi` bound to EMF's namespace
/// `http://www.omg.org/XMI`, with the extra attributes `attributes`, holding
/// `body` after its name.
fn emf_root(attributes: &str, body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"http://www.omg.org/XMI\" \
         xmi:id=\"p\"{attributes}><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// The `xmi:XMI` wrapper holding the AssuranceCasePackage `case`, which holds
/// `body`, then the ArgumentPackage `args` and the ArtifactPackage `arts`:
/// roots a same-file proxy names (SACM23-XMI-030).
fn proxied(body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
         <sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"case\"/>{body}</sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:id=\"args\"><name content=\"args\"/></sacm:ArgumentPackage>\
         <sacm:ArtifactPackage xmi:id=\"arts\"><name content=\"arts\"/></sacm:ArtifactPackage></xmi:XMI>"
    )
}

/// The eight templates of slice 0073a (SACM23-XMI-019, SACM23-XMI-030; ADR
/// 0042): an XMI-namespace element XMI does not define under `xmi:XMI`, the
/// five kept forms of a same-file proxy, and the two checks a strict write
/// makes of XMI metadata.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn metadata_cases() -> Vec<Case> {
    vec![
        case(
            "SACM-XMI-003/wrapper-element-kept",
            tolerant(wrapper("", "<xmi:foo/>")),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[("element", t("xmi:foo")), ("container", t("xmi:XMI"))],
            concat!(
                "the element {element:?} in {container:?} is in the XMI namespace, but XMI 2.5.1 defines no such element there",
                kd!()
            ),
            "the element \"xmi:foo\" in \"xmi:XMI\" is in the XMI namespace, but XMI 2.5.1 defines no such element there, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/proxy-of-no-root",
            tolerant(proxied("<argumentPackage xmi:idref=\"nope\"/>")),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("argumentPackage")),
                ("target", t("nope")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of {target:?}, which is no root of the document, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"argumentPackage\" is a proxy of \"nope\", which is no root of the document, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/proxy-wrong-kind",
            tolerant(proxied("<argumentPackage xmi:idref=\"arts\"/>")),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("argumentPackage")),
                ("target", t("arts")),
                ("target_class", t("ArtifactPackage")),
                ("class", t("ArgumentPackage")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of the root {target:?}, of the class {target_class}, which is not of the class {class}, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"argumentPackage\" is a proxy of the root \"arts\", of the class ArtifactPackage, which is not of the class ArgumentPackage, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        // The root `args` keeps an `href` of its own, with its own
        // `attribute-kept`, whose message is another.
        case(
            "SACM-XMI-003/proxy-of-linked-root",
            tolerant(proxied("<argumentPackage xmi:idref=\"args\"/>").replace(
                "<sacm:ArgumentPackage xmi:id=\"args\">",
                "<sacm:ArgumentPackage xmi:id=\"args\" href=\"o.xmi#x\">",
            )),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("argumentPackage")),
                ("target", t("args")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of the root {target:?}, which carries an href or an xmi:idref of its own, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"argumentPackage\" is a proxy of the root \"args\", which carries an href or an xmi:idref of its own, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        // The root `args` keeps an `xmi:type` that only a root can carry
        // (LIB-COMPAT-020), with its own `attribute-kept`, whose message is
        // another.
        case(
            "SACM-XMI-003/proxy-of-typed-root",
            tolerant(proxied("<argumentPackage xmi:idref=\"args\"/>").replace(
                "<sacm:ArgumentPackage xmi:id=\"args\">",
                "<sacm:ArgumentPackage xmi:id=\"args\" xmi:type=\"sacm:ArtifactPackage\">",
            )),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("argumentPackage")),
                ("target", t("args")),
                ("attribute", t("xmi:type")),
                ("value", t("sacm:ArtifactPackage")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of the root {target:?}, which keeps the type attribute {attribute}={value:?} that only a root can carry, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"argumentPackage\" is a proxy of the root \"args\", which keeps the type attribute xmi:type=\"sacm:ArtifactPackage\" that only a root can carry, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        // Two proxies of `args`, one in `case` and one in `two`: each is
        // `proxy-shared`, and only the message of the first names `case`.
        case(
            "SACM-XMI-003/proxy-shared",
            tolerant(proxied("<argumentPackage xmi:idref=\"args\"/>").replace(
                "<sacm:ArgumentPackage xmi:id=\"args\">",
                "<sacm:AssuranceCasePackage xmi:id=\"two\"><name content=\"two\"/>\
                 <argumentPackage xmi:idref=\"args\"/></sacm:AssuranceCasePackage>\
                 <sacm:ArgumentPackage xmi:id=\"args\">",
            )),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("argumentPackage")),
                ("target", t("args")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of the root {target:?}, which another proxy names too, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"argumentPackage\" is a proxy of the root \"args\", which another proxy names too, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/proxy-cycle",
            tolerant(proxied("<assuranceCasePackage xmi:idref=\"case\"/>")),
            (WARNING, "SACM23-XMI-030", Some("7.10.1")),
            &[
                ("element", t("assuranceCasePackage")),
                ("target", t("case")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
            ],
            "the element {element:?} is a proxy of the root {target:?}, which holds this proxy itself, directly or through other proxies, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"assuranceCasePackage\" is a proxy of the root \"case\", which holds this proxy itself, directly or through other proxies, so it is kept as written as preserved content of the AssuranceCasePackage \"case\", which a tolerant write writes back and a strict write refuses",
        ),
        // The checks a strict write makes of XMI metadata.
        case(
            "SACM-REF-001/metadata-dangling",
            write(wrapper("", "<xmi:Difference target=\"p nope\"/>")),
            (ERROR, "SACM23-XMI-019", None),
            &[
                ("attribute", t("target")),
                ("element", t("xmi:Difference")),
                ("target", t("nope")),
            ],
            "the attribute {attribute:?} of the element {element:?}, kept as XMI metadata, names {target:?}, which is no xmi:id of the document (XMI.xsd types it xsd:IDREF or xsd:IDREFS)",
            "the attribute \"target\" of the element \"xmi:Difference\", kept as XMI metadata, names \"nope\", which is no xmi:id of the document (XMI.xsd types it xsd:IDREF or xsd:IDREFS)",
        ),
        case(
            "SACM-ID-002/metadata-id-not-an-ncname",
            write(package("<xmi:Extension xmi:id=\"1x\"/>")),
            (ERROR, "SACM23-XMI-019", Some("7.6.1")),
            &[("element", t("xmi:Extension")), ("xmi_id", t("1x"))],
            concat!(
                "the element {element:?}, kept as XMI metadata, has the xmi:id {xmi_id:?}",
                not_ncname!()
            ),
            "the element \"xmi:Extension\", kept as XMI metadata, has the xmi:id \"1x\", which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
        ),
    ]
}

/// The three Warnings of a root a tolerant read reads in another SACM
/// namespace (slice 0072; LIB-COMPAT-004, ADR 0041 D4, D5): an older SACM
/// revision's, with an Event's `occurence`, and a SACM-like one. No clause.
fn namespace_cases() -> Vec<Case> {
    let root_in = |uri: &str| {
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{uri}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\">\
             <name content=\"p\"/></sacm:ArgumentPackage>"
        )
    };
    vec![
        case(
            "SACM-XMI-008/other-revision",
            tolerant(root_in("http://www.omg.org/spec/SACM/2.2/Argumentation")),
            (WARNING, "LIB-COMPAT-004", None),
            &[
                ("class", t("ArgumentPackage")),
                ("id", t("p")),
                (
                    "namespace",
                    t("http://www.omg.org/spec/SACM/2.2/Argumentation"),
                ),
                ("revision", t("2.2")),
            ],
            "the {class} {id:?} is in the namespace {namespace:?} of SACM {revision}, so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3",
            "the ArgumentPackage \"p\" is in the namespace \"http://www.omg.org/spec/SACM/2.2/Argumentation\" of SACM 2.2, so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3",
        ),
        case(
            "SACM-XMI-008/occurence",
            tolerant(format!(
                "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20210218\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"arts\"><name content=\"arts\"/>\
                 <artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\" occurence=\"2021-02-18\">\
                 <name content=\"e1\"/></artifactElement></sacm:ArtifactPackage>"
            )),
            (WARNING, "LIB-COMPAT-004", None),
            &[("id", t("e1")), ("revision", t("2.2"))],
            "the Event {id:?} gives its date as the attribute \"occurence\", the SACM {revision} spelling, which is read as its date",
            "the Event \"e1\" gives its date as the attribute \"occurence\", the SACM 2.2 spelling, which is read as its date",
        ),
        case(
            "SACM-XMI-002/sacm-like",
            tolerant(root_in("urn:example:sacm")),
            (WARNING, "LIB-COMPAT-004", None),
            &[
                ("class", t("ArgumentPackage")),
                ("id", t("p")),
                ("namespace", t("urn:example:sacm")),
            ],
            "the {class} {id:?} is in the namespace {namespace:?}, which names SACM but is not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\", so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3",
            "the ArgumentPackage \"p\" is in the namespace \"urn:example:sacm\", which names SACM but is not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\", so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3",
        ),
    ]
}

/// The 19 plain reports of a read.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn plain_reader_cases() -> Vec<Case> {
    vec![
        case(
            "SACM-XMI-004/generated-id",
            tolerant(package(
                "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"c\"/></argumentElement>",
            )),
            (INFO, "SACM23-XMI-020", Some("7.6.1")),
            &[("class", t("Claim")), ("id", t("claim_1"))],
            "the {class} has no xmi:id, which XMI 2.5.1 §7.6.1 makes optional, so it is read with the generated id {id:?}, which write writes",
            "the Claim has no xmi:id, which XMI 2.5.1 §7.6.1 makes optional, so it is read with the generated id \"claim_1\", which write writes",
        ),
        case(
            "SACM-ID-004/trimmed-id",
            tolerant(package(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\" c1 \"><name content=\"c1\"/></argumentElement>",
            )),
            (INFO, "SACM23-XMI-025", Some("7.6.1")),
            &[
                ("written", t(" c1 ")),
                ("class", t("Claim")),
                ("id", t("c1")),
            ],
            concat!(
                "the xmi:id {written:?} of the {class} {id:?} is read as {id:?}",
                trim!()
            ),
            "the xmi:id \" c1 \" of the Claim \"c1\" is read as \"c1\": an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed",
        ),
        case(
            "SACM-ID-004/trimmed-id-of-name",
            tolerant(package(&bare_claim(
                "c1",
                "",
                "<name xmi:id=\" n1 \" content=\"c1\"/>",
            ))),
            (INFO, "SACM23-XMI-025", Some("7.6.1")),
            &[
                ("written", t(" n1 ")),
                ("class", t("Claim")),
                ("id", t("c1")),
                ("value", t("n1")),
            ],
            concat!(
                "the xmi:id {written:?} of the name of the {class} {id:?} is read as {value:?}",
                trim!()
            ),
            "the xmi:id \" n1 \" of the name of the Claim \"c1\" is read as \"n1\": an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed",
        ),
        case(
            "SACM-ID-004/trimmed-id-of-text",
            tolerant(note_in_claim(
                "<content xmi:id=\" t1 \"><value content=\"a\"/></content>",
            )),
            (INFO, "SACM23-XMI-025", Some("7.6.1")),
            &[
                ("written", t(" t1 ")),
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("value", t("t1")),
            ],
            concat!(
                "the xmi:id {written:?} of the {role} of the {class} {id:?} is read as {value:?}",
                trim!()
            ),
            "the xmi:id \" t1 \" of the content of the Note \"n\" is read as \"t1\": an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed",
        ),
        case(
            "SACM-ID-004/trimmed-id-of-text-value",
            tolerant(note_in_claim(
                "<content><value xmi:id=\" v1 \" content=\"a\"/></content>",
            )),
            (INFO, "SACM23-XMI-025", Some("7.6.1")),
            &[
                ("written", t(" v1 ")),
                ("role", t("content")),
                ("class", t("Note")),
                ("id", t("n")),
                ("value", t("v1")),
            ],
            concat!(
                "the xmi:id {written:?} of a value of the {role} of the {class} {id:?} is read as {value:?}",
                trim!()
            ),
            "the xmi:id \" v1 \" of a value of the content of the Note \"n\" is read as \"v1\": an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed",
        ),
        case(
            "SACM-XMI-010/xsi-type",
            tolerant(package(
                "<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentElement>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("class", t("Claim")), ("id", t("c1"))],
            concat!("the {class} {id:?}", xsi!()),
            "the Claim \"c1\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-010/xsi-type-on-name",
            tolerant(package(&bare_claim(
                "c1",
                "",
                "<name xsi:type=\"sacm:LangString\" content=\"c1\"/>",
            ))),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("class", t("Claim")), ("id", t("c1"))],
            concat!("the name of the {class} {id:?}", xsi!()),
            "the name of the Claim \"c1\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-010/xsi-type-on-text",
            tolerant(note_in_claim(
                "<content xsi:type=\"sacm:MultiLangString\"><value content=\"a\"/></content>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!("the {role} of the {class} {id:?}", xsi!()),
            "the content of the Note \"n\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-010/xsi-type-on-text-value",
            tolerant(note_in_claim(
                "<content><value xsi:type=\"sacm:LangString\" content=\"a\"/></content>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!("a value of the {role} of the {class} {id:?}", xsi!()),
            "a value of the content of the Note \"n\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-010/xsi-type-on-name-value",
            tolerant(package(&bare_claim(
                "c1",
                "",
                "<name><value xsi:type=\"sacm:LangString\" content=\"c1\"/></name>",
            ))),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("class", t("Claim")), ("id", t("c1"))],
            concat!("the value in the name of the {class} {id:?}", xsi!()),
            "the value in the name of the Claim \"c1\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-010/xsi-type-on-proxy",
            tolerant(format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
                 <argumentPackage xsi:type=\"sacm:ArgumentPackageInterface\" href=\"b.xmi#api\"/>\
                 </sacm:AssuranceCasePackage>"
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[
                ("element", t("argumentPackage")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
                ("class", t("ArgumentPackageInterface")),
            ],
            "the proxy {element:?} in the {holder_class} {holder_id:?} is typed {class} with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
            "the proxy \"argumentPackage\" in the AssuranceCasePackage \"case\" is typed ArgumentPackageInterface with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        case(
            "SACM-XMI-011/version-on-xmi-element",
            tolerant(wrapper(" xmi:version=\"2.5.1\"", "")),
            (INFO, "LIB-COMPAT-001", Some("7.5.3")),
            &[("value", t("2.5.1"))],
            "the xmi:version {value:?} of the XMI element is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
            "the xmi:version \"2.5.1\" of the XMI element is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
        ),
        case(
            "SACM-XMI-011/version-on-root",
            tolerant(format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" xmi:version=\"2.5.1\"><name content=\"p\"/></sacm:ArgumentPackage>"
            )),
            (INFO, "LIB-COMPAT-001", Some("7.5.3")),
            &[
                ("value", t("2.5.1")),
                ("class", t("ArgumentPackage")),
                ("id", t("p")),
            ],
            "the xmi:version {value:?} of the {class} {id:?} is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
            "the xmi:version \"2.5.1\" of the ArgumentPackage \"p\" is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it",
        ),
        case(
            "SACM-ID-002/empty-id",
            tolerant(package(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"><name content=\"c\"/></argumentElement>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[("class", t("Claim")), ("id", t("claim_1"))],
            concat!(
                "the xmi:id of the {class}",
                empty!(),
                "it is read with the generated id {id:?}, which write writes"
            ),
            "the xmi:id of the Claim is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read with the generated id \"claim_1\", which write writes",
        ),
        case(
            "SACM-ID-002/empty-id-of-name",
            tolerant(package(&bare_claim(
                "c1",
                "",
                "<name xmi:id=\"\" content=\"c1\"/>",
            ))),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[("class", t("Claim")), ("id", t("c1"))],
            concat!(
                "the xmi:id of the name of the {class} {id:?}",
                empty!(),
                "it is read without one"
            ),
            "the xmi:id of the name of the Claim \"c1\" is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one",
        ),
        case(
            "SACM-ID-002/empty-id-of-text",
            tolerant(note_in_claim(
                "<content xmi:id=\"\"><value content=\"a\"/></content>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!(
                "the xmi:id of the {role} of the {class} {id:?}",
                empty!(),
                "it is read without one"
            ),
            "the xmi:id of the content of the Note \"n\" is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one",
        ),
        case(
            "SACM-ID-002/empty-id-of-text-value",
            tolerant(note_in_claim(
                "<content><value xmi:id=\"\" content=\"a\"/></content>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!(
                "the xmi:id of a value of the {role} of the {class} {id:?}",
                empty!(),
                "it is read without one"
            ),
            "the xmi:id of a value of the content of the Note \"n\" is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one",
        ),
        case(
            "SACM-ID-002/empty-id-of-xmi-element",
            tolerant(wrapper(" xmi:id=\"\"", "")),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[],
            concat!(
                "the xmi:id of the XMI element",
                empty!(),
                "it is kept as written, outside the id space"
            ),
            "the xmi:id of the XMI element is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space",
        ),
        case(
            "SACM-ID-002/empty-id-of-kept-element",
            tolerant(package(&claim("c1", "", "<v:thing xmi:id=\"\"/>"))),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[
                ("element", t("v:thing")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            concat!(
                "the xmi:id of the element {element:?} kept in the {holder_class} {holder_id:?}",
                empty!(),
                "it is kept as written, outside the id space"
            ),
            "the xmi:id of the element \"v:thing\" kept in the Claim \"c1\" is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space",
        ),
        case(
            "SACM-ID-002/empty-id-of-kept-element-by-document",
            // An element XMI does not define, kept as the document's content
            // (an `xmi:Documentation` until slice 0073a read it as XMI
            // metadata, SACM23-XMI-019).
            tolerant(wrapper("", "<xmi:foo xmi:id=\"\"/>")),
            (WARNING, "LIB-COMPAT-001", Some("7.6.1")),
            &[("element", t("xmi:foo"))],
            concat!(
                "the xmi:id of the element {element:?} kept in the document",
                empty!(),
                "it is kept as written, outside the id space"
            ),
            "the xmi:id of the element \"xmi:foo\" kept in the document is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space",
        ),
    ]
}

/// The 6 SACM-XMI-003 forms of preserved content.
fn preserved_cases() -> Vec<Case> {
    vec![
        case(
            "SACM-XMI-003/attribute-kept",
            tolerant(package(&claim("c1", " v:colour=\"red\"", ""))),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[
                ("attribute", t("v:colour")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            "the attribute {attribute:?} is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the attribute \"v:colour\" is not SACM 2.3 where it stands, so it is kept as written as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/element-kept",
            tolerant(package(&claim("c1", "", "<v:thing/>"))),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[
                ("element", t("v:thing")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            "the element {element:?} is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the element \"v:thing\" is not SACM 2.3 where it stands, so it is kept as written as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/text-kept",
            tolerant(package(&claim("c1", "", "stray"))),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[("holder_class", t("Claim")), ("holder_id", t("c1"))],
            "text is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "text is not SACM 2.3 where it stands, so it is kept as written as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/wrapper-attribute-kept",
            tolerant(wrapper(" foo=\"bar\"", "")),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[("attribute", t("foo")), ("element", t("xmi:XMI"))],
            concat!(
                "the attribute {attribute:?} of the element {element:?} is not SACM 2.3 where it stands",
                kd!()
            ),
            "the attribute \"foo\" of the element \"xmi:XMI\" is not SACM 2.3 where it stands, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/wrapper-text-kept",
            tolerant(wrapper("", "stray")),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[("element", t("xmi:XMI"))],
            concat!(
                "text in the element {element:?} is not SACM 2.3 where it stands",
                kd!()
            ),
            "text in the element \"xmi:XMI\" is not SACM 2.3 where it stands, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-003/prefix-renamed",
            tolerant(package(&format!(
                "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
                 xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
            ))),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[
                ("attribute", t("sacm:extra")),
                ("element", t("argumentElement")),
                ("prefix", t("sacm")),
                ("namespace", t("urn:example:other")),
                ("pinned_namespace", t(SACM_NS)),
                ("renamed_prefix", t("ns1")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            concat!(
                "the attribute {attribute:?} of the element {element:?} is not SACM 2.3 where it stands, and its prefix {prefix:?} is bound to {namespace:?}, which the written element binds to {pinned_namespace:?}, so it is written with the prefix {renamed_prefix:?}",
                ke!()
            ),
            "the attribute \"sacm:extra\" of the element \"argumentElement\" is not SACM 2.3 where it stands, and its prefix \"sacm\" is bound to \"urn:example:other\", which the written element binds to \"http://www.omg.org/spec/SACM/20220301\", so it is written with the prefix \"ns1\", so it is kept as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses",
        ),
    ]
}

/// The 9 SACM-XMI-012 shapes a tolerant read reads.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn legacy_cases() -> Vec<Case> {
    let named = |name: &str| {
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">{name}</sacm:ArgumentPackage>")
    };
    vec![
        case(
            "SACM-XMI-012/name-attribute-read",
            tolerant(format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\" name=\"Arguments\"/>"
            )),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("ArgumentPackage")), ("id", t("arguments"))],
            concat!(
                "the name of the {class} {id:?} is given as the attribute \"name\"",
                legacy!()
            ),
            "the name of the ArgumentPackage \"arguments\" is given as the attribute \"name\", a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/content-on-element-read",
            tolerant(package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\" content=\"x\"/>",
            ))),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("Note")), ("id", t("n"))],
            concat!(
                "the content of the {class} {id:?} is given on the element itself",
                legacy!()
            ),
            "the content of the Note \"n\" is given on the element itself, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/key-attribute-read",
            tolerant(package(&claim(
                "c1",
                "",
                "<taggedValue xmi:id=\"t\" key=\"k\"/>",
            ))),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("TaggedValue")), ("id", t("t"))],
            concat!(
                "the key of the {class} {id:?} is given as the attribute \"key\"",
                legacy!()
            ),
            "the key of the TaggedValue \"t\" is given as the attribute \"key\", a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/shorthand-read",
            tolerant(note_in_claim("<content content=\"x\" lang=\"en\"/>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!(
                "the {role} of the {class} {id:?} is given in the single-language shorthand",
                legacy!()
            ),
            "the content of the Note \"n\" is given in the single-language shorthand, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/name-text-read",
            tolerant(named("<name>p</name>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            concat!(
                "the name of the {class} {id:?} has its content as text",
                legacy!()
            ),
            "the name of the ArgumentPackage \"p\" has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/value-text-read",
            tolerant(note_in_claim("<content><value>x</value></content>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            concat!(
                "a value of the {role} of the {class} {id:?} has its content as text",
                legacy!()
            ),
            "a value of the content of the Note \"n\" has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/name-value-text-read",
            tolerant(named("<name><value>A</value></name>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            concat!(
                "the value in the name of the {class} {id:?} has its content as text",
                legacy!()
            ),
            "the value in the name of the ArgumentPackage \"p\" has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/name-content-lang-read",
            tolerant(named("<name><content lang=\"en\">p</content></name>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            concat!(
                "the name of the {class} {id:?} holds a content carrying lang",
                legacy!()
            ),
            "the name of the ArgumentPackage \"p\" holds a content carrying lang, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
        case(
            "SACM-XMI-012/name-value-read",
            tolerant(named("<name><value content=\"p\"/></name>")),
            (WARNING, "LIB-COMPAT-008", Some("7.8.6")),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            concat!("the name of the {class} {id:?} holds a value", legacy!()),
            "the name of the ArgumentPackage \"p\" holds a value, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes",
        ),
    ]
}

/// The 16 SACM-XMI-013 spellings: `-read` from a tolerant read, `-kept` from
/// a strict one.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn spelling_cases() -> Vec<Case> {
    vec![
        case(
            "SACM-XMI-013/group-end-read",
            tolerant(spelled_group()),
            (WARNING, "SACM23-XMI-009", Some("11.2")),
            &[("element", t("argumentElement"))],
            concat!(
                "\"argumentationElement\" in the element {element:?} is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement",
                read_as!("argumentElement")
            ),
            "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement; it is read as argumentElement, which write writes",
        ),
        case(
            "SACM-XMI-013/group-end-kept",
            strict(spelled_group()),
            (ERROR, "SACM23-XMI-009", Some("11.2")),
            &[("element", t("argumentElement"))],
            concat!(
                "\"argumentationElement\" in the element {element:?} is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement",
                kept_as!("argumentElement")
            ),
            "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement",
        ),
        case(
            "SACM-XMI-013/event-date-read",
            tolerant(event(" occurece=\"2024-01-01\"")),
            (WARNING, "SACM23-XMI-009", Some("12.9")),
            &[("element", t("artifactElement"))],
            concat!(
                "\"occurece\" in the element {element:?} is the machine-readable model's spelling, and the attribute of an Event is date (§12.9)",
                read_as!("date")
            ),
            "\"occurece\" in the element \"artifactElement\" is the machine-readable model's spelling, and the attribute of an Event is date (§12.9); it is read as date, which write writes",
        ),
        case(
            "SACM-XMI-013/event-date-kept",
            strict(event(" occurece=\"2024-01-01\"")),
            (ERROR, "SACM23-XMI-009", Some("12.9")),
            &[("element", t("artifactElement"))],
            concat!(
                "\"occurece\" in the element {element:?} is the machine-readable model's spelling, and the attribute of an Event is date (§12.9)",
                kept_as!("date")
            ),
            "\"occurece\" in the element \"artifactElement\" is the machine-readable model's spelling, and the attribute of an Event is date (§12.9); it is kept as preserved content, and a tolerant read reads the name as date",
        ),
        case(
            "SACM-XMI-013/package-role-read",
            tolerant(nested(SPELLED_ROLE)),
            (WARNING, "SACM23-XMI-009", Some("11.4")),
            &[
                ("element", t("argumentationElement")),
                ("container", t("argumentPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is spelled as in the text of §11.4, and the role is argumentElement",
                read_as!("argumentElement")
            ),
            "the element \"argumentationElement\" in the element \"argumentPackage\" is spelled as in the text of §11.4, and the role is argumentElement; it is read as argumentElement, which write writes",
        ),
        case(
            "SACM-XMI-013/package-role-kept",
            strict(nested(SPELLED_ROLE)),
            (ERROR, "SACM23-XMI-009", Some("11.4")),
            &[
                ("element", t("argumentationElement")),
                ("container", t("argumentPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is spelled as in the text of §11.4, and the role is argumentElement",
                kept_as!("argumentElement")
            ),
            "the element \"argumentationElement\" in the element \"argumentPackage\" is spelled as in the text of §11.4, and the role is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement",
        ),
        case(
            "SACM-XMI-013/package-role-in-root-read",
            tolerant(package(SPELLED_ROLE)),
            (WARNING, "SACM23-XMI-009", Some("11.4")),
            &[
                ("element", t("argumentationElement")),
                ("container", t("sacm:ArgumentPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is spelled as in the text of §11.4, and the role is argumentElement",
                read_as!("argumentElement")
            ),
            "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" is spelled as in the text of §11.4, and the role is argumentElement; it is read as argumentElement, which write writes",
        ),
        case(
            "SACM-XMI-013/package-role-in-root-kept",
            strict(package(SPELLED_ROLE)),
            (ERROR, "SACM23-XMI-009", Some("11.4")),
            &[
                ("element", t("argumentationElement")),
                ("container", t("sacm:ArgumentPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is spelled as in the text of §11.4, and the role is argumentElement",
                kept_as!("argumentElement")
            ),
            "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" is spelled as in the text of §11.4, and the role is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement",
        ),
        case(
            "SACM-XMI-013/terminology-role-read",
            tolerant(nested_terminology(SPELLED_TERM)),
            (WARNING, "SACM23-XMI-028", Some("10.4")),
            &[
                ("element", t("TerminologyElement")),
                ("container", t("terminologyPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement",
                read_as!("terminologyElement")
            ),
            "the element \"TerminologyElement\" in the element \"terminologyPackage\" is spelled as in the text of §10.4, and the role is terminologyElement; it is read as terminologyElement, which write writes",
        ),
        case(
            "SACM-XMI-013/terminology-role-kept",
            strict(nested_terminology(SPELLED_TERM)),
            (ERROR, "SACM23-XMI-028", Some("10.4")),
            &[
                ("element", t("TerminologyElement")),
                ("container", t("terminologyPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement",
                kept_as!("terminologyElement")
            ),
            "the element \"TerminologyElement\" in the element \"terminologyPackage\" is spelled as in the text of §10.4, and the role is terminologyElement; it is kept as preserved content, and a tolerant read reads the name as terminologyElement",
        ),
        case(
            "SACM-XMI-013/terminology-role-in-root-read",
            tolerant(terminology(SPELLED_TERM)),
            (WARNING, "SACM23-XMI-028", Some("10.4")),
            &[
                ("element", t("TerminologyElement")),
                ("container", t("sacm:TerminologyPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement",
                read_as!("terminologyElement")
            ),
            "the element \"TerminologyElement\" in the root element \"sacm:TerminologyPackage\" is spelled as in the text of §10.4, and the role is terminologyElement; it is read as terminologyElement, which write writes",
        ),
        case(
            "SACM-XMI-013/terminology-role-in-root-kept",
            strict(terminology(SPELLED_TERM)),
            (ERROR, "SACM23-XMI-028", Some("10.4")),
            &[
                ("element", t("TerminologyElement")),
                ("container", t("sacm:TerminologyPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement",
                kept_as!("terminologyElement")
            ),
            "the element \"TerminologyElement\" in the root element \"sacm:TerminologyPackage\" is spelled as in the text of §10.4, and the role is terminologyElement; it is kept as preserved content, and a tolerant read reads the name as terminologyElement",
        ),
        case(
            "SACM-XMI-013/model-type-read",
            tolerant(nested_artifacts(SPELLED_TYPE)),
            (WARNING, "SACM23-XMI-009", Some("12.14")),
            &[
                ("element", t("artifactElement")),
                ("container", t("artifactPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship",
                read_as!("ArtifactAssetRelationship")
            ),
            "the element \"artifactElement\" in the element \"artifactPackage\" is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is read as ArtifactAssetRelationship, which write writes",
        ),
        case(
            "SACM-XMI-013/model-type-kept",
            strict(nested_artifacts(SPELLED_TYPE)),
            (ERROR, "SACM23-XMI-009", Some("12.14")),
            &[
                ("element", t("artifactElement")),
                ("container", t("artifactPackage")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship",
                kept_as!("ArtifactAssetRelationship")
            ),
            "the element \"artifactElement\" in the element \"artifactPackage\" is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is kept as preserved content, and a tolerant read reads the name as ArtifactAssetRelationship",
        ),
        case(
            "SACM-XMI-013/model-type-in-root-read",
            tolerant(artifacts(SPELLED_TYPE)),
            (WARNING, "SACM23-XMI-009", Some("12.14")),
            &[
                ("element", t("artifactElement")),
                ("container", t("sacm:ArtifactPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship",
                read_as!("ArtifactAssetRelationship")
            ),
            "the element \"artifactElement\" in the root element \"sacm:ArtifactPackage\" is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is read as ArtifactAssetRelationship, which write writes",
        ),
        case(
            "SACM-XMI-013/model-type-in-root-kept",
            strict(artifacts(SPELLED_TYPE)),
            (ERROR, "SACM23-XMI-009", Some("12.14")),
            &[
                ("element", t("artifactElement")),
                ("container", t("sacm:ArtifactPackage")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship",
                kept_as!("ArtifactAssetRelationship")
            ),
            "the element \"artifactElement\" in the root element \"sacm:ArtifactPackage\" is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is kept as preserved content, and a tolerant read reads the name as ArtifactAssetRelationship",
        ),
    ]
}

/// The 6 forms of content a tolerant read keeps where a strict read
/// refuses it, but the three of SACM-XMI-003 above: the kept forms of
/// SACM-XMI-005, SACM-ENUM-001 and SACM-MULT-001. Slice 0073a removes the
/// eight of SACM-XMI-014 and slice 0073b the four of SACM-XMI-007, whose
/// codes no input gives any more (SACM23-XMI-019, ADR 0042; SACM23-XMI-029,
/// ADR 0040 amendment 1).
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn kept_cases() -> Vec<Case> {
    vec![
        // --- SACM-XMI-005 --------------------------------------------------------------
        case(
            "SACM-XMI-005/missing-type-kept",
            tolerant(nested("<argumentElement xmi:id=\"c1\"/>")),
            (WARNING, "LIB-COMPAT-001", Some("7.6.3")),
            &[
                ("element", t("argumentElement")),
                ("container", t("argumentPackage")),
                ("role", t("argumentElement")),
                ("type", t("ArgumentationElement")),
                ("holder_class", t("ArgumentPackage")),
                ("holder_id", t("ap")),
            ],
            concat!(
                "the element {element:?} in the element {container:?} has no type, and its role {role} has the abstract type {type}, so its class is not known (XMI 2.5.1 §7.6.3)",
                ke!()
            ),
            "the element \"argumentElement\" in the element \"argumentPackage\" has no type, and its role argumentElement has the abstract type ArgumentationElement, so its class is not known (XMI 2.5.1 §7.6.3), so it is kept as preserved content of the ArgumentPackage \"ap\", which a tolerant write writes back and a strict write refuses",
        ),
        case(
            "SACM-XMI-005/missing-type-in-root-kept",
            tolerant(package("<argumentElement xmi:id=\"c1\"/>")),
            (WARNING, "LIB-COMPAT-001", Some("7.6.3")),
            &[
                ("element", t("argumentElement")),
                ("container", t("sacm:ArgumentPackage")),
                ("role", t("argumentElement")),
                ("type", t("ArgumentationElement")),
                ("holder_class", t("ArgumentPackage")),
                ("holder_id", t("p")),
            ],
            concat!(
                "the element {element:?} in the root element {container:?} has no type, and its role {role} has the abstract type {type}, so its class is not known (XMI 2.5.1 §7.6.3)",
                ke!()
            ),
            "the element \"argumentElement\" in the root element \"sacm:ArgumentPackage\" has no type, and its role argumentElement has the abstract type ArgumentationElement, so its class is not known (XMI 2.5.1 §7.6.3), so it is kept as preserved content of the ArgumentPackage \"p\", which a tolerant write writes back and a strict write refuses",
        ),
        // --- SACM-ENUM-001 -------------------------------------------------------------
        case(
            "SACM-ENUM-001/declaration-kept",
            tolerant(package(&claim(
                "c1",
                " assertionDeclaration=\"Assumed\"",
                "",
            ))),
            (WARNING, "SACM23-VAL-006", None),
            &[
                ("value", t("Assumed")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            concat!(
                "the assertionDeclaration {value:?} is no literal: the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited",
                ke!(),
                ", and the property is read as not written"
            ),
            "the assertionDeclaration \"Assumed\" is no literal: the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited, so it is kept as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses, and the property is read as not written",
        ),
        case(
            "SACM-ENUM-001/boolean-kept",
            tolerant(package(&claim("c1", " isCitation=\"yes\"", ""))),
            (WARNING, "SACM23-VAL-006", None),
            &[
                ("property", t("isCitation")),
                ("value", t("yes")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            concat!(
                "the {property} {value:?} is no literal: the literals are true, false, 1 and 0",
                ke!(),
                ", and the property is read as not written"
            ),
            "the isCitation \"yes\" is no literal: the literals are true, false, 1 and 0, so it is kept as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses, and the property is read as not written",
        ),
        // --- SACM-MULT-001 -------------------------------------------------------------
        case(
            "SACM-MULT-001/repeat-kept",
            tolerant(package(&claim("c1", "", "<name content=\"again\"/>"))),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[
                ("property", t("name")),
                ("element", t("c1")),
                ("holder_class", t("Claim")),
                ("holder_id", t("c1")),
            ],
            concat!(
                "the {property} of the element {element:?} is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence",
                ke!(),
                "; the first occurrence is read"
            ),
            "the name of the element \"c1\" is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence, so it is kept as preserved content of the Claim \"c1\", which a tolerant write writes back and a strict write refuses; the first occurrence is read",
        ),
        case(
            "SACM-MULT-001/repeat-attribute-kept",
            tolerant(package(&claim(
                "c1",
                "",
                "<taggedValue xmi:id=\"t\" key=\"k\" content=\"a\" value=\"b\"/>",
            ))),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[
                ("property", t("content.value.content")),
                ("element", t("t")),
                ("holder_class", t("TaggedValue")),
                ("holder_id", t("t")),
            ],
            concat!(
                "the {property} of the element {element:?} is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence",
                ke!(),
                "; the attribute that gave it first is kept with it, and the element has no content from them"
            ),
            "the content.value.content of the element \"t\" is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence, so it is kept as preserved content of the TaggedValue \"t\", which a tolerant write writes back and a strict write refuses; the attribute that gave it first is kept with it, and the element has no content from them",
        ),
    ]
}

/// More sites of the templates that more than one site gives, each checked
/// as a case of the table is, but not counted among the 106.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per site, reads best whole"
)]
fn sites() -> Vec<Case> {
    vec![
        // SACM-MULT-001/missing: a key, a value, and the ends of relationships.
        case(
            "SACM-MULT-001/missing",
            validate(package(&claim("c1", "", "<taggedValue xmi:id=\"t\"/>"))),
            (ERROR, "SACM23-VAL-010", Some("8.12")),
            &[
                ("class", t("TaggedValue")),
                ("id", t("t")),
                ("property", t("key")),
                ("type", t("MultiLangString")),
                ("multiplicity", t("1")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "TaggedValue \"t\" has no key (key:MultiLangString[1])",
        ),
        case(
            "SACM-MULT-001/missing",
            validate(terminology(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\"><name content=\"t1\"/></terminologyElement>",
            )),
            (ERROR, "SACM23-TERM-003", Some("10.9")),
            &[
                ("class", t("Term")),
                ("id", t("t1")),
                ("property", t("value")),
                ("type", t("String")),
                ("multiplicity", t("1")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "Term \"t1\" has no value (value:String[1])",
        ),
        case(
            "SACM-MULT-001/missing",
            validate(package(&argument(
                "AssertedInference",
                "i1",
                " target=\"p\"",
            ))),
            (ERROR, "SACM23-VAL-005", Some("11.13")),
            &[
                ("class", t("AssertedInference")),
                ("id", t("i1")),
                ("property", t("source")),
                ("type", t("ArgumentAsset")),
                ("multiplicity", t("1..*")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "AssertedInference \"i1\" has no source (source:ArgumentAsset[1..*])",
        ),
        case(
            "SACM-MULT-001/missing",
            validate(package(&format!(
                "{}{}",
                claim("c1", "", ""),
                argument("AssertedInference", "i1", " source=\"c1\"")
            ))),
            (ERROR, "SACM23-VAL-005", Some("11.13")),
            &[
                ("class", t("AssertedInference")),
                ("id", t("i1")),
                ("property", t("target")),
                ("type", t("ArgumentAsset")),
                ("multiplicity", t("1")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "AssertedInference \"i1\" has no target (target:ArgumentAsset[1])",
        ),
        case(
            "SACM-MULT-001/missing",
            validate(artifacts(
                "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a1\"><name content=\"a1\"/></artifactElement>\
                 <artifactElement xmi:type=\"sacm:ArtifactAssetRelationship\" xmi:id=\"r1\" source=\"a1\"><name content=\"r1\"/></artifactElement>",
            )),
            (ERROR, "SACM23-ART-010", Some("12.14")),
            &[
                ("class", t("ArtifactAssetRelationship")),
                ("id", t("r1")),
                ("property", t("target")),
                ("type", t("ArtifactAsset")),
                ("multiplicity", t("1..*")),
            ],
            "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])",
            "ArtifactAssetRelationship \"r1\" has no target (target:ArtifactAsset[1..*])",
        ),
        // SACM-REF-001/dangling: a strict write's schema check (LIB-COMPAT-007).
        case(
            "SACM-REF-001/dangling",
            write(package(&claim(
                "c1",
                " isCitation=\"true\" citedElement=\"nope\"",
                "",
            ))),
            (ERROR, "SACM23-VAL-003", Some("8.2")),
            &[
                ("class", t("Claim")),
                ("id", t("c1")),
                ("end", t("citedElement")),
                ("target", t("nope")),
                ("type", t("SACMElement")),
            ],
            "{class} {id:?} has the {end} {target:?}, which names no element of the document (the end's type is {type})",
            "Claim \"c1\" has the citedElement \"nope\", which names no element of the document (the end's type is SACMElement)",
        ),
        // SACM-XMI-010/xsi-type: on a root.
        case(
            "SACM-XMI-010/xsi-type",
            tolerant(format!(
                "<sacm:ArgumentPackage{NS} xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>"
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            concat!("the {class} {id:?}", xsi!()),
            "the ArgumentPackage \"p\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        // SACM-XMI-010/xsi-type-on-proxy: a same-file proxy that is kept, a
        // strict read: an Error.
        case(
            "SACM-XMI-010/xsi-type-on-proxy",
            strict(format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
                 <argumentPackage xsi:type=\"sacm:ArgumentPackage\" xmi:idref=\"nope\"/>\
                 </sacm:AssuranceCasePackage>"
            )),
            (ERROR, "LIB-COMPAT-001", Some("9.5.2")),
            &[
                ("element", t("argumentPackage")),
                ("holder_class", t("AssuranceCasePackage")),
                ("holder_id", t("case")),
                ("class", t("ArgumentPackage")),
            ],
            "the proxy {element:?} in the {holder_class} {holder_id:?} is typed {class} with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
            "the proxy \"argumentPackage\" in the AssuranceCasePackage \"case\" is typed ArgumentPackage with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        // SACM-XMI-010/xsi-type, a strict read: an Error.
        case(
            "SACM-XMI-010/xsi-type",
            strict(package(
                "<argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentElement>",
            )),
            (ERROR, "LIB-COMPAT-001", Some("9.5.2")),
            &[("class", t("Claim")), ("id", t("c1"))],
            concat!("the {class} {id:?}", xsi!()),
            "the Claim \"c1\" is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type",
        ),
        // SACM-XMI-003/attribute-kept: a lang that gives no content (`finish`).
        case(
            "SACM-XMI-003/attribute-kept",
            tolerant(package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\" lang=\"en\"/>",
            ))),
            (WARNING, "LIB-COMPAT-003", Some("7.3.3")),
            &[
                ("attribute", t("lang")),
                ("holder_class", t("Note")),
                ("holder_id", t("n")),
            ],
            "the attribute {attribute:?} is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses",
            "the attribute \"lang\" is not SACM 2.3 where it stands, so it is kept as written as preserved content of the Note \"n\", which a tolerant write writes back and a strict write refuses",
        ),
        // SACM-XMI-013/group-end-read: the end as an element.
        case(
            "SACM-XMI-013/group-end-read",
            tolerant(package(&format!(
                "{}<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\"><name content=\"g\"/><argumentationElement xmi:idref=\"c1\"/></argumentElement>",
                claim("c1", "", "")
            ))),
            (WARNING, "SACM23-XMI-009", Some("11.2")),
            &[("element", t("argumentElement"))],
            concat!(
                "\"argumentationElement\" in the element {element:?} is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement",
                read_as!("argumentElement")
            ),
            "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement; it is read as argumentElement, which write writes",
        ),
        // SACM-XMI-013/event-date-kept: the date as a value element.
        case(
            "SACM-XMI-013/event-date-kept",
            strict(artifacts(
                "<artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\"><name content=\"e1\"/><occurece>2024-01-01</occurece></artifactElement>",
            )),
            (ERROR, "SACM23-XMI-009", Some("12.9")),
            &[("element", t("artifactElement"))],
            concat!(
                "\"occurece\" in the element {element:?} is the machine-readable model's spelling, and the attribute of an Event is date (§12.9)",
                kept_as!("date")
            ),
            "\"occurece\" in the element \"artifactElement\" is the machine-readable model's spelling, and the attribute of an Event is date (§12.9); it is kept as preserved content, and a tolerant read reads the name as date",
        ),
        // SACM-MULT-001/repeat-kept: a repeated expression (slice 0065).
        case(
            "SACM-MULT-001/repeat-kept",
            tolerant(terminology(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\"><name content=\"blade\"/></terminologyElement>\
                 <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t2\" value=\"t2\"><name xmi:type=\"sacm:ExpressionLangString\" expression=\"blade\"><expression xmi:idref=\"blade\"/></name></terminologyElement>",
            )),
            (WARNING, "LIB-COMPAT-001", Some("9.5.2")),
            &[
                ("property", t("name.expression")),
                ("element", t("t2")),
                ("holder_class", t("Term")),
                ("holder_id", t("t2")),
            ],
            concat!(
                "the {property} of the element {element:?} is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence",
                ke!(),
                "; the first occurrence is read"
            ),
            "the name.expression of the element \"t2\" is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence, so it is kept as preserved content of the Term \"t2\", which a tolerant write writes back and a strict write refuses; the first occurrence is read",
        ),
    ]
}

// --- Running a case -----------------------------------------------------------------

/// The document a tolerant read of `input` returns.
fn tolerant_document(template: &str, input: &str) -> Document {
    match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {template}: setup: a tolerant read refuses {input}: {error}"),
    }
}

/// The diagnostics of `case`'s step.
fn diagnostics(case: &Case) -> Vec<Diagnostic> {
    let template = case.template;
    let read = |input: &str, mode: Mode| match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(mode),
    ) {
        Ok(output) => output.diagnostics().to_vec(),
        Err(error) => panic!("{ROW}: {template}: a {mode:?} read refuses {input}: {error}"),
    };
    match &case.step {
        Step::Tolerant(input) => read(input, Mode::Tolerant),
        Step::Declared(input) => match xmi::read(input.as_bytes(), &declared()) {
            Ok(output) => output.diagnostics().to_vec(),
            Err(error) => panic!("{ROW}: {template}: a declared read refuses {input}: {error}"),
        },
        Step::Strict(input) => read(input, Mode::Strict),
        Step::Validate(input) => validation::validate(&tolerant_document(template, input)),
        Step::Write(input) => {
            let document = tolerant_document(template, input);
            match xmi::write(&document, &WriteOptions::default()) {
                Ok(text) => panic!("{ROW}: {template}: a strict write writes {text}"),
                Err(error) => error.diagnostics().to_vec(),
            }
        }
    }
}

/// The one diagnostic of `case`'s step with its template's code and its
/// message.
fn found(case: &Case) -> Diagnostic {
    let template = case.template;
    let all = diagnostics(case);
    let mut matching = all
        .iter()
        .filter(|diagnostic| {
            diagnostic.code().as_str() == code_of(template) && diagnostic.message() == case.message
        })
        .cloned();
    let (Some(first), None) = (matching.next(), matching.next()) else {
        let of_code: Vec<&str> = all
            .iter()
            .filter(|diagnostic| diagnostic.code().as_str() == code_of(template))
            .map(Diagnostic::message)
            .collect();
        panic!(
            "{ROW}: {template}: the step does not give exactly one {} diagnostic with the \
             message {:?}; it gives {of_code:#?}",
            code_of(template),
            case.message
        );
    };
    first
}

/// The code text of `template`: the part before the `/`.
fn code_of(template: &str) -> &str {
    template.split_once('/').map_or(template, |(code, _)| code)
}

/// The placeholders of `pattern` in order, with their format: `""` or `":?"`.
fn placeholders(pattern: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let inner = &after[..close];
        let (name, format) = inner.find(':').map_or((inner, ""), |at| inner.split_at(at));
        out.push((name, format));
        rest = &after[close + 1..];
    }
    out
}

/// `pattern` with each placeholder replaced by its argument: Text as it is
/// for `{n}` and Debug-quoted for `{n:?}`, a Count in decimal for `{n}`.
fn render(template: &str, pattern: &str, arguments: &[(&str, Value)]) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            panic!("{ROW}: {template}: an unclosed placeholder in {pattern:?}");
        };
        let inner = &after[..close];
        let (name, format) = inner.find(':').map_or((inner, ""), |at| inner.split_at(at));
        let Some(&(_, value)) = arguments.iter().find(|(given, _)| *given == name) else {
            panic!("{ROW}: {template}: the placeholder {name:?} names no argument");
        };
        match (format, value) {
            ("", Value::Text(text)) => out.push_str(text),
            (":?", Value::Text(text)) => {
                let _ = write!(out, "{text:?}");
            }
            ("", Value::Count(count)) => out.push_str(&count.to_string()),
            _ => panic!("{ROW}: {template}: {{{inner}}} does not take {value:?}"),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Checks the table row of `case` against itself: the placeholders and the
/// arguments agree in names, kinds and order, `row` is declared only where
/// the pattern names it, and then last, and the pattern rendered with the
/// arguments is the message.
fn check_row(case: &Case) {
    let template = case.template;
    let mut first_seen: Vec<&str> = Vec::new();
    for (name, format) in placeholders(case.pattern) {
        let Some(&(_, value)) = case.arguments.iter().find(|(given, _)| *given == name) else {
            panic!("{ROW}: {template}: the placeholder {name:?} names no argument");
        };
        let fits = matches!(
            (format, value),
            ("" | ":?", Value::Text(_)) | ("", Value::Count(_))
        );
        assert!(
            fits,
            "{ROW}: {template}: {{{name}{format}}} does not take {value:?}"
        );
        if !first_seen.contains(&name) {
            first_seen.push(name);
        }
    }
    let declared: Vec<&str> = case.arguments.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        declared, first_seen,
        "{ROW}: {template}: the arguments are not those the pattern names, in the order it \
         first names them"
    );
    if declared.contains(&"row") {
        assert_eq!(
            declared.last(),
            Some(&"row"),
            "{ROW}: {template}: row is not the last argument"
        );
    }
    assert_eq!(
        render(template, case.pattern, &case.arguments),
        case.message,
        "{ROW}: {template}: the pattern rendered with the arguments is not the message"
    );
}

/// Checks that `case`'s step gives its message with its code, severity, row
/// and clause.
fn check_message(case: &Case) {
    let template = case.template;
    let diagnostic = found(case);
    assert_eq!(
        (
            diagnostic.severity(),
            diagnostic.requirement_id(),
            diagnostic.clause()
        ),
        (case.severity, case.row, case.clause),
        "{ROW}: {template}: the severity, row and clause"
    );
}

#[test]
fn lib_api_018_each_diagnostic_template_input_gives_its_message() {
    let validation = validation_cases();
    assert_eq!(
        validation.len(),
        VALIDATION_TEMPLATES,
        "{ROW}: the validation inputs"
    );
    assert!(
        validation
            .iter()
            .all(|case| matches!(case.step, Step::Validate(_))),
        "{ROW}: every validation template's input is a validate"
    );
    let mut cases = validation;
    cases.extend(reader_cases());
    // The inputs are the 106 templates, once each.
    let mut ids: Vec<&str> = cases.iter().map(|case| case.template).collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        (count, ids.len()),
        (DIAGNOSTIC_TEMPLATES, DIAGNOSTIC_TEMPLATES),
        "{ROW}: the table does not hold the {DIAGNOSTIC_TEMPLATES} diagnostic templates once \
         each"
    );
    let sites = sites();
    for site in &sites {
        assert!(
            ids.contains(&site.template),
            "{ROW}: {}: a site of a template the table does not hold",
            site.template
        );
    }
    for case in cases.iter().chain(&sites) {
        check_row(case);
        check_message(case);
    }
}

/// Acceptance 2: with two descriptions, the Claim `CLM_132` gives one
/// SACM-MULT-001 Warning, with its template and arguments.
#[test]
fn lib_api_018_a_claim_with_two_descriptions_has_the_too_many_template_and_its_arguments() {
    let document = tolerant_document("SACM-MULT-001/too-many", CLM_132);
    let found = validation::validate(&document);
    assert_eq!(
        found.len(),
        1,
        "{ROW}: CLM_132 gives one finding: {found:?}"
    );
    let diagnostic = &found[0];
    assert_eq!(
        (
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.requirement_id(),
            diagnostic.clause(),
            diagnostic.message(),
            diagnostic.affected(),
        ),
        (
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-011",
            Some("8.6"),
            "Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])",
            &[ElementId::from("CLM_132")][..],
        ),
        "{ROW}: the finding of CLM_132"
    );
    {
        use sacm::diagnostic::ArgumentValue;

        assert_eq!(
            diagnostic.template(),
            "SACM-MULT-001/too-many",
            "{ROW}: the template of CLM_132's finding"
        );
        let arguments: Vec<(&str, ArgumentValue)> = diagnostic
            .arguments()
            .iter()
            .map(|argument| (argument.name(), argument.value().clone()))
            .collect();
        let text = |value: &str| ArgumentValue::Text(value.to_owned());
        assert_eq!(
            arguments,
            [
                ("class", text("Claim")),
                ("id", text("CLM_132")),
                ("count", ArgumentValue::Count(2)),
                ("property", text("description")),
                ("type", text("Description")),
                ("multiplicity", text("0..1")),
            ],
            "{ROW}: the arguments of CLM_132's finding"
        );
    }
}

/// Acceptance 6: the argument types and the details are plain values.
#[test]
fn lib_api_018_the_argument_types_and_details_have_their_traits() {
    fn value<T: Clone + std::fmt::Debug + Eq + std::hash::Hash + Send + Sync + 'static>() {}
    fn ordered<T: Ord>() {}
    value::<sacm::diagnostic::Argument>();
    value::<sacm::diagnostic::ArgumentValue>();
    value::<sacm::diagnostic::Details>();
    value::<Diagnostic>();
    ordered::<sacm::diagnostic::Argument>();
    ordered::<sacm::diagnostic::ArgumentValue>();
    ordered::<Diagnostic>();
}

mod api {
    use super::{
        CLM_132, Case, ROW, Value, code_of, found, reader_cases, sites, tolerant_document,
        validation_cases,
    };
    use sacm::diagnostic::{Argument, ArgumentValue, Code, Diagnostic, Severity, SourceLocation};
    use sacm::model::ElementId;
    use sacm::validation;

    /// The seven keys of ADR 0011: code, severity, row, clause, affected
    /// ids, location and message.
    type Keys = (
        Code,
        Severity,
        String,
        Option<String>,
        Vec<ElementId>,
        Option<SourceLocation>,
        String,
    );

    fn keys(diagnostic: &Diagnostic) -> Keys {
        (
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.requirement_id().to_owned(),
            diagnostic.clause().map(str::to_owned),
            diagnostic.affected().to_vec(),
            diagnostic.location(),
            diagnostic.message().to_owned(),
        )
    }

    /// The one finding of `validate` for [`CLM_132`].
    fn two_descriptions_finding() -> Diagnostic {
        let mut found = validation::validate(&tolerant_document("SACM-MULT-001/too-many", CLM_132));
        assert_eq!(found.len(), 1, "{ROW}: setup: CLM_132 gives {found:?}");
        found.remove(0)
    }

    /// The expected arguments of `case`, as the API states them.
    fn expected(case: &Case) -> Vec<(&'static str, ArgumentValue)> {
        case.arguments
            .iter()
            .map(|&(name, value)| {
                let value = match value {
                    Value::Text(text) => ArgumentValue::Text(text.to_owned()),
                    Value::Count(count) => ArgumentValue::Count(count),
                };
                (name, value)
            })
            .collect()
    }

    fn named(arguments: &[Argument]) -> Vec<(&'static str, ArgumentValue)> {
        arguments
            .iter()
            .map(|argument| (argument.name(), argument.value().clone()))
            .collect()
    }

    #[test]
    fn lib_api_018_each_diagnostic_template_input_has_its_code_template_and_arguments() {
        let mut cases = validation_cases();
        cases.extend(reader_cases());
        cases.extend(sites());
        for case in &cases {
            let template = case.template;
            let diagnostic = found(case);
            assert_eq!(
                diagnostic.code().as_str(),
                code_of(template),
                "{ROW}: {template}: code()"
            );
            assert_eq!(
                diagnostic.template(),
                template,
                "{ROW}: {template}: template()"
            );
            assert_eq!(
                named(diagnostic.arguments()),
                expected(case),
                "{ROW}: {template}: arguments() by name, kind and value"
            );
        }
    }

    /// Acceptance 4: a diagnostic built with `test-support`'s `new` has its
    /// code's text as its template and no arguments.
    #[test]
    fn lib_api_018_a_diagnostic_built_with_new_has_its_code_as_template_and_no_arguments() {
        for code in [
            Code::MULTIPLICITY_VIOLATION,
            Code::REF_DANGLING,
            Code::XMI_MISSING_ID,
        ] {
            let built = Diagnostic::new(code, Severity::Error, "SACM23-VAL-018", "a message")
                .with_clause("8.6");
            assert_eq!(
                built.template(),
                code.as_str(),
                "{ROW}: {code}: the template of a diagnostic built with new"
            );
            assert!(
                built.arguments().is_empty(),
                "{ROW}: {code}: a diagnostic built with new has arguments: {:?}",
                built.arguments()
            );
            assert_eq!(
                built.message(),
                "a message",
                "{ROW}: {code}: the message is kept as given"
            );
        }
        // Built with the seven keys of a finding the crate emits, it is not
        // `==` to it: the emitted one has its template and arguments.
        let emitted = two_descriptions_finding();
        let built = Diagnostic::new(
            emitted.code(),
            emitted.severity(),
            "SACM23-VAL-011",
            emitted.message().to_owned(),
        )
        .with_clause("8.6")
        .with_affected(emitted.affected().to_vec());
        assert_eq!(
            keys(&built),
            keys(&emitted),
            "{ROW}: setup: the built twin has the seven keys of the emitted finding"
        );
        assert_ne!(
            built, emitted,
            "{ROW}: a diagnostic built with new is == to an emitted one"
        );
        assert_eq!(
            (built.template(), emitted.template()),
            ("SACM-MULT-001", "SACM-MULT-001/too-many"),
            "{ROW}: the templates of the built twin and of the emitted finding"
        );
    }

    /// Acceptance 6: diagnostics equal in the seven keys of ADR 0011 order
    /// by template id, then by arguments, argument by argument; `cmp` is
    /// `Equal` exactly when they are `==`. Built from records, which keep
    /// every field as given.
    #[cfg(feature = "serde")]
    #[test]
    fn lib_api_018_diagnostics_equal_in_the_seven_keys_order_by_template_then_arguments() {
        use std::cmp::Ordering;

        let record = |details: &str| -> Diagnostic {
            let text = format!(
                r#"{{"code":"SACM-MULT-001","severity":"error","requirement_id":"R","clause":null,"affected":[],"location":null,"message":"m"{details}}}"#
            );
            serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("{ROW}: setup: {text} does not read: {error}"))
        };
        let missing = |class: &str, id: &str| {
            record(&format!(
                r#","template":"SACM-MULT-001/missing","arguments":{{"class":"{class}","id":"{id}","property":"name","type":"LangString","multiplicity":"1"}}"#
            ))
        };
        let too_many = |count: u64| {
            record(&format!(
                r#","template":"SACM-MULT-001/too-many","arguments":{{"class":"Claim","id":"a","count":{count},"property":"description","type":"Description","multiplicity":"0..1"}}"#
            ))
        };
        // In order: the code's text first, as a prefix of every id of its
        // code; then by template id; then argument by argument, by value.
        let ordered = [
            record(""),
            missing("Claim", "a"),
            missing("Claim", "b"),
            missing("Note", "a"),
            too_many(2),
            too_many(10),
        ];
        let built = Diagnostic::new(Code::MULTIPLICITY_VIOLATION, Severity::Error, "R", "m");
        let keys: Vec<_> = ordered.iter().map(keys).collect();
        assert!(
            keys.iter().all(|key| *key == keys[0]),
            "{ROW}: setup: the records are not equal in the seven keys"
        );
        assert_eq!(
            ordered.iter().map(Diagnostic::template).collect::<Vec<_>>(),
            [
                "SACM-MULT-001",
                "SACM-MULT-001/missing",
                "SACM-MULT-001/missing",
                "SACM-MULT-001/missing",
                "SACM-MULT-001/too-many",
                "SACM-MULT-001/too-many",
            ],
            "{ROW}: setup: the templates read"
        );
        for (i, a) in ordered.iter().enumerate() {
            for (j, b) in ordered.iter().enumerate() {
                assert_eq!(
                    a.cmp(b),
                    i.cmp(&j),
                    "{ROW}: diagnostic {i} against {j}: {a:?} and {b:?}"
                );
                assert_eq!(
                    a.cmp(b) == Ordering::Equal,
                    a == b,
                    "{ROW}: cmp is Equal exactly when == ({i}, {j})"
                );
                assert_eq!(a.partial_cmp(b), Some(a.cmp(b)), "{ROW}: partial_cmp");
            }
        }
        // A diagnostic built with `new` is the record with neither.
        assert_eq!(built, ordered[0], "{ROW}: new and the 0.1.0 record");
        assert_eq!(
            built.cmp(&ordered[0]),
            Ordering::Equal,
            "{ROW}: new's order"
        );
        assert_eq!(
            built.cmp(&ordered[1]),
            Ordering::Less,
            "{ROW}: the code's text sorts before the code's templates"
        );
        let mut shuffled = vec![
            ordered[4].clone(),
            ordered[1].clone(),
            ordered[5].clone(),
            ordered[0].clone(),
            ordered[3].clone(),
            ordered[2].clone(),
        ];
        shuffled.sort();
        assert_eq!(shuffled, ordered, "{ROW}: sorting by the order");
        // The seven keys still come first: another message sorts by it,
        // whatever the template.
        let earlier_message =
            Diagnostic::new(Code::MULTIPLICITY_VIOLATION, Severity::Error, "R", "a");
        assert_eq!(
            earlier_message.cmp(&too_many(10)),
            Ordering::Less,
            "{ROW}: the message is compared before the template"
        );
    }
}
