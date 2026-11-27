//! The message templates: each English sentence form a diagnostic's or a
//! refusal's message is made from, under a stable id, with the arguments it
//! takes (LIB-API-017, LIB-API-018; ADR 0030 amendment 3).
//!
//! One entry per template, in the fixed syntax that
//! `xtask/tests/message_templates.rs` parses and checks against the
//! `## Message templates` table of `docs/sacm/diagnostics.md`:
//!
//! ```text
//! NAME = CODE_CONSTANT, Carrier, "<CODE>/<form>",
//!     [Text("name"), Count("name"), Prose("name")],
//!     "the English pattern, with {name}, {name:?} and {name:04X}";
//! ```
//!
//! The carrier is `Diagnostic` for a [`Diagnostic`](super::Diagnostic)'s
//! message, `Read("<Variant>")` for a `ReadError` variant,
//! `Write("<Variant>")` for a `WriteError` variant, or `Unread` for the
//! refusal `xmi/refusal.rs` builds from its list of what this version does
//! not read. A const is named after its id: without `SACM-`, in upper case,
//! with `-` and `/` as `_`. [`render`] builds the variable part of a message
//! from a template and its arguments.

use std::fmt::Write as _;

use super::{Argument, ArgumentValue, Code};

/// One message template.
#[derive(Debug)]
pub(crate) struct Template {
    /// The stable id, `<CODE>/<form>`.
    pub(crate) id: &'static str,
    /// The catalog code of what carries it.
    pub(crate) code: Code,
    /// What carries the message.
    #[cfg_attr(
        not(any(feature = "serde", test)),
        expect(dead_code, reason = "serde and the tests check a template's carrier")
    )]
    pub(crate) carrier: Carrier,
    /// The arguments, by name and kind, in the order the pattern first names
    /// them; an `Unread` refusal's `row`, which the label names, comes last,
    /// and so does a diagnostic's, which its pattern names last.
    #[cfg_attr(
        not(any(feature = "serde", test)),
        expect(dead_code, reason = "serde and the tests check a template's arguments")
    )]
    pub(crate) arguments: &'static [(&'static str, Kind)],
    /// The English pattern.
    pub(crate) pattern: &'static str,
}

/// What carries a template's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Carrier {
    /// A [`Diagnostic`](super::Diagnostic)'s message.
    Diagnostic,
    /// The `ReadError` variant of this name.
    Read(&'static str),
    /// The `WriteError` variant of this name.
    Write(&'static str),
    /// The refusal of content this version does not read
    /// (`xmi/refusal.rs`).
    Unread,
}

/// The kind of an argument's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// [`ArgumentValue::Text`].
    Text,
    /// [`ArgumentValue::Count`].
    Count,
    /// [`ArgumentValue::Prose`].
    Prose,
}

/// Defines each template as a static, and lists them all in [`TEMPLATES`],
/// in source order.
macro_rules! templates {
    ($(
        $name:ident = $code:ident, $carrier:ident $(($variant:literal))?, $id:literal,
            [$($kind:ident($argument:literal)),*],
            $pattern:literal;
    )+) => {
        $(
            #[doc = concat!("The template `", $id, "`: `", $pattern, "`.")]
            pub(crate) static $name: Template = Template {
                id: $id,
                code: Code::$code,
                carrier: Carrier::$carrier $(($variant))?,
                arguments: &[$(($argument, Kind::$kind)),*],
                pattern: $pattern,
            };
        )+

        /// Every template, in source order.
        pub(crate) static TEMPLATES: &[&Template] = &[$(&$name),+];
    };
}

