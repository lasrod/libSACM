//! The message templates: each English sentence form a refusal's message is
//! made from, under a stable id, with the arguments it takes (LIB-API-017;
//! ADR 0030 amendment 3).
//!
//! One entry per template, in the fixed syntax that
//! `xtask/tests/message_templates.rs` parses and checks against the
//! `## Message templates` table of `docs/sacm/diagnostics.md`:
//!
//! ```text
//! NAME = CODE_CONSTANT, Carrier, "<CODE>/<form>",
//!     [Text("name"), Count("name")],
//!     "the English pattern, with {name}, {name:?} and {name:04X}";
//! ```
//!
//! The carrier is `Read("<Variant>")` for a `ReadError` variant,
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
    /// them; an `Unread` refusal's `row`, which the label names, comes last.
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
    SEC_002_UNHELD_OUTSIDE = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-outside",
        [Text("error")],
        "the content outside every element cannot be held in the document: {error}";
    SEC_002_UNHELD_ROOT = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-root",
        [Text("element"), Text("error")],
        "the root element {element:?} cannot be held in the document: {error}";
    SEC_002_UNHELD_ELEMENT = PARSER_LIMIT, Unread, "SACM-SEC-002/unheld-element",
        [Text("element"), Text("error")],
        "the element {element:?} cannot be held in the document: {error}";
    XML_002_BYTE_ORDER_MARK = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/byte-order-mark",
        [Text("encoding")],
        "the input starts with a {encoding} byte order mark, and only UTF-8, ASCII and UTF-16 are read";
    XML_002_DETECTED_ENCODING = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/detected-encoding",
        [Text("encoding")],
        "the input is encoded in {encoding}, and only UTF-8, ASCII and UTF-16 starting with U+FEFF are read";
    XML_002_DECLARED_ENCODING = XML_ENCODING_NOT_READ, Unread, "SACM-XML-002/declared-encoding",
        [Text("encoding")],
        "the declared encoding {encoding:?} is not supported: without a UTF-16 byte order mark, only UTF-8 and ASCII are read";
    XML_003_UNREPRESENTABLE_CHARACTER = XML_UNREPRESENTABLE_CHARACTER, Write("UnrepresentableCharacter"), "SACM-XML-003/unrepresentable-character",
        [Text("element"), Text("attribute"), Count("code_point")],
        "the element {element:?} cannot be written: its {attribute} holds U+{code_point:04X}, which XML 1.0 cannot represent";
    XMI_001_NOT_A_PACKAGE = XMI_INVALID_ROOT, Read("InvalidRoot"), "SACM-XMI-001/not-a-package",
        [Text("element")],
        "the root element {element:?} is not a SACM package";
    XMI_001_NO_PACKAGE = XMI_INVALID_ROOT, Read("NoPackage"), "SACM-XMI-001/no-package",
        [],
        "the XMI element holds no SACM package";
    XMI_001_NO_ROOT_TO_WRITE = XMI_INVALID_ROOT, Write("NoRoot"), "SACM-XMI-001/no-root-to-write",
        [],
        "the document has no root to write";
    XMI_002_OTHER_NAMESPACE = XMI_NAMESPACE_NOT_PINNED, Unread, "SACM-XMI-002/other-namespace",
        [Text("element"), Text("namespace"), Text("row")],
        "the root element {element:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"";
    XMI_002_NO_NAMESPACE = XMI_NAMESPACE_NOT_PINNED, Unread, "SACM-XMI-002/no-namespace",
        [Text("element"), Text("row")],
        "the root element {element:?} is in no namespace, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"";
    XMI_003_UNREAD_ATTRIBUTE = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/unread-attribute",
        [Text("attribute"), Text("element")],
        "the attribute {attribute:?} of the element {element:?} is not read in this release";
    XMI_003_WRAPPER_TEXT = XMI_UNKNOWN_CONTENT, Unread, "SACM-XMI-003/wrapper-text",
        [Text("element")],
        "the element {element:?} holds text, which has no element to be kept in";
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
    XMI_007_UNREAD_ATTRIBUTE = XMI_EXTERNAL_HREF, Unread, "SACM-XMI-007/unread-attribute",
        [Text("attribute"), Text("element"), Text("row")],
        "the attribute {attribute:?} of the element {element:?} is not read in this release";
    XMI_007_PROXY_CHILD = XMI_EXTERNAL_HREF, Unread, "SACM-XMI-007/proxy-child",
        [Text("element"), Text("container"), Text("class"), Text("row")],
        "the element {element:?} in the element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release";
    XMI_007_PROXY_CHILD_OF_ROOT = XMI_EXTERNAL_HREF, Unread, "SACM-XMI-007/proxy-child-of-root",
        [Text("element"), Text("container"), Text("class"), Text("row")],
        "the element {element:?} in the root element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release";
    XMI_007_HREF = XMI_EXTERNAL_HREF, Unread, "SACM-XMI-007/href",
        [Text("href"), Text("element"), Text("row")],
        "the href {href:?} of the reference element {element:?} names no element of this document by its id; only href=\"#id\" is read in this release";
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
    XMI_014_XMI_ROOT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/xmi-root",
        [Text("element"), Text("row")],
        "the XMI element {element:?} is not read as a root in this release";
    XMI_014_UNREAD_ATTRIBUTE = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/unread-attribute",
        [Text("attribute"), Text("element"), Text("row")],
        "the attribute {attribute:?} of the element {element:?} is not read in this release";
    XMI_014_NESTED_XMI_ELEMENT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/nested-xmi-element",
        [Text("element"), Text("container"), Text("row")],
        "the XMI element {element:?} in {container:?} is not read in this release";
    XMI_014_INTERFACE_ELEMENT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/interface-element",
        [Text("element"), Text("container"), Text("class"), Text("row")],
        "the reference element {element:?} in the element {container:?} is the {class} end \"interface\", which is not read in this release";
    XMI_014_UNREAD_ROOT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/unread-root",
        [Text("element"), Text("row")],
        "the root element {element:?} is not read in this release";
    XMI_014_XMI_CHILD = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/xmi-child",
        [Text("element"), Text("container"), Text("row")],
        "the XMI element {element:?} in the element {container:?} is not read in this release";
    XMI_014_XMI_CHILD_OF_ROOT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/xmi-child-of-root",
        [Text("element"), Text("container"), Text("row")],
        "the XMI element {element:?} in the root element {container:?} is not read in this release";
    XMI_014_PROXY_CHILD = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/proxy-child",
        [Text("element"), Text("container"), Text("class"), Text("row")],
        "the element {element:?} in the element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release";
    XMI_014_PROXY_CHILD_OF_ROOT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/proxy-child-of-root",
        [Text("element"), Text("container"), Text("class"), Text("row")],
        "the element {element:?} in the root element {container:?} is in reference form, a proxy of an element of the class {class}, which is not read in this release";
    XMI_014_UNREAD_KIND = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/unread-kind",
        [Text("class"), Text("element"), Text("container"), Text("row")],
        "the {class} {element:?} in the element {container:?} is not read in this release";
    XMI_014_UNREAD_KIND_IN_ROOT = XMI_UNREAD_CONTENT, Unread, "SACM-XMI-014/unread-kind-in-root",
        [Text("class"), Text("element"), Text("container"), Text("row")],
        "the {class} {element:?} in the root element {container:?} is not read in this release";
    XMI_015_NOT_XMI_NAMESPACE = XMI_WRAPPER_NAMESPACE, Unread, "SACM-XMI-015/not-xmi-namespace",
        [Text("element"), Text("row")],
        "the element {element:?} is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element";
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
        [Text("expected")],
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
        [Text("expected")],
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
/// placeholder written from the argument it names. `{n}` writes a Text as it
/// is and a Count in decimal, `{n:?}` a Text Debug-quoted, and `{n:04X}` a
/// Count as at least four uppercase hex digits. A placeholder that names no
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
            (Some(ArgumentValue::Text(text)), "") => out.write_str(text),
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
            && matches!(
                template.code.as_str(),
                "SACM-XMI-014" | "SACM-XMI-007" | "SACM-XMI-002" | "SACM-XMI-015"
            )
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
        // SACM-XMI-014/expression-lang-string (SACM23-BASE-009).
        assert_eq!(count, 102, "LIB-API-017: the number of templates");
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
                // `{n}` takes Text or a Count in decimal, `{n:?}` Text,
                // `{n:04X}` a Count.
                let fits = matches!(
                    (format, kind),
                    ("" | "?", Kind::Text) | ("" | "04X", Kind::Count)
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
            let without_row: Vec<&str> = declared
                .iter()
                .copied()
                .filter(|name| *name != "row")
                .collect();
            assert_eq!(
                without_row, first_seen,
                "LIB-API-017: {id}: the arguments other than row are not those the pattern \
                 names, in the order it first names them"
            );
            if takes_row(template) {
                assert_eq!(
                    template.arguments.last(),
                    Some(&("row", Kind::Text)),
                    "LIB-API-017: {id}: row is not the last argument"
                );
                assert_eq!(declared.iter().filter(|name| **name == "row").count(), 1);
            } else {
                assert!(
                    !declared.contains(&"row"),
                    "LIB-API-017: {id}: only the Unsupported templates of SACM-XMI-014, -007, \
                     -002 and -015 take row"
                );
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
        let large = Argument::count_of("n", usize::MAX);
        assert_eq!(
            large.value(),
            &ArgumentValue::Count(u64::try_from(usize::MAX).unwrap_or(u64::MAX))
        );
    }
}
