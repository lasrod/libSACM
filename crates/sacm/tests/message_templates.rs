//! Every read and write refusal has a code, a template and arguments
//! (LIB-API-017, acceptance 4; ADR 0030 amendment 3).
//!
//! One input per refusal template of `docs/sacm/diagnostics.md`'s
//! `## Message templates` table: a strict read, or a strict write of a
//! document built through the public API. For each, the table states the
//! template id, its arguments by name, kind and value, the English pattern
//! the template names (as the catalog lists it), and the whole `Display`.
//!
//! - `lib_api_017_each_refusal_template_input_gives_its_0_1_0_display` runs
//!   on today's API. The `Display` of each input is pinned byte for byte as
//!   0.1.0 wrote it, and the table is checked against itself: the pattern,
//!   rendered with the arguments, is the variable part of the `Display`;
//!   every placeholder names an argument of the right kind; the arguments
//!   come in the order the pattern first names them, `row` last and only on
//!   the `Unsupported` templates of SACM-XMI-014, -007, -002 and -015, whose
//!   `Display` ends with the label of their code; and the inputs and the
//!   templates no input reaches together are the 102 refusal templates,
//!   once each.
//! - `lib_api_017_each_refusal_template_has_its_code_template_and_arguments`
//!   checks that `code()`, `template()` and `arguments()` are the
//!   table's.
//!
//! Twelve templates have no input through the public API, and are built by
//! a unit test of their crate-private constructors instead (see
//! [`NO_INPUT`]): the three SACM-SEC-002 capacity limits and the three
//! `SACM-SEC-002/unheld-*` refusals, which acceptance 4 names; five
//! SACM-XML-001 forms that need a DTD's entity declarations, which every
//! read refuses first as a DOCTYPE (SACM-SEC-001); and
//! `SACM-XMI-012/name-value-text`, whose shape always follows the
//! `name-value` shape of the same `value`, so a strict read, which refuses
//! the first shape, never refuses it.
//!
//! Slice 0065 reads `ExpressionLangString` (SACM23-BASE-009) and removes the
//! template `SACM-XMI-014/expression-lang-string`, which leaves 102.

use std::fmt::Write as _;

use sacm::edit::{self, CreateArgumentPackage};
use sacm::model::{Document, LangString};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-API-017";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The number of refusal templates (acceptance 4): 102 once slice 0065
/// removes `SACM-XMI-014/expression-lang-string` (SACM23-BASE-009).
const REFUSAL_TEMPLATES: usize = 102;

/// The refusal templates no input reaches through the public API, and why.
/// A unit test of their crate-private constructors builds each.
const NO_INPUT: [(&str, &str); 12] = [
    (
        "SACM-SEC-002/nodes",
        "the parser's node limit is beyond a test's size",
    ),
    (
        "SACM-SEC-002/attributes",
        "the parser's attribute limit is beyond a test's size",
    ),
    (
        "SACM-SEC-002/namespaces",
        "the parser's namespace limit is beyond a test's size",
    ),
    (
        "SACM-SEC-002/unheld-outside",
        "checked input never fails to be held",
    ),
    (
        "SACM-SEC-002/unheld-root",
        "checked input never fails to be held",
    ),
    (
        "SACM-SEC-002/unheld-element",
        "checked input never fails to be held",
    ),
    (
        "SACM-XML-001/entity-closes-element",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/entity-loop",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/lt-in-attribute-value",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/invalid-external-id",
        "it is inside a DOCTYPE, which is refused first",
    ),
    (
        "SACM-XML-001/unresolved-entity",
        "it needs an external entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XMI-012/name-value-text",
        "the name-value shape of the same value comes first, and is the one refused",
    ),
];

// --- The table -----------------------------------------------------------------------

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

/// What is refused: the bytes of a strict read, or a document a strict write
/// refuses.
enum Input {
    Read(Vec<u8>),
    Write(Document),
}