templates! {
    SEC_001_DOCTYPE = XML_DOCTYPE_REJECTED, Read("Doctype"), "SACM-SEC-001/doctype",
        [],
        "the input has a DOCTYPE, which is refused";
    SEC_002_NODES = PARSER_LIMIT, Unread, "SACM-SEC-002/nodes",
        [],
        "the document has more nodes or nesting levels than the parser can hold";
    SEC_002_ATTRIBUTES = PARSER_LIMIT, Unread, "SACM-SEC-002/attributes",
        [],
        "the document has more attributes than the parser can hold";
    SEC_002_NAMESPACES = PARSER_LIMIT, Unread, "SACM-SEC-002/namespaces",
        [],
        "the document declares more namespaces than the parser can hold";
    SEC_003_INPUT_BYTES = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/input-bytes",
        [Count("size"), Count("max")],
        "the input is {size} bytes long, over the size limit of {max} bytes";
    SEC_003_KEPT_NAMESPACES = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/kept-namespaces",
        [Count("max")],
        "the namespace names and bindings that preserved content keeps come to more than the size limit of {max} bytes";
    SEC_003_KEPT_TEXT = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/kept-text",
        [Count("max")],
        "the namespace text and diagnostics that the read keeps come to more than the size limit of {max} bytes";
    SEC_003_DEPTH = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/depth",
        [Count("max")],
        "an element is nested deeper than the depth limit of {max}";
    SEC_003_NODES = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/nodes",
        [Count("max")],
        "the input has more XML nodes than the node limit of {max}";
    SEC_003_NAMESPACE_COPIES = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/namespace-copies",
        [Count("max")],
        "the elements that declare namespaces have more bindings in scope in all than the node limit of {max}";
    SEC_003_NAMESPACE_BINDINGS = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/namespace-bindings",
        [Count("max")],
        "an element has more namespace bindings in scope than the limit of {max}";
    SEC_003_ATTRIBUTES = LIMIT_EXCEEDED, Read("LimitExceeded"), "SACM-SEC-003/attributes",
        [Count("max")],
        "an element has more attributes than the limit of {max}";
    SEC_002_UNHELD_OUTSIDE = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-outside",
        [Prose("error")],
        "the content outside every element cannot be held in the document: {error}";
    SEC_002_UNHELD_ROOT = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-root",
        [Text("element"), Prose("error")],
        "the root element {element:?} cannot be held in the document: {error}";
    SEC_002_UNHELD_ELEMENT = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-element",
        [Text("element"), Prose("error")],
        "the element {element:?} cannot be held in the document: {error}";
    XML_002_BYTE_ORDER_MARK = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/byte-order-mark",
        [Prose("encoding")],
        "the input starts with a {encoding} byte order mark, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 are read";
    XML_002_DETECTED_ENCODING = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/detected-encoding",
        [Prose("encoding")],
        "the input is encoded in {encoding}, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 starting with U+FEFF are read";
    XML_002_DECLARED_ENCODING = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/declared-encoding",
        [Text("encoding")],
        "the declared encoding {encoding:?} is not supported: without a UTF-16 byte order mark, only UTF-8, ASCII and ISO-8859-1 are read";
    XML_003_UNREPRESENTABLE_CHARACTER = XML_UNREPRESENTABLE_CHARACTER, Write("UnrepresentableCharacter"), "SACM-XML-003/unrepresentable-character",
        [Text("element"), Prose("attribute"), Count("code_point")],
        "the element {element:?} cannot be written: its {attribute} holds U+{code_point:04X}, which XML 1.0 cannot represent";
    XML_003_UNREPRESENTABLE_CHARACTER_IN_VALUE = XML_UNREPRESENTABLE_CHARACTER, Write("UnrepresentableCharacter"), "SACM-XML-003/unrepresentable-character-in-value",
        [Text("element"), Prose("attribute"), Count("position"), Count("code_point")],
        "the element {element:?} cannot be written: its {attribute} at index {position} holds U+{code_point:04X}, which XML 1.0 cannot represent";
    XMI_001_NOT_A_PACKAGE = XMI_INVALID_ROOT, Read("InvalidRoot"), "SACM-XMI-001/not-a-package",
        [Text("element")],
        "the root element {element:?} is not a SACM package";
    XMI_001_NO_PACKAGE = XMI_INVALID_ROOT, Read("NoPackage"), "SACM-XMI-001/no-package",
        [],
        "the XMI element holds no SACM package";
    XMI_001_NO_ROOT_TO_WRITE = XMI_INVALID_ROOT, Write("NoRoot"), "SACM-XMI-001/no-root-to-write",
        [],
        "the document has no root to write";
    XMI_001_NOT_A_SACM_NAMESPACE = XMI_INVALID_ROOT, Read("InvalidRoot"), "SACM-XMI-001/not-a-sacm-namespace",
        [Text("element"), Text("namespace")],
        "the root element {element:?} is in the namespace {namespace:?}, which does not name SACM";
    XMI_001_NO_NAMESPACE = XMI_INVALID_ROOT, Read("InvalidRoot"), "SACM-XMI-001/no-namespace",
        [Text("element")],
        "the root element {element:?} is in no namespace, so it is not a SACM package";
    XMI_001_XMI_NAMESPACE_ROOT = XMI_INVALID_ROOT, Read("InvalidRoot"), "SACM-XMI-001/xmi-namespace-root",
        [Text("element")],
        "the document element {element:?} is in the XMI namespace but is not xmi:XMI, so the document holds no SACM package";
    XMI_002_OTHER_NAMESPACE = XMI_NAMESPACE_NOT_PINNED, Unread, "SACM-XMI-002/other-namespace",
        [Text("element"), Text("namespace"), Text("row")],
        "the root element {element:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"";
    XMI_008_OTHER_REVISION_ROOT = XMI_OTHER_REVISION, Unread, "SACM-XMI-008/other-revision-root",
        [Text("element"), Text("namespace"), Text("revision"), Text("row")],
        "the root element {element:?} is in the namespace {namespace:?} of SACM {revision}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"";
    XMI_008_SACM_2_4 = XMI_OTHER_REVISION, Unread, "SACM-XMI-008/sacm-2-4",
        [Text("element"), Text("namespace")],
        "the root element {element:?} is in the SACM 2.4 namespace {namespace:?}, and SACM 2.4 restructures the metamodel, so it is not read as SACM 2.3 until LIB-COMPAT-018 decides how";
    XMI_008_OTHER_REVISION = XMI_OTHER_REVISION, Diagnostic, "SACM-XMI-008/other-revision",
        [Text("class"), Text("id"), Text("namespace"), Text("revision")],
        "the {class} {id:?} is in the namespace {namespace:?} of SACM {revision}, so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3";
    XMI_008_OCCURENCE = XMI_OTHER_REVISION, Diagnostic, "SACM-XMI-008/occurence",
        [Text("id"), Text("revision")],
        "the Event {id:?} gives its date as the attribute \"occurence\", the SACM {revision} spelling, which is read as its date";
    XMI_002_SACM_LIKE = XMI_NAMESPACE_NOT_PINNED, Diagnostic, "SACM-XMI-002/sacm-like",
        [Text("class"), Text("id"), Text("namespace")],
        "the {class} {id:?} is in the namespace {namespace:?}, which names SACM but is not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\", so it is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved content, and a write writes SACM 2.3";
    XMI_003_UNREAD_ATTRIBUTE = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/unread-attribute",
        [Text("attribute"), Text("element")],
        "the attribute {attribute:?} of the element {element:?} is not read in this release";
    XMI_003_WRAPPER_TEXT = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/wrapper-text",
        [Text("element")],
        "the element {element:?} holds text, which has no element to be kept in";
    XMI_003_WRAPPER_ELEMENT = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/wrapper-element",
        [Text("element"), Text("container")],
        "the element {element:?} in {container:?} is in the XMI namespace, but XMI 2.5.1 defines no such element there, and it has no element to be kept in";
    XMI_003_UNWRITABLE_PREFIX = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/unwritable-prefix",
        [Text("attribute"), Text("element"), Text("prefix"), Text("namespace"), Text("pinned_namespace")],
        "the attribute {attribute:?} of the element {element:?} is not SACM 2.3, and cannot be kept: its prefix {prefix:?} is bound to {namespace:?}, and the written element binds it to {pinned_namespace:?}";
    XMI_005_MISSING_TYPE = XMI_MISSING_TYPE, Read("MissingType"), "SACM-XMI-005/missing-type",
        [Text("role"), Text("type")],
        "the element {role:?} has no xmi:type, and its role has the abstract type {type}";
    XMI_006_ONE_HOLDER = XMI_STRICT_SAVE_REFUSED, Write("PreservedContent"), "SACM-XMI-006/one-holder",
        [Text("id")],
        "the document cannot be written in strict mode: the element {id:?} holds preserved content";
    XMI_006_HOLDERS = XMI_STRICT_SAVE_REFUSED, Write("PreservedContent"), "SACM-XMI-006/holders",
        [Count("count"), Text("id")],
        "the document cannot be written in strict mode: {count} elements hold preserved content, the first is {id:?}";
    XMI_006_OUTSIDE_ELEMENTS = XMI_STRICT_SAVE_REFUSED, Write("PreservedContent"), "SACM-XMI-006/outside-elements",
        [],
        "the document cannot be written in strict mode: the document holds preserved content outside its elements";
    XMI_012_NAME_ATTRIBUTE = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/name-attribute",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} is given as the attribute \"name\"";
    XMI_012_CONTENT_ON_ELEMENT = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/content-on-element",
        [Text("class"), Text("id")],
        "the content of the {class} {id:?} is given on the element itself";
    XMI_012_KEY_ATTRIBUTE = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/key-attribute",
        [Text("class"), Text("id")],
        "the key of the {class} {id:?} is given as the attribute \"key\"";
    XMI_012_SHORTHAND = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/shorthand",
        [Text("role"), Text("class"), Text("id")],
        "the {role} of the {class} {id:?} is given in the single-language shorthand";
    XMI_012_NAME_TEXT = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/name-text",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} has its content as text";
    XMI_012_VALUE_TEXT = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/value-text",
        [Text("role"), Text("class"), Text("id")],
        "a value of the {role} of the {class} {id:?} has its content as text";
    XMI_012_NAME_VALUE_TEXT = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/name-value-text",
        [Text("class"), Text("id")],
        "the value in the name of the {class} {id:?} has its content as text";
    XMI_012_NAME_CONTENT_LANG = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/name-content-lang",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} holds a content carrying lang";
    XMI_012_NAME_VALUE = XMI_LEGACY_SHAPE, Read("LegacyShape"), "SACM-XMI-012/name-value",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} holds a value";
    XMI_015_NOT_XMI_NAMESPACE = XMI_WRAPPER_NAMESPACE, Unread, "SACM-XMI-015/not-xmi-namespace",
        [Text("element"), Text("row")],
        "the element {element:?} is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element";
    XMI_015_NOT_XMI_ELEMENT = XMI_WRAPPER_NAMESPACE, Unread, "SACM-XMI-015/not-xmi-element",
        [Text("element")],
        "the element {element:?} is in neither the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\" nor \"http://www.omg.org/XMI\", so it is not read as the XMI element";
    XMI_015_EMF_XMI_NAMESPACE = XMI_WRAPPER_NAMESPACE, Unread, "SACM-XMI-015/emf-xmi-namespace",
        [Text("name"), Text("row")],
        "the name {name:?} is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\"";
    XMI_016_SCHEMA_ERROR = XMI_STRICT_SAVE_INVALID, Write("Invalid"), "SACM-XMI-016/schema-error",
        [Text("first_code"), Text("first_id")],
        "the document cannot be written in strict mode: 1 schema error, {first_code} on {first_id:?}";
    XMI_016_SCHEMA_ERRORS = XMI_STRICT_SAVE_INVALID, Write("Invalid"), "SACM-XMI-016/schema-errors",
        [Count("count"), Text("first_code"), Text("first_id")],
        "the document cannot be written in strict mode: {count} schema errors, the first is {first_code} on {first_id:?}";
    ID_001_DUPLICATE_ID = ID_DUPLICATE, Read("DuplicateId"), "SACM-ID-001/duplicate-id",
        [Text("id"), Count("count")],
        "the xmi:id {id:?} is carried by {count} elements";
    ID_002_READ_ID_NOT_AN_NCNAME = ID_INVALID, Read("InvalidId"), "SACM-ID-002/read-id-not-an-ncname",
        [Text("id")],
        "the xmi:id {id:?} is not an NCName";
    MULT_001_BOTH_FORMS = MULTIPLICITY_VIOLATION, Read("RepeatedValue"), "SACM-MULT-001/both-forms",
        [Text("property"), Text("element")],
        "the property {property:?} of the element {element:?} is given both as an attribute and as elements, which XMI 2.5.1 rule 2h forbids";
    MULT_001_REPEATED_VALUE = MULTIPLICITY_VIOLATION, Read("RepeatedValue"), "SACM-MULT-001/repeated-value",
        [Text("property"), Text("element"), Count("count")],
        "the single-valued property {property:?} of the element {element:?} is given {count} times";
    ENUM_001_NOT_A_LITERAL = ENUM_INVALID_LITERAL, Read("InvalidLiteral"), "SACM-ENUM-001/not-a-literal",
        [Text("value"), Text("property"), Text("element")],
        "the value {value:?} of the property {property:?} of the element {element:?} is not a literal";
    XML_001_INVALID_UTF8 = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/invalid-utf8",
        [],
        "a byte sequence is not valid UTF-8";
    XML_001_UNPAIRED_SURROGATE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unpaired-surrogate",
        [Count("unit")],
        "the UTF-16 code unit {unit:04X} is an unpaired surrogate";
    XML_001_ODD_UTF16_BYTE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/odd-utf16-byte",
        [],
        "the UTF-16 input ends with a byte that is not a whole code unit";
    XML_001_ENCODING_CONTRADICTS_BOM = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/encoding-contradicts-bom",
        [Text("encoding")],
        "the declared encoding {encoding:?} contradicts the UTF-16 byte order mark";
    XML_001_ENCODING_CONTRADICTS_UTF8_BOM = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/encoding-contradicts-utf8-bom",
        [Text("encoding")],
        "the declared encoding {encoding:?} contradicts the UTF-8 byte order mark";
    XML_001_NON_ASCII_CHARACTER = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/non-ascii-character",
        [Text("encoding"), Count("code_point")],
        "the declared encoding is {encoding:?}, but the input holds the non-ASCII character U+{code_point:04X}";
    XML_001_XML_VERSION = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xml-version",
        [Text("version")],
        "the XML version {version:?} is not \"1.\" followed by digits";
    XML_001_ENCODING_NAME = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/encoding-name",
        [Text("encoding")],
        "the encoding name {encoding:?} is not a valid encoding name";
    XML_001_STANDALONE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/standalone",
        [Text("value")],
        "the standalone value {value:?} is not \"yes\" or \"no\"";
    XML_001_UNKNOWN_PSEUDO_ATTRIBUTE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unknown-pseudo-attribute",
        [Text("name")],
        "the XML declaration has the unknown pseudo-attribute {name:?}";
    XML_001_PSEUDO_ATTRIBUTE_ORDER = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/pseudo-attribute-order",
        [Text("name")],
        "the pseudo-attribute {name:?} is out of order in the XML declaration";
    XML_001_PSEUDO_ATTRIBUTE_SPACE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/pseudo-attribute-space",
        [Text("name")],
        "the pseudo-attribute {name:?} does not follow white space in the XML declaration";
    XML_001_DECLARATION_CHARACTER = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/declaration-character",
        [Prose("expected")],
        "an unexpected character in the XML declaration where {expected} was expected";
    XML_001_DECLARATION_END = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/declaration-end",
        [],
        "the input ends inside the XML declaration";
    XML_001_RESERVED_PI_TARGET = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/reserved-pi-target",
        [Text("target")],
        "a processing instruction has the reserved target {target:?}";
    XML_001_PI_TARGET_COLON = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/pi-target-colon",
        [Text("target")],
        "the processing instruction target {target:?} contains a colon";
    XML_001_PI_TARGET_END = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/pi-target-end",
        [Text("target")],
        "the processing instruction target {target:?} is not followed by white space or \"?>\"";
    XML_001_EMPTY_PREFIX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/empty-prefix",
        [Text("name")],
        "the name {name:?} has an empty namespace prefix";
    XML_001_XMLNS_PREFIX_DECLARED = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xmlns-prefix-declared",
        [],
        "the namespace prefix \"xmlns\" is declared, and it cannot be declared";
    XML_001_PREFIX_UNDECLARED = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/prefix-undeclared",
        [Text("prefix")],
        "the namespace prefix {prefix:?} is declared with an empty value, and a prefix cannot be undeclared";
    XML_001_REFERENCE_TO_SURROGATE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/reference-to-surrogate",
        [Text("reference")],
        "the character reference {reference:?} refers to a surrogate, which is not a character";
    XML_001_REFERENCE_ABOVE_MAX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/reference-above-max",
        [Text("reference")],
        "the character reference {reference:?} refers to a code point above U+10FFFF, which is not a character";
    XML_001_NO_ROOT = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/no-root",
        [],
        "the document has no root element";
    XML_001_UNCLOSED_ROOT = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unclosed-root",
        [],
        "the root element is not closed";
    XML_001_UNEXPECTED_END = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unexpected-end",
        [],
        "the input ends before the document is complete";
    XML_001_XML_PREFIX_URI = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xml-prefix-uri",
        [],
        "the \"xml\" prefix is bound to a namespace other than the XML namespace";
    XML_001_XML_URI_PREFIX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xml-uri-prefix",
        [],
        "the XML namespace is bound to a prefix other than \"xml\"";
    XML_001_XMLNS_URI = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xmlns-uri",
        [],
        "the \"xmlns\" namespace is declared, which is not allowed";
    XML_001_XMLNS_ELEMENT_PREFIX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/xmlns-element-prefix",
        [],
        "an element name has the prefix \"xmlns\"";
    XML_001_DUPLICATE_PREFIX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/duplicate-prefix",
        [Text("prefix")],
        "the namespace prefix {prefix:?} is declared twice on one element";
    XML_001_UNKNOWN_PREFIX = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unknown-prefix",
        [Text("prefix")],
        "the namespace prefix {prefix:?} is not declared";
    XML_001_END_TAG_MISMATCH = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/end-tag-mismatch",
        [Text("end_tag"), Text("start_tag")],
        "the end tag {end_tag:?} does not match the start tag {start_tag:?}";
    XML_001_ENTITY_CLOSES_ELEMENT = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/entity-closes-element",
        [],
        "an entity's replacement text closes an element";
    XML_001_UNKNOWN_ENTITY = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unknown-entity",
        [Text("entity")],
        "the entity {entity:?} is not declared";
    XML_001_MALFORMED_REFERENCE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/malformed-reference",
        [],
        "a \"&\" does not start an entity or character reference";
    XML_001_ENTITY_LOOP = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/entity-loop",
        [],
        "entity references nest too deeply";
    XML_001_LT_IN_ATTRIBUTE_VALUE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/lt-in-attribute-value",
        [],
        "an attribute value holds an unescaped \"<\"";
    XML_001_DUPLICATE_ATTRIBUTE = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/duplicate-attribute",
        [Text("attribute")],
        "an element has the attribute {attribute:?} more than once";
    XML_001_MISPLACED_DECLARATION = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/misplaced-declaration",
        [],
        "an XML declaration is not at the start of the document";
    XML_001_INVALID_NAME = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/invalid-name",
        [],
        "a name is not valid";
    XML_001_NON_XML_CHARACTER = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/non-xml-character",
        [Count("code_point")],
        "the character U+{code_point:04X} is not allowed in XML";
    XML_001_UNEXPECTED_CHARACTER = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unexpected-character",
        [Prose("expected")],
        "an unexpected character where {expected} was expected";
    XML_001_EXPECTED_STRING = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/expected-string",
        [Text("expected")],
        "{expected:?} was expected";
    XML_001_INVALID_EXTERNAL_ID = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/invalid-external-id",
        [],
        "an external identifier is not valid";
    XML_001_UNRESOLVED_ENTITY = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unresolved-entity",
        [],
        "an external entity could not be resolved";
    XML_001_INVALID_COMMENT = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/invalid-comment",
        [],
        "a comment holds \"--\" or ends with \"-\"";
    XML_001_CDATA_END_IN_TEXT = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/cdata-end-in-text",
        [],
        "character data holds \"]]>\"";
    XML_001_UNKNOWN_TOKEN = XML_MALFORMED, Read("Malformed"), "SACM-XML-001/unknown-token",
        [],
        "the input has markup or text that is not allowed here";
    // The diagnostic templates (LIB-API-018): those of validation, then those
    // of the reader.
    MULT_001_MISSING = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/missing",
        [Text("class"), Text("id"), Text("property"), Text("type"), Text("multiplicity")],
        "{class} {id:?} has no {property} ({property}:{type}[{multiplicity}])";
    MULT_001_TOO_FEW = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/too-few",
        [Text("class"), Text("id"), Count("count"), Text("property"), Text("type"), Text("multiplicity")],
        "{class} {id:?} has {count} distinct {property} ({property}:{type}[{multiplicity}])";
    MULT_001_TOO_MANY = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/too-many",
        [Text("class"), Text("id"), Count("count"), Text("property"), Text("type"), Text("multiplicity")],
        "{class} {id:?} has {count} {property}s ({property}:{type}[{multiplicity}])";
    MULT_001_NO_VALUE = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/no-value",
        [Text("role"), Text("class"), Text("id")],
        "the {role} of {class} {id:?} has no value (value:LangString[1..*])";
    MULT_001_REPEATED_LANG = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/repeated-lang",
        [Text("role"), Text("class"), Text("id"), Count("count"), Text("lang")],
        "the {role} of {class} {id:?} has {count} values in the language {lang:?}, and each value's lang must be unique";
    MULT_001_REPEATED_ABSENT_LANG = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/repeated-absent-lang",
        [Text("role"), Text("class"), Text("id"), Count("count")],
        "the {role} of {class} {id:?} has {count} values in no language (lang absent), and each value's lang must be unique";
    MULT_001_REPEATED_REFERENCE = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/repeated-reference",
        [Text("class"), Text("id"), Text("target"), Count("count"), Text("end")],
        "{class} {id:?} holds {target:?} {count} times in its {end}, whose ids are unique in SACM 2.3 (the repeat is kept)";
    MULT_001_NAME_WITHOUT_EXPRESSION = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/name-without-expression",
        [Text("class"), Text("id")],
        "the name of {class} {id:?} is an ExpressionLangString with no expression (expression:ExpressionElement[1])";
    MULT_001_VALUE_WITHOUT_EXPRESSION = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/value-without-expression",
        [Count("position"), Text("role"), Text("class"), Text("id")],
        "value {position} of the {role} of {class} {id:?} is an ExpressionLangString with no expression (expression:ExpressionElement[1])";
    EXPR_001_NAME_WITH_CONTENT = EXPRESSION_WITH_CONTENT, Diagnostic, "SACM-EXPR-001/name-with-content",
        [Text("class"), Text("id"), Text("expression")],
        "the name of {class} {id:?} has the expression {expression:?} and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty";
    EXPR_001_VALUE_WITH_CONTENT = EXPRESSION_WITH_CONTENT, Diagnostic, "SACM-EXPR-001/value-with-content",
        [Count("position"), Text("role"), Text("class"), Text("id"), Text("expression")],
        "value {position} of the {role} of {class} {id:?} has the expression {expression:?} and a content that is not empty, and §8.4 says that if expression is not empty, then content should be empty";
    ABS_001_IMPLEMENTATION_CONSTRAINT = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/implementation-constraint",
        [Text("class"), Text("id")],
        "{class} {id:?} has an implementationConstraint but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)";
    ABS_001_IMPLEMENTATION_CONSTRAINTS = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/implementation-constraints",
        [Text("class"), Text("id"), Count("count")],
        "{class} {id:?} has {count} implementationConstraints but isAbstract is not true: implementation constraints are only specified on an abstract element (§8.6)";
    ABS_001_ABSTRACT_HOLDER = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/abstract-holder",
        [Text("class"), Text("id"), Text("target")],
        "{class} {id:?} has the abstractForm {target:?} but is itself abstract (isAbstract is true); §8.2 gives abstractForm to a concrete element";
    ABS_001_FORM_NOT_ABSTRACT = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/form-not-abstract",
        [Text("target"), Text("class"), Text("id")],
        "the abstractForm {target:?} of {class} {id:?} is not abstract: its isAbstract should be true (§8.2)";
    ABS_001_FORM_OF_ANOTHER_KIND = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/form-of-another-kind",
        [Text("target"), Text("class"), Text("id"), Text("target_class")],
        "the abstractForm {target:?} of {class} {id:?} is of the kind {target_class}, not {class}: it should be of the same type (§8.2)";
    ABS_001_ABSTRACT_ELEMENT = ABSTRACT_FORM_INVALID, Diagnostic, "SACM-ABS-001/abstract-element",
        [Text("class"), Text("id"), Text("target"), Text("target_class")],
        "{class} {id:?} is concrete but has the element {target:?}, an abstract {target_class}: a concrete Expression references only concrete ExpressionElements (§10.10)";
    ID_003_DUPLICATE_GID = GID_DUPLICATE, Diagnostic, "SACM-ID-003/duplicate-gid",
        [Count("count"), Text("gid"), Text("elements")],
        "{count} elements hold the gid {gid:?}, which must be unique within the model instance: {elements}";
    PKG_001_MIXED_CONTENT = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/mixed-content",
        [Text("class"), Text("id"), Text("nested_class"), Text("nested_id"), Text("child_class"), Text("child_id")],
        "{class} {id:?} nests the {nested_class} {nested_id:?}, so it may contain only ArgumentPackages, but it also contains {child_class} {child_id:?}";
    PKG_001_NOT_A_CITATION = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/not-a-citation",
        [Text("class"), Text("id"), Text("child_class"), Text("child_id"), Text("clause")],
        "{class} {id:?} contains {child_class} {child_id:?}, which is no citation (isCitation true and a citedElement), but §{clause} allows only citations in it";
    PKG_001_NOT_A_CITATION_OR_RELATIONSHIP = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/not-a-citation-or-relationship",
        [Text("class"), Text("id"), Text("child_class"), Text("child_id"), Text("clause"), Text("relationship")],
        "{class} {id:?} contains {child_class} {child_id:?}, which is no citation (isCitation true and a citedElement), but §{clause} allows only citations and {relationship}s in it";
    PKG_001_CITED_KIND = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/cited-kind",
        [Text("class"), Text("id"), Text("child_class"), Text("child_id"), Text("target"), Text("target_class"), Text("clause"), Text("expected")],
        "{class} {id:?} contains {child_class} {child_id:?}, which cites {target:?} of the kind {target_class}, but §{clause} has an interface cite only {expected}s";
    PKG_001_CITED_OUTSIDE = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/cited-outside",
        [Text("class"), Text("id"), Text("child_class"), Text("child_id"), Text("target"), Text("clause"), Text("package")],
        "{class} {id:?} contains {child_class} {child_id:?}, which cites {target:?}, but §{clause} has an interface cite only elements within the package it implements, {package:?}";
    PKG_001_NOT_AN_INTERFACE = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/not-an-interface",
        [Text("class"), Text("id"), Text("child_class"), Text("child_id"), Text("role"), Text("expected")],
        "{class} {id:?} contains {child_class} {child_id:?} in its {role}, but §9.3 allows only {expected}s there";
    PKG_001_BINDING_PARTICIPANT = PACKAGE_CONTENT_INVALID, Diagnostic, "SACM-PKG-001/binding-participant",
        [Text("class"), Text("id"), Text("target"), Text("target_class")],
        "{class} {id:?} has the participantPackage {target:?}, of the kind {target_class}: a binding binds packages, not bindings";
    PKG_002_OUTSIDE_IMPLEMENTED_PACKAGE = INTERFACE_PLACEMENT, Diagnostic, "SACM-PKG-002/outside-implemented-package",
        [Text("class"), Text("id"), Text("package_class"), Text("package"), Text("clause")],
        "{class} {id:?} implements the {package_class} {package:?} but does not reside inside it, as §{clause} says an interface does";
    ID_002_NOT_AN_NCNAME = ID_INVALID, Diagnostic, "SACM-ID-002/not-an-ncname",
        [Text("class"), Text("id")],
        "{class} {id:?} has an id that is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    ID_002_NAME_ID_NOT_AN_NCNAME = ID_INVALID, Diagnostic, "SACM-ID-002/name-id-not-an-ncname",
        [Text("class"), Text("id"), Text("xmi_id")],
        "the name of {class} {id:?} has the xmi:id {xmi_id:?}, which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    ID_002_TEXT_ID_NOT_AN_NCNAME = ID_INVALID, Diagnostic, "SACM-ID-002/text-id-not-an-ncname",
        [Text("role"), Text("class"), Text("id"), Text("xmi_id")],
        "the {role} of {class} {id:?} has the xmi:id {xmi_id:?}, which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    ID_002_VALUE_ID_NOT_AN_NCNAME = ID_INVALID, Diagnostic, "SACM-ID-002/value-id-not-an-ncname",
        [Count("position"), Text("role"), Text("class"), Text("id"), Text("xmi_id")],
        "value {position} of the {role} of {class} {id:?} has the xmi:id {xmi_id:?}, which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    ID_002_METADATA_ID_NOT_AN_NCNAME = ID_INVALID, Diagnostic, "SACM-ID-002/metadata-id-not-an-ncname",
        [Text("element"), Text("xmi_id")],
        "the element {element:?}, kept as XMI metadata, has the xmi:id {xmi_id:?}, which is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)";
    REF_001_DANGLING = REF_DANGLING, Diagnostic, "SACM-REF-001/dangling",
        [Text("class"), Text("id"), Text("end"), Text("target"), Text("type")],
        "{class} {id:?} has the {end} {target:?}, which names no element of the document (the end's type is {type})";
    REF_001_METADATA_DANGLING = REF_DANGLING, Diagnostic, "SACM-REF-001/metadata-dangling",
        [Text("attribute"), Text("element"), Text("target")],
        "the attribute {attribute:?} of the element {element:?}, kept as XMI metadata, names {target:?}, which is no xmi:id of the document (XMI.xsd types it xsd:IDREF or xsd:IDREFS)";
    REF_002_WRONG_TYPE = REF_WRONG_TYPE, Diagnostic, "SACM-REF-002/wrong-type",
        [Text("class"), Text("id"), Text("end"), Text("target"), Text("target_class"), Text("type")],
        "{class} {id:?} has the {end} {target:?}, whose kind {target_class} does not conform to the end's type {type}";
    REF_002_NOT_AN_ARTIFACT_REFERENCE = REF_WRONG_TYPE, Diagnostic, "SACM-REF-002/not-an-artifact-reference",
        [Text("class"), Text("id"), Text("end"), Text("target"), Text("target_class"), Text("clause")],
        "{class} {id:?} has the {end} {target:?}, of the kind {target_class}, but §{clause} requires each {end} of the {class} to be an ArtifactReference";
    REF_003_PRESERVED_TARGET = REF_PRESERVED_TARGET, Diagnostic, "SACM-REF-003/preserved-target",
        [Text("class"), Text("id"), Text("end"), Text("target"), Text("type")],
        "{class} {id:?} has the {end} {target:?}, which names an element the document holds only as preserved content, so its type is not checked (the end's type is {type})";
    CITE_001_NOT_A_CITATION = CITATION_INVALID, Diagnostic, "SACM-CITE-001/not-a-citation",
        [Text("class"), Text("id"), Text("target")],
        "{class} {id:?} has the citedElement {target:?} but isCitation is not true (§8.2: citedElement <> null implies isCitation = true)";
    CITE_002_NOT_A_CITATION = AS_CITED_INVALID, Diagnostic, "SACM-CITE-002/not-a-citation",
        [Text("class"), Text("id")],
        "{class} {id:?} is declared asCited but isCitation is not true: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)";
    CITE_002_NO_CITED_ELEMENT = AS_CITED_INVALID, Diagnostic, "SACM-CITE-002/no-cited-element",
        [Text("class"), Text("id")],
        "{class} {id:?} is declared asCited but has no citedElement: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)";
    CITE_002_CITED_NOT_AN_ASSERTION = AS_CITED_INVALID, Diagnostic, "SACM-CITE-002/cited-not-an-assertion",
        [Text("class"), Text("id"), Text("target"), Text("target_class")],
        "{class} {id:?} is declared asCited but its citedElement {target:?} is of the kind {target_class}, which is no Assertion: an asCited Assertion takes its declaration from the Assertion it cites (§11.8)";
    XMI_004_GENERATED_ID = XMI_MISSING_ID, Diagnostic, "SACM-XMI-004/generated-id",
        [Text("class"), Text("id")],
        "the {class} has no xmi:id, which XMI 2.5.1 §7.6.1 makes optional, so it is read with the generated id {id:?}, which write writes";
    ID_004_TRIMMED_ID = ID_TRIMMED, Diagnostic, "SACM-ID-004/trimmed-id",
        [Text("written"), Text("class"), Text("id")],
        "the xmi:id {written:?} of the {class} {id:?} is read as {id:?}: an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed";
    ID_004_TRIMMED_ID_OF_NAME = ID_TRIMMED, Diagnostic, "SACM-ID-004/trimmed-id-of-name",
        [Text("written"), Text("class"), Text("id"), Text("value")],
        "the xmi:id {written:?} of the name of the {class} {id:?} is read as {value:?}: an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed";
    ID_004_TRIMMED_ID_OF_TEXT = ID_TRIMMED, Diagnostic, "SACM-ID-004/trimmed-id-of-text",
        [Text("written"), Text("role"), Text("class"), Text("id"), Text("value")],
        "the xmi:id {written:?} of the {role} of the {class} {id:?} is read as {value:?}: an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed";
    ID_004_TRIMMED_ID_OF_TEXT_VALUE = ID_TRIMMED, Diagnostic, "SACM-ID-004/trimmed-id-of-text-value",
        [Text("written"), Text("role"), Text("class"), Text("id"), Text("value")],
        "the xmi:id {written:?} of a value of the {role} of the {class} {id:?} is read as {value:?}: an xmi:id is an xsd:ID, whose white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed";
    XMI_010_XSI_TYPE = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type",
        [Text("class"), Text("id")],
        "the {class} {id:?} is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_010_XSI_TYPE_ON_NAME = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type-on-name",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_010_XSI_TYPE_ON_TEXT = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type-on-text",
        [Text("role"), Text("class"), Text("id")],
        "the {role} of the {class} {id:?} is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_010_XSI_TYPE_ON_TEXT_VALUE = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type-on-text-value",
        [Text("role"), Text("class"), Text("id")],
        "a value of the {role} of the {class} {id:?} is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_010_XSI_TYPE_ON_NAME_VALUE = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type-on-name-value",
        [Text("class"), Text("id")],
        "the value in the name of the {class} {id:?} is typed with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_010_XSI_TYPE_ON_PROXY = XMI_XSI_TYPE, Diagnostic, "SACM-XMI-010/xsi-type-on-proxy",
        [Text("element"), Text("holder_class"), Text("holder_id"), Text("class")],
        "the proxy {element:?} in the {holder_class} {holder_id:?} is typed {class} with xsi:type, which XMI 2.5.1 allows only when org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; it is read as xmi:type";
    XMI_011_VERSION_ON_XMI_ELEMENT = XMI_VERSION_IGNORED, Diagnostic, "SACM-XMI-011/version-on-xmi-element",
        [Text("value")],
        "the xmi:version {value:?} of the XMI element is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it";
    XMI_011_VERSION_ON_ROOT = XMI_VERSION_IGNORED, Diagnostic, "SACM-XMI-011/version-on-root",
        [Text("value"), Text("class"), Text("id")],
        "the xmi:version {value:?} of the {class} {id:?} is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it";
    XMI_011_EMF_VERSION_ON_XMI_ELEMENT = XMI_VERSION_IGNORED, Diagnostic, "SACM-XMI-011/emf-version-on-xmi-element",
        [Text("attribute"), Text("value")],
        "the {attribute} {value:?} of the XMI element is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it";
    XMI_011_EMF_VERSION_ON_ROOT = XMI_VERSION_IGNORED, Diagnostic, "SACM-XMI-011/emf-version-on-root",
        [Text("attribute"), Text("value"), Text("class"), Text("id")],
        "the {attribute} {value:?} of the {class} {id:?} is in the namespace \"http://www.omg.org/XMI\" and is ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3), and write does not write it";
    XMI_015_EMF_XMI_NAMESPACE_READ = XMI_WRAPPER_NAMESPACE, Diagnostic, "SACM-XMI-015/emf-xmi-namespace-read",
        [Text("name")],
        "the name {name:?} is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\", so the XMI names of the document in it are read as XMI 2.5.1's, which write writes";
    XMI_017_PATHS_READ = XMI_FRAGMENT_PATH, Diagnostic, "SACM-XMI-017/paths-read",
        [Count("count"), Text("first")],
        "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define ({count} in all, the first {first:?}); each names an element of the document, so it is read as that element's id, which write writes";
    XMI_017_PATHS_PARTLY_READ = XMI_FRAGMENT_PATH, Diagnostic, "SACM-XMI-017/paths-partly-read",
        [Count("count"), Text("first"), Count("kept")],
        "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not define ({count} in all, the first {first:?}); each that names an element of the document is read as that element's id, which write writes, and those that name none ({kept}) are kept as written";
    ID_002_EMPTY_ID = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id",
        [Text("class"), Text("id")],
        "the xmi:id of the {class} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read with the generated id {id:?}, which write writes";
    ID_002_EMPTY_ID_OF_NAME = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-name",
        [Text("class"), Text("id")],
        "the xmi:id of the name of the {class} {id:?} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one";
    ID_002_EMPTY_ID_OF_TEXT = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-text",
        [Text("role"), Text("class"), Text("id")],
        "the xmi:id of the {role} of the {class} {id:?} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one";
    ID_002_EMPTY_ID_OF_TEXT_VALUE = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-text-value",
        [Text("role"), Text("class"), Text("id")],
        "the xmi:id of a value of the {role} of the {class} {id:?} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is read without one";
    ID_002_EMPTY_ID_OF_XMI_ELEMENT = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-xmi-element",
        [],
        "the xmi:id of the XMI element is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space";
    ID_002_EMPTY_ID_OF_KEPT_ELEMENT = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-kept-element",
        [Text("element"), Text("holder_class"), Text("holder_id")],
        "the xmi:id of the element {element:?} kept in the {holder_class} {holder_id:?} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space";
    ID_002_EMPTY_ID_OF_KEPT_ELEMENT_BY_DOCUMENT = ID_INVALID, Diagnostic, "SACM-ID-002/empty-id-of-kept-element-by-document",
        [Text("element")],
        "the xmi:id of the element {element:?} kept in the document is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so it is kept as written, outside the id space";
    XMI_003_ATTRIBUTE_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/attribute-kept",
        [Text("attribute"), Text("holder_class"), Text("holder_id")],
        "the attribute {attribute:?} is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_ELEMENT_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/element-kept",
        [Text("element"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_TEXT_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/text-kept",
        [Text("holder_class"), Text("holder_id")],
        "text is not SACM 2.3 where it stands, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_WRAPPER_ATTRIBUTE_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/wrapper-attribute-kept",
        [Text("attribute"), Text("element")],
        "the attribute {attribute:?} of the element {element:?} is not SACM 2.3 where it stands, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses";
    XMI_003_WRAPPER_TEXT_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/wrapper-text-kept",
        [Text("element")],
        "text in the element {element:?} is not SACM 2.3 where it stands, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses";
    XMI_003_WRAPPER_ELEMENT_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/wrapper-element-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in {container:?} is in the XMI namespace, but XMI 2.5.1 defines no such element there, so it is kept as preserved content of the document, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_OF_NO_ROOT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-of-no-root",
        [Text("element"), Text("target"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of {target:?}, which is no root of the document, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_WRONG_KIND = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-wrong-kind",
        [Text("element"), Text("target"), Text("target_class"), Text("class"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of the root {target:?}, of the class {target_class}, which is not of the class {class}, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_SUBCLASS_TYPE_KEPT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/subclass-type-kept",
        [Text("type"), Text("class"), Text("id")],
        "the type {type:?} of the {class} {id:?} is not SACM 2.3, but the application declared it a subclass of {class}, so the element is read as {class} and its type is kept as preserved content, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_OF_LINKED_ROOT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-of-linked-root",
        [Text("element"), Text("target"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of the root {target:?}, which carries an href or an xmi:idref of its own, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_OF_TYPED_ROOT = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-of-typed-root",
        [Text("element"), Text("target"), Text("attribute"), Text("value"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of the root {target:?}, which keeps the type attribute {attribute}={value:?} that only a root can carry, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_SHARED = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-shared",
        [Text("element"), Text("target"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of the root {target:?}, which another proxy names too, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_PROXY_CYCLE = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/proxy-cycle",
        [Text("element"), Text("target"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} is a proxy of the root {target:?}, which holds this proxy itself, directly or through other proxies, so it is kept as written as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_003_PREFIX_RENAMED = XMI_UNKNOWN_CONTENT, Diagnostic, "SACM-XMI-003/prefix-renamed",
        [Text("attribute"), Text("element"), Text("prefix"), Text("namespace"), Text("pinned_namespace"), Text("renamed_prefix"), Text("holder_class"), Text("holder_id")],
        "the attribute {attribute:?} of the element {element:?} is not SACM 2.3 where it stands, and its prefix {prefix:?} is bound to {namespace:?}, which the written element binds to {pinned_namespace:?}, so it is written with the prefix {renamed_prefix:?}, so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_012_NAME_ATTRIBUTE_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/name-attribute-read",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} is given as the attribute \"name\", a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_CONTENT_ON_ELEMENT_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/content-on-element-read",
        [Text("class"), Text("id")],
        "the content of the {class} {id:?} is given on the element itself, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_KEY_ATTRIBUTE_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/key-attribute-read",
        [Text("class"), Text("id")],
        "the key of the {class} {id:?} is given as the attribute \"key\", a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_SHORTHAND_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/shorthand-read",
        [Text("role"), Text("class"), Text("id")],
        "the {role} of the {class} {id:?} is given in the single-language shorthand, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_NAME_TEXT_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/name-text-read",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_VALUE_TEXT_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/value-text-read",
        [Text("role"), Text("class"), Text("id")],
        "a value of the {role} of the {class} {id:?} has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_NAME_VALUE_TEXT_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/name-value-text-read",
        [Text("class"), Text("id")],
        "the value in the name of the {class} {id:?} has its content as text, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_NAME_CONTENT_LANG_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/name-content-lang-read",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} holds a content carrying lang, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_012_NAME_VALUE_READ = XMI_LEGACY_SHAPE, Diagnostic, "SACM-XMI-012/name-value-read",
        [Text("class"), Text("id")],
        "the name of the {class} {id:?} holds a value, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its standard form, which write writes";
    XMI_013_GROUP_END_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/group-end-read",
        [Text("element")],
        "\"argumentationElement\" in the element {element:?} is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement; it is read as argumentElement, which write writes";
    XMI_013_GROUP_END_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/group-end-kept",
        [Text("element")],
        "\"argumentationElement\" in the element {element:?} is spelled as in the text of §11.2, and the end of an ArgumentGroup is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement";
    XMI_013_EVENT_DATE_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/event-date-read",
        [Text("element")],
        "\"occurece\" in the element {element:?} is the machine-readable model's spelling, and the attribute of an Event is date (§12.9); it is read as date, which write writes";
    XMI_013_EVENT_DATE_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/event-date-kept",
        [Text("element")],
        "\"occurece\" in the element {element:?} is the machine-readable model's spelling, and the attribute of an Event is date (§12.9); it is kept as preserved content, and a tolerant read reads the name as date";
    XMI_013_PACKAGE_ROLE_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/package-role-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is spelled as in the text of §11.4, and the role is argumentElement; it is read as argumentElement, which write writes";
    XMI_013_PACKAGE_ROLE_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/package-role-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is spelled as in the text of §11.4, and the role is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement";
    XMI_013_PACKAGE_ROLE_IN_ROOT_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/package-role-in-root-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is spelled as in the text of §11.4, and the role is argumentElement; it is read as argumentElement, which write writes";
    XMI_013_PACKAGE_ROLE_IN_ROOT_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/package-role-in-root-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is spelled as in the text of §11.4, and the role is argumentElement; it is kept as preserved content, and a tolerant read reads the name as argumentElement";
    XMI_013_TERMINOLOGY_ROLE_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/terminology-role-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement; it is read as terminologyElement, which write writes";
    XMI_013_TERMINOLOGY_ROLE_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/terminology-role-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement; it is kept as preserved content, and a tolerant read reads the name as terminologyElement";
    XMI_013_TERMINOLOGY_ROLE_IN_ROOT_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/terminology-role-in-root-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement; it is read as terminologyElement, which write writes";
    XMI_013_TERMINOLOGY_ROLE_IN_ROOT_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/terminology-role-in-root-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is spelled as in the text of §10.4, and the role is terminologyElement; it is kept as preserved content, and a tolerant read reads the name as terminologyElement";
    XMI_013_MODEL_TYPE_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/model-type-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is read as ArtifactAssetRelationship, which write writes";
    XMI_013_MODEL_TYPE_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/model-type-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is kept as preserved content, and a tolerant read reads the name as ArtifactAssetRelationship";
    XMI_013_MODEL_TYPE_IN_ROOT_READ = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/model-type-in-root-read",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is read as ArtifactAssetRelationship, which write writes";
    XMI_013_MODEL_TYPE_IN_ROOT_KEPT = XMI_SPELLING_VARIANT, Diagnostic, "SACM-XMI-013/model-type-in-root-kept",
        [Text("element"), Text("container")],
        "the element {element:?} in the root element {container:?} is typed ArtifactAssertedRelationship, the machine-readable model's spelling of ArtifactAssetRelationship; it is kept as preserved content, and a tolerant read reads the name as ArtifactAssetRelationship";
    XMI_005_MISSING_TYPE_KEPT = XMI_MISSING_TYPE, Diagnostic, "SACM-XMI-005/missing-type-kept",
        [Text("element"), Text("container"), Text("role"), Text("type"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} in the element {container:?} has no type, and its role {role} has the abstract type {type}, so its class is not known (XMI 2.5.1 §7.6.3), so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    XMI_005_MISSING_TYPE_IN_ROOT_KEPT = XMI_MISSING_TYPE, Diagnostic, "SACM-XMI-005/missing-type-in-root-kept",
        [Text("element"), Text("container"), Text("role"), Text("type"), Text("holder_class"), Text("holder_id")],
        "the element {element:?} in the root element {container:?} has no type, and its role {role} has the abstract type {type}, so its class is not known (XMI 2.5.1 §7.6.3), so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses";
    ENUM_001_DECLARATION_KEPT = ENUM_INVALID_LITERAL, Diagnostic, "SACM-ENUM-001/declaration-kept",
        [Text("value"), Text("holder_class"), Text("holder_id")],
        "the assertionDeclaration {value:?} is no literal: the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited, so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses, and the property is read as not written";
    ENUM_001_BOOLEAN_KEPT = ENUM_INVALID_LITERAL, Diagnostic, "SACM-ENUM-001/boolean-kept",
        [Text("property"), Text("value"), Text("holder_class"), Text("holder_id")],
        "the {property} {value:?} is no literal: the literals are true, false, 1 and 0, so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses, and the property is read as not written";
    MULT_001_REPEAT_KEPT = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/repeat-kept",
        [Text("property"), Text("element"), Text("holder_class"), Text("holder_id")],
        "the {property} of the element {element:?} is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence, so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses; the first occurrence is read";
    MULT_001_REPEAT_ATTRIBUTE_KEPT = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/repeat-attribute-kept",
        [Text("property"), Text("element"), Text("holder_class"), Text("holder_id")],
        "the {property} of the element {element:?} is given more than once, which XMI 2.5.1 §9.5.2 rule 2h does not allow, and this is not its first occurrence, so it is kept as preserved content of the {holder_class} {holder_id:?}, which a tolerant write writes back and a strict write refuses; the attribute that gave it first is kept with it, and the element has no content from them";
}

/// The template whose id is `id`, if the crate has one.
#[cfg_attr(
    not(any(feature = "serde", test)),
    expect(dead_code, reason = "only serde and the tests look templates up")
)]
pub(crate) fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().copied().find(|template| template.id == id)
}

/// The variable part of a message: `template`'s pattern with each
/// placeholder written from the argument it names. `{n}` writes a Text or
/// Prose as it is and a Count in decimal, `{n:?}` a Text Debug-quoted, and
/// `{n:04X}` a Count as at least four uppercase hex digits. A placeholder that names no
/// argument, or an argument of the other kind, is written back as written,
/// and so is a `{` that no `}` closes; the renderer never panics.
pub(crate) fn render(template: &Template, arguments: &[Argument]) -> String {
    let mut out = String::with_capacity(template.pattern.len());
    let mut rest = template.pattern;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let Some((inner, tail)) = after.split_once('}') else {
            out.push('{');
            out.push_str(after);
            return out;
        };
        let (name, format) = inner.split_once(':').unwrap_or((inner, ""));
        let value = arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value);
        // Writing to a `String` cannot fail.
        let _ = match (value, format) {
            (Some(ArgumentValue::Text(text) | ArgumentValue::Prose(text)), "") => {
                out.write_str(text)
            }
            (Some(ArgumentValue::Text(text)), "?") => write!(out, "{text:?}"),
            (Some(ArgumentValue::Count(count)), "") => write!(out, "{count}"),
            (Some(ArgumentValue::Count(count)), "04X") => write!(out, "{count:04X}"),
            _ => write!(out, "{{{inner}}}"),
        };
        rest = tail;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{Carrier, Kind, TEMPLATES, Template, find, render};
    use crate::diagnostic::{Argument, ArgumentValue};

    /// Whether `id` matches `SACM-[A-Z]+-[0-9]{3}/[a-z0-9]+(-[a-z0-9]+)*`.
    fn is_template_id(id: &str) -> bool {
        let Some((code, form)) = id.split_once('/') else {
            return false;
        };
        let Some(rest) = code.strip_prefix("SACM-") else {
            return false;
        };
        let Some((area, number)) = rest.split_once('-') else {
            return false;
        };
        !area.is_empty()
            && area.bytes().all(|b| b.is_ascii_uppercase())
            && number.len() == 3
            && number.bytes().all(|b| b.is_ascii_digit())
            && !form.is_empty()
            && form.split('-').all(|word| {
                !word.is_empty()
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
    }

    /// The const name a template with id `id` has.
    fn const_name(id: &str) -> String {
        id.strip_prefix("SACM-")
            .unwrap_or(id)
            .to_ascii_uppercase()
            .replace(['-', '/'], "_")
    }

    /// The placeholders of `pattern`, with their format: `""`, `"?"` or
    /// `"04X"`.
    fn placeholders(pattern: &str) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        let mut rest = pattern;
        while let Some((_, after)) = rest.split_once('{') {
            let Some((inner, tail)) = after.split_once('}') else {
                break;
            };
            out.push(inner.split_once(':').unwrap_or((inner, "")));
            rest = tail;
        }
        out
    }

    /// Whether `template` is an `Unsupported` template whose label names
    /// its row.
    fn takes_row(template: &Template) -> bool {
        template.carrier == Carrier::Unread
            && matches!(template.code.as_str(), "SACM-XMI-002" | "SACM-XMI-015")
            && template.id != "SACM-XMI-015/not-xmi-element"
            || template.id == "SACM-XMI-008/other-revision-root"
    }

    #[test]
    fn the_template_ids_are_unique_well_formed_and_prefixed_by_their_code() {
        let mut ids: Vec<&str> = TEMPLATES.iter().map(|template| template.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "LIB-API-017: a template id repeats");
        for template in TEMPLATES {
            assert!(
                is_template_id(template.id),
                "LIB-API-017: {} is not a template id",
                template.id
            );
            assert_eq!(
                template.id.split_once('/').map(|(code, _)| code),
                Some(template.code.as_str()),
                "LIB-API-017: {}: the prefix is not its code's text",
                template.id
            );
            assert_eq!(find(template.id).map(|found| found.id), Some(template.id));
        }
        assert!(find("SACM-XML-001").is_none() && find("SACM-XML-001/none").is_none());
        // Slice 0063 added 103 refusal templates; slice 0065 removed
        // SACM-XMI-014/expression-lang-string (SACM23-BASE-009); slice 0064
        // added 102 diagnostic templates (LIB-API-018); slice 0067 removed
        // the four refusals of unread kinds and ends and their four kept
        // forms (ADR 0035 amendment 1); slice 0068 added eight diagnostic
        // templates (SACM23-VAL-013, -014, -015, -023); slice 0072 added
        // eleven refusal and three diagnostic templates and removed
        // SACM-XMI-002/no-namespace (LIB-SEC-003, LIB-COMPAT-004,
        // LIB-COMPAT-016); slice 0073a removed the seven refusals and eight
        // kept forms of SACM-XMI-014 and added two refusal and seven
        // diagnostic templates (SACM23-XMI-019, SACM23-XMI-030; ADR 0042),
        // and, in its fix round, SACM-XMI-003/proxy-of-linked-root; slice
        // 0073b removed the four refusals and four kept forms of
        // SACM-XMI-007 (SACM23-XMI-029; ADR 0040 amendment 1), and, in its
        // fix round, added SACM-XMI-010/xsi-type-on-proxy; slice 0075a added
        // two refusal and three diagnostic templates (LIB-COMPAT-002; ADR
        // 0044); slice 0075b added one diagnostic template (LIB-COMPAT-017;
        // ADR 0044 D4); slice 0077 added one refusal template (LIB-SEC-007;
        // ADR 0041 amendment 2); slice 0079 added one diagnostic template
        // (LIB-COMPAT-020); slice 0081 added two diagnostic templates
        // (SACM23-XMI-032); slice 0082 added one refusal template
        // (LIB-API-013).
        assert_eq!(count, 217, "LIB-API-017: the number of templates");
        let diagnostics = TEMPLATES
            .iter()
            .filter(|template| template.carrier == Carrier::Diagnostic)
            .count();
        assert_eq!(diagnostics, 113, "LIB-API-018: the diagnostic templates");
        assert!(!is_template_id("SACM-XML-001"));
        assert!(!is_template_id("SACM-xml-001/a"));
        assert!(!is_template_id("SACM-XML-01/a"));
        assert!(!is_template_id("SACM-XML-001/A"));
        assert!(!is_template_id("SACM-XML-001/a--b"));
        assert!(!is_template_id("SACM-XML-001/-a"));
        assert!(is_template_id("SACM-XML-001/a-b2"));
    }

    #[test]
    fn each_template_const_is_named_after_its_id() {
        // The table's source: each entry's const name, then its id.
        let source = include_str!("template.rs");
        let body = source
            .split_once("templates! {")
            .and_then(|(_, rest)| rest.split_once("\n}\n"))
            .map_or("", |(body, _)| body);
        let mut entries = 0;
        for line in body.lines() {
            let Some((name, rest)) = line.trim().split_once(" = ") else {
                continue;
            };
            let Some(id) = rest
                .split(", \"")
                .nth(1)
                .and_then(|tail| tail.split('"').next())
            else {
                continue;
            };
            assert_eq!(
                name,
                const_name(id),
                "LIB-API-017: the const of {id} is not named after it"
            );
            assert!(find(id).is_some(), "LIB-API-017: {id} is not in TEMPLATES");
            entries += 1;
        }
        assert_eq!(entries, TEMPLATES.len(), "LIB-API-017: entries read");
    }

    #[test]
    fn every_placeholder_names_a_declared_argument_of_its_kind_in_order() {
        for template in TEMPLATES {
            let id = template.id;
            let mut first_seen: Vec<&str> = Vec::new();
            for (name, format) in placeholders(template.pattern) {
                let Some(&(_, kind)) = template
                    .arguments
                    .iter()
                    .find(|(declared, _)| *declared == name)
                else {
                    panic!("LIB-API-017: {id}: {{{name}}} names no declared argument");
                };
                // `{n}` takes Text, Prose or a Count in decimal, `{n:?}`
                // Text, `{n:04X}` a Count.
                let fits = matches!(
                    (format, kind),
                    ("" | "?", Kind::Text) | ("" | "04X", Kind::Count) | ("", Kind::Prose)
                );
                assert!(
                    fits,
                    "LIB-API-017: {id}: {{{name}:{format}}} takes no {kind:?}"
                );
                if !first_seen.contains(&name) {
                    first_seen.push(name);
                }
            }
            let declared: Vec<&str> = template.arguments.iter().map(|(name, _)| *name).collect();
            let rows = declared.iter().filter(|name| **name == "row").count();
            if template.carrier == Carrier::Unread {
                // An `Unread` template's `row` is named by its label, not its
                // pattern.
                let without_row: Vec<&str> = declared
                    .iter()
                    .copied()
                    .filter(|name| *name != "row")
                    .collect();
                assert_eq!(
                    without_row, first_seen,
                    "LIB-API-017: {id}: the arguments other than row are not those the pattern names, in the order it first names them"
                );
                if takes_row(template) {
                    assert_eq!(
                        template.arguments.last(),
                        Some(&("row", Kind::Text)),
                        "LIB-API-017: {id}: row is not the last argument"
                    );
                    assert_eq!(rows, 1, "LIB-API-017: {id}: row is declared more than once");
                } else {
                    assert_eq!(
                        rows, 0,
                        "LIB-API-017: {id}: only the Unsupported templates of SACM-XMI-002, -008 (other-revision-root) and -015 take row"
                    );
                }
                continue;
            }
            assert_eq!(
                declared, first_seen,
                "LIB-API-017: {id}: the arguments are not those the pattern names, in the order it first names them"
            );
            match template.carrier {
                Carrier::Diagnostic => {
                    // A diagnostic's `row` only where its pattern names it,
                    // and then last (LIB-API-018).
                    if rows > 0 {
                        assert_eq!(
                            (template.arguments.last(), rows),
                            (Some(&("row", Kind::Text)), 1),
                            "LIB-API-018: {id}: row is not declared once, last"
                        );
                    }
                }
                Carrier::Read(_) | Carrier::Write(_) | Carrier::Unread => assert_eq!(
                    rows, 0,
                    "LIB-API-017: {id}: a ReadError or WriteError template takes no row"
                ),
            }
        }
    }

    #[test]
    fn the_renderer_writes_text_debug_quoted_text_and_counts() {
        static SAMPLE: Template = Template {
            id: "SACM-XML-001/sample",
            code: crate::diagnostic::Code::XML_MALFORMED,
            carrier: Carrier::Read("Malformed"),
            arguments: &[("a", Kind::Text), ("n", Kind::Count)],
            pattern: "{a} {a:?} {n} U+{n:04X} {missing} {a:04X} {n:?} {b",
        };
        // `{p}` writes Prose as it is; `{p:?}` and `{p:04X}` take no Prose
        // and are written back as written (LIB-DIAG-004).
        static PROSE: Template = Template {
            id: "SACM-XML-001/prose-sample",
            code: crate::diagnostic::Code::XML_MALFORMED,
            carrier: Carrier::Read("Malformed"),
            arguments: &[("p", Kind::Prose)],
            pattern: "{p} {p:?} {p:04X}",
        };
        let arguments = [Argument::text("a", "x\"y"), Argument::count("n", 10)];
        assert_eq!(
            render(&SAMPLE, &arguments),
            "x\"y \"x\\\"y\" 10 U+000A {missing} {a:04X} {n:?} {b",
            "LIB-API-017: the renderer"
        );
        assert_eq!(
            render(
                &SAMPLE,
                &[Argument::text("a", ""), Argument::count("n", 0x1_0FFFF)]
            ),
            " \"\" 1114111 U+10FFFF {missing} {a:04X} {n:?} {b"
        );
        assert_eq!(
            render(&PROSE, &[Argument::prose("p", "\"=\"")]),
            "\"=\" {p:?} {p:04X}",
            "LIB-DIAG-004: the renderer writes Prose as it is"
        );
        let large = Argument::count_of("n", usize::MAX);
        assert_eq!(
            large.value(),
            &ArgumentValue::Count(u64::try_from(usize::MAX).unwrap_or(u64::MAX))
        );
    }
}