/// One refusal template and its input.
struct Case {
    /// The template id, `<CODE>/<form>`.
    template: &'static str,
    input: Input,
    /// The arguments, in the template's order.
    arguments: Vec<(&'static str, Value)>,
    /// The template's English pattern: the variable part of the `Display`.
    english: &'static str,
    /// The whole `Display`, byte for byte as 0.1.0 wrote it.
    display: &'static str,
}

fn read(
    template: &'static str,
    input: impl Into<Vec<u8>>,
    arguments: &[(&'static str, Value)],
    english: &'static str,
    display: &'static str,
) -> Case {
    Case {
        template,
        input: Input::Read(input.into()),
        arguments: arguments.to_vec(),
        english,
        display,
    }
}

fn write(
    template: &'static str,
    document: Document,
    arguments: &[(&'static str, Value)],
    english: &'static str,
    display: &'static str,
) -> Case {
    Case {
        template,
        input: Input::Write(document),
        arguments: arguments.to_vec(),
        english,
        display,
    }
}

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// A root ArgumentPackage `p` whose name element is `name`.
fn named_package(name: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">{name}</sacm:ArgumentPackage>")
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
    )
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

/// The `xmi:XMI` wrapper with `attributes` and `body` before its one root.
fn wrapper(attributes: &str, body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>{body}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
    )
}

/// `text` encoded as UTF-16 LE, with a byte order mark or without.
fn utf16le(text: &str, mark: bool) -> Vec<u8> {
    let mut bytes = if mark { vec![0xFF, 0xFE] } else { Vec::new() };
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

/// The document a tolerant read of `input` returns.
fn tolerant_document(input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: a tolerant read refuses {input}: {error}"),
    }
}

/// A document holding the root ArgumentPackage `arguments`, whose name holds
/// U+0001.
fn unrepresentable_name() -> Document {
    let mut document = Document::new();
    let name = LangString::new("bad\u{1}");
    if let Err(error) = edit::apply(
        &mut document,
        CreateArgumentPackage::root(name).with_id("arguments"),
    ) {
        panic!("{ROW}: setup: the package is not created: {error}");
    }
    document
}

/// The 91 templates an input reaches, in the catalog's grouping.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn cases() -> Vec<Case> {
    let dangling = |id: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\" isCitation=\"true\" \
             citedElement=\"nope\"><name content=\"{id}\"/></argumentElement>"
        )
    };
    vec![
        // --- ReadError::Doctype --------------------------------------------------------
        read(
            "SACM-SEC-001/doctype",
            "<!DOCTYPE r>\n<r/>",
            &[],
            "the input has a DOCTYPE, which is refused",
            "the input has a DOCTYPE, which is refused at line 1, column 1",
        ),
        // --- ReadError::Unsupported, SACM-XML-002 --------------------------------------
        read(
            "SACM-XML-002/byte-order-mark",
            {
                let mut bytes = vec![0x00, 0x00, 0xFE, 0xFF];
                bytes.extend("<r/>".chars().flat_map(|c| u32::from(c).to_be_bytes()));
                bytes
            },
            &[("encoding", t("UTF-32 (big-endian)"))],
            "the input starts with a {encoding} byte order mark, and only UTF-8, ASCII and UTF-16 are read",
            "the reader cannot represent the input at line 1, column 1: the input starts with a UTF-32 (big-endian) byte order mark, and only UTF-8, ASCII and UTF-16 are read (a limit of this version)",
        ),
        read(
            "SACM-XML-002/detected-encoding",
            utf16le("<?xml version=\"1.0\" encoding=\"UTF-16LE\"?><r/>", false),
            &[("encoding", t("UTF-16 (little-endian)"))],
            "the input is encoded in {encoding}, and only UTF-8, ASCII and UTF-16 starting with U+FEFF are read",
            "the reader cannot represent the input at line 1, column 1: the input is encoded in UTF-16 (little-endian), and only UTF-8, ASCII and UTF-16 starting with U+FEFF are read (a limit of this version)",
        ),
        read(
            "SACM-XML-002/declared-encoding",
            "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r/>",
            &[("encoding", t("ISO-8859-1"))],
            "the declared encoding {encoding:?} is not supported: without a UTF-16 byte order mark, only UTF-8 and ASCII are read",
            "the reader cannot represent the input at line 1, column 31: the declared encoding \"ISO-8859-1\" is not supported: without a UTF-16 byte order mark, only UTF-8 and ASCII are read (a limit of this version)",
        ),
        // --- WriteError::UnrepresentableCharacter --------------------------------------
        write(
            "SACM-XML-003/unrepresentable-character",
            unrepresentable_name(),
            &[
                ("element", t("arguments")),
                ("attribute", t("name's content")),
                ("code_point", n(1)),
            ],
            "the element {element:?} cannot be written: its {attribute} holds U+{code_point:04X}, which XML 1.0 cannot represent",
            "the element \"arguments\" cannot be written: its name's content holds U+0001, which XML 1.0 cannot represent",
        ),
        // --- SACM-XMI-001 --------------------------------------------------------------
        read(
            "SACM-XMI-001/not-a-package",
            "<r/>",
            &[("element", t("r"))],
            "the root element {element:?} is not a SACM package",
            "the root element \"r\" is not a SACM package at line 1, column 1",
        ),
        read(
            "SACM-XMI-001/no-package",
            format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"),
            &[],
            "the XMI element holds no SACM package",
            "the XMI element holds no SACM package at line 1, column 1",
        ),
        write(
            "SACM-XMI-001/no-root-to-write",
            Document::new(),
            &[],
            "the document has no root to write",
            "the document has no root to write",
        ),
        // --- ReadError::Unsupported, SACM-XMI-002 --------------------------------------
        read(
            "SACM-XMI-002/other-namespace",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
            ),
            &[
                ("element", t("sacm:ArgumentPackage")),
                (
                    "namespace",
                    t("http://www.omg.org/spec/SACM/2.2/argumentation"),
                ),
                ("row", t("LIB-COMPAT-004")),
            ],
            "the root element {element:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"",
            "the reader cannot represent the input at line 1, column 1: the root element \"sacm:ArgumentPackage\" is in the namespace \"http://www.omg.org/spec/SACM/2.2/argumentation\", not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)",
        ),
        read(
            "SACM-XMI-002/no-namespace",
            format!("<ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
            &[
                ("element", t("ArgumentPackage")),
                ("row", t("LIB-COMPAT-004")),
            ],
            "the root element {element:?} is in no namespace, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"",
            "the reader cannot represent the input at line 1, column 1: the root element \"ArgumentPackage\" is in no namespace, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)",
        ),
        // --- ReadError::Unsupported, SACM-XMI-003 --------------------------------------
        read(
            "SACM-XMI-003/unread-attribute",
            wrapper(" foo=\"bar\"", ""),
            &[("attribute", t("foo")), ("element", t("xmi:XMI"))],
            "the attribute {attribute:?} of the element {element:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 110: the attribute \"foo\" of the element \"xmi:XMI\" is not read in this release (a defect in the input)",
        ),
        read(
            "SACM-XMI-003/wrapper-text",
            wrapper("", "stray"),
            &[("element", t("xmi:XMI"))],
            "the element {element:?} holds text, which has no element to be kept in",
            "the reader cannot represent the input at line 1, column 110: the element \"xmi:XMI\" holds text, which has no element to be kept in (a defect in the input)",
        ),
        read(
            "SACM-XMI-003/unwritable-prefix",
            package(&format!(
                "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
                 xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
            )),
            &[
                ("attribute", t("sacm:extra")),
                ("element", t("argumentElement")),
                ("prefix", t("sacm")),
                ("namespace", t("urn:example:other")),
                ("pinned_namespace", t(SACM_NS)),
            ],
            "the attribute {attribute:?} of the element {element:?} is not SACM 2.3, and cannot be kept: its prefix {prefix:?} is bound to {namespace:?}, and the written element binds it to {pinned_namespace:?}",
            "the reader cannot represent the input at line 1, column 280: the attribute \"sacm:extra\" of the element \"argumentElement\" is not SACM 2.3, and cannot be kept: its prefix \"sacm\" is bound to \"urn:example:other\", and the written element binds it to \"http://www.omg.org/spec/SACM/20220301\" (a defect in the input)",
        ),
        // --- ReadError::MissingType ----------------------------------------------------
        read(
            "SACM-XMI-005/missing-type",
            package("<argumentElement xmi:id=\"c1\"/>"),
            &[
                ("role", t("argumentElement")),
                ("type", t("ArgumentationElement")),
            ],
            "the element {role:?} has no xmi:type, and its role has the abstract type {type}",
            "the element \"argumentElement\" has no xmi:type, and its role has the abstract type ArgumentationElement at line 1, column 153: give it an xmi:type that names its class",
        ),
        // --- WriteError::PreservedContent ----------------------------------------------
        write(
            "SACM-XMI-006/one-holder",
            tolerant_document(&format!(
                "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
                 <name content=\"p\"/></sacm:ArgumentPackage>"
            )),
            &[("id", t("p"))],
            "the document cannot be written in strict mode: the element {id:?} holds preserved content",
            "the document cannot be written in strict mode: the element \"p\" holds preserved content",
        ),
        write(
            "SACM-XMI-006/holders",
            tolerant_document(&format!(
                "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
                 <name content=\"p\"/>{}</sacm:ArgumentPackage>",
                claim("c1", " v:size=\"2\"", "")
            )),
            &[("count", n(2)), ("id", t("p"))],
            "the document cannot be written in strict mode: {count} elements hold preserved content, the first is {id:?}",
            "the document cannot be written in strict mode: 2 elements hold preserved content, the first is \"p\"",
        ),
        write(
            "SACM-XMI-006/outside-elements",
            tolerant_document(&wrapper("", "stray")),
            &[],
            "the document cannot be written in strict mode: the document holds preserved content outside its elements",
            "the document cannot be written in strict mode: the document holds preserved content outside its elements",
        ),
        // --- ReadError::Unsupported, SACM-XMI-007 --------------------------------------
        read(
            "SACM-XMI-007/unread-attribute",
            package(&format!(
                "{}{}",
                claim(
                    "c1",
                    "",
                    "<metaClaim href=\"#c2\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xlink:type=\"simple\"/>"
                ),
                claim("c2", "", "")
            )),
            &[
                ("attribute", t("xlink:type")),
                ("element", t("metaClaim")),
                ("row", t("SACM23-XMI-027")),
            ],
            "the attribute {attribute:?} of the element {element:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 289: the attribute \"xlink:type\" of the element \"metaClaim\" is not read in this release (a limit of this version; SACM23-XMI-027 reads it)",
        ),
        read(
            "SACM-XMI-007/proxy-child",
            nested("<argumentElement href=\"other.sacm.xmi#c\"/>"),
            &[
                ("element", t("argumentElement")),
                ("container", t("argumentPackage")),
                ("class", t("ArgumentationElement")),
                ("row", t("SACM23-XMI-027")),
            ],
            "the element {element:?} in the element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release",
            "the reader cannot represent the input at line 1, column 245: the element \"argumentElement\" in the element \"argumentPackage\" is in reference form, a proxy of an element of the class ArgumentationElement, which is not read in this release (a limit of this version; SACM23-XMI-027 reads it)",
        ),
        read(
            "SACM-XMI-007/proxy-child-of-root",
            package("<argumentElement href=\"other.sacm.xmi#c\"/>"),
            &[
                ("element", t("argumentElement")),
                ("container", t("sacm:ArgumentPackage")),
                ("class", t("ArgumentationElement")),
                ("row", t("SACM23-XMI-027")),
            ],
            "the element {element:?} in the root element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release",
            "the reader cannot represent the input at line 1, column 153: the element \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is in reference form, a proxy of an element of the class ArgumentationElement, which is not read in this release (a limit of this version; SACM23-XMI-027 reads it)",
        ),
        read(
            "SACM-XMI-007/href",
            package(&claim("c1", "", "<metaClaim href=\"other.sacm.xmi#c\"/>")),
            &[
                ("href", t("other.sacm.xmi#c")),
                ("element", t("metaClaim")),
                ("row", t("SACM23-XMI-027")),
            ],
            "the href {href:?} of the reference element {element:?} names no element of this document by its id; only href=\"#id\" is read in this release",
            "the reader cannot represent the input at line 1, column 235: the href \"other.sacm.xmi#c\" of the reference element \"metaClaim\" names no element of this document by its id; only href=\"#id\" is read in this release (a limit of this version; SACM23-XMI-027 reads it)",
        ),
        // --- ReadError::LegacyShape, SACM-XMI-012 --------------------------------------
        read(
            "SACM-XMI-012/name-attribute",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"arguments\" name=\"Arguments\"/>"),
            &[("class", t("ArgumentPackage")), ("id", t("arguments"))],
            "the name of the {class} {id:?} is given as the attribute \"name\"",
            "the name of the ArgumentPackage \"arguments\" is given as the attribute \"name\", a shape of the assurance-forge C++ library at line 1, column 142: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/content-on-element",
            package(&claim("c1", "", "<note xmi:id=\"n\" content=\"x\"/>")),
            &[("class", t("Note")), ("id", t("n"))],
            "the content of the {class} {id:?} is given on the element itself",
            "the content of the Note \"n\" is given on the element itself, a shape of the assurance-forge C++ library at line 1, column 241: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/key-attribute",
            package(&claim("c1", "", "<taggedValue xmi:id=\"t\" key=\"k\"/>")),
            &[("class", t("TaggedValue")), ("id", t("t"))],
            "the key of the {class} {id:?} is given as the attribute \"key\"",
            "the key of the TaggedValue \"t\" is given as the attribute \"key\", a shape of the assurance-forge C++ library at line 1, column 248: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/shorthand",
            package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\"><content content=\"x\" lang=\"en\"/></note>",
            )),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            "the {role} of the {class} {id:?} is given in the single-language shorthand",
            "the content of the Note \"n\" is given in the single-language shorthand, a shape of the assurance-forge C++ library at line 1, column 250: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-text",
            named_package("<name>p</name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} has its content as text",
            "the name of the ArgumentPackage \"p\" has its content as text, a shape of the assurance-forge C++ library at line 1, column 140: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/value-text",
            package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\"><content><value>x</value></content></note>",
            )),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            "a value of the {role} of the {class} {id:?} has its content as text",
            "a value of the content of the Note \"n\" has its content as text, a shape of the assurance-forge C++ library at line 1, column 257: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-content-lang",
            named_package("<name><content lang=\"en\">p</content></name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} holds a content carrying lang",
            "the name of the ArgumentPackage \"p\" holds a content carrying lang, a shape of the assurance-forge C++ library at line 1, column 149: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-value",
            named_package("<name><value content=\"p\"/></name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} holds a value",
            "the name of the ArgumentPackage \"p\" holds a value, a shape of the assurance-forge C++ library at line 1, column 140: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        // --- ReadError::Unsupported, SACM-XMI-014 --------------------------------------
        read(
            "SACM-XMI-014/xmi-root",
            format!("<xmi:Documentation xmlns:xmi=\"{XMI_NS}\"/>"),
            &[
                ("element", t("xmi:Documentation")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the XMI element {element:?} is not read as a root in this release",
            "the reader cannot represent the input at line 1, column 1: the XMI element \"xmi:Documentation\" is not read as a root in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        read(
            "SACM-XMI-014/unread-attribute",
            package(&claim("c1", " xmi:uuid=\"u1\"", "")),
            &[
                ("attribute", t("xmi:uuid")),
                ("element", t("argumentElement")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the attribute {attribute:?} of the element {element:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 204: the attribute \"xmi:uuid\" of the element \"argumentElement\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        read(
            "SACM-XMI-014/nested-xmi-element",
            wrapper("", "<xmi:Documentation/>"),
            &[
                ("element", t("xmi:Documentation")),
                ("container", t("xmi:XMI")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the XMI element {element:?} in {container:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 110: the XMI element \"xmi:Documentation\" in \"xmi:XMI\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        // A TerminologyPackage's interface end: slice 0066 reads the
        // ArgumentPackage's this case sampled (SACM23-ARG-014).
        read(
            "SACM-XMI-014/interface-element",
            format!(
                "<sacm:TerminologyPackage{NS} xmi:id=\"t\"><name content=\"t\"/>\
                 <interface xmi:idref=\"p\"/></sacm:TerminologyPackage>"
            ),
            &[
                ("element", t("interface")),
                ("container", t("sacm:TerminologyPackage")),
                ("class", t("TerminologyPackage")),
                ("row", t("SACM23-TERM-007")),
            ],
            "the reference element {element:?} in the element {container:?} is the {class} end \"interface\", which is not read in this release",
            "the reader cannot represent the input at line 1, column 156: the reference element \"interface\" in the element \"sacm:TerminologyPackage\" is the TerminologyPackage end \"interface\", which is not read in this release (a limit of this version; SACM23-TERM-007 reads it)",
        ),
        // A TerminologyPackageInterface root: slice 0066 reads the
        // ArgumentPackageInterface root this case sampled (SACM23-ARG-014).
        read(
            "SACM-XMI-014/unread-root",
            format!("<sacm:TerminologyPackageInterface{NS} xmi:id=\"i\"/>"),
            &[
                ("element", t("sacm:TerminologyPackageInterface")),
                ("row", t("SACM23-TERM-007")),
            ],
            "the root element {element:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 1: the root element \"sacm:TerminologyPackageInterface\" is not read in this release (a limit of this version; SACM23-TERM-007 reads it)",
        ),
        read(
            "SACM-XMI-014/xmi-child",
            package(&claim("c1", "", "<xmi:Extension extender=\"v\"/>")),
            &[
                ("element", t("xmi:Extension")),
                ("container", t("argumentElement")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the XMI element {element:?} in the element {container:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 224: the XMI element \"xmi:Extension\" in the element \"argumentElement\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        read(
            "SACM-XMI-014/xmi-child-of-root",
            package("<xmi:Extension extender=\"v\"/>"),
            &[
                ("element", t("xmi:Extension")),
                ("container", t("sacm:ArgumentPackage")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the XMI element {element:?} in the root element {container:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 153: the XMI element \"xmi:Extension\" in the root element \"sacm:ArgumentPackage\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        read(
            "SACM-XMI-014/proxy-child",
            nested("<argumentElement xmi:idref=\"ap\"/>"),
            &[
                ("element", t("argumentElement")),
                ("container", t("argumentPackage")),
                ("class", t("ArgumentationElement")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the element {element:?} in the element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release",
            "the reader cannot represent the input at line 1, column 245: the element \"argumentElement\" in the element \"argumentPackage\" is in reference form, a proxy of an element of the class ArgumentationElement, which is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        read(
            "SACM-XMI-014/proxy-child-of-root",
            package("<argumentElement xmi:idref=\"p\"/>"),
            &[
                ("element", t("argumentElement")),
                ("container", t("sacm:ArgumentPackage")),
                ("class", t("ArgumentationElement")),
                ("row", t("SACM23-XMI-019")),
            ],
            "the element {element:?} in the root element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release",
            "the reader cannot represent the input at line 1, column 153: the element \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is in reference form, a proxy of an element of the class ArgumentationElement, which is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
        // A TerminologyPackageInterface in a nested AssuranceCasePackage:
        // slice 0066 reads the ArgumentPackageInterface in an
        // ArgumentPackage this case sampled (SACM23-ARG-014).
        read(
            "SACM-XMI-014/unread-kind",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
                 <assuranceCasePackage xmi:id=\"sub\"><name content=\"sub\"/>\
                 <terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"i\"/>\
                 </assuranceCasePackage></sacm:AssuranceCasePackage>"
            ),
            &[
                ("class", t("TerminologyPackageInterface")),
                ("element", t("terminologyPackage")),
                ("container", t("assuranceCasePackage")),
                ("row", t("SACM23-TERM-007")),
            ],
            "the {class} {element:?} in the element {container:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 220: the TerminologyPackageInterface \"terminologyPackage\" in the element \"assuranceCasePackage\" is not read in this release (a limit of this version; SACM23-TERM-007 reads it)",
        ),
        read(
            "SACM-XMI-014/unread-kind-in-root",
            terminology_interface_in_a_case(),
            &[
                ("class", t("TerminologyPackageInterface")),
                ("element", t("terminologyPackage")),
                ("container", t("sacm:AssuranceCasePackage")),
                ("row", t("SACM23-TERM-007")),
            ],
            "the {class} {element:?} in the root element {container:?} is not read in this release",
            "the reader cannot represent the input at line 3, column 3: the TerminologyPackageInterface \"terminologyPackage\" in the root element \"sacm:AssuranceCasePackage\" is not read in this release (a limit of this version; SACM23-TERM-007 reads it)",
        ),
        // --- ReadError::Unsupported, SACM-XMI-015 --------------------------------------
        read(
            "SACM-XMI-015/not-xmi-namespace",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage x:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
            ),
            &[("element", t("xmi:XMI")), ("row", t("LIB-COMPAT-002"))],
            "the element {element:?} is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element",
            "the reader cannot represent the input at line 1, column 1: the element \"xmi:XMI\" is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element (strict mode never reads it; LIB-COMPAT-002 is to read it in tolerant mode)",
        ),
        // --- WriteError::Invalid, SACM-XMI-016 -----------------------------------------
        write(
            "SACM-XMI-016/schema-error",
            tolerant_document(&package(&dangling("c1"))),
            &[("first_code", t("SACM-REF-001")), ("first_id", t("c1"))],
            "the document cannot be written in strict mode: 1 schema error, {first_code} on {first_id:?}",
            "the document cannot be written in strict mode: 1 schema error, SACM-REF-001 on \"c1\"",
        ),
        write(
            "SACM-XMI-016/schema-errors",
            tolerant_document(&package(&format!("{}{}", dangling("c1"), dangling("c2")))),
            &[
                ("count", n(2)),
                ("first_code", t("SACM-REF-001")),
                ("first_id", t("c1")),
            ],
            "the document cannot be written in strict mode: {count} schema errors, the first is {first_code} on {first_id:?}",
            "the document cannot be written in strict mode: 2 schema errors, the first is SACM-REF-001 on \"c1\"",
        ),
        // --- ReadError::DuplicateId, InvalidId -----------------------------------------
        read(
            "SACM-ID-001/duplicate-id",
            package(&format!("{}{}", claim("c1", "", ""), claim("c1", "", ""))),
            &[("id", t("c1")), ("count", n(2))],
            "the xmi:id {id:?} is carried by {count} elements",
            "the xmi:id \"c1\" is carried by 2 elements, at line 1, column 153; line 1, column 242",
        ),
        read(
            "SACM-ID-002/read-id-not-an-ncname",
            package("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>"),
            &[("id", t(""))],
            "the xmi:id {id:?} is not an NCName",
            "the xmi:id \"\" is not an NCName at line 1, column 153",
        ),
        // --- ReadError::RepeatedValue, InvalidLiteral ----------------------------------
        read(
            "SACM-MULT-001/both-forms",
            package(
                "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"c2 c3\">\
                 <source xmi:idref=\"c4\"/></argumentElement>",
            ),
            &[("property", t("source")), ("element", t("i1"))],
            "the property {property:?} of the element {element:?} is given both as an attribute and as elements, which XMI 2.5.1 rule 2h forbids",
            "the property \"source\" of the element \"i1\" is given both as an attribute and as elements, which XMI 2.5.1 rule 2h forbids, at line 1, column 216; line 1, column 231",
        ),
        read(
            "SACM-MULT-001/repeated-value",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\"><name content=\"A\"><content>A</content></name></sacm:ArgumentPackage>"
            ),
            &[
                ("property", t("name.content")),
                ("element", t("arguments")),
                ("count", n(2)),
            ],
            "the single-valued property {property:?} of the element {element:?} is given {count} times",
            "the single-valued property \"name.content\" of the element \"arguments\" is given 2 times, at line 1, column 148; line 1, column 160",
        ),
        read(
            "SACM-ENUM-001/not-a-literal",
            package(&claim("c1", " assertionDeclaration=\"Assumed\"", "")),
            &[
                ("value", t("Assumed")),
                ("property", t("assertionDeclaration")),
                ("element", t("c1")),
            ],
            "the value {value:?} of the property {property:?} of the element {element:?} is not a literal",
            "the value \"Assumed\" of the property \"assertionDeclaration\" of the element \"c1\" is not a literal at line 1, column 204: the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited",
        ),
        // --- ReadError::Malformed, SACM-XML-001 ----------------------------------------
        read(
            "SACM-XML-001/invalid-utf8",
            &b"<r>\xFF</r>"[..],
            &[],
            "a byte sequence is not valid UTF-8",
            "the input is not well-formed XML at line 1, column 4: a byte sequence is not valid UTF-8",
        ),
        read(
            "SACM-XML-001/unpaired-surrogate",
            {
                let mut bytes = utf16le("<r>", true);
                bytes.extend([0x00, 0xD8]);
                bytes.extend(utf16le("</r>", false));
                bytes
            },
            &[("unit", n(0xD800))],
            "the UTF-16 code unit {unit:04X} is an unpaired surrogate",
            "the input is not well-formed XML at line 1, column 4: the UTF-16 code unit D800 is an unpaired surrogate",
        ),
        read(
            "SACM-XML-001/odd-utf16-byte",
            {
                let mut bytes = utf16le("<r/>", true);
                bytes.push(0x20);
                bytes
            },
            &[],
            "the UTF-16 input ends with a byte that is not a whole code unit",
            "the input is not well-formed XML at line 1, column 5: the UTF-16 input ends with a byte that is not a whole code unit",
        ),
        read(
            "SACM-XML-001/encoding-contradicts-bom",
            utf16le("<?xml version=\"1.0\" encoding=\"UTF-8\"?><r/>", true),
            &[("encoding", t("UTF-8"))],
            "the declared encoding {encoding:?} contradicts the UTF-16 byte order mark",
            "the input is not well-formed XML at line 1, column 31: the declared encoding \"UTF-8\" contradicts the UTF-16 byte order mark",
        ),
        read(
            "SACM-XML-001/non-ascii-character",
            "<?xml version=\"1.0\" encoding=\"US-ASCII\"?><r>\u{E9}</r>",
            &[("encoding", t("US-ASCII")), ("code_point", n(0xE9))],
            "the declared encoding is {encoding:?}, but the input holds the non-ASCII character U+{code_point:04X}",
            "the input is not well-formed XML at line 1, column 45: the declared encoding is \"US-ASCII\", but the input holds the non-ASCII character U+00E9",
        ),
        read(
            "SACM-XML-001/xml-version",
            "<?xml version=\"2.0\"?><r/>",
            &[("version", t("2.0"))],
            "the XML version {version:?} is not \"1.\" followed by digits",
            "the input is not well-formed XML at line 1, column 16: the XML version \"2.0\" is not \"1.\" followed by digits",
        ),
        read(
            "SACM-XML-001/encoding-name",
            "<?xml version=\"1.0\" encoding=\"8bit\"?><r/>",
            &[("encoding", t("8bit"))],
            "the encoding name {encoding:?} is not a valid encoding name",
            "the input is not well-formed XML at line 1, column 31: the encoding name \"8bit\" is not a valid encoding name",
        ),
        read(
            "SACM-XML-001/standalone",
            "<?xml version=\"1.0\" standalone=\"maybe\"?><r/>",
            &[("value", t("maybe"))],
            "the standalone value {value:?} is not \"yes\" or \"no\"",
            "the input is not well-formed XML at line 1, column 33: the standalone value \"maybe\" is not \"yes\" or \"no\"",
        ),
        read(
            "SACM-XML-001/unknown-pseudo-attribute",
            "<?xml version=\"1.0\" foo=\"bar\"?><r/>",
            &[("name", t("foo"))],
            "the XML declaration has the unknown pseudo-attribute {name:?}",
            "the input is not well-formed XML at line 1, column 21: the XML declaration has the unknown pseudo-attribute \"foo\"",
        ),
        read(
            "SACM-XML-001/pseudo-attribute-order",
            "<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?><r/>",
            &[("name", t("encoding"))],
            "the pseudo-attribute {name:?} is out of order in the XML declaration",
            "the input is not well-formed XML at line 1, column 38: the pseudo-attribute \"encoding\" is out of order in the XML declaration",
        ),
        read(
            "SACM-XML-001/pseudo-attribute-space",
            "<?xml version=\"1.0\"encoding=\"UTF-8\"?><r/>",
            &[("name", t("encoding"))],
            "the pseudo-attribute {name:?} does not follow white space in the XML declaration",
            "the input is not well-formed XML at line 1, column 20: the pseudo-attribute \"encoding\" does not follow white space in the XML declaration",
        ),
        read(
            "SACM-XML-001/declaration-character",
            "<?xml ?><r/>",
            &[("expected", t("\"version\""))],
            "an unexpected character in the XML declaration where {expected} was expected",
            "the input is not well-formed XML at line 1, column 7: an unexpected character in the XML declaration where \"version\" was expected",
        ),
        read(
            "SACM-XML-001/declaration-end",
            "<?xml version=\"1.0\"",
            &[],
            "the input ends inside the XML declaration",
            "the input is not well-formed XML at line 1, column 20: the input ends inside the XML declaration",
        ),
        read(
            "SACM-XML-001/reserved-pi-target",
            "<r><?Xml?></r>",
            &[("target", t("Xml"))],
            "a processing instruction has the reserved target {target:?}",
            "the input is not well-formed XML at line 1, column 4: a processing instruction has the reserved target \"Xml\"",
        ),
        read(
            "SACM-XML-001/pi-target-colon",
            "<r><?a:b?></r>",
            &[("target", t("a:b"))],
            "the processing instruction target {target:?} contains a colon",
            "the input is not well-formed XML at line 1, column 4: the processing instruction target \"a:b\" contains a colon",
        ),
        read(
            "SACM-XML-001/pi-target-end",
            "<r><?pi\u{A0}x?></r>",
            &[("target", t("pi"))],
            "the processing instruction target {target:?} is not followed by white space or \"?>\"",
            "the input is not well-formed XML at line 1, column 8: the processing instruction target \"pi\" is not followed by white space or \"?>\"",
        ),
        read(
            "SACM-XML-001/empty-prefix",
            "<r :a=\"1\"/>",
            &[("name", t(":a"))],
            "the name {name:?} has an empty namespace prefix",
            "the input is not well-formed XML at line 1, column 4: the name \":a\" has an empty namespace prefix",
        ),
        read(
            "SACM-XML-001/xmlns-prefix-declared",
            "<r xmlns:xmlns=\"x\"/>",
            &[],
            "the namespace prefix \"xmlns\" is declared, and it cannot be declared",
            "the input is not well-formed XML at line 1, column 4: the namespace prefix \"xmlns\" is declared, and it cannot be declared",
        ),
        read(
            "SACM-XML-001/prefix-undeclared",
            "<r xmlns:p=\"\"/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is declared with an empty value, and a prefix cannot be undeclared",
            "the input is not well-formed XML at line 1, column 4: the namespace prefix \"p\" is declared with an empty value, and a prefix cannot be undeclared",
        ),
        read(
            "SACM-XML-001/reference-to-surrogate",
            "<r a=\"&#xD800;\"/>",
            &[("reference", t("&#xD800;"))],
            "the character reference {reference:?} refers to a surrogate, which is not a character",
            "the input is not well-formed XML at line 1, column 7: the character reference \"&#xD800;\" refers to a surrogate, which is not a character",
        ),
        read(
            "SACM-XML-001/reference-above-max",
            "<r a=\"&#x110000;\"/>",
            &[("reference", t("&#x110000;"))],
            "the character reference {reference:?} refers to a code point above U+10FFFF, which is not a character",
            "the input is not well-formed XML at line 1, column 7: the character reference \"&#x110000;\" refers to a code point above U+10FFFF, which is not a character",
        ),
        read(
            "SACM-XML-001/no-root",
            "<!-- c -->",
            &[],
            "the document has no root element",
            "the input is not well-formed XML at line 1, column 11: the document has no root element",
        ),
        read(
            "SACM-XML-001/unclosed-root",
            "<r>",
            &[],
            "the root element is not closed",
            "the input is not well-formed XML at line 1, column 4: the root element is not closed",
        ),
        read(
            "SACM-XML-001/unexpected-end",
            "<r a=",
            &[],
            "the input ends before the document is complete",
            "the input is not well-formed XML at line 1, column 6: the input ends before the document is complete",
        ),
        read(
            "SACM-XML-001/xml-prefix-uri",
            "<r xmlns:xml=\"urn:x\"/>",
            &[],
            "the \"xml\" prefix is bound to a namespace other than the XML namespace",
            "the input is not well-formed XML at line 1, column 4: the \"xml\" prefix is bound to a namespace other than the XML namespace",
        ),
        read(
            "SACM-XML-001/xml-uri-prefix",
            "<r xmlns:p=\"http://www.w3.org/XML/1998/namespace\"/>",
            &[],
            "the XML namespace is bound to a prefix other than \"xml\"",
            "the input is not well-formed XML at line 1, column 4: the XML namespace is bound to a prefix other than \"xml\"",
        ),
        read(
            "SACM-XML-001/xmlns-uri",
            "<r xmlns:p=\"http://www.w3.org/2000/xmlns/\"/>",
            &[],
            "the \"xmlns\" namespace is declared, which is not allowed",
            "the input is not well-formed XML at line 1, column 4: the \"xmlns\" namespace is declared, which is not allowed",
        ),
        read(
            "SACM-XML-001/xmlns-element-prefix",
            "<xmlns:r/>",
            &[],
            "an element name has the prefix \"xmlns\"",
            "the input is not well-formed XML at line 1, column 2: an element name has the prefix \"xmlns\"",
        ),
        read(
            "SACM-XML-001/duplicate-prefix",
            "<r xmlns:p=\"a\" xmlns:p=\"b\"/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is declared twice on one element",
            "the input is not well-formed XML at line 1, column 16: the namespace prefix \"p\" is declared twice on one element",
        ),
        read(
            "SACM-XML-001/unknown-prefix",
            "<p:r/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is not declared",
            "the input is not well-formed XML at line 1, column 2: the namespace prefix \"p\" is not declared",
        ),
        read(
            "SACM-XML-001/end-tag-mismatch",
            "<r></s>",
            &[("end_tag", t("s")), ("start_tag", t("r"))],
            "the end tag {end_tag:?} does not match the start tag {start_tag:?}",
            "the input is not well-formed XML at line 1, column 4: the end tag \"s\" does not match the start tag \"r\"",
        ),
        read(
            "SACM-XML-001/unknown-entity",
            "<r>&foo;</r>",
            &[("entity", t("foo"))],
            "the entity {entity:?} is not declared",
            "the input is not well-formed XML at line 1, column 4: the entity \"foo\" is not declared",
        ),
        read(
            "SACM-XML-001/malformed-reference",
            "<r>& </r>",
            &[],
            "a \"&\" does not start an entity or character reference",
            "the input is not well-formed XML at line 1, column 4: a \"&\" does not start an entity or character reference",
        ),
        read(
            "SACM-XML-001/duplicate-attribute",
            "<r a=\"1\" a=\"2\"/>",
            &[("attribute", t("a"))],
            "an element has the attribute {attribute:?} more than once",
            "the input is not well-formed XML at line 1, column 10: an element has the attribute \"a\" more than once",
        ),
        read(
            "SACM-XML-001/misplaced-declaration",
            "<r/><?xml version=\"1.0\"?>",
            &[],
            "an XML declaration is not at the start of the document",
            "the input is not well-formed XML at line 1, column 5: an XML declaration is not at the start of the document",
        ),
        read(
            "SACM-XML-001/invalid-name",
            "<r><1/></r>",
            &[],
            "a name is not valid",
            "the input is not well-formed XML at line 1, column 5: a name is not valid",
        ),
        read(
            "SACM-XML-001/non-xml-character",
            "<r>\u{1}</r>",
            &[("code_point", n(1))],
            "the character U+{code_point:04X} is not allowed in XML",
            "the input is not well-formed XML at line 1, column 4: the character U+0001 is not allowed in XML",
        ),
        read(
            "SACM-XML-001/unexpected-character",
            "<r a/>",
            &[("expected", t("\"=\""))],
            "an unexpected character where {expected} was expected",
            "the input is not well-formed XML at line 1, column 5: an unexpected character where \"=\" was expected",
        ),
        read(
            "SACM-XML-001/expected-string",
            "<r><!--",
            &[("expected", t("-->"))],
            "{expected:?} was expected",
            "the input is not well-formed XML at line 1, column 8: \"-->\" was expected",
        ),
        read(
            "SACM-XML-001/invalid-comment",
            "<r><!-- a -- b --></r>",
            &[],
            "a comment holds \"--\" or ends with \"-\"",
            "the input is not well-formed XML at line 1, column 4: a comment holds \"--\" or ends with \"-\"",
        ),
        read(
            "SACM-XML-001/cdata-end-in-text",
            "<r>]]></r>",
            &[],
            "character data holds \"]]>\"",
            "the input is not well-formed XML at line 1, column 7: character data holds \"]]>\"",
        ),
        read(
            "SACM-XML-001/unknown-token",
            "<r/>x",
            &[],
            "the input has markup or text that is not allowed here",
            "the input is not well-formed XML at line 1, column 5: the input has markup or text that is not allowed here",
        ),
    ]
}

/// The `AssuranceCasePackage` `case` of acceptance 2, holding a
/// `TerminologyPackageInterface` at line 3, column 3.
fn terminology_interface_in_a_case() -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"case\"/>\n  \
         <terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"ti\"/>\n\
         </sacm:AssuranceCasePackage>"
    )
}

// --- Running a case ------------------------------------------------------------------

/// A refusal of either kind.
enum Refusal {
    Read(ReadError),
    Write(WriteError),
}

impl Refusal {
    fn display(&self) -> String {
        match self {
            Self::Read(error) => error.to_string(),
            Self::Write(error) => error.to_string(),
        }
    }
}

/// The refusal of `case`'s input: a strict read, or a strict write.
fn refusal(case: &Case) -> Refusal {
    let template = case.template;
    match &case.input {
        Input::Read(bytes) => {
            match xmi::read(bytes, &ReadOptions::default().with_mode(Mode::Strict)) {
                Ok(output) => panic!(
                    "{ROW}: {template}: a strict read accepts the input, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => Refusal::Read(error),
            }
        }
        Input::Write(document) => match xmi::write(document, &WriteOptions::default()) {
            Ok(text) => panic!("{ROW}: {template}: a strict write writes the document: {text}"),
            Err(error) => Refusal::Write(error),
        },
    }
}

/// The code text of `template`: the part before the `/`.
fn code_of(template: &str) -> &str {
    template.split_once('/').map_or(template, |(code, _)| code)
}

/// The placeholders of `english` in order, with their format: `""`, `":?"` or
/// `":04X"`.
fn placeholders(english: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = english;
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

/// `english` with each placeholder replaced by its argument: Text as it is
/// for `{n}`, Debug-quoted for `{n:?}`, a Count as at least four uppercase
/// hex digits for `{n:04X}` and in decimal for `{n}`.
fn render(template: &str, english: &str, arguments: &[(&str, Value)]) -> String {
    let mut out = String::new();
    let mut rest = english;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            panic!("{ROW}: {template}: an unclosed placeholder in {english:?}");
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
            (":04X", Value::Count(count)) => {
                let _ = write!(out, "{count:04X}");
            }
            _ => panic!("{ROW}: {template}: {{{inner}}} does not take {value:?}"),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The label an `Unsupported` refusal of `code` ends with, if its code is
/// one of an `Unsupported` refusal, given its `row` argument.
fn label(code: &str, row: Option<&str>) -> Option<String> {
    match (code, row) {
        ("SACM-XMI-014" | "SACM-XMI-007", Some(row)) => {
            Some(format!("(a limit of this version; {row} reads it)"))
        }
        ("SACM-XMI-002" | "SACM-XMI-015", Some(row)) => Some(format!(
            "(strict mode never reads it; {row} is to read it in tolerant mode)"
        )),
        ("SACM-XML-002" | "SACM-SEC-002", None) => Some("(a limit of this version)".to_owned()),
        ("SACM-XMI-003", None) => Some("(a defect in the input)".to_owned()),
        _ => None,
    }
}

/// Whether `template`'s refusal is `ReadError::Unsupported`: its code is one
/// only `Unsupported` carries.
fn is_unsupported(template: &str) -> bool {
    matches!(
        code_of(template),
        "SACM-XMI-014"
            | "SACM-XMI-007"
            | "SACM-XMI-002"
            | "SACM-XMI-015"
            | "SACM-XML-002"
            | "SACM-SEC-002"
            | "SACM-XMI-003"
    )
}

/// Checks the table row of `case` against itself: the placeholders and the
/// arguments agree in names, kinds and order, and `row` is last exactly on
/// the `Unsupported` templates of SACM-XMI-014, -007, -002 and -015.
fn check_row(case: &Case) {
    let template = case.template;
    let mut first_seen: Vec<&str> = Vec::new();
    for (name, format) in placeholders(case.english) {
        let Some(&(_, value)) = case.arguments.iter().find(|(given, _)| *given == name) else {
            panic!("{ROW}: {template}: the placeholder {name:?} names no argument");
        };
        let fits = matches!(
            (format, value),
            ("" | ":?", Value::Text(_)) | ("" | ":04X", Value::Count(_))
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
    let without_row: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|name| *name != "row")
        .collect();
    assert_eq!(
        without_row, first_seen,
        "{ROW}: {template}: the arguments other than row are not those the pattern names, in the \
         order it first names them"
    );
    let takes_row = matches!(
        code_of(template),
        "SACM-XMI-014" | "SACM-XMI-007" | "SACM-XMI-002" | "SACM-XMI-015"
    );
    if takes_row {
        assert_eq!(
            declared.last(),
            Some(&"row"),
            "{ROW}: {template}: row is not the last argument"
        );
        assert_eq!(
            declared.iter().filter(|name| **name == "row").count(),
            1,
            "{ROW}: {template}: row is declared more than once"
        );
    } else {
        assert!(
            !declared.contains(&"row"),
            "{ROW}: {template}: only the Unsupported templates of SACM-XMI-014, -007, -002 and \
             -015 take row"
        );
    }
}

#[test]
fn lib_api_017_each_refusal_template_input_gives_its_0_1_0_display() {
    let cases = cases();
    // The inputs and the templates without one are the 102, once each.
    let mut ids: Vec<&str> = cases.iter().map(|case| case.template).collect();
    ids.extend(NO_INPUT.iter().map(|(template, _)| *template));
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        (count, ids.len()),
        (REFUSAL_TEMPLATES, REFUSAL_TEMPLATES),
        "{ROW}: the table and NO_INPUT do not hold the {REFUSAL_TEMPLATES} refusal templates \
         once each"
    );
    for case in &cases {
        let template = case.template;
        check_row(case);
        let display = refusal(case).display();
        assert_eq!(
            display, case.display,
            "{ROW}: {template}: the Display is not 0.1.0's"
        );
        let variable = render(template, case.english, &case.arguments);
        assert!(
            display.contains(&variable),
            "{ROW}: {template}: the Display {display:?} does not hold the rendered pattern \
             {variable:?}"
        );
        if is_unsupported(template) {
            let row = case
                .arguments
                .iter()
                .find_map(|&(name, value)| match (name, value) {
                    ("row", Value::Text(row)) => Some(row),
                    _ => None,
                });
            let Some(label) = label(code_of(template), row) else {
                panic!("{ROW}: {template}: no label for its code and row {row:?}");
            };
            assert!(
                display.ends_with(&format!("{variable} {label}")),
                "{ROW}: {template}: the Display {display:?} does not end with the pattern and \
                 the label {label:?}"
            );
        }
    }
}

/// Text in the `value` of a `name` is a C++ shape of its own: a tolerant
/// read reports it, after the `name-value` shape of the `value` itself, with
/// the English of `SACM-XMI-012/name-value-text`, the template no strict
/// read refuses (LIB-API-017, acceptance 4).
#[test]
fn lib_api_017_text_in_the_value_of_a_name_is_reported_with_its_template_s_english() {
    let input = named_package("<name><value>A</value></name>");
    let output = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: a tolerant read refuses {input}: {error}"),
    };
    let found: Vec<(Option<(u32, u32)>, &str)> = output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == sacm::diagnostic::Code::XMI_LEGACY_SHAPE)
        .map(|found| {
            (
                found.location().map(|at| (at.line(), at.column())),
                found.message(),
            )
        })
        .collect();
    let column = |what: &str| {
        input
            .find(what)
            .and_then(|offset| u32::try_from(offset + 1).ok())
            .unwrap_or_else(|| panic!("{ROW}: setup: no {what:?} in {input}"))
    };
    assert_eq!(
        (column("<value"), column("A</value>")),
        (140, 147),
        "{ROW}: setup"
    );
    assert_eq!(
        found,
        [
            (
                Some((1, 140)),
                "the name of the ArgumentPackage \"p\" holds a value, a shape of the \
                 assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its \
                 standard form, which write writes",
            ),
            (
                Some((1, 147)),
                "the value in the name of the ArgumentPackage \"p\" has its content as text, a \
                 shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is \
                 read as its standard form, which write writes",
            ),
        ],
        "{ROW}: the two SACM-XMI-012 Warnings of text in a name's value"
    );
}

mod api {
    use super::{Case, ROW, Refusal, Value, cases, code_of, refusal};
    use sacm::diagnostic::{Argument, ArgumentValue};

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
    fn lib_api_017_each_refusal_template_has_its_code_template_and_arguments() {
        for case in cases() {
            let template = case.template;
            let (code, id, arguments) = match refusal(&case) {
                Refusal::Read(error) => (error.code(), error.template(), named(error.arguments())),
                Refusal::Write(error) => (error.code(), error.template(), named(error.arguments())),
            };
            assert_eq!(
                code.as_str(),
                code_of(template),
                "{ROW}: {template}: code()"
            );
            assert_eq!(id, template, "{ROW}: {template}: template()");
            assert_eq!(
                arguments,
                expected(&case),
                "{ROW}: {template}: arguments() by name, kind and value"
            );
        }
    }
}
