//! The interchange roots of an XMI document, their names and the package
//! content under them (SACM23-XMI-001, SACM23-XMI-005, SACM23-XMI-012,
//! SACM23-XMI-014, SACM23-XMI-015, SACM23-XMI-016, SACM23-XMI-017,
//! SACM23-BASE-004, -005, -006, -007, -008, SACM23-ARG-004 to -013,
//! SACM23-ART-001, -003, -008, -009, -011; formal/23-05-08 §2.2, §2.4,
//! §8.2-§8.12, §11.2-§11.18, §12.2, §12.3, §12.6, §12.8, §12.12, §12.13; XMI
//! 2.5.1 §7.5.3, §7.6.3, §7.8.4, §7.8.6, §7.10; ADR 0015, ADR 0016, ADR 0021,
//! ADR 0025, ADR 0026, ADR 0027, ADR 0028, ADR 0034).
//!
//! An interchange root is the document element or, when that is the XMI
//! element, each of its element children outside the XMI namespace. The
//! roots are read in two passes over a tree roxmltree has accepted:
//!
//! 1. Every root is classified by its local name alone. An `XMI` element
//!    outside the XMI namespace, or any other XMI-namespace element, as the
//!    document element is `Unsupported`. An XMI element with no root is
//!    `NoPackage`; otherwise the first root that is not one of the twelve
//!    package kinds is `InvalidRoot` (both SACM-XMI-001). This pass comes
//!    first, so SACM-XMI-001 takes precedence among the refusals found while
//!    reading the roots, and which inputs get it does not depend on what this
//!    release can read (ADR 0016 D3).
//! 2. In document order, the XMI element's attributes, then each root and
//!    whatever is beside it, are checked, and each root is added to the
//!    document with its tree. A root's namespace and kind, its `xmi:id` (and
//!    whether an earlier element has it) and its type are checked first, all
//!    located at the root, then its other attributes, then its content in
//!    document order. A `name` is read into its element's name
//!    (SACM23-XMI-012). A child element is checked the same way, at its start
//!    tag first: its role, its type and kind, and its `xmi:id`
//!    (SACM23-XMI-016); then its attributes, of which an Assertion's
//!    `assertionDeclaration` is read (SACM23-XMI-017), and its content.
//!    Content that does not conform where it stands is kept as preserved
//!    content of the element it sits in, with one SACM-XMI-003 per construct
//!    (LIB-COMPAT-003, ADR 0033): an attribute, a child with its subtree,
//!    text, or a `name`, `content`, `key`, value or reference element that
//!    does not conform inside, kept whole. A part is read whole first, and
//!    what it reported and the ids it took are given back if it is
//!    preserved after all. In strict mode the first conforming thing this
//!    release does not read is `Unsupported`, at its location, labelled by
//!    its entry of `refusal::Unread` (LIB-API-011); nothing is dropped (ADR
//!    0015, D-PRESERVE).
//!
//! In tolerant mode, every strict refusal but the fatal ones (malformed or
//! unsafe input, a root that is no SACM 2.3 package, a repeated `xmi:id`,
//! an encoding and a limit) is a [`Fault::Keep`]: the construct it is found
//! in is kept as preserved content, with one Warning at the strict
//! refusal's location (ADR 0035 D1). The construct is the attribute, for one
//! on a start tag; the whole part, for one inside a `name`, `content`,
//! `key`, value or reference element; the whole child, for a child whose
//! kind or form is not read; and the later occurrence, for a repeat. A value
//! kept because it is no literal counts as its property's occurrence, so a
//! later one is a repeat. An empty `xmi:id` is read as none, with a
//! generated id for an element. What `xmi:XMI` carries and holds besides its
//! roots is kept as the document's own preserved content, placed among the
//! roots (ADR 0035 D3); every package kind is read as a root (ADR 0035
//! amendment 1).
//!
//! Before pass 2, every `xmi:id` value in the input is collected, so that the
//! id generated for an element without one is no id written anywhere in the
//! file, even later (SACM23-XMI-020; ADR 0024). An id's value is its
//! `xsd:ID` value: XML white space around an `NCName` is trimmed, with
//! SACM-ID-004 Info (SACM23-XMI-025), and any other id is held as written.
//! Element ids and the ids of names, `content`s, `key`s and `value`s share
//! one id space (SACM23-XMI-022).
//!
//! The `UtilityElement`s of every readable element are read as its children,
//! each with its `content` and, on a `TaggedValue`, its `key`: a role element
//! holding one `value` per `LangString`, each read like a `name`, with the
//! `xmi:id`s of both kept by role and position (SACM23-BASE-004,
//! SACM23-BASE-005; ADR 0025).
//!
//! The name and text shapes of the assurance-forge C++ library that map one
//! to one onto a `LangString` or a `MultiLangString` are read as their
//! standard form, each with one SACM-XMI-012 Warning (LIB-COMPAT-008; ADR
//! 0022 amendment 1): `name="…"` on a `ModelElement`, text as the `content` of
//! a `name` or `value`, a `name` holding one `value` or one `content`
//! carrying `lang`, the single-language shorthand of a `content` or `key`
//! (`lang`/`content` attributes or text, with no `value`), `lang=`,
//! `content=` or text on a `UtilityElement`, and `key=` and `value=` on a
//! `TaggedValue`. A strict read reads them the same way and then, if the rest
//! of the input read without a refusal, refuses the first one, so that every
//! refusal both modes share comes first. A field given both in a shape and
//! in the standard form, or twice, is `RepeatedValue`.
//!
//! `xsi:nil` on a value element is an `xsd:boolean` (SACM23-XMI-026; XMI
//! 2.5.1 §9.4.1): true reads as if the property were not written, which
//! still counts as an occurrence, false reads the content, and a nil element
//! holds no characters or elements.
//!
//! A name's `lang` and `content` (and those of a `value`), every element's
//! `gid`, `isCitation` and `isAbstract`, an Assertion's
//! `assertionDeclaration` and an asserted relationship's `isCounter` are
//! read from the attribute or from a value element: an element in no
//! namespace named after the property, whose character content is the value
//! (SACM23-XMI-014, SACM23-BASE-006, SACM23-BASE-007, SACM23-BASE-008; XMI
//! 2.5.1 §7.8.4; ADR 0021 amendment 1). A flag is an `xsd:boolean`. The
//! reference ends of the element's kind (every element's `citedElement` and
//! `abstractForm`, and the argumentation ends of ADR 0027) are read from the
//! attribute, whose value is the target ids separated by XML white space
//! (SACM23-XMI-005), or from reference elements with one `xmi:idref` or
//! `href="#id"` each (SACM23-XMI-015), and held as ids, resolved or not (ADR
//! 0026). A multi-valued end (`metaClaim`, `source`,
//! `referencedArtifactElement`, a group's `argumentElement` or
//! `artifactElement`) holds every token of the attribute, or one id per
//! element, in document order. A flag or end the kind lacks is not read. An
//! `argumentElement` element is a reference element under an
//! `ArgumentGroup` and a child under an `ArgumentPackage`, and an
//! `artifactElement` element one under an `ArtifactGroup` and a child under
//! an `ArtifactPackage`: the parent's kind decides, and no class has both
//! (ADR 0028, ADR 0034), so an element nested in a group's end is refused as
//! a reference element would be, never read as a child. The id pre-pass also collects every id a reference may name, so
//! that no generated id makes a dangling reference resolve. A property given
//! more than once, a second `name`, `content` or `key` included, is
//! `RepeatedValue`
//! (SACM-MULT-001; XMI 2.5.1 §9.5.2 rule 2h; SACM23-XMI-021), and a
//! declaration or flag that is not a literal is `InvalidLiteral`
//! (SACM-ENUM-001, SACM23-VAL-006; ADR 0021 amendment 2, ADR 0026).
//!
//! Names are matched by namespace URI and local name, and a type value is
//! resolved as a `QName` against the namespaces in scope, so the choice of
//! prefix does not matter (SACM23-XMI-002). An element under `xmlns=""` is
//! in no namespace.
//!
//! The XMI element itself may carry only namespace declarations and
//! `xmi:version`, and hold, besides its roots, only white space, comments and
//! processing instructions; it is no element of the document, so a strict
//! read refuses anything else on it (a defect, or XMI metadata not read
//! yet), and a tolerant read keeps it as the document's preserved content.
//! An `xmi:version`, here or on a root, is ignored
//! with any value: the XMI namespace identifies the version (XMI 2.5.1
//! §7.5.3), and the writer does not write it (SACM23-XMI-024).
//!
//! Two attributes are read and reported (LIB-COMPAT-001, ADR 0022), one
//! diagnostic per attribute at its name: an `xsi:type` is SACM-XMI-010, an
//! Error in strict mode and a Warning in tolerant mode, and an `xmi:version`
//! is SACM-XMI-011, Info. A strict read refuses the C++ shapes above and
//! what a tolerant read keeps. A
//! message names the element by its kind and id, never by the name or prefix
//! written, so the choice of prefix does not change it (SACM23-XMI-002).

use std::collections::BTreeSet;

use roxmltree::{Attribute, Node};

use super::input::{Locator, is_xml_space, locate};
use super::limits::Budget;
use super::namespaces::{
    self, Classes, RootNamespace, SACM_NS, SACM_PREFIX, Types, XMI_NS, XMI_PREFIX,
};
use super::preserve;
use super::refusal::{Tolerant, Unread, unsupported};
use super::{Limits, Mode, ReadError};
use crate::diagnostic::template::{self, Template};
use crate::diagnostic::{Argument, Code, Details, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, Attr, DeclaredType, Document, ElementId, ElementKind,
    Flag, Href, IdGenerator, Item, LangString, Placed, Preserved, ReferenceEnd, Role,
    StringAttribute, Text, TextRole, flag_named, is_ncname, reference_end_named, role_named,
};

/// The machine-readable model's spelling of the class
/// `ArtifactAssetRelationship` (§12.14, D-TEXT-MODEL), which a tolerant read
/// reads as a type (SACM23-XMI-009).
const MODEL_SPELLING: &str = "ArtifactAssertedRelationship";

/// The §11.2 and §11.4 text spelling of `argumentElement`, which a tolerant
/// read reads as the role of an ArgumentPackage and the reference end of an
/// `ArgumentGroup` (SACM23-XMI-009).
const TEXT_SPELLING: &str = "argumentationElement";

/// The name `TEXT_SPELLING` stands for: the model's `argumentElement`.
const TEXT_RESOLVED: &str = "argumentElement";

/// The machine-readable model's spelling of the attribute `date` of an
/// `Event` (§12.9, D-TEXT-MODEL, spec defect D17), which a tolerant read
/// reads as an attribute or a value element (SACM23-XMI-009).
const EVENT_SPELLING: &str = "occurece";

/// The SACM 2.0 to 2.2 spelling of the attribute `date` of an `Event`, in
/// their `emof.xml` (`Event.occurence`), which a tolerant read reads as the
/// date in a root of an older revision (LIB-COMPAT-004, ADR 0041 D5).
const OLDER_EVENT_SPELLING: &str = "occurence";

/// The row of the reading of other SACM namespaces (ADR 0041).
const OTHER_NAMESPACE_ROW: &str = "LIB-COMPAT-004";

/// The §10.4 text spelling of `terminologyElement`, which a tolerant read
/// reads as the role of a TerminologyPackage (SACM23-XMI-028, spec defect
/// D9).
const TERMINOLOGY_SPELLING: &str = "TerminologyElement";

/// The name `TERMINOLOGY_SPELLING` stands for: the model's
/// `terminologyElement`.
const TERMINOLOGY_RESOLVED: &str = "terminologyElement";

/// A spelling where formal/23-05-08 and ptc/22-03-13 disagree and the crate
/// follows the other (D-TEXT-MODEL): what a tolerant read reads as the
/// resolved name, and a strict read keeps as preserved content, each with one
/// SACM-XMI-013 (SACM23-XMI-009, ADR 0034 amendment 2; SACM23-XMI-028,
/// amendment 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spelling {
    /// The type `ArtifactAssertedRelationship` (§12.14).
    Type,
    /// An Event's `occurece` (§12.9 `date`).
    Date,
    /// `argumentationElement` as an ArgumentPackage's role (§11.4).
    Role,
    /// `argumentationElement` as an `ArgumentGroup`'s end (§11.2).
    End,
    /// `TerminologyElement` as a TerminologyPackage's role (§10.4).
    TerminologyRole,
}

impl Spelling {
    /// The name it is read as, which `xmi::write` writes.
    const fn resolved(self) -> &'static str {
        match self {
            Self::Type => "ArtifactAssetRelationship",
            Self::Date => "date",
            Self::Role | Self::End => TEXT_RESOLVED,
            Self::TerminologyRole => TERMINOLOGY_RESOLVED,
        }
    }

    /// The name as written: the text's or the model's spelling.
    const fn written(self) -> &'static str {
        match self {
            Self::Type => MODEL_SPELLING,
            Self::Date => EVENT_SPELLING,
            Self::Role | Self::End => TEXT_SPELLING,
            Self::TerminologyRole => TERMINOLOGY_SPELLING,
        }
    }

    /// The requirement of its diagnostics (SACM-XMI-013).
    const fn row(self) -> &'static str {
        match self {
            Self::TerminologyRole => "SACM23-XMI-028",
            Self::Type | Self::Date | Self::Role | Self::End => "SACM23-XMI-009",
        }
    }

    /// Whether it names a role of the parent, so that its diagnostic affects
    /// the parent rather than the child.
    const fn is_role(self) -> bool {
        matches!(self, Self::Role | Self::TerminologyRole)
    }

    /// The clause of the text that names it.
    const fn clause(self) -> &'static str {
        match self {
            Self::Type => "12.14",
            Self::Date => "12.9",
            Self::Role => "11.4",
            Self::End => "11.2",
            Self::TerminologyRole => "10.4",
        }
    }

    /// The templates of its diagnostic: the Warning of a tolerant read, which
    /// reads it, and the Error of a strict read, which keeps it
    /// (SACM-XMI-013). `in_root` says whether the element a role or a type
    /// is spelled in is the root element.
    fn templates(self, in_root: bool) -> (&'static Template, &'static Template) {
        match (self, in_root) {
            (Self::End, _) => (
                &template::XMI_013_GROUP_END_READ,
                &template::XMI_013_GROUP_END_KEPT,
            ),
            (Self::Date, _) => (
                &template::XMI_013_EVENT_DATE_READ,
                &template::XMI_013_EVENT_DATE_KEPT,
            ),
            (Self::Role, false) => (
                &template::XMI_013_PACKAGE_ROLE_READ,
                &template::XMI_013_PACKAGE_ROLE_KEPT,
            ),
            (Self::Role, true) => (
                &template::XMI_013_PACKAGE_ROLE_IN_ROOT_READ,
                &template::XMI_013_PACKAGE_ROLE_IN_ROOT_KEPT,
            ),
            (Self::TerminologyRole, false) => (
                &template::XMI_013_TERMINOLOGY_ROLE_READ,
                &template::XMI_013_TERMINOLOGY_ROLE_KEPT,
            ),
            (Self::TerminologyRole, true) => (
                &template::XMI_013_TERMINOLOGY_ROLE_IN_ROOT_READ,
                &template::XMI_013_TERMINOLOGY_ROLE_IN_ROOT_KEPT,
            ),
            (Self::Type, false) => (
                &template::XMI_013_MODEL_TYPE_READ,
                &template::XMI_013_MODEL_TYPE_KEPT,
            ),
            (Self::Type, true) => (
                &template::XMI_013_MODEL_TYPE_IN_ROOT_READ,
                &template::XMI_013_MODEL_TYPE_IN_ROOT_KEPT,
            ),
        }
    }
}

/// A spelling met in the input, with the arguments its message is made
/// from (SACM-XMI-013).
struct Spelled {
    spelling: Spelling,
    /// Whether a role or type is spelled in the root element.
    in_root: bool,
    arguments: Vec<Argument>,
}

impl Spelled {
    /// `spelling`, an end or an attribute, on the element written
    /// `element`.
    fn on(spelling: Spelling, element: &str) -> Self {
        Self {
            spelling,
            in_root: false,
            arguments: vec![Argument::text("element", element)],
        }
    }

    /// `spelling`, a role or a type, of the child written `element` in the
    /// element written `container`, the root element if `in_root`.
    fn of_child(spelling: Spelling, element: &str, container: &str, in_root: bool) -> Self {
        Self {
            spelling,
            in_root,
            arguments: vec![
                Argument::text("element", element),
                Argument::text("container", container),
            ],
        }
    }

    /// The template of the Warning of a tolerant read.
    fn read(&self) -> &'static Template {
        self.spelling.templates(self.in_root).0
    }

    /// The template of the Error of a strict read.
    fn kept(&self) -> &'static Template {
        self.spelling.templates(self.in_root).1
    }
}

/// The `XLink` namespace, whose attributes on a reference element are a
/// cross-document link not read yet (SACM23-XMI-029).
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// The requirement of the diagnostics for preserved content.
const PRESERVED_ROW: &str = "LIB-COMPAT-003";

/// The requirement of the Warnings for content a tolerant read keeps where
/// a strict read refuses it (ADR 0035 D1).
const TOLERANT_ROW: &str = "LIB-COMPAT-001";

/// The requirement of the Warnings for conforming content not read yet
/// (ADR 0035 D2).
const UNREAD_ROW: &str = "LIB-API-011";

/// Why a part of the input is not read: a refusal, content that does not
/// conform, which the caller keeps as preserved content (ADR 0033), or, in
/// tolerant mode, content a strict read refuses, which the caller keeps with
/// a Warning (ADR 0035 D1).
enum Fault {
    /// The input is refused.
    Refused(ReadError),
    /// The part does not conform: the caller preserves the whole of it.
    Defect,
    /// A tolerant read keeps the construct as preserved content, with this
    /// Warning in place of the strict refusal.
    Keep(Keep),
}

impl From<ReadError> for Fault {
    fn from(error: ReadError) -> Self {
        Self::Refused(error)
    }
}

/// The arguments `arguments`, all Text, by name and value, in order.
fn text_arguments(arguments: &[(&'static str, &str)]) -> Vec<Argument> {
    arguments
        .iter()
        .map(|&(name, value)| Argument::text(name, value))
        .collect()
}

/// The details of a message of `template` whose arguments are all Text:
/// `arguments`, by name and value, in the template's order.
fn text_details(template: &'static Template, arguments: &[(&'static str, &str)]) -> Details {
    Details::new(template, text_arguments(arguments))
}

/// The arguments that name the element `id` of `kind`: its class and id.
fn named(kind: ElementKind, id: &str) -> Vec<Argument> {
    vec![
        Argument::text("class", kind.name()),
        Argument::text("id", id),
    ]
}

/// A C++ shape of a name or a text (SACM-XMI-012, LIB-COMPAT-008): its
/// refusal's template, and the template of the Warning a read gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    NameAttribute,
    ContentOnElement,
    KeyAttribute,
    Shorthand,
    NameText,
    ValueText,
    NameValueText,
    NameContentLang,
    NameValue,
}

impl Shape {
    /// Every shape.
    #[cfg(test)]
    const ALL: [Self; 9] = [
        Self::NameAttribute,
        Self::ContentOnElement,
        Self::KeyAttribute,
        Self::Shorthand,
        Self::NameText,
        Self::ValueText,
        Self::NameValueText,
        Self::NameContentLang,
        Self::NameValue,
    ];

    /// The template of the strict read's refusal.
    fn refusal(self) -> &'static Template {
        match self {
            Self::NameAttribute => &template::XMI_012_NAME_ATTRIBUTE,
            Self::ContentOnElement => &template::XMI_012_CONTENT_ON_ELEMENT,
            Self::KeyAttribute => &template::XMI_012_KEY_ATTRIBUTE,
            Self::Shorthand => &template::XMI_012_SHORTHAND,
            Self::NameText => &template::XMI_012_NAME_TEXT,
            Self::ValueText => &template::XMI_012_VALUE_TEXT,
            Self::NameValueText => &template::XMI_012_NAME_VALUE_TEXT,
            Self::NameContentLang => &template::XMI_012_NAME_CONTENT_LANG,
            Self::NameValue => &template::XMI_012_NAME_VALUE,
        }
    }

    /// The template of the Warning: the refusal's sentence, then that the
    /// shape is read as its standard form.
    fn read(self) -> &'static Template {
        match self {
            Self::NameAttribute => &template::XMI_012_NAME_ATTRIBUTE_READ,
            Self::ContentOnElement => &template::XMI_012_CONTENT_ON_ELEMENT_READ,
            Self::KeyAttribute => &template::XMI_012_KEY_ATTRIBUTE_READ,
            Self::Shorthand => &template::XMI_012_SHORTHAND_READ,
            Self::NameText => &template::XMI_012_NAME_TEXT_READ,
            Self::ValueText => &template::XMI_012_VALUE_TEXT_READ,
            Self::NameValueText => &template::XMI_012_NAME_VALUE_TEXT_READ,
            Self::NameContentLang => &template::XMI_012_NAME_CONTENT_LANG_READ,
            Self::NameValue => &template::XMI_012_NAME_VALUE_READ,
        }
    }
}

/// Content a tolerant read keeps in an element where a strict read refuses
/// it, by the refusal's template: the element column of the (reason,
/// holder) table of the kept forms (ADR 0035 D1, D2). Each reason has its
/// refusal and its kept form, so none lacks a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementReason {
    /// SACM-XMI-007/unread-attribute.
    ExternalAttribute,
    /// SACM-XMI-007/proxy-child.
    ExternalProxyChild,
    /// SACM-XMI-007/proxy-child-of-root.
    ExternalProxyChildOfRoot,
    /// SACM-XMI-007/href.
    Href,
}

impl ElementReason {
    /// Every reason.
    #[cfg(test)]
    const ALL: [Self; 4] = [
        Self::ExternalAttribute,
        Self::ExternalProxyChild,
        Self::ExternalProxyChildOfRoot,
        Self::Href,
    ];

    /// The template of the strict read's refusal.
    fn refusal(self) -> &'static Template {
        match self {
            Self::ExternalAttribute => &template::XMI_007_UNREAD_ATTRIBUTE,
            Self::ExternalProxyChild => &template::XMI_007_PROXY_CHILD,
            Self::ExternalProxyChildOfRoot => &template::XMI_007_PROXY_CHILD_OF_ROOT,
            Self::Href => &template::XMI_007_HREF,
        }
    }

    /// The template of the tolerant read's Warning, which names the element
    /// that holds the content.
    fn kept(self) -> &'static Template {
        match self {
            Self::ExternalAttribute => &template::XMI_007_UNREAD_ATTRIBUTE_KEPT,
            Self::ExternalProxyChild => &template::XMI_007_PROXY_CHILD_KEPT,
            Self::ExternalProxyChildOfRoot => &template::XMI_007_PROXY_CHILD_OF_ROOT_KEPT,
            Self::Href => &template::XMI_007_HREF_KEPT,
        }
    }
}

/// Content a tolerant read keeps as the document's preserved content where
/// a strict read refuses it, by the refusal's template: the document column
/// of the (reason, holder) table of the kept forms (ADR 0035 D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocumentReason {
    /// SACM-XMI-003/unread-attribute, on the XMI element.
    Attribute,
    /// SACM-XMI-003/wrapper-text.
    Text,
    /// SACM-XMI-003/wrapper-element: an XMI-namespace element in the XMI
    /// element that XMI 2.5.1 does not define there (SACM23-XMI-019).
    Element,
}

impl DocumentReason {
    /// Every reason.
    #[cfg(test)]
    const ALL: [Self; 3] = [Self::Attribute, Self::Text, Self::Element];

    /// The template of the strict read's refusal.
    fn refusal(self) -> &'static Template {
        match self {
            Self::Attribute => &template::XMI_003_UNREAD_ATTRIBUTE,
            Self::Text => &template::XMI_003_WRAPPER_TEXT,
            Self::Element => &template::XMI_003_WRAPPER_ELEMENT,
        }
    }

    /// The template of the tolerant read's Warning, which names the
    /// document.
    fn kept(self) -> &'static Template {
        match self {
            Self::Attribute => &template::XMI_003_WRAPPER_ATTRIBUTE_KEPT,
            Self::Text => &template::XMI_003_WRAPPER_TEXT_KEPT,
            Self::Element => &template::XMI_003_WRAPPER_ELEMENT_KEPT,
        }
    }
}

/// The requirement and clause of the Warning for content kept with `code`
/// where a strict read refuses it: LIB-COMPAT-003, clause 7.3.3, for content
/// that does not conform (SACM-XMI-003), and LIB-API-011 for conforming
/// content not read yet (ADR 0035 D1, D2).
fn kept_row(code: Code) -> (&'static str, Option<&'static str>) {
    if code == Code::XMI_UNKNOWN_CONTENT {
        (PRESERVED_ROW, Some("7.3.3"))
    } else {
        (UNREAD_ROW, None)
    }
}

/// The Warning a tolerant read gives for a construct it keeps as preserved
/// content of an element where a strict read refuses it (ADR 0035 D1, D2):
/// at the strict refusal's byte offset. Or the Error a strict read gives for
/// a spelling it keeps (SACM-XMI-013, SACM23-XMI-009).
struct Keep {
    /// Where the strict refusal is.
    offset: usize,
    /// A Warning, but an Error for a spelling a strict read keeps.
    severity: Severity,
    /// The matrix row the Warning enforces.
    requirement: &'static str,
    clause: Option<&'static str>,
    /// The template of the message.
    template: &'static Template,
    /// The arguments before the holder's.
    arguments: Vec<Argument>,
    /// Whether the message names the element that holds the construct: its
    /// class and id follow `arguments`. A spelling's names none.
    names_holder: bool,
    /// The row that reads the construct, the last argument, if it has one.
    row: Option<&'static str>,
    /// Whether the construct is a value that is no literal, which counts as
    /// an occurrence of its property (ADR 0035, SACM23-VAL-006).
    is_literal: bool,
}

impl Keep {
    /// The Warning of `template`, made from `arguments` and the holder's
    /// class and id, at byte `offset`, enforcing `requirement` and
    /// `clause`.
    fn held(
        offset: usize,
        requirement: &'static str,
        clause: Option<&'static str>,
        template: &'static Template,
        arguments: Vec<Argument>,
    ) -> Self {
        Self {
            offset,
            severity: Severity::Warning,
            requirement,
            clause,
            template,
            arguments,
            names_holder: true,
            row: None,
            is_literal: false,
        }
    }

    /// The Warning of `unread`, kept in an element for `reason` at byte
    /// `offset`, whose refusal is made from `arguments`.
    fn unread(
        offset: usize,
        unread: Unread,
        reason: ElementReason,
        arguments: Vec<Argument>,
    ) -> Self {
        let template = reason.kept();
        let (requirement, clause) = kept_row(template.code);
        Self {
            row: unread.refusal().row(),
            ..Self::held(offset, requirement, clause, template, arguments)
        }
    }

    /// The Warning of the value `value` of `property`, given at byte
    /// `offset`, that is no literal (SACM-ENUM-001, SACM23-VAL-006).
    fn literal(offset: usize, property: &str, value: &str) -> Self {
        let (template, arguments) = if property == DECLARATION {
            (
                &template::ENUM_001_DECLARATION_KEPT,
                vec![Argument::text("value", value)],
            )
        } else {
            (
                &template::ENUM_001_BOOLEAN_KEPT,
                vec![
                    Argument::text("property", property),
                    Argument::text("value", value),
                ],
            )
        };
        Self {
            is_literal: true,
            ..Self::held(offset, "SACM23-VAL-006", None, template, arguments)
        }
    }

    /// The Warning of an occurrence of the property `property` of the
    /// element `owner` at byte `offset`, after its first (SACM-MULT-001;
    /// XMI 2.5.1 §9.5.2 rule 2h).
    fn repeat(offset: usize, owner: &ElementId, property: &str) -> Self {
        Self::held(
            offset,
            TOLERANT_ROW,
            Some("9.5.2"),
            &template::MULT_001_REPEAT_KEPT,
            vec![
                Argument::text("property", property),
                Argument::text("element", owner.as_str()),
            ],
        )
    }

    /// The Error of a strict read for `spelled` at byte `offset`, which it
    /// keeps as preserved content (SACM-XMI-013, SACM23-XMI-009; ADR 0034
    /// amendment 2). Its message names no holder.
    fn spelling(offset: usize, spelled: Spelled) -> Self {
        Self {
            offset,
            severity: Severity::Error,
            requirement: spelled.spelling.row(),
            clause: Some(spelled.spelling.clause()),
            template: spelled.kept(),
            arguments: spelled.arguments,
            names_holder: false,
            row: None,
            is_literal: false,
        }
    }

    /// The diagnostic, for the holder `holder`, with the byte offset it is
    /// to be located at.
    fn into_diagnostic(self, holder: &ElementHolder) -> (usize, Diagnostic) {
        let Self {
            offset,
            severity,
            requirement,
            clause,
            template,
            mut arguments,
            names_holder,
            row,
            is_literal: _,
        } = self;
        if names_holder {
            arguments.extend(holder.arguments());
        }
        arguments.extend(row.map(|row| Argument::text("row", row)));
        let mut diagnostic = Diagnostic::emit(template, severity, requirement, arguments)
            .set_affected([holder.id.clone()]);
        if let Some(clause) = clause {
            diagnostic = diagnostic.set_clause(clause);
        }
        (offset, diagnostic)
    }
}

/// The Warning a tolerant read gives for a construct it keeps as the
/// document's preserved content where a strict read refuses it (ADR 0035
/// D3): at the strict refusal's byte offset, affecting no element.
struct DocumentKeep {
    /// Where the strict refusal is.
    offset: usize,
    template: &'static Template,
    /// The arguments, the row last if the construct has one.
    arguments: Vec<Argument>,
}

impl DocumentKeep {
    /// The Warning of `unread`, kept in the document for `reason` at byte
    /// `offset`, whose refusal is made from `arguments`.
    fn unread(
        offset: usize,
        unread: Unread,
        reason: DocumentReason,
        mut arguments: Vec<Argument>,
    ) -> Self {
        arguments.extend(unread.refusal().row().map(|row| Argument::text("row", row)));
        Self {
            offset,
            template: reason.kept(),
            arguments,
        }
    }

    /// The diagnostic, with the byte offset it is to be located at.
    fn into_diagnostic(self) -> (usize, Diagnostic) {
        let (requirement, clause) = kept_row(self.template.code);
        let mut diagnostic = Diagnostic::emit(
            self.template,
            Severity::Warning,
            requirement,
            self.arguments,
        );
        if let Some(clause) = clause {
            diagnostic = diagnostic.set_clause(clause);
        }
        (self.offset, diagnostic)
    }
}

/// What holds preserved content: an element, or the document outside every
/// element (ADR 0035 D3).
trait Holder {
    /// The ids a diagnostic about its preserved content affects: the
    /// element's, or none for the document.
    fn affected(&self) -> Option<ElementId>;

    /// The template and arguments of the SACM-ID-002 Warning of an empty
    /// `xmi:id` on the element written `element`, kept in it.
    fn empty_kept_id(&self, element: &str) -> (&'static Template, Vec<Argument>);
}

/// The element `id` of `kind`, which holds preserved content.
struct ElementHolder {
    kind: ElementKind,
    id: ElementId,
}

impl ElementHolder {
    /// The arguments a message that names it ends with: `holder_class` and
    /// `holder_id`.
    fn arguments(&self) -> [Argument; 2] {
        [
            Argument::text("holder_class", self.kind.name()),
            Argument::text("holder_id", self.id.as_str()),
        ]
    }
}

impl Holder for ElementHolder {
    fn affected(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn empty_kept_id(&self, element: &str) -> (&'static Template, Vec<Argument>) {
        let mut arguments = vec![Argument::text("element", element)];
        arguments.extend(self.arguments());
        (&template::ID_002_EMPTY_ID_OF_KEPT_ELEMENT, arguments)
    }
}

/// The document, on or under the `xmi:XMI` element, which holds preserved
/// content outside every element.
struct DocumentHolder;

impl Holder for DocumentHolder {
    fn affected(&self) -> Option<ElementId> {
        None
    }

    fn empty_kept_id(&self, element: &str) -> (&'static Template, Vec<Argument>) {
        (
            &template::ID_002_EMPTY_ID_OF_KEPT_ELEMENT_BY_DOCUMENT,
            vec![Argument::text("element", element)],
        )
    }
}

/// What a result of reading a property part says about its occurrences
/// (ADR 0035, SACM23-VAL-006): whether it counts as one, which it does if it
/// was read, or kept for a value that is no literal. A part preserved for
/// another reason is no occurrence (ADR 0033).
fn counts<T>(result: &Result<T, Fault>) -> bool {
    match result {
        Ok(_) => true,
        Err(Fault::Keep(keep)) => keep.is_literal,
        Err(Fault::Refused(_) | Fault::Defect) => false,
    }
}

/// The fault a tolerant read keeps with a Warning, or a refusal.
fn fault(kept: Result<Keep, ReadError>) -> Fault {
    match kept {
        Ok(keep) => Fault::Keep(keep),
        Err(error) => Fault::Refused(error),
    }
}

/// `arguments`, if a read in `mode` keeps `unread` at byte `offset`: a
/// tolerant read of an entry it keeps. Otherwise, in strict mode and for an
/// entry a tolerant read refuses, the refusal of `refusal` made from them
/// (ADR 0035 D1, D2).
fn refuse_unless_kept(
    text: &str,
    mode: Mode,
    offset: usize,
    unread: Unread,
    refusal: &'static Template,
    arguments: Vec<Argument>,
) -> Result<Vec<Argument>, ReadError> {
    match (mode, unread.tolerant()) {
        (Mode::Tolerant, Tolerant::Kept(_)) => Ok(arguments),
        _ => Err(unsupported(
            text,
            offset,
            unread,
            Details::new(refusal, arguments),
        )),
    }
}

/// The fault of `unread` at byte `offset` in `mode`, kept in an element for
/// `reason`: its refusal in strict mode and when a tolerant read refuses it
/// too; otherwise the Warning a tolerant read keeps it with (ADR 0035 D1,
/// D2).
fn unread_fault(
    text: &str,
    mode: Mode,
    offset: usize,
    unread: Unread,
    reason: ElementReason,
    arguments: Vec<Argument>,
) -> Fault {
    fault(keep_or_refuse(
        text, mode, offset, unread, reason, arguments,
    ))
}

/// What a read in `mode` does with `unread` at byte `offset`, kept in an
/// element for `reason`: the Warning a tolerant read keeps it with, or, in
/// strict mode and for an entry a tolerant read refuses, its refusal, made
/// from `arguments`.
fn keep_or_refuse(
    text: &str,
    mode: Mode,
    offset: usize,
    unread: Unread,
    reason: ElementReason,
    arguments: Vec<Argument>,
) -> Result<Keep, ReadError> {
    let arguments = refuse_unless_kept(text, mode, offset, unread, reason.refusal(), arguments)?;
    Ok(Keep::unread(offset, unread, reason, arguments))
}

/// The fault of the value `value` of `property` of the element `element`,
/// given at byte `offset`, that is no literal: `InvalidLiteral` in strict
/// mode (SACM-ENUM-001), kept with a Warning in tolerant mode (ADR 0035).
fn literal_fault(
    text: &str,
    mode: Mode,
    offset: usize,
    element: &ElementId,
    property: &str,
    value: &str,
) -> Fault {
    match mode {
        Mode::Strict => Fault::Refused(ReadError::invalid_literal(
            element.clone(),
            property.to_owned(),
            value.to_owned(),
            locate(text, offset),
        )),
        Mode::Tolerant => Fault::Keep(Keep::literal(offset, property, value)),
    }
}

/// What a read in `mode` does with an occurrence, at byte `at`, of the
/// property `property` of the element `owner` after its first: the strict
/// `refusal`, `RepeatedValue`, or the Warning a tolerant read keeps the
/// occurrence with (ADR 0035 D1). The refusal is built only in strict mode,
/// so a tolerant read of many repeats stays linear.
fn repeat_or_refuse(
    mode: Mode,
    at: usize,
    owner: &ElementId,
    property: &str,
    refusal: impl FnOnce() -> ReadError,
) -> Result<Keep, ReadError> {
    match mode {
        Mode::Strict => Err(refusal()),
        Mode::Tolerant => Ok(Keep::repeat(at, owner, property)),
    }
}

/// The XML Schema instance namespace, of `xsi:type` and `xsi:nil`, which are
/// read but never written (ADR 0017 T1; SACM23-XMI-026).
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The requirement the read diagnostics of `xsi:type` and `xmi:version`
/// enforce.
const MODES_ROW: &str = "LIB-COMPAT-001";

/// The requirement of the Info for an element without an `xmi:id`.
const MISSING_ID_ROW: &str = "SACM23-XMI-020";

/// The requirement of the diagnostics of a same-file proxy kept instead of
/// resolved.
const PROXY_ROW: &str = "SACM23-XMI-030";

/// The requirement of the Info for an `xmi:id` read trimmed.
const TRIMMED_ID_ROW: &str = "SACM23-XMI-025";

/// The requirement of the Warnings for the C++ name and text shapes.
const LEGACY_ROW: &str = "LIB-COMPAT-008";

/// The Assertion property read as an attribute or a value element
/// (formal/23-05-08 §11.10).
const DECLARATION: &str = "assertionDeclaration";

/// The `SACMElement` property read as an attribute or a value element
/// (formal/23-05-08 §8.2).
const GID: &str = "gid";

/// The document whose roots `tree` holds, read in `mode`, and the
/// diagnostics found, located and in input order, not yet sorted. `text` is the text `tree` was parsed from.
pub(super) fn read(
    text: &str,
    tree: &roxmltree::Document<'_>,
    mode: Mode,
    limits: &Limits,
) -> Result<(Document, Vec<Diagnostic>), ReadError> {
    let element = tree.root_element();
    // The class of each root's namespace value, classified once (ADR 0041
    // D4).
    let mut classes = Classes::default();
    let is_wrapper = classify(text, element, mode, &mut classes)?;
    let mut document = Document::new();
    let mut found = Findings {
        text,
        mode,
        ids: BTreeSet::new(),
        id_log: Vec::new(),
        reserved: reserved_ids(element),
        generator: IdGenerator::default(),
        order: 0,
        diagnostics: Vec::new(),
        legacy: None,
        classes,
        types: Types::new(namespaces::PINNED),
        revision: None,
        budget: Budget::new(text, limits),
    };
    // What the document holds outside every element: its XMI metadata
    // (SACM23-XMI-019, ADR 0042) and its preserved content (ADR 0035 D3).
    let mut outside = Kept::new(DocumentHolder);
    let mut metadata = Kept::new(DocumentHolder);
    let mut roots = Roots::default();
    if is_wrapper {
        read_wrapper(&mut found, element, &mut roots, &mut outside, &mut metadata)?;
    } else {
        read_root(&mut found, element, &mut roots, &mut outside)?;
    }
    // Once every root is read: the same-file proxies, then the elements
    // (SACM23-XMI-030). What the document holds outside every element keeps
    // its place among the roots that stay.
    let stays = resolve(&mut found, tree, &mut roots)?;
    hold(text, &mut document, roots.read)?;
    let mut outside = outside.into_preserved();
    let mut metadata = metadata.into_preserved();
    outside.shift(&stays);
    metadata.shift(&stays);
    document
        .set_document_metadata(metadata)
        .and_then(|()| document.set_document_preserved(outside))
        .map_err(|error| {
            unsupported(
                text,
                element.range().start,
                Unread::Unheld,
                Details::new(
                    &template::SEC_002_UNHELD_OUTSIDE,
                    vec![Argument::text("error", error)],
                ),
            )
        })?;
    let Findings {
        generator,
        diagnostics,
        legacy,
        ..
    } = found;
    // A strict read never applies a tolerance (ADR 0022 amendment 1).
    if let Some((offset, element, shape)) = legacy {
        return Err(ReadError::legacy_shape(
            element,
            shape,
            locate(text, offset),
        ));
    }
    // An edit generates above every id generated here (ADR 0024).
    generator.commit(&mut document);
    Ok((document, located(text, diagnostics)))
}

/// `diagnostics`, each located at its byte offset of `text`, in input order.
///
/// They are found in reading order, which is not always input order: an
/// element's `xmi:id` is read before the attributes written ahead of it. So
/// they are sorted by offset first (stably, so equal offsets keep reading
/// order), and one advancing [`Locator`] then locates them all in one pass
/// over the text, which keeps the read linear in the input.
fn located(text: &str, mut diagnostics: Vec<(usize, Diagnostic)>) -> Vec<Diagnostic> {
    diagnostics.sort_by_key(|&(offset, _)| offset);
    let mut locator = Locator::new(text);
    diagnostics
        .into_iter()
        .map(|(offset, diagnostic)| match locator.locate(offset) {
            Some(location) => diagnostic.set_location(location),
            None => diagnostic,
        })
        .collect()
}

/// What pass 2 reads with: the input text, the mode, the ids read so far,
/// the ids a generated one must avoid, and the diagnostics found so far,
/// each with the byte offset it is to be located at, which [`located`]
/// resolves once the read is done.
struct Findings<'t, 'a> {
    text: &'t str,
    mode: Mode,
    /// The value of every `xmi:id` read so far, of elements and names, in
    /// every root's tree (SACM23-VAL-001, SACM23-XMI-022), preserved content
    /// included (ADR 0033).
    ids: BTreeSet<&'a str>,
    /// The values of `ids`, in the order they were added, so that a part
    /// that is preserved after all can give back the ids read inside it
    /// ([`Findings::rollback`]).
    id_log: Vec<&'a str>,
    /// The ids a generated one must avoid, collected by [`reserved_ids`]
    /// before anything is read, so that none is generated, even one that
    /// comes later: the value of every `xmi:id` in the input (SACM23-XMI-020),
    /// and every id a reference may name, that is each token of a reference
    /// attribute, each `xmi:idref` value and each `href` fragment, so that a
    /// dangling reference never resolves to a generated element
    /// (SACM23-XMI-005, ADR 0026).
    reserved: BTreeSet<&'a str>,
    /// The ids generated for elements without an `xmi:id`.
    generator: IdGenerator,
    /// The number of elements met so far: the next one's place in document
    /// order.
    order: usize,
    /// The diagnostics found so far, in reading order, each with its byte
    /// offset in `text`.
    diagnostics: Vec<(usize, Diagnostic)>,
    /// In strict mode, the first C++ shape read, by offset: its byte
    /// offset, the element it belongs to and what it is, which [`read`]
    /// refuses once the input has read whole (LIB-COMPAT-008).
    legacy: Option<(usize, ElementId, Details)>,
    /// The namespaces of the root being read that count as SACM for types
    /// (ADR 0041 D5).
    types: Types<'a>,
    /// The class of each namespace value met as a root's, from pass 1
    /// (LIB-COMPAT-004).
    classes: Classes<'a>,
    /// The SACM revision of the root being read, if it is an older one's
    /// (LIB-COMPAT-004).
    revision: Option<&'static str>,
    /// What the read keeps besides the parsed tree, counted against the
    /// size limit: the namespace text of preserved content and the text of
    /// each diagnostic (LIB-SEC-003; ADR 0041 D2, amendment 1).
    budget: Budget<'t>,
}

/// What [`Findings`] holds before a part is read, so that a part that turns
/// out not to conform can be preserved as if it had not been read.
struct Checkpoint {
    /// The number of diagnostics found.
    diagnostics: usize,
    /// The first C++ shape found in strict mode.
    legacy: Option<(usize, ElementId, Details)>,
    /// The number of ids read.
    ids: usize,
}

impl<'a> Findings<'_, 'a> {
    /// Records `value`, an `xmi:id` value, as read; `false` if it was read
    /// before.
    fn insert_id(&mut self, value: &'a str) -> bool {
        let inserted = self.ids.insert(value);
        if inserted {
            self.id_log.push(value);
        }
        inserted
    }

    /// Where the read is, before a part that may be preserved.
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            diagnostics: self.diagnostics.len(),
            legacy: self.legacy.clone(),
            ids: self.id_log.len(),
        }
    }

    /// Forgets everything found since `checkpoint`: the part is preserved,
    /// and nothing inside a preserved part is read or reported (ADR 0033).
    fn rollback(&mut self, checkpoint: Checkpoint) {
        self.diagnostics.truncate(checkpoint.diagnostics);
        self.legacy = checkpoint.legacy;
        for id in self.id_log.drain(checkpoint.ids..) {
            self.ids.remove(id);
        }
    }

    /// Reports the construct `template` describes, made from `arguments`, at
    /// byte `offset`, preserved in `holder` (SACM-XMI-003, LIB-COMPAT-003;
    /// XMI 2.5.1 §7.3.3): an Error in strict mode and a Warning in tolerant
    /// mode.
    fn unknown(
        &mut self,
        offset: usize,
        template: &'static Template,
        mut arguments: Vec<Argument>,
        holder: &ElementHolder,
    ) -> Result<(), ReadError> {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        arguments.extend(holder.arguments());
        let diagnostic = Diagnostic::emit(template, severity, PRESERVED_ROW, arguments)
            .set_clause("7.3.3")
            .set_affected(holder.affected());
        self.push_at(diagnostic, offset)
    }
}

impl Findings<'_, '_> {
    /// What this read does with `unread` at byte `offset`, kept outside
    /// every element for `reason`: the Warning a tolerant read keeps it
    /// with, or its refusal, made from `arguments` ([`refuse_unless_kept`]).
    fn refuse_or_keep_in_document(
        &self,
        offset: usize,
        unread: Unread,
        reason: DocumentReason,
        arguments: Vec<Argument>,
    ) -> Result<DocumentKeep, ReadError> {
        let arguments = refuse_unless_kept(
            self.text,
            self.mode,
            offset,
            unread,
            reason.refusal(),
            arguments,
        )?;
        Ok(DocumentKeep::unread(offset, unread, reason, arguments))
    }

    /// What this read does with `unread` at byte `offset`, kept in an
    /// element for `reason`, as a [`Fault`] ([`keep_or_refuse`]). Every
    /// refusal site in an element that a tolerant read may keep asks this.
    fn unread_fault(
        &self,
        offset: usize,
        unread: Unread,
        reason: ElementReason,
        arguments: Vec<Argument>,
    ) -> Fault {
        unread_fault(self.text, self.mode, offset, unread, reason, arguments)
    }

    /// Reports `keep`, a construct kept as preserved content of `holder`: one
    /// Warning at the strict refusal's location, affecting the holder (ADR
    /// 0035 D1).
    fn keep(&mut self, keep: Keep, holder: &ElementHolder) -> Result<(), ReadError> {
        let (offset, diagnostic) = keep.into_diagnostic(holder);
        self.push_at(diagnostic, offset)
    }

    /// Reports `keep`, a construct kept as the document's preserved content:
    /// one Warning at the strict refusal's location, affecting no element
    /// (ADR 0035 D3).
    fn keep_in_document(&mut self, keep: DocumentKeep) -> Result<(), ReadError> {
        let (offset, diagnostic) = keep.into_diagnostic();
        self.push_at(diagnostic, offset)
    }

    /// Reports `spelled`, which a tolerant read read at byte `offset` as its
    /// resolved name: one SACM-XMI-013 Warning affecting `affected`, the
    /// element the spelling belongs to (SACM23-XMI-009; ADR 0034 amendment
    /// 2).
    fn spelled(
        &mut self,
        offset: usize,
        spelled: Spelled,
        affected: &ElementId,
    ) -> Result<(), ReadError> {
        let template = spelled.read();
        let Spelled {
            spelling,
            arguments,
            ..
        } = spelled;
        let diagnostic = Diagnostic::emit(template, Severity::Warning, spelling.row(), arguments)
            .set_clause(spelling.clause())
            .set_affected([affected.clone()]);
        self.push_at(diagnostic, offset)
    }

    /// Reports the role or type `spelled` of the child `node`, the element
    /// `child` of `parent`, which a tolerant read read, at its start tag, as
    /// [`spelled`](Self::spelled) does: a role belongs to the parent, a type
    /// to the child (SACM23-XMI-009).
    fn spelled_child(
        &mut self,
        node: Node<'_, '_>,
        spelled: Spelled,
        parent: &ElementId,
        child: &ElementId,
    ) -> Result<(), ReadError> {
        let affected = if spelled.spelling.is_role() {
            parent
        } else {
            child
        };
        self.spelled(node.range().start, spelled, affected)
    }

    /// Reports the empty `xmi:id` of `node`, in the message of `template`
    /// made from `arguments`, affecting `affected`: one SACM-ID-002 Warning
    /// at the start tag of `node` in a tolerant read, which reads on (ADR
    /// 0035 D1).
    fn empty_id(
        &mut self,
        node: Node<'_, '_>,
        template: &'static Template,
        arguments: Vec<Argument>,
        affected: Option<ElementId>,
    ) -> Result<(), ReadError> {
        let diagnostic = Diagnostic::emit(template, Severity::Warning, TOLERANT_ROW, arguments)
            .set_clause("7.6.1")
            .set_affected(affected);
        self.push_at(diagnostic, node.range().start)
    }
}

impl Findings<'_, '_> {
    /// The next element's place in document order.
    fn next_order(&mut self) -> usize {
        let order = self.order;
        self.order = self.order.saturating_add(1);
        order
    }

    /// The id of `node`, an element of `kind` without an `xmi:id`: the next
    /// generated one that no `xmi:id` in the input holds, reported as
    /// SACM-XMI-004 Info at the element's start tag (SACM23-XMI-020;
    /// XMI 2.5.1 §7.6.1; ADR 0024).
    fn generate_id(
        &mut self,
        node: Node<'_, '_>,
        kind: ElementKind,
    ) -> Result<ElementId, ReadError> {
        let id = self.next_id(kind);
        let diagnostic = Diagnostic::emit(
            &template::XMI_004_GENERATED_ID,
            Severity::Info,
            MISSING_ID_ROW,
            named(kind, id.as_str()),
        )
        .set_clause("7.6.1")
        .set_affected([id.clone()]);
        self.push_at(diagnostic, node.range().start)?;
        Ok(id)
    }

    /// The next generated id of `kind` that no `xmi:id` in the input holds
    /// and no reference names (ADR 0024).
    fn next_id(&mut self, kind: ElementKind) -> ElementId {
        let reserved = &self.reserved;
        self.generator
            .next(kind, |candidate| reserved.contains(candidate))
    }

    /// Reports the `xmi:id` `attribute`, read without the white space around
    /// it, in the message of `template` made from `arguments`, which affects
    /// the element `affected` (SACM-ID-004; XMI 2.5.1 §7.6.1; ADR 0024).
    fn trimmed_id(
        &mut self,
        attribute: &Attribute<'_, '_>,
        template: &'static Template,
        arguments: Vec<Argument>,
        affected: &str,
    ) -> Result<(), ReadError> {
        let diagnostic = Diagnostic::emit(template, Severity::Info, TRIMMED_ID_ROW, arguments)
            .set_clause("7.6.1")
            .set_affected([ElementId::new(affected)]);
        self.push(diagnostic, attribute)
    }

    /// Reports the `xsi:type` `attribute`, in the message of `template` made
    /// from `arguments`, which affects the element `id` (SACM-XMI-010; XMI
    /// 2.5.1 §9.5.2 rule 2g).
    fn xsi_type(
        &mut self,
        attribute: &Attribute<'_, '_>,
        template: &'static Template,
        arguments: Vec<Argument>,
        id: &str,
    ) -> Result<(), ReadError> {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        let diagnostic = Diagnostic::emit(template, severity, MODES_ROW, arguments)
            .set_clause("9.5.2")
            .set_affected([ElementId::new(id)]);
        self.push(diagnostic, attribute)
    }

    /// Reports the `xmi:version` `attribute`, in the message of `template`
    /// made from its value and `arguments`, which affects the root `id`, or
    /// nothing on the XMI element (SACM-XMI-011; XMI 2.5.1 §7.5.3).
    fn xmi_version(
        &mut self,
        attribute: &Attribute<'_, '_>,
        template: &'static Template,
        arguments: Vec<Argument>,
        id: Option<&str>,
    ) -> Result<(), ReadError> {
        let mut all = vec![Argument::text("value", attribute.value())];
        all.extend(arguments);
        let diagnostic = Diagnostic::emit(template, Severity::Info, MODES_ROW, all)
            .set_clause("7.5.3")
            .set_affected(id.map(ElementId::new));
        self.push(diagnostic, attribute)
    }

    /// Reports the C++ shape `shape` at byte `offset`, made from
    /// `arguments`, which gives a name or a text of the element `owner`: one
    /// SACM-XMI-012 Warning, and in strict mode the refusal [`read`] returns
    /// if this is the first shape (LIB-COMPAT-008; XMI 2.5.1 §7.8.6; ADR
    /// 0022 amendment 1).
    fn legacy_shape(
        &mut self,
        offset: usize,
        owner: &ElementId,
        shape: Shape,
        arguments: Vec<Argument>,
    ) -> Result<(), ReadError> {
        let is_first = self
            .legacy
            .as_ref()
            .is_none_or(|(first, _, _)| offset < *first);
        if self.mode == Mode::Strict && is_first {
            self.legacy = Some((
                offset,
                owner.clone(),
                Details::new(shape.refusal(), arguments.clone()),
            ));
        }
        let diagnostic = Diagnostic::emit(shape.read(), Severity::Warning, LEGACY_ROW, arguments)
            .set_clause("7.8.6")
            .set_affected([owner.clone()]);
        self.push_at(diagnostic, offset)
    }

    /// Reports `proxy`, kept for `why` instead of resolved, at its start tag
    /// (SACM23-XMI-030): one SACM-XMI-003 affecting its container, an Error
    /// in strict mode and a Warning in tolerant mode, with clause 7.10.1.
    fn unresolved(&mut self, proxy: &Proxy, why: Unresolved) -> Result<(), ReadError> {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        let mut arguments = vec![
            Argument::text("element", proxy.written.as_str()),
            Argument::text("target", proxy.link.target.as_str()),
        ];
        let template = match why {
            Unresolved::NoRoot => &template::XMI_003_PROXY_OF_NO_ROOT,
            Unresolved::WrongKind {
                target_class,
                class,
            } => {
                arguments.push(Argument::text("target_class", target_class.name()));
                arguments.push(Argument::text("class", class));
                &template::XMI_003_PROXY_WRONG_KIND
            }
            Unresolved::Shared => &template::XMI_003_PROXY_SHARED,
            Unresolved::Cycle => &template::XMI_003_PROXY_CYCLE,
        };
        arguments.extend([
            Argument::text("holder_class", proxy.container_kind.name()),
            Argument::text("holder_id", proxy.container.as_str()),
        ]);
        let diagnostic = Diagnostic::emit(template, severity, PROXY_ROW, arguments)
            .set_clause("7.10.1")
            .set_affected([proxy.container.clone()]);
        self.push_at(diagnostic, proxy.at)
    }

    /// Adds `diagnostic`, located at the name of `attribute`.
    fn push(
        &mut self,
        diagnostic: Diagnostic,
        attribute: &Attribute<'_, '_>,
    ) -> Result<(), ReadError> {
        self.push_at(diagnostic, attribute.range_qname().start)
    }

    /// Adds `diagnostic`, to be located at byte `offset` of the text, after
    /// counting its text against the size limit there. The first one that
    /// passes the limit refuses the read there: the error is returned, and
    /// every caller returns it at once, so nothing is read or reported after
    /// it (LIB-SEC-003, ADR 0041 amendment 1).
    fn push_at(&mut self, diagnostic: Diagnostic, offset: usize) -> Result<(), ReadError> {
        self.budget.keep_diagnostic(&diagnostic, offset)?;
        self.diagnostics.push((offset, diagnostic));
        Ok(())
    }

    /// Reports the root `id` of `kind`, whose start tag is at byte `offset`,
    /// in the namespace `uri` of the class `class`: one SACM-XMI-008 Warning
    /// for an older revision's, one SACM-XMI-002 for a SACM-like one, and
    /// nothing for the pinned one (LIB-COMPAT-004).
    fn root_namespace(
        &mut self,
        class: RootNamespace<'_>,
        uri: &str,
        offset: usize,
        kind: ElementKind,
        id: &str,
    ) -> Result<(), ReadError> {
        let mut arguments = named(kind, id);
        arguments.push(Argument::text("namespace", uri));
        let template = match class {
            RootNamespace::OtherRevision { revision, .. } => {
                arguments.push(Argument::text("revision", revision));
                &template::XMI_008_OTHER_REVISION
            }
            RootNamespace::SacmLike { .. } => &template::XMI_002_SACM_LIKE,
            RootNamespace::Pinned | RootNamespace::Sacm24 | RootNamespace::NotSacm => return Ok(()),
        };
        let diagnostic =
            Diagnostic::emit(template, Severity::Warning, OTHER_NAMESPACE_ROW, arguments)
                .set_affected([ElementId::new(id)]);
        self.push_at(diagnostic, offset)
    }

    /// Reports the `occurence` at byte `offset` of the Event `id`, read as
    /// its date in a root of SACM `revision` (SACM-XMI-008).
    fn occurence(
        &mut self,
        offset: usize,
        id: &ElementId,
        revision: &str,
    ) -> Result<(), ReadError> {
        let arguments = vec![
            Argument::text("id", id.as_str()),
            Argument::text("revision", revision),
        ];
        let diagnostic = Diagnostic::emit(
            &template::XMI_008_OCCURENCE,
            Severity::Warning,
            OTHER_NAMESPACE_ROW,
            arguments,
        )
        .set_affected([id.clone()]);
        self.push_at(diagnostic, offset)
    }

    /// Counts `bytes` of namespace text that preserved content keeps at byte
    /// `offset` against the size limit, in the budget the diagnostics share,
    /// and refuses it there once the budget passes the limit (LIB-SEC-003,
    /// ADR 0041 D2). The count never decreases, a part read and then
    /// preserved included.
    fn keep_namespace_text(&self, bytes: u64, offset: usize) -> Result<(), ReadError> {
        self.budget.keep_namespace_text(bytes, offset)
    }
}

/// The ids a generated one must avoid, in the tree under `element`, on any
/// element: the value of every `xmi:id`, as [`id_value`] gives it (ADR
/// 0024), and every id a reference may hold: each token of an attribute in
/// no namespace named as a reference end, each `xmi:idref` value, trimmed,
/// and each `href` fragment (ADR 0026). So a dangling reference never
/// resolves to a generated element. It refuses nothing, and may hold more
/// than is read, which only skips more candidates.
fn reserved_ids<'a>(element: Node<'a, '_>) -> BTreeSet<&'a str> {
    let mut reserved = BTreeSet::new();
    for attribute in element.descendants().flat_map(|node| node.attributes()) {
        if is_xmi(&attribute, "id") {
            reserved.insert(id_value(attribute.value()));
        } else {
            reserved.extend(reference_tokens(&attribute));
        }
    }
    reserved
}

/// The ids `attribute` may name as a reference (ADR 0026, ADR 0033): its
/// value, trimmed, for an `xmi:idref`; the fragment of an `href`, trimmed;
/// and each token of an attribute in no namespace named as a reference end,
/// or as the spelling `argumentationElement` of one (SACM23-XMI-009). None
/// for any other attribute.
fn reference_tokens<'a>(attribute: &Attribute<'a, '_>) -> Vec<&'a str> {
    let value = attribute.value();
    if is_xmi(attribute, "idref") {
        vec![value.trim_matches(is_xml_space)]
    } else if is_unqualified(attribute, "href") {
        value
            .trim_matches(is_xml_space)
            .strip_prefix('#')
            .into_iter()
            .collect()
    } else if attribute.namespace().is_none()
        && (attribute.name() == TEXT_SPELLING
            || ReferenceEnd::ALL
                .iter()
                .any(|end| end.name() == attribute.name()))
    {
        id_tokens(value).collect()
    } else {
        Vec::new()
    }
}

/// The ids of an `xsd:IDREFS` value: its tokens between XML white space,
/// each exactly as written (XML Schema 1.0 Part 2 §3.3.10, §4.3.6; ADR 0026).
fn id_tokens(value: &str) -> impl Iterator<Item = &str> {
    value.split(is_xml_space).filter(|token| !token.is_empty())
}

/// The byte offset in `text` of the first character, as written, of each
/// id token of `attribute`'s value, in order (SACM23-XMI-005): the value is
/// normalized again from the source (XML 1.0 §3.3.3), with each character
/// reference standing for its character at its `&`, and each literal white
/// space, a CRLF counting once, for a space. `None` when that does not give
/// the tokens of the parsed value, which no well-formed input causes.
fn token_starts(text: &str, attribute: &Attribute<'_, '_>) -> Option<Vec<usize>> {
    let range = attribute.range_value();
    let raw = text.get(range.clone())?;
    let mut starts = Vec::new();
    let mut token = String::new();
    let mut tokens = id_tokens(attribute.value());
    let mut rest = raw.char_indices().peekable();
    // Ends the token being built, if any, checking it against the parsed one.
    let mut close = |token: &mut String| -> Option<()> {
        if !token.is_empty() {
            (tokens.next()? == token.as_str()).then_some(())?;
            token.clear();
        }
        Some(())
    };
    while let Some((offset, c)) = rest.next() {
        let normalized = match c {
            '&' => {
                let end = raw.get(offset..)?.find(';')?;
                let name = raw.get(offset + 1..offset + end)?;
                while rest.next_if(|&(at, _)| at <= offset + end).is_some() {}
                character_reference(name)?
            }
            '\r' => {
                rest.next_if(|&(_, next)| next == '\n');
                ' '
            }
            other => other,
        };
        if is_xml_space(normalized) {
            close(&mut token)?;
        } else {
            if token.is_empty() {
                starts.push(range.start + offset);
            }
            token.push(normalized);
        }
    }
    close(&mut token)?;
    tokens.next().is_none().then_some(starts)
}

/// The character the reference `&name;` stands for: a predefined entity
/// (XML 1.0 §4.6) or a character reference (§4.1). `None` for any other.
fn character_reference(name: &str) -> Option<char> {
    let code = match name {
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "amp" => return Some('&'),
        "apos" => return Some('\''),
        "quot" => return Some('"'),
        _ => match name.strip_prefix("#x") {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => name.strip_prefix('#')?.parse().ok()?,
        },
    };
    char::from_u32(code)
}

/// The `xsd:ID` value of the `xmi:id` value `raw` (SACM23-XMI-025; XMI
/// 2.5.1 §7.6.1; XML Schema 1.0 Part 2 §4.3.6; ADR 0024): without the XML
/// white space (#x20, #x9, #xA, #xD) at both ends when that leaves an
/// `NCName`, and otherwise `raw` as written (D-ID). Every id read is
/// compared, deduplicated and held by this value; it differs from `raw`
/// exactly when the id was trimmed.
fn id_value(raw: &str) -> &str {
    let trimmed = raw.trim_matches(is_xml_space);
    if is_ncname(trimmed) { trimmed } else { raw }
}

/// Pass 1: whether the document element is the XMI element, after checking
/// every root: first, in document order, that each is named as a package
/// kind and is in a SACM namespace (`InvalidRoot` otherwise), then that each
/// is in a namespace `mode` reads (ADR 0041 D4). Each namespace value is
/// classified once, into `classes`.
fn classify<'a>(
    text: &str,
    element: Node<'a, '_>,
    mode: Mode,
    classes: &mut Classes<'a>,
) -> Result<bool, ReadError> {
    let name = element.tag_name();
    match (name.namespace(), name.name()) {
        (Some(XMI_NS), "XMI") => {
            let roots: Vec<Node<'_, '_>> = element
                .children()
                .filter(|child| child.is_element() && child.tag_name().namespace() != Some(XMI_NS))
                .collect();
            if roots.is_empty() {
                return Err(ReadError::NoPackage {
                    location: locate(text, element.range().start),
                });
            }
            for &root in &roots {
                check_root(text, root, classes)?;
            }
            for &root in &roots {
                check_root_namespace(text, root, mode, classes)?;
            }
            Ok(true)
        }
        // Another XMI element is XMI metadata, or no XMI element at all: as
        // the document element, it holds no SACM package (SACM23-XMI-019).
        (Some(XMI_NS), _) => Err(ReadError::invalid_root(
            locate(text, element.range().start),
            written_name(text, element),
            &template::XMI_001_XMI_NAMESPACE_ROOT,
            Vec::new(),
        )),
        // An XMI element outside the XMI namespace (ADR 0016 D1), which
        // LIB-COMPAT-002 may read.
        (_, "XMI") => Err(unsupported(
            text,
            element.range().start,
            Unread::OtherXmiNamespace,
            text_details(
                &template::XMI_015_NOT_XMI_NAMESPACE,
                &[("element", &written_name(text, element))],
            ),
        )),
        _ => {
            check_root(text, element, classes)?;
            check_root_namespace(text, element, mode, classes)?;
            Ok(false)
        }
    }
}

/// Refuses `root` as `InvalidRoot` if its local name is none of the package
/// kinds, or if it is in no namespace or one that does not name SACM
/// (SACM-XMI-001; ADR 0041 D4).
fn check_root<'a>(
    text: &str,
    root: Node<'a, '_>,
    classes: &mut Classes<'a>,
) -> Result<(), ReadError> {
    if package_kind(&root).is_none() {
        return Err(invalid_root(text, root));
    }
    let uri = root.tag_name().namespace().filter(|uri| !uri.is_empty());
    if classes.of(uri) != RootNamespace::NotSacm {
        return Ok(());
    }
    let location = locate(text, root.range().start);
    let element = written_name(text, root);
    Err(match uri {
        Some(uri) => ReadError::invalid_root(
            location,
            element,
            &template::XMI_001_NOT_A_SACM_NAMESPACE,
            vec![Argument::text("namespace", uri)],
        ),
        None => ReadError::invalid_root(
            location,
            element,
            &template::XMI_001_NO_NAMESPACE,
            Vec::new(),
        ),
    })
}

/// Refuses `root`, a package kind in a SACM namespace, as `Unsupported` if
/// `mode` does not read its namespace: a SACM 2.4 root in both modes, and in
/// strict mode one in an older revision's or a SACM-like namespace, which a
/// tolerant read reads (ADR 0041 D4, D6). The refusal, which copies the
/// namespace, is built only when the root is refused.
fn check_root_namespace<'a>(
    text: &str,
    root: Node<'a, '_>,
    mode: Mode,
    classes: &mut Classes<'a>,
) -> Result<(), ReadError> {
    let Some(uri) = root.tag_name().namespace() else {
        return Ok(());
    };
    let class = classes.of(Some(uri));
    let unread = match class {
        RootNamespace::Sacm24 => Unread::Sacm24,
        RootNamespace::OtherRevision { .. } => Unread::OtherRevision,
        RootNamespace::SacmLike { .. } => Unread::OtherSacmNamespace,
        RootNamespace::Pinned | RootNamespace::NotSacm => return Ok(()),
    };
    if mode == Mode::Tolerant && matches!(unread.tolerant(), Tolerant::Read(_)) {
        return Ok(());
    }
    let element = written_name(text, root);
    let details = match class {
        RootNamespace::OtherRevision { revision, .. } => text_details(
            &template::XMI_008_OTHER_REVISION_ROOT,
            &[
                ("element", &element),
                ("namespace", uri),
                ("revision", revision),
            ],
        ),
        RootNamespace::SacmLike { .. } => text_details(
            &template::XMI_002_OTHER_NAMESPACE,
            &[("element", &element), ("namespace", uri)],
        ),
        RootNamespace::Sacm24 | RootNamespace::Pinned | RootNamespace::NotSacm => text_details(
            &template::XMI_008_SACM_2_4,
            &[("element", &element), ("namespace", uri)],
        ),
    };
    Err(unsupported(text, root.range().start, unread, details))
}

/// The package kind that `node`'s local name names exactly, if any.
fn package_kind(node: &Node<'_, '_>) -> Option<ElementKind> {
    let local = node.tag_name().name();
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.is_package() && kind.name() == local)
}

/// The `InvalidRoot` refusal of `root`, which is no package kind.
fn invalid_root(text: &str, root: Node<'_, '_>) -> ReadError {
    ReadError::invalid_root(
        locate(text, root.range().start),
        written_name(text, root),
        &template::XMI_001_NOT_A_PACKAGE,
        Vec::new(),
    )
}

/// The XMI elements XMI.xsd declares that XMI metadata holds under the XMI
/// element (XMI 2.5.1 §7.5.3 to §7.5.6, SACM23-XMI-019).
const WRAPPER_METADATA: [&str; 9] = [
    "documentation",
    "Documentation",
    "difference",
    "Difference",
    "Add",
    "Replace",
    "Delete",
    "extension",
    "Extension",
];

/// Pass 2 for the XMI element: checks its attributes, then reads its roots
/// and checks the content beside them, in document order. Its XMI metadata
/// goes to `metadata`, the rest of what it holds besides its roots to
/// `outside`, and the roots' trees to `roots`.
fn read_wrapper<'a>(
    found: &mut Findings<'_, 'a>,
    wrapper: Node<'a, '_>,
    roots: &mut Roots,
    outside: &mut Kept<DocumentHolder>,
    metadata: &mut Kept<DocumentHolder>,
) -> Result<(), ReadError> {
    let text = found.text;
    let name = written_name(text, wrapper);
    // The wrapper is no element of the document: its XMI metadata is the
    // document's (SACM23-XMI-019, ADR 0042); what it carries besides that
    // and `xmi:version` a strict read refuses, and a tolerant read keeps as
    // the document's preserved content (ADR 0033, ADR 0035 D3).
    for attribute in wrapper.attributes() {
        if is_xmi(&attribute, "version") {
            found.xmi_version(
                &attribute,
                &template::XMI_011_VERSION_ON_XMI_ELEMENT,
                Vec::new(),
                None,
            )?;
            continue;
        }
        if is_wrapper_metadata(text, &attribute) {
            // Its `xmi:id` is in the id space, as every metadata id is; an
            // empty one identifies nothing, and a tolerant read keeps it as
            // written outside it, with SACM-ID-002 (ADR 0035 D1, ADR 0042 D5).
            if is_xmi(&attribute, "id") {
                if attribute.value().is_empty() && found.mode == Mode::Tolerant {
                    found.empty_id(
                        wrapper,
                        &template::ID_002_EMPTY_ID_OF_XMI_ELEMENT,
                        Vec::new(),
                        None,
                    )?;
                } else {
                    let value = checked_id(found, wrapper, &attribute)?;
                    metadata.ids.push(ElementId::new(value));
                }
            }
            metadata.take_metadata_attribute(found, wrapper, &attribute)?;
            continue;
        }
        let at = attribute.range_qname().start;
        let arguments = attribute_arguments(text, &attribute, &name);
        let keep = found.refuse_or_keep_in_document(
            at,
            Unread::WrapperContent,
            DocumentReason::Attribute,
            arguments,
        )?;
        outside.keep_attribute(found, wrapper, &attribute, keep)?;
    }
    for child in wrapper.children() {
        if !child.is_element() {
            if let Some(first) = text_offset(text, child) {
                let keep = found.refuse_or_keep_in_document(
                    first,
                    Unread::WrapperContent,
                    DocumentReason::Text,
                    vec![Argument::text("element", name.as_str())],
                )?;
                outside.keep_text(found, child, keep)?;
            }
        } else if child.tag_name().namespace() == Some(XMI_NS) {
            if WRAPPER_METADATA.contains(&child.tag_name().name()) {
                // XMI metadata, kept whole and placed among the roots.
                metadata.typed = outside.typed;
                metadata.take_element(found, child)?;
            } else {
                // XMI 2.5.1 defines no such element here: a defect.
                let keep = found.refuse_or_keep_in_document(
                    child.range().start,
                    Unread::WrapperContent,
                    DocumentReason::Element,
                    text_arguments(&[
                        ("element", &written_name(text, child)),
                        ("container", &name),
                    ]),
                )?;
                outside.keep_element(found, child, keep)?;
            }
        } else {
            read_root(found, child, roots, outside)?;
        }
    }
    Ok(())
}

/// One element of a root's tree, held until the whole tree is read and then
/// inserted, so that a name written after the children is known when its
/// element is inserted.
struct Pending {
    /// The element's place in document order.
    order: usize,
    /// The id of the element's parent; `None` for the root.
    parent: Option<ElementId>,
    kind: ElementKind,
    /// The element's id: its `xmi:id`'s value, or a generated one.
    id: ElementId,
    /// The `gid` read, as the attribute or as a value element.
    gid: Option<String>,
    /// Whether the `gid` is given, a nil value element included
    /// (SACM23-XMI-026).
    has_gid: bool,
    /// The name read, as a `name` element or, in the C++ shape, a `name`
    /// attribute (LIB-COMPAT-008).
    name: Option<LangString>,
    /// The `xmi:id` of the name, if it has one (SACM23-XMI-022).
    name_id: Option<ElementId>,
    /// The `content` of a `UtilityElement` read from its `content` element,
    /// with its ids.
    content: Option<Text>,
    /// The `content` of a `UtilityElement` given on the element itself, in
    /// the C++ shape (LIB-COMPAT-008); [`finish`](Self::finish) makes it
    /// the content.
    shorthand: Shorthand,
    /// The `key` of a `TaggedValue`, with its ids.
    key: Option<Text>,
    /// The `location` of a `Resource`, with its ids.
    location: Option<Text>,
    /// Each string-valued attribute given, as the attribute or as a value
    /// element, with its value exactly as read, or `None` for a nil value
    /// element, which counts as given (SACM23-XMI-026; D-DATE).
    strings: Vec<(StringAttribute, Option<String>)>,
    /// The `assertionDeclaration` read, if it is written, as the attribute
    /// or as a value element.
    declaration: Option<AssertionDeclaration>,
    /// Each flag given, as the attribute or as a value element, with its
    /// value (SACM23-BASE-007, SACM23-BASE-008).
    flags: Vec<(Flag, bool)>,
    /// Each reference end given, in the attribute or the element form, with
    /// its targets in document order (SACM23-XMI-005, SACM23-XMI-015).
    references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    /// Each reference end given an `href` with a document part, with its
    /// `href`s in document order (SACM23-XMI-027, ADR 0040 D2).
    hrefs: Vec<(ReferenceEnd, Vec<Href>)>,
    /// The `name`, `content` and `key` a tolerant read kept as preserved
    /// content for a value in it that is no literal: each counts as given
    /// (ADR 0035, SACM23-VAL-006).
    given: Vec<&'static str>,
    /// The element's name as written in its start tag.
    written: String,
    /// The byte offset of its start tag.
    at: usize,
    /// Its preserved content so far (LIB-COMPAT-003, ADR 0033).
    kept: Kept<ElementHolder>,
    /// Its XMI metadata so far (SACM23-XMI-019, ADR 0042), placed among its
    /// typed children as preserved content is.
    metadata: Kept<ElementHolder>,
    /// The XMI metadata of its name, read with the name.
    name_metadata: Option<Preserved>,
    /// The place in document order of its parent; `None` for a root.
    parent_order: Option<usize>,
    /// Its place among its parent's children, or among the roots, in
    /// document order: its `order`, or for a root a proxy moves, the
    /// proxy's (SACM23-XMI-030).
    place: usize,
    /// Which of its typed slots survive, if a proxy it holds is kept: the
    /// proxy's slot does not (SACM23-XMI-030).
    slots: Option<Vec<bool>>,
}

/// The preserved content of one element, or of the document outside every
/// element, as it is read (ADR 0033, ADR 0035 D3).
struct Kept<H> {
    /// What holds it.
    holder: H,
    /// The number of typed children read so far (the roots, for the
    /// document): the index the next preserved child is placed at.
    typed: usize,
    /// The attributes, each with the byte offset of its name: sorted by it
    /// when the element is held, so they keep their input order.
    attributes: Vec<(usize, Attr)>,
    /// The bindings their prefixes need.
    namespaces: Vec<(String, String)>,
    /// The prefix `nsN` given to each namespace whose attributes were
    /// written with the prefix `sacm` or `xmi` bound to it, by URI (ADR
    /// 0035).
    renamed: Vec<(String, String)>,
    /// The children, in input order.
    children: Vec<Placed>,
    /// The `xmi:id` value of each element of the children.
    ids: Vec<ElementId>,
    /// Each id the content may name as a reference.
    references: Vec<ElementId>,
    /// The byte offset of each preserved attribute's name and child's start
    /// tag: no occurrence of a property (`RepeatedValue`).
    offsets: BTreeSet<usize>,
}

impl<H: Holder> Kept<H> {
    /// Nothing yet, held by `holder`.
    fn new(holder: H) -> Self {
        Self {
            holder,
            typed: 0,
            attributes: Vec::new(),
            namespaces: Vec::new(),
            renamed: Vec::new(),
            children: Vec::new(),
            ids: Vec::new(),
            references: Vec::new(),
            offsets: BTreeSet::new(),
        }
    }

    /// Whether the construct at byte `offset` is preserved.
    fn holds(&self, offset: usize) -> bool {
        self.offsets.contains(&offset)
    }

    /// Takes `attribute` of `node` as preserved content, with the bindings
    /// its prefix needs and the ids it may name, and returns it with the
    /// byte offset of its name.
    ///
    /// A prefix `sacm` or `xmi` bound to another namespace than SACM 2.3 or
    /// XMI 2.5.1 cannot be written back on the holder's start tag, where the
    /// writer binds them so. A strict read refuses it at byte `refuse_at`;
    /// a tolerant read replaces it by the first prefix `nsN`, from `ns1`,
    /// that is not bound at `node` nor used by the holder's other preserved
    /// attributes, the same for every attribute of that namespace, and
    /// returns the Warning that names it, as preserved content of an element
    /// (ADR 0035; a defect).
    fn take_attribute<'a>(
        &mut self,
        found: &Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        refuse_at: usize,
    ) -> Result<Option<Keep>, ReadError> {
        let text = found.text;
        let at = attribute.range_qname().start;
        let (mut attr, mut kept) = preserve::attr(text, attribute);
        let bindings = self.namespaces.len();
        let mut renamed = None;
        if let Some(prefix) = attr.name.prefix.clone() {
            let uri = attribute.namespace().unwrap_or_default();
            let pinned = match prefix.as_str() {
                SACM_PREFIX => Some(SACM_NS),
                XMI_PREFIX => Some(XMI_NS),
                _ => None,
            };
            match pinned {
                Some(pinned) if pinned != uri => {
                    let written = attr.name.written();
                    let element = written_name(text, node);
                    let arguments = text_arguments(&[
                        ("attribute", &written),
                        ("element", &element),
                        ("prefix", &prefix),
                        ("namespace", uri),
                        ("pinned_namespace", pinned),
                    ]);
                    let mut arguments = refuse_unless_kept(
                        text,
                        found.mode,
                        refuse_at,
                        Unread::UnwritablePrefix,
                        &template::XMI_003_UNWRITABLE_PREFIX,
                        arguments,
                    )?;
                    let new = self.rename(node, uri);
                    arguments.push(Argument::text("renamed_prefix", new.as_str()));
                    let keep = Keep::held(
                        refuse_at,
                        PRESERVED_ROW,
                        Some("7.3.3"),
                        &template::XMI_003_PREFIX_RENAMED,
                        arguments,
                    );
                    attr.name.prefix = Some(new);
                    renamed = Some(keep);
                }
                Some(_) => {}
                None if prefix == "xml" => {}
                None => self.namespaces.push((prefix, uri.to_owned())),
            }
        }
        for (prefix, uri) in self.namespaces.get(bindings..).unwrap_or_default() {
            kept = kept.saturating_add(preserve::binding_bytes(Some(prefix), uri));
        }
        found.keep_namespace_text(kept, at)?;
        self.references.extend(
            reference_tokens(attribute)
                .into_iter()
                .filter(|token| !token.is_empty())
                .map(ElementId::new),
        );
        self.offsets.insert(at);
        self.attributes.push((at, attr));
        Ok(renamed)
    }

    /// The prefix that attributes in the namespace `uri`, written with a
    /// prefix bound to another namespace at `node`, are written with: the
    /// one given before, or the first `nsN` that is not bound at `node` and
    /// that no binding of the holder's preserved attributes uses, which it
    /// binds to `uri` (ADR 0035).
    fn rename(&mut self, node: Node<'_, '_>, uri: &str) -> String {
        if let Some((_, prefix)) = self.renamed.iter().find(|(held, _)| held == uri) {
            return prefix.clone();
        }
        // The numbers N of the prefixes `nsN` that are taken: bound at `node`,
        // or used by a binding the holder keeps. They are finitely many, each
        // step passes a distinct one, so the loop ends. A prefix such as
        // `ns01` takes 1 too, which only skips a free candidate.
        let taken: BTreeSet<usize> = node
            .namespaces()
            .filter_map(roxmltree::Namespace::name)
            .chain(self.namespaces.iter().map(|(prefix, _)| prefix.as_str()))
            .filter_map(|prefix| prefix.strip_prefix("ns")?.parse().ok())
            .collect();
        let mut n = 1_usize;
        while taken.contains(&n) {
            n = n.saturating_add(1);
        }
        let prefix = format!("ns{n}");
        self.namespaces.push((prefix.clone(), uri.to_owned()));
        self.renamed.push((uri.to_owned(), prefix.clone()));
        prefix
    }

    /// Takes `node`, an element, with its subtree, as preserved content
    /// placed before the next typed child (ADR 0033). Nothing inside it is
    /// read or reported, but each `xmi:id` in it is checked: one read before
    /// is `DuplicateId`, and an empty one `InvalidId` in strict mode; a
    /// tolerant read keeps an empty one as written, outside the id space,
    /// with one SACM-ID-002 Warning affecting the holder (ADR 0035 D1).
    fn take_element<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
    ) -> Result<(), ReadError> {
        let text = found.text;
        for element in node.descendants().filter(Node::is_element) {
            for attribute in element.attributes() {
                if is_xmi(&attribute, "id") {
                    if attribute.value().is_empty() && found.mode == Mode::Tolerant {
                        let (template, arguments) =
                            self.holder.empty_kept_id(&written_name(text, element));
                        found.empty_id(element, template, arguments, self.holder.affected())?;
                        continue;
                    }
                    let value = checked_id(found, element, &attribute)?;
                    self.ids.push(ElementId::new(value));
                } else {
                    self.references.extend(
                        reference_tokens(&attribute)
                            .into_iter()
                            .filter(|token| !token.is_empty())
                            .map(ElementId::new),
                    );
                }
            }
        }
        let at = node.range().start;
        let fragment = preserve::fragment(text, node, &found.budget)?;
        self.offsets.insert(at);
        self.children.push(Placed {
            index: self.typed,
            item: Item::Element(fragment),
        });
        Ok(())
    }

    /// Takes `node`, a child of the holder, as preserved text if it is text
    /// that is not all XML white space, trimmed of it (ADR 0033), and returns
    /// where its first character that is not literal white space is, and
    /// whether it is a new piece. Text right after preserved text, with no
    /// child between, joins it after an LF, as a read of the written output
    /// gives it.
    fn take_text(&mut self, text: &str, node: Node<'_, '_>) -> Option<(usize, bool)> {
        let first = text_offset(text, node)?;
        let piece = node.text().unwrap_or_default().trim_matches(is_xml_space);
        if let Some(Placed {
            index,
            item: Item::Text(joined),
        }) = self.children.last_mut()
        {
            if *index == self.typed {
                joined.push('\n');
                joined.push_str(piece);
                return Some((first, false));
            }
        }
        self.children.push(Placed {
            index: self.typed,
            item: Item::Text(piece.to_owned()),
        });
        Some((first, true))
    }

    /// Takes `attribute` of `node`, XMI metadata (ADR 0042), with the
    /// bindings its prefix needs and the ids it may name, as
    /// [`take_attribute`](Self::take_attribute) does. Its prefix is one the
    /// writer writes back ([`is_writable_prefix`]), so it is never renamed
    /// and never refused for its prefix.
    fn take_metadata_attribute<'a>(
        &mut self,
        found: &Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
    ) -> Result<(), ReadError> {
        let at = attribute.range_qname().start;
        // No rename: the caller checked the prefix, so there is no Warning.
        let _ = self.take_attribute(found, node, attribute, at)?;
        Ok(())
    }

    /// The preserved content, its attributes in input order and its
    /// bindings by prefix.
    fn into_preserved(self) -> Preserved {
        let Self {
            holder: _,
            typed: _,
            mut attributes,
            mut namespaces,
            renamed: _,
            children,
            ids,
            references,
            offsets: _,
        } = self;
        attributes.sort_by_key(|(offset, _)| *offset);
        namespaces.sort();
        namespaces.dedup();
        Preserved {
            namespaces,
            attributes: attributes.into_iter().map(|(_, attr)| attr).collect(),
            children,
            ids,
            references,
        }
    }
}

impl Kept<ElementHolder> {
    /// Keeps `attribute` of `node`, the holder's element, as preserved
    /// content, reported at its name with SACM-XMI-003 (ADR 0033): see
    /// [`take_attribute`](Self::take_attribute), which refuses it at byte
    /// `refuse_at` in strict mode if its prefix cannot be written back.
    fn preserve_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        refuse_at: usize,
    ) -> Result<(), ReadError> {
        if let Some(renamed) = self.take_attribute(found, node, attribute, refuse_at)? {
            found.keep(renamed, &self.holder)?;
        } else {
            let arguments = vec![Argument::text(
                "attribute",
                attribute_written(found.text, attribute),
            )];
            found.unknown(
                attribute.range_qname().start,
                &template::XMI_003_ATTRIBUTE_KEPT,
                arguments,
                &self.holder,
            )?;
        }
        Ok(())
    }

    /// Keeps `attribute` of `node`, the holder's element, as preserved
    /// content, reported as `keep` says, in place of a strict refusal (ADR
    /// 0035 D1). A prefix that cannot be written back is replaced, as
    /// [`take_attribute`](Self::take_attribute) does, with no other Warning.
    fn keep_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        keep: Keep,
    ) -> Result<(), ReadError> {
        self.take_attribute(found, node, attribute, keep.offset)?;
        found.keep(keep, &self.holder)?;
        Ok(())
    }

    /// Keeps `node`, a child element of the holder, with its subtree, as
    /// preserved content, reported at its start tag with SACM-XMI-003 (ADR
    /// 0033); see [`take_element`](Self::take_element).
    fn preserve_element<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
    ) -> Result<(), ReadError> {
        self.take_element(found, node)?;
        let arguments = vec![Argument::text("element", written_name(found.text, node))];
        found.unknown(
            node.range().start,
            &template::XMI_003_ELEMENT_KEPT,
            arguments,
            &self.holder,
        )?;
        Ok(())
    }

    /// Keeps `node`, a child element of the holder, with its subtree, as
    /// preserved content, reported as `keep` says, in place of a strict
    /// refusal (ADR 0035 D1); see [`take_element`](Self::take_element).
    fn keep_element<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        keep: Keep,
    ) -> Result<(), ReadError> {
        self.take_element(found, node)?;
        found.keep(keep, &self.holder)?;
        Ok(())
    }

    /// Takes the proxies `proxies`, each with its slot among the typed
    /// children, in slot order, as preserved content, each placed where it
    /// stood: after every child placed at or before its slot (SACM23-XMI-030).
    /// The caller then drops their slots. One merge, so linear in the
    /// children and the proxies. A proxy carries no `xmi:id`; the id it
    /// names counts as a reference.
    fn take_proxies<'a>(
        &mut self,
        found: &Findings<'_, 'a>,
        proxies: &[(usize, Node<'a, '_>)],
    ) -> Result<(), ReadError> {
        let children = std::mem::take(&mut self.children);
        let mut merged = Vec::with_capacity(children.len().saturating_add(proxies.len()));
        let mut proxies = proxies.iter().peekable();
        let mut take = |merged: &mut Vec<Placed>, slot: usize, node: Node<'a, '_>| {
            for attribute in node.attributes() {
                self.references.extend(
                    reference_tokens(&attribute)
                        .into_iter()
                        .filter(|token| !token.is_empty())
                        .map(ElementId::new),
                );
            }
            self.offsets.insert(node.range().start);
            let fragment = preserve::fragment(found.text, node, &found.budget)?;
            merged.push(Placed {
                index: slot,
                item: Item::Element(fragment),
            });
            Ok::<(), ReadError>(())
        };
        for placed in children {
            while let Some(&&(slot, node)) = proxies.peek() {
                if placed.index <= slot {
                    break;
                }
                take(&mut merged, slot, node)?;
                proxies.next();
            }
            merged.push(placed);
        }
        for &(slot, node) in proxies {
            take(&mut merged, slot, node)?;
        }
        self.children = merged;
        Ok(())
    }

    /// Keeps `node`, a child of the holder, as preserved text if it is text
    /// that is not all XML white space, reported at its first character that
    /// is not literal white space with SACM-XMI-003 (ADR 0033); see
    /// [`take_text`](Self::take_text).
    fn preserve_text(
        &mut self,
        found: &mut Findings<'_, '_>,
        node: Node<'_, '_>,
    ) -> Result<(), ReadError> {
        if let Some((first, true)) = self.take_text(found.text, node) {
            found.unknown(
                first,
                &template::XMI_003_TEXT_KEPT,
                Vec::new(),
                &self.holder,
            )?;
        }
        Ok(())
    }

    /// Keeps `node`, text, as preserved text, reported as `keep` says when
    /// it is a new piece, in place of a strict refusal (ADR 0035 D1); see
    /// [`take_text`](Self::take_text).
    fn keep_text(
        &mut self,
        found: &mut Findings<'_, '_>,
        node: Node<'_, '_>,
        keep: Keep,
    ) -> Result<(), ReadError> {
        if let Some((_, true)) = self.take_text(found.text, node) {
            found.keep(keep, &self.holder)?;
        }
        Ok(())
    }
}

impl Kept<DocumentHolder> {
    /// Keeps `attribute` of `node`, the XMI element, as the document's
    /// preserved content, reported as `keep` says, in place of a strict
    /// refusal (ADR 0035 D1, D3). A prefix that cannot be written back is
    /// replaced, as [`take_attribute`](Self::take_attribute) does, with no
    /// other Warning.
    fn keep_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        keep: DocumentKeep,
    ) -> Result<(), ReadError> {
        self.take_attribute(found, node, attribute, keep.offset)?;
        found.keep_in_document(keep)?;
        Ok(())
    }

    /// Keeps `node`, an element beside the roots or a root, with its
    /// subtree, as the document's preserved content, reported as `keep`
    /// says, in place of a strict refusal (ADR 0035 D1, D3); see
    /// [`take_element`](Self::take_element).
    fn keep_element<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        keep: DocumentKeep,
    ) -> Result<(), ReadError> {
        self.take_element(found, node)?;
        found.keep_in_document(keep)?;
        Ok(())
    }

    /// Keeps `node`, text in the XMI element, as the document's preserved
    /// text, reported as `keep` says when it is a new piece, in place of a
    /// strict refusal (ADR 0035 D1, D3); see [`take_text`](Self::take_text).
    fn keep_text(
        &mut self,
        found: &mut Findings<'_, '_>,
        node: Node<'_, '_>,
        keep: DocumentKeep,
    ) -> Result<(), ReadError> {
        if let Some((_, true)) = self.take_text(found.text, node) {
            found.keep_in_document(keep)?;
        }
        Ok(())
    }
}

impl Pending {
    /// The text read into `role`.
    fn text_mut(&mut self, role: TextRole) -> &mut Option<Text> {
        match role {
            TextRole::Content => &mut self.content,
            TextRole::Key => &mut self.key,
            TextRole::Location => &mut self.location,
        }
    }

    /// Whether `role` is given already: by its role element, or, for the
    /// `content`, in the C++ shape on the element itself.
    fn has_text(&self, role: TextRole) -> bool {
        let given = match role {
            TextRole::Content => self.content.is_some() || self.shorthand.is_given(),
            TextRole::Key => self.key.is_some(),
            TextRole::Location => self.location.is_some(),
        };
        given || self.given.contains(&role.name())
    }

    /// Reads `node`, the value element of the string-valued `attribute` in
    /// `holder`, this element's node, whose kind holds it (D-DATE): checked
    /// whole first, then against an earlier occurrence, the attribute
    /// included, as a `gid` is. A nil one reads as absent and counts as
    /// given (SACM23-XMI-026).
    fn read_string_element(
        &mut self,
        found: &Findings<'_, '_>,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        attribute: StringAttribute,
    ) -> Result<(), Fault> {
        let value = read_value(found, node, &self.id);
        let counted = counts(&value);
        let repeats = counted && self.strings.iter().any(|(given, _)| *given == attribute);
        if counted && !repeats {
            // Given, with no value until it is known: one kept in tolerant
            // mode for an `xsi:nil` that is no literal stays absent.
            self.strings.push((attribute, None));
        }
        let value = value?;
        if repeats {
            let name = attribute.name();
            return Err(self.repeat(found, holder, node.range().start, name, name));
        }
        if let Some((_, held)) = self
            .strings
            .iter_mut()
            .find(|(given, _)| *given == attribute)
        {
            *held = value;
        }
        Ok(())
    }

    /// Reads `node`, text in `holder`, this element's node, a
    /// `UtilityElement`: the C++ shape of its `content` (LIB-COMPAT-008),
    /// with any `lang=` and `content=` it carries. Text that is all white
    /// space is no shape but joins one. A `content` element given as well is
    /// `RepeatedValue`, and so is `content=` with text; a tolerant read
    /// keeps the text as preserved content instead, with one SACM-MULT-001
    /// Warning at it (ADR 0035 D1).
    fn read_utility_text(
        &mut self,
        found: &mut Findings<'_, '_>,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
    ) -> Result<(), ReadError> {
        let text = found.text;
        let Some(first) = self.shorthand.add_text(text, node) else {
            return Ok(());
        };
        let keep = if self.content.is_some() {
            repeat_or_refuse(found.mode, first, &self.id, "content", || {
                repeated_content(
                    text,
                    holder,
                    &self.id,
                    self.kind,
                    &self.shorthand,
                    &self.kept,
                )
            })?
        } else if let Some(attribute) = self.shorthand.content_at {
            let property = "content.value.content";
            repeat_or_refuse(found.mode, first, &self.id, property, || {
                repeated(text, &self.id, self.kind, property, [attribute, first])
            })?
        } else {
            return Ok(());
        };
        // The text is no part of the content: it is kept as it is. With no
        // text where the shape starts, the characters gathered are never
        // read (`Shorthand::into_lang_string`), and the next text that is not
        // all white space is a repeat too.
        self.shorthand.text_at = None;
        self.kept.keep_text(found, node, keep)?;
        Ok(())
    }

    /// Ends the element once its content is read: a `content` given in the
    /// C++ shape on a `UtilityElement` becomes its content, a
    /// `MultiLangString` of one value, reported at the shape
    /// (LIB-COMPAT-008). A `lang` attribute that gives no content is no
    /// shape: it is preserved (ADR 0033).
    fn finish(&mut self, found: &mut Findings<'_, '_>) -> Result<(), ReadError> {
        let mut shorthand = std::mem::take(&mut self.shorthand);
        let lang = shorthand.lang_attr.take();
        let (value, at) = match shorthand.into_lang_string() {
            Ok(Some(given)) => given,
            Ok(None) => return Ok(()),
            Err(at) => {
                if let Some(attr) = lang {
                    let arguments = vec![Argument::text("attribute", attr.name.written())];
                    self.kept.offsets.insert(at);
                    self.kept.attributes.push((at, attr));
                    found.unknown(
                        at,
                        &template::XMI_003_ATTRIBUTE_KEPT,
                        arguments,
                        &self.kept.holder,
                    )?;
                }
                return Ok(());
            }
        };
        let shape = named(self.kind, self.id.as_str());
        found.legacy_shape(at, &self.id, Shape::ContentOnElement, shape)?;
        let mut held = Text::default();
        held.push(value, None, Preserved::default());
        self.content = Some(held);
        Ok(())
    }

    /// Reads `node`, a `name` in `holder`, this element's node, a
    /// `ModelElement` (SACM23-XMI-012): checked whole first, then against an
    /// earlier occurrence, the C++ `name` attribute included (SACM23-XMI-021).
    /// A name kept in tolerant mode for a value in it that is no literal
    /// counts as an occurrence (ADR 0035).
    fn read_name_part<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        holder: Node<'a, '_>,
        node: Node<'a, '_>,
    ) -> Result<(), Fault> {
        let name = read_name(found, node, self.kind, &self.id);
        let counted = counts(&name);
        let repeats = counted && (self.name.is_some() || self.given.contains(&"name"));
        if counted && !repeats {
            self.given.push("name");
        }
        let (name, name_id, metadata) = name?;
        if repeats {
            return Err(self.repeat(found, holder, node.range().start, "name", "name"));
        }
        self.name = Some(name);
        self.name_id = name_id;
        self.name_metadata = Some(metadata);
        Ok(())
    }

    /// Reads `node`, the `assertionDeclaration` value element in `holder`,
    /// this element's node, an Assertion (SACM23-XMI-014): checked whole
    /// first, then against an earlier occurrence, the attribute included. A
    /// nil one reads as one not written, `asserted` (SACM23-XMI-026); one
    /// that is no literal, kept in tolerant mode, counts as an occurrence, and
    /// the Assertion holds `asserted` (ADR 0035).
    fn read_declaration_part(
        &mut self,
        found: &Findings<'_, '_>,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
    ) -> Result<(), Fault> {
        let at = node.range().start;
        let declaration = match read_value(found, node, &self.id) {
            Ok(Some(value)) => parse_declaration(found, &value, at, &self.id),
            Ok(None) => Ok(AssertionDeclaration::Asserted),
            Err(fault) => Err(fault),
        };
        let counted = counts(&declaration);
        let repeats = counted && self.declaration.is_some();
        if counted && !repeats {
            // Held until the value is known: the default, for one that is
            // no literal.
            self.declaration = Some(AssertionDeclaration::Asserted);
        }
        let declaration = declaration?;
        if repeats {
            return Err(self.repeat(found, holder, at, DECLARATION, DECLARATION));
        }
        self.declaration = Some(declaration);
        Ok(())
    }

    /// Reads `node`, the value element of the flag `flag` in `holder`, this
    /// element's node (SACM23-BASE-007, SACM23-BASE-008): checked whole
    /// first, then against an earlier occurrence, the attribute included. A
    /// value that is no literal, kept in tolerant mode, counts as an
    /// occurrence, and the flag holds false (ADR 0035).
    fn read_flag_element(
        &mut self,
        found: &Findings<'_, '_>,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        flag: Flag,
    ) -> Result<(), Fault> {
        let at = node.range().start;
        // A nil flag reads as the flag not written: false (SACM23-XMI-026).
        let value = match read_value(found, node, &self.id) {
            Ok(Some(value)) => parse_flag(found, &value, at, &self.id, flag),
            Ok(None) => Ok(false),
            Err(fault) => Err(fault),
        };
        let counted = counts(&value);
        let repeats = counted && self.flags.iter().any(|(given, _)| *given == flag);
        if counted && !repeats {
            self.flags.push((flag, false));
        }
        let value = value?;
        if repeats {
            let property = flag.name();
            return Err(self.repeat(found, holder, at, property, property));
        }
        if let Some((_, held)) = self.flags.iter_mut().find(|(given, _)| *given == flag) {
            *held = value;
        }
        Ok(())
    }

    /// Reads `node`, a reference element of the end `end` in `holder`, this
    /// element's node (SACM23-XMI-015): checked whole first
    /// ([`read_reference`]), then against an earlier occurrence, the
    /// attribute (XMI 2.5.1 rule 2h) or an earlier element.
    ///
    /// On a multi-valued end, each element appends its target, in document
    /// order (XMI 2.5.1 rule 2b; ADR 0027), unless the end is also given as
    /// an attribute, which is `RepeatedValue`. On a single-valued end any
    /// second occurrence is `RepeatedValue`.
    fn read_reference_element(
        &mut self,
        found: &Findings<'_, '_>,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        end: ReferenceEnd,
    ) -> Result<(), Fault> {
        let link = read_reference(found, node, true)?;
        // An occurrence of the end is an id or an `href` (ADR 0040 D5).
        let given = self.references.iter().any(|(held, _)| *held == end)
            || self.hrefs.iter().any(|(held, _)| *held == end);
        if given {
            let property = end.name();
            // The attribute of the end, or a spelling of it a tolerant read
            // read (SACM23-XMI-009); one kept as preserved content is none.
            let in_attribute = holder.attributes().any(|attribute| {
                (is_unqualified(&attribute, property)
                    || (attribute.namespace().is_none()
                        && spelled_end(self.kind, attribute.name()) == Some(end)))
                    && !self.kept.holds(attribute.range_qname().start)
            });
            if !end.is_many() || in_attribute {
                let at = node.range().start;
                return Err(self.repeat(found, holder, at, property, property));
            }
        }
        match link {
            Link::Id(target) => push_value(&mut self.references, end, target),
            Link::Href(href) => push_value(&mut self.hrefs, end, href),
        }
        Ok(())
    }

    /// The fault of the occurrence at byte `at` of the property `property`
    /// of this element, whose node is `holder`, given more than once as the
    /// attribute or the value element `local`: the `RepeatedValue` refusal,
    /// as [`repeated_value`] finds it, without the occurrences kept as
    /// preserved content, which are none (ADR 0033), or, in tolerant mode,
    /// the Warning the occurrence is kept with (ADR 0035 D1).
    fn repeat(
        &self,
        found: &Findings<'_, '_>,
        holder: Node<'_, '_>,
        at: usize,
        local: &str,
        property: &str,
    ) -> Fault {
        let refusal = || {
            repeated_value(
                found.text, holder, local, &self.id, self.kind, property, &self.kept,
            )
        };
        fault(repeat_or_refuse(
            found.mode, at, &self.id, property, refusal,
        ))
    }

    /// Reads `attribute` of `node`, this element's node, a `UtilityElement`:
    /// the C++ shape of its content (LIB-COMPAT-008), `lang=`, `content=` or,
    /// on a `TaggedValue`, `value=`. A second attribute giving the content is
    /// `RepeatedValue` in strict mode. A tolerant read keeps the shape whole,
    /// both attributes, and the element has no content from it, as for a
    /// repeat inside a `content` (ADR 0035 D1): kept alone, the later one
    /// would be read again before the content written.
    fn read_content_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
    ) -> Result<(), ReadError> {
        let text = found.text;
        let Err(earlier) = self.shorthand.read_attribute(text, attribute) else {
            return Ok(());
        };
        let at = attribute.range_qname().start;
        let property = "content.value.content";
        let (id, kind) = (&self.id, self.kind);
        let mut kept = fault(repeat_or_refuse(found.mode, at, id, property, || {
            repeated(text, id, kind, property, [earlier, at])
        }));
        if let Fault::Keep(keep) = &mut kept {
            // The repeat's form that says the first attribute is kept too.
            keep.template = &template::MULT_001_REPEAT_ATTRIBUTE_KEPT;
            let first = node
                .attributes()
                .find(|given| given.range_qname().start == earlier);
            if let Some(first) = first {
                self.kept.take_attribute(found, node, &first, earlier)?;
            }
            self.shorthand.content = None;
            self.shorthand.content_at = None;
        }
        self.keep_attribute(found, node, attribute, kept)
    }

    /// Reads `attribute`, the C++ `key="…"` of this element, a `TaggedValue`,
    /// as its `key`: one value with no `lang`, reported (LIB-COMPAT-008).
    fn read_key_attribute(
        &mut self,
        found: &mut Findings<'_, '_>,
        attribute: &Attribute<'_, '_>,
    ) -> Result<(), ReadError> {
        let shape = named(self.kind, self.id.as_str());
        found.legacy_shape(
            attribute.range_qname().start,
            &self.id,
            Shape::KeyAttribute,
            shape,
        )?;
        let mut value = LangString::default();
        value.set_content(Some(attribute.value().to_owned()));
        let mut held = Text::default();
        held.push(value, None, Preserved::default());
        self.key = Some(held);
        Ok(())
    }

    /// Keeps `attribute` of `node`, this element's node, as `fault` says: a
    /// refusal is returned, a construct a tolerant read keeps is preserved
    /// with its Warning (ADR 0035 D1), and one that does not conform with
    /// SACM-XMI-003 (ADR 0033).
    fn keep_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        fault: Fault,
    ) -> Result<(), ReadError> {
        match fault {
            Fault::Refused(error) => Err(error),
            Fault::Keep(keep) => self.kept.keep_attribute(found, node, attribute, keep),
            Fault::Defect => {
                let at = attribute.range_qname().start;
                self.kept.preserve_attribute(found, node, attribute, at)
            }
        }
    }

    /// Reads `attribute` of `node`, this element's node, as its date if it
    /// is an Event's `occurence`, the spelling of SACM 2.0 to 2.2, in a root
    /// of an older revision, and the date is not given too, which is read
    /// instead; and returns whether it was (LIB-COMPAT-004, SACM-XMI-008).
    fn read_older_date(
        &mut self,
        found: &mut Findings<'_, '_>,
        node: Node<'_, '_>,
        attribute: &Attribute<'_, '_>,
    ) -> Result<bool, ReadError> {
        let Some(revision) = found.revision.filter(|_| {
            self.kind == ElementKind::Event
                && is_unqualified(attribute, OLDER_EVENT_SPELLING)
                && !gives(node, StringAttribute::Date.name())
        }) else {
            return Ok(false);
        };
        self.strings
            .push((StringAttribute::Date, Some(attribute.value().to_owned())));
        found.occurence(attribute.range_qname().start, &self.id, revision)?;
        Ok(true)
    }

    /// Reads `attribute` of `node`, this element's node, if it is a spelling
    /// of SACM23-XMI-009 on its kind (an Event's `occurece`, an
    /// `ArgumentGroup`'s `argumentationElement`), and returns whether it was
    /// one. A tolerant read reads it as the resolved name, with one
    /// SACM-XMI-013 Warning at its name, under every rule of that name (an
    /// end into `start`), unless that name is given too, as an attribute or
    /// an element, which is read instead: the spelling is then the repeat
    /// (SACM-MULT-001). A strict read keeps it as preserved content with one
    /// SACM-XMI-013 Error (ADR 0034 amendment 2).
    fn read_spelled_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        start: &mut StartTag,
    ) -> Result<bool, ReadError> {
        if attribute.namespace().is_some() {
            return Ok(false);
        }
        let at = attribute.range_qname().start;
        let (spelling, resolved) = if spelled_string(self.kind, attribute.name()).is_some() {
            (Spelling::Date, StringAttribute::Date.name())
        } else if spelled_end(self.kind, attribute.name()).is_some() {
            (Spelling::End, TEXT_RESOLVED)
        } else {
            return Ok(false);
        };
        let spelled = Spelled::on(spelling, &self.written);
        let fault = if found.mode == Mode::Strict {
            Fault::Keep(Keep::spelling(at, spelled))
        } else if gives(node, resolved) {
            self.repeat(found, node, at, resolved, resolved)
        } else {
            let read = if spelling == Spelling::Date {
                let value = attribute.value().to_owned();
                self.strings.push((StringAttribute::Date, Some(value)));
                Ok(Given::Read)
            } else {
                start.read_end(
                    found,
                    attribute,
                    self.kind,
                    ReferenceEnd::GroupMember,
                    &self.id,
                )
            };
            match read {
                Ok(Given::Read) => {
                    found.spelled(at, spelled, &self.id)?;
                    return Ok(true);
                }
                Ok(Given::NoId | Given::Other) => Fault::Defect,
                Err(fault) => fault,
            }
        };
        self.keep_attribute(found, node, attribute, fault)?;
        Ok(true)
    }
}

/// What the start tag of an element gives for its `SACMElement` flags and
/// reference ends (ADR 0026).
#[derive(Default)]
struct StartTag {
    /// Each flag given, with its value.
    flags: Vec<(Flag, bool)>,
    /// Each reference end given, with its targets.
    references: Vec<(ReferenceEnd, Vec<ElementId>)>,
}

/// What [`StartTag::read`] made of an attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Given {
    /// A flag or a reference end of the element, read.
    Read,
    /// A reference end of the element that holds no id, which does not
    /// conform: the caller preserves it (ADR 0033), never reading it as no
    /// reference.
    NoId,
    /// Neither a flag nor a reference end of the element.
    Other,
}

impl StartTag {
    /// Reads `attribute` of the element `id`, of `kind`, if it is a flag or
    /// a reference end of the element in no namespace, and says what it
    /// was. Attributes are unique in a start tag (XML 1.0 §3.1), so none
    /// repeats here.
    ///
    /// A flag is an `xsd:boolean` ([`parse_flag`]). A reference holds the
    /// ids its value lists, each token exactly as written, in order and
    /// with repeats (SACM23-XMI-005, ADR 0027): none is [`Given::NoId`],
    /// and two or more on a single-valued end is `RepeatedValue`, with one
    /// location per token, each at the attribute's name. A flag or an end
    /// the kind lacks, such as `isCounter` on a Claim, is
    /// [`Given::Other`].
    ///
    /// In tolerant mode, a flag that is no literal, and two ids on a
    /// single-valued end, are [`Fault::Keep`]: the caller keeps the
    /// attribute as preserved content, and it counts as the property's
    /// occurrence, holding false or no id (ADR 0035 D1).
    fn read(
        &mut self,
        found: &Findings<'_, '_>,
        attribute: &Attribute<'_, '_>,
        kind: ElementKind,
        id: &ElementId,
    ) -> Result<Given, Fault> {
        if attribute.namespace().is_some() {
            return Ok(Given::Other);
        }
        let at = attribute.range_qname().start;
        let local = attribute.name();
        if let Some(flag) = flag_named(kind, local) {
            let value = parse_flag(found, attribute.value(), at, id, flag);
            self.flags.push((flag, *value.as_ref().unwrap_or(&false)));
            value?;
            return Ok(Given::Read);
        }
        let Some(end) = reference_end_named(kind, local) else {
            return Ok(Given::Other);
        };
        self.read_end(found, attribute, kind, end, id)
    }

    /// Reads `attribute` of the element `id` as its reference end `end`, as
    /// [`read`](Self::read) does: the end `attribute` names, or, for a
    /// spelling a tolerant read reads, the end it stands for
    /// (SACM23-XMI-009).
    fn read_end(
        &mut self,
        found: &Findings<'_, '_>,
        attribute: &Attribute<'_, '_>,
        kind: ElementKind,
        end: ReferenceEnd,
        id: &ElementId,
    ) -> Result<Given, Fault> {
        let text = found.text;
        let at = attribute.range_qname().start;
        let local = attribute.name();
        let value = attribute.value();
        let count = id_tokens(value).count();
        if count == 0 {
            return Ok(Given::NoId);
        }
        if count > 1 && !end.is_many() {
            let starts = token_starts(text, attribute)
                .unwrap_or_else(|| std::iter::repeat_n(at, count).collect());
            let second = starts.get(1).copied().unwrap_or(at);
            self.references.push((end, Vec::new()));
            return Err(fault(repeat_or_refuse(
                found.mode,
                second,
                id,
                local,
                || {
                    let mut locator = Locator::new(text);
                    ReadError::repeated_value(
                        id.clone(),
                        kind,
                        local.to_owned(),
                        starts
                            .into_iter()
                            .filter_map(|start| locator.locate(start))
                            .collect(),
                    )
                },
            )));
        }
        let targets = id_tokens(value).map(ElementId::new).collect();
        self.references.push((end, targets));
        Ok(Given::Read)
    }
}

/// The single-language shorthand of a `MultiLangString`, a C++ shape
/// (LIB-COMPAT-008): its one `LangString`, given by a `content` attribute
/// (on a `TaggedValue` also `value`) or by text, with an optional `lang`
/// attribute, on a `content` or `key` role element or on a
/// `UtilityElement` itself. A `lang` alone gives no shape.
#[derive(Default)]
struct Shorthand {
    lang: Option<String>,
    /// The byte offset of the `lang` attribute.
    lang_at: Option<usize>,
    /// The `lang` attribute as written, kept as preserved content if it
    /// gives no shape (ADR 0033).
    lang_attr: Option<Attr>,
    /// The content given by an attribute.
    content: Option<String>,
    /// The byte offset of the attribute that gave the content.
    content_at: Option<usize>,
    /// Every text read so far, joined in document order.
    characters: String,
    /// The first text character that is not literal white space, once some
    /// text is not all white space.
    text_at: Option<usize>,
}

impl Shorthand {
    /// Whether the shape is given: a content, by an attribute or by text.
    fn is_given(&self) -> bool {
        self.content_at.is_some() || self.text_at.is_some()
    }

    /// Where the shape is, if it is given: its first attribute's name, else
    /// its first text character that is not literal white space.
    fn at(&self) -> Option<usize> {
        if !self.is_given() {
            return None;
        }
        [self.lang_at, self.content_at]
            .into_iter()
            .flatten()
            .min()
            .or(self.text_at)
    }

    /// Reads `attribute`, the `lang`, or one that gives the content.
    /// Returns the byte offset of an earlier attribute that gave the content,
    /// if this one gives it again.
    fn read_attribute(&mut self, text: &str, attribute: &Attribute<'_, '_>) -> Result<(), usize> {
        let at = attribute.range_qname().start;
        let value = attribute.value().to_owned();
        if attribute.name() == "lang" {
            self.lang = Some(value);
            self.lang_at = Some(at);
            self.lang_attr = Some(preserve::attr(text, attribute).0);
        } else {
            if let Some(earlier) = self.content_at {
                return Err(earlier);
            }
            self.content = Some(value);
            self.content_at = Some(at);
        }
        Ok(())
    }

    /// Adds `node`, a child that is not an element, to the text, and
    /// returns where it is if it is the first that is not all white space.
    fn add_text(&mut self, text: &str, node: Node<'_, '_>) -> Option<usize> {
        if node.is_text() {
            self.characters.push_str(node.text().unwrap_or_default());
        }
        if self.text_at.is_some() {
            return None;
        }
        let first = text_offset(text, node)?;
        self.text_at = Some(first);
        Some(first)
    }

    /// The `LangString` given, and where the shape is, if it is given: the
    /// text, all of it, is its content when some of it is not white space.
    /// A `lang` with no content is no shape: `Err` holds where the
    /// attribute is, which is then not read.
    fn into_lang_string(self) -> Result<Option<(LangString, usize)>, usize> {
        let Some(at) = self.at() else {
            return self.lang_at.map_or(Ok(None), Err);
        };
        let mut value = LangString::default();
        value.set_lang(self.lang);
        value.set_content(if self.text_at.is_some() {
            Some(self.characters)
        } else {
            self.content
        });
        Ok(Some((value, at)))
    }
}

/// What pass 2 has read of the roots before any is held: every element of
/// their trees, the place in document order of each root, and the same-file
/// proxies (SACM23-XMI-030), each in reading order.
#[derive(Default)]
struct Roots {
    read: Vec<Pending>,
    orders: Vec<usize>,
    proxies: Vec<Proxy>,
}

/// A same-file proxy as read (SACM23-XMI-030): what it says, where it is,
/// and the element that holds it.
struct Proxy {
    /// The proxy element, in the parsed tree.
    node: roxmltree::NodeId,
    /// The byte offset of its start tag.
    at: usize,
    /// Its name as written: the role.
    written: String,
    link: ProxyLink,
    /// The element that holds it, its kind and its place in document order.
    container: ElementId,
    container_kind: ElementKind,
    container_order: usize,
    /// The index among the roots of the root whose tree holds it.
    tree: usize,
    /// Its index among the typed children of its container: the place the
    /// root it names takes.
    slot: usize,
    /// Its place in document order, which the root it names takes.
    order: usize,
}

/// Pass 2 for one root: checks it and its tree in document order, and adds
/// the elements read to `roots`, to be held once every root is read (ADR
/// 0010 D2, SACM23-XMI-030); `outside`, the document's preserved content,
/// counts it among the roots (ADR 0035 D3). The tree is walked with a
/// stack, not by recursion, so a deep document cannot exhaust the call
/// stack.
fn read_root<'a>(
    found: &mut Findings<'_, 'a>,
    root: Node<'a, '_>,
    roots: &mut Roots,
    outside: &mut Kept<DocumentHolder>,
) -> Result<(), ReadError> {
    let first = root_start(found, root)?;
    let tree = roots.orders.len();
    roots.orders.push(first.order);
    // The elements whose content is being read, innermost last, each with
    // its node and the rest of its content, and the elements read whole.
    // Content is, on a ModelElement, one `name` (SACM23-XMI-012) and on an
    // Assertion one `assertionDeclaration` value element (SACM23-XMI-014);
    // on a UtilityElement one `content`, on a TaggedValue one `key`
    // (SACM23-BASE-004, SACM23-BASE-005), and on a Resource one `location`
    // (SACM23-ART-005); on every element one `gid` value element
    // (SACM23-BASE-006), one value element per string-valued attribute and
    // one per flag of its kind, one
    // reference element per single-valued end of its kind and any number per
    // multi-valued end (ADR 0026, ADR 0027); readable children
    // (SACM23-XMI-016), same-file proxies (SACM23-XMI-030), XMI extensions
    // (SACM23-XMI-019), white space, comments and processing instructions.
    let mut open = vec![(first, root, root.children())];
    while let Some((element, holder, content)) = open.last_mut() {
        let Some(node) = content.next() else {
            if let Some((mut element, _, _)) = open.pop() {
                element.finish(found)?;
                roots.read.push(element);
            }
            continue;
        };
        let holder = *holder;
        if !node.is_element() {
            if element.kind.is_a(AbstractClass::UtilityElement) {
                // Text on a UtilityElement is the C++ form of its content.
                element.read_utility_text(found, holder, node)?;
            } else {
                // Text in element-only content does not conform (ADR 0033).
                element.kept.preserve_text(found, node)?;
            }
            continue;
        }
        // An extension is XMI metadata, kept whole where it stands among
        // the typed children (SACM23-XMI-019, ADR 0042).
        if is_extension(node) {
            element.metadata.typed = element.kept.typed;
            element.metadata.take_element(found, node)?;
            continue;
        }
        // A property part is read whole, and kept as preserved content
        // instead, as if it had not been read, if it turns out not to
        // conform (ADR 0033), or, in tolerant mode, if a strict read
        // refuses it (ADR 0035 D1). So is a child.
        let checkpoint = found.checkpoint();
        let outcome = match read_part(found, element, holder, node) {
            Ok(true) => Ok(None),
            Ok(false) => match child_start(found, node, element) {
                Ok(Child::Preserve) => Err(Fault::Defect),
                Ok(child) => Ok(Some(child)),
                Err(fault) => Err(fault),
            },
            Err(fault) => Err(fault),
        };
        match outcome {
            Ok(None) => {}
            Ok(Some(Child::Element(child))) => {
                element.kept.typed = element.kept.typed.saturating_add(1);
                open.push((*child, node, node.children()));
            }
            Ok(Some(Child::Proxy(link))) => {
                let proxy = read_proxy(found, node, link, element, tree)?;
                roots.proxies.push(proxy);
                element.kept.typed = element.kept.typed.saturating_add(1);
            }
            Ok(Some(Child::Preserve)) | Err(Fault::Defect) => {
                found.rollback(checkpoint);
                element.kept.preserve_element(found, node)?;
            }
            Err(Fault::Refused(error)) => return Err(error),
            Err(Fault::Keep(keep)) => {
                found.rollback(checkpoint);
                element.kept.keep_element(found, node, keep)?;
            }
        }
    }
    outside.typed = outside.typed.saturating_add(1);
    Ok(())
}

/// The proxy `node`, which `link` describes, a child of `container` in the
/// tree of the root `tree`, as read (SACM23-XMI-030): it takes the next typed
/// slot of its container and the next place in document order. Its
/// `xsi:type` is reported as SACM-XMI-010 on the element it names, and a
/// role or type spelling a tolerant read read as SACM-XMI-013, at its start
/// tag, as a child's are.
fn read_proxy(
    found: &mut Findings<'_, '_>,
    node: Node<'_, '_>,
    mut link: ProxyLink,
    container: &Pending,
    tree: usize,
) -> Result<Proxy, ReadError> {
    let target = ElementId::new(link.target.as_str());
    if let Some(spelled) = link.spelled.take() {
        found.spelled_child(node, spelled, &container.id, &target)?;
    }
    if let Some(class) = link.class {
        for attribute in node.attributes().filter(is_xsi_type) {
            let arguments = named(class, target.as_str());
            found.xsi_type(
                &attribute,
                &template::XMI_010_XSI_TYPE,
                arguments,
                target.as_str(),
            )?;
        }
    }
    Ok(Proxy {
        node: node.id(),
        at: node.range().start,
        written: written_name(found.text, node),
        link,
        container: container.id.clone(),
        container_kind: container.kind,
        container_order: container.order,
        tree,
        slot: container.kept.typed,
        order: found.next_order(),
    })
}

/// Reads `node`, an element in `holder`, the node of `element`, if it is a
/// property part of the element, and returns whether it was one. A part is
/// a `name` on a `ModelElement` (SACM23-XMI-012), an `assertionDeclaration`
/// value element on an Assertion (SACM23-XMI-014), a `content` on a
/// `UtilityElement` and a `key` on a `TaggedValue` (SACM23-BASE-004,
/// SACM23-BASE-005), a `gid` value element on every element
/// (SACM23-BASE-006), a value element per string-valued attribute of its
/// kind (`version`, `date`, `startTime`, `endTime`; SACM23-ART-002,
/// SACM23-ART-004), a `location` on a `Resource` (SACM23-ART-005), a value
/// element per flag of its kind, and a
/// reference element per end of its kind, one per single-valued end and
/// any number per multi-valued end (ADR 0026, ADR 0027). Each is checked
/// whole first, then against the occurrences before it that were read, so a
/// second is `RepeatedValue` (SACM23-XMI-021; ADR 0021 amendment 2).
///
/// A part that does not conform, and a `name` on a `UtilityElement`, is
/// [`Fault::Defect`]: the caller keeps it whole as preserved content, and
/// the property is not given (ADR 0033). The spellings
/// `argumentationElement` on an `ArgumentGroup` and `occurece` on an `Event`
/// are read in tolerant mode as `argumentElement` and `date`, with one
/// SACM-XMI-013 Warning each, under every rule of those names; `occurece`
/// beside a `date` is the repeat. A strict read keeps them with an Error
/// (SACM23-XMI-009).
fn read_part<'a>(
    found: &mut Findings<'_, 'a>,
    element: &mut Pending,
    holder: Node<'a, '_>,
    node: Node<'a, '_>,
) -> Result<bool, Fault> {
    let text = found.text;
    let local = node.tag_name().name();
    let at = node.range().start;
    if read_spelled_part(found, element, holder, node)? {
        return Ok(true);
    }
    // Each part is checked whole first, then against the occurrences before
    // it; one kept in tolerant mode for a value that is no literal counts
    // as an occurrence (ADR 0035).
    if is_value_element(node, "name") {
        if element.kind.is_a(AbstractClass::UtilityElement) {
            // A UtilityElement has no name.
            return Err(Fault::Defect);
        }
        element.read_name_part(found, holder, node)?;
    } else if is_value_element(node, GID) {
        let value = read_value(found, node, &element.id);
        let repeats = counts(&value) && std::mem::replace(&mut element.has_gid, true);
        let value = value?;
        if repeats {
            return Err(element.repeat(found, holder, at, GID, GID));
        }
        element.gid = value;
    } else if let Some(attribute) = string_attribute_named(element.kind, local)
        .filter(|attribute| is_value_element(node, attribute.name()))
    {
        element.read_string_element(found, holder, node, attribute)?;
    } else if let Some(role) = TextRole::ALL
        .into_iter()
        .find(|role| role.is_held_by(element.kind) && is_value_element(node, role.name()))
    {
        let held = read_text(found, node, role, element.kind, &element.id);
        let counted = counts(&held);
        let repeats = counted && element.has_text(role);
        if counted && !repeats {
            element.given.push(role.name());
        }
        let held = held?;
        if repeats {
            return Err(match role {
                TextRole::Content => {
                    let refusal = || {
                        repeated_content(
                            text,
                            holder,
                            &element.id,
                            element.kind,
                            &element.shorthand,
                            &element.kept,
                        )
                    };
                    fault(repeat_or_refuse(
                        found.mode,
                        at,
                        &element.id,
                        "content",
                        refusal,
                    ))
                }
                TextRole::Key | TextRole::Location => {
                    let name = role.name();
                    element.repeat(found, holder, at, name, name)
                }
            });
        }
        *element.text_mut(role) = Some(held);
    } else if element.kind.is_a(AbstractClass::Assertion) && is_value_element(node, DECLARATION) {
        element.read_declaration_part(found, holder, node)?;
    } else if let Some(flag) =
        flag_named(element.kind, local).filter(|flag| is_value_element(node, flag.name()))
    {
        element.read_flag_element(found, holder, node, flag)?;
    } else if let Some(end) =
        reference_end_named(element.kind, local).filter(|end| is_value_element(node, end.name()))
    {
        element.read_reference_element(found, holder, node, end)?;
    } else {
        return Ok(false);
    }
    Ok(true)
}

/// Reads `node`, an element in `holder`, the node of `element`, if it is a
/// spelling of SACM23-XMI-009 on the element's kind, and returns whether it
/// was one: an `ArgumentGroup`'s end `argumentationElement` (§11.2), read as a
/// reference element of `argumentElement`, or an Event's `occurece`, read as
/// its `date` value element (§12.9), unless `date` is given too, as an
/// attribute or an element, which is read instead: the spelling is then the
/// repeat (SACM-MULT-001). Each is read under every rule of the resolved
/// name, with one SACM-XMI-013 Warning at its start tag once it is read. A
/// strict read keeps it as preserved content with one SACM-XMI-013 Error
/// (ADR 0034 amendment 2).
fn read_spelled_part<'a>(
    found: &mut Findings<'_, 'a>,
    element: &mut Pending,
    holder: Node<'a, '_>,
    node: Node<'a, '_>,
) -> Result<bool, Fault> {
    let local = node.tag_name().name();
    if !is_value_element(node, local) {
        return Ok(false);
    }
    let at = node.range().start;
    let spelling = if spelled_end(element.kind, local).is_some() {
        Spelling::End
    } else if spelled_string(element.kind, local).is_some() {
        Spelling::Date
    } else {
        return Ok(false);
    };
    let spelled = Spelled::on(spelling, &element.written);
    if found.mode == Mode::Strict {
        return Err(Fault::Keep(Keep::spelling(at, spelled)));
    }
    if spelling == Spelling::End {
        element.read_reference_element(found, holder, node, ReferenceEnd::GroupMember)?;
    } else {
        let date = StringAttribute::Date.name();
        if gives(holder, date) {
            return Err(element.repeat(found, holder, at, date, date));
        }
        element.read_string_element(found, holder, node, StringAttribute::Date)?;
    }
    found.spelled(at, spelled, &element.id)?;
    Ok(true)
}

/// Why a same-file proxy is kept instead of resolved (SACM23-XMI-030), in
/// the order of the checks: the id names no root; the root's kind is not of
/// the role's declared type or of the proxy's type, which is `class`;
/// another proxy names the root too; or the proxies form a cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unresolved {
    NoRoot,
    WrongKind {
        target_class: ElementKind,
        class: &'static str,
    },
    Shared,
    Cycle,
}

/// Resolves the same-file proxies of `roots` once every root is read
/// (SACM23-XMI-030; XMI 2.5.1 §7.10.1 "When following the link from a
/// proxy, the definition of the proxy is replaced by the referenced
/// element"), and returns which roots stay roots, by index.
///
/// A proxy whose id names a root of a kind its role and its type admit,
/// that no other such proxy names, and that is in no cycle, moves the root
/// into its place: the root gets the proxy's container as its parent and
/// the proxy's place in document order. Chains are followed, since each
/// moved root keeps its tree. Every other proxy is kept whole as preserved
/// content of its container, at its place, with one SACM-XMI-003 at its
/// `<` affecting the container: an Error in strict mode, a Warning in
/// tolerant mode. Linear in the roots and the proxies, but for the sort of
/// the root ids and the grouping by container.
fn resolve(
    found: &mut Findings<'_, '_>,
    tree: &roxmltree::Document<'_>,
    roots: &mut Roots,
) -> Result<Vec<bool>, ReadError> {
    roots.read.sort_by_key(|element| element.order);
    let index_of = |read: &[Pending], order: usize| {
        read.binary_search_by_key(&order, |element| element.order)
            .ok()
    };
    let mut stays = vec![true; roots.orders.len()];
    if roots.proxies.is_empty() {
        return Ok(stays);
    }
    let mut outcomes = proxy_roots(roots);
    mark_shared(&mut outcomes, roots.orders.len());
    mark_cycles(&mut outcomes, &roots.proxies, roots.orders.len());
    // Each resolved root moves; each kept proxy is reported, then kept in
    // its container, at its place, which is no typed slot any more.
    let mut kept: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let Some(proxy) = roots.proxies.get(index) else {
            continue;
        };
        match outcome {
            Ok(root) => {
                if let Some(flag) = stays.get_mut(root) {
                    *flag = false;
                }
                let moved = roots
                    .orders
                    .get(root)
                    .and_then(|&order| index_of(&roots.read, order));
                if let Some(element) = moved.and_then(|index| roots.read.get_mut(index)) {
                    element.parent = Some(proxy.container.clone());
                    element.parent_order = Some(proxy.container_order);
                    element.place = proxy.order;
                }
            }
            Err(why) => {
                found.unresolved(proxy, why)?;
                if let Some(container) = index_of(&roots.read, proxy.container_order) {
                    kept.entry(container).or_default().push(index);
                }
            }
        }
    }
    for (container, held) in kept {
        let Some(element) = roots.read.get_mut(container) else {
            continue;
        };
        let mut slots = vec![true; element.kept.typed];
        let mut nodes = Vec::with_capacity(held.len());
        for proxy in held.iter().filter_map(|&index| roots.proxies.get(index)) {
            if let Some(slot) = slots.get_mut(proxy.slot) {
                *slot = false;
            }
            if let Some(node) = tree.get_node(proxy.node) {
                nodes.push((proxy.slot, node));
            }
        }
        element.kept.take_proxies(found, &nodes)?;
        element.slots = Some(slots);
    }
    Ok(stays)
}

/// Checks 1 and 2 of [`resolve`] for each proxy of `roots`, whose elements
/// are sorted by place: the index of the root it names, or why it is kept,
/// as no root is named, or one of a kind its role or its type does not
/// admit.
fn proxy_roots(roots: &Roots) -> Vec<Result<usize, Unresolved>> {
    let index_of = |read: &[Pending], order: usize| {
        read.binary_search_by_key(&order, |element| element.order)
            .ok()
    };
    // Each root by its id, with its kind.
    let mut kinds = Vec::with_capacity(roots.orders.len());
    let mut by_id = std::collections::BTreeMap::new();
    for (root, &order) in roots.orders.iter().enumerate() {
        let element = index_of(&roots.read, order).and_then(|index| roots.read.get(index));
        kinds.push(element.map(|element| element.kind));
        if let Some(element) = element {
            by_id.insert(element.id.as_str().to_owned(), root);
        }
    }
    // Checks 1 and 2, proxy by proxy.
    roots
        .proxies
        .iter()
        .map(|proxy| {
            let link = &proxy.link;
            let root = by_id
                .get(link.target.as_str())
                .copied()
                .ok_or(Unresolved::NoRoot)?;
            let Some(kind) = kinds.get(root).copied().flatten() else {
                return Err(Unresolved::NoRoot);
            };
            let fits =
                link.declared.admits(kind) && link.class.is_none_or(|class| kind.is_kind_of(class));
            if fits {
                Ok(root)
            } else {
                Err(Unresolved::WrongKind {
                    target_class: kind,
                    class: link.class.map_or(link.declared.name(), ElementKind::name),
                })
            }
        })
        .collect()
}

/// Check 3 of [`resolve`]: each proxy that passed checks 1 and 2 and names a
/// root that another such proxy names too is kept, as shared. `count` is the
/// number of roots.
fn mark_shared(outcomes: &mut [Result<usize, Unresolved>], count: usize) {
    let mut naming = vec![0_usize; count];
    for &root in outcomes.iter().flatten() {
        if let Some(count) = naming.get_mut(root) {
            *count = count.saturating_add(1);
        }
    }
    for outcome in outcomes.iter_mut() {
        if let Ok(root) = *outcome {
            if naming.get(root).is_some_and(|&count| count > 1) {
                *outcome = Err(Unresolved::Shared);
            }
        }
    }
}

/// Check 4 of [`resolve`]: each proxy left that is part of a cycle of
/// proxies (a self-link included) is kept. `count` is the number of roots.
fn mark_cycles(outcomes: &mut [Result<usize, Unresolved>], proxies: &[Proxy], count: usize) {
    // Check 4: the rest name each root at most once, so following, from
    // each root, the tree of the proxy that names it is a walk along one
    // path; one that comes back to itself is a cycle. Each root is walked
    // once, so this is linear.
    let mut named_by: Vec<Option<usize>> = vec![None; count];
    for (proxy, outcome) in outcomes.iter().enumerate() {
        if let Some(slot) = outcome
            .as_ref()
            .ok()
            .and_then(|&root| named_by.get_mut(root))
        {
            *slot = Some(proxy);
        }
    }
    let next = |root: usize| {
        named_by
            .get(root)
            .copied()
            .flatten()
            .and_then(|proxy| proxies.get(proxy))
            .map(|proxy| proxy.tree)
    };
    // 0: not walked; 1: on the path being walked; 2: walked.
    let mut state = vec![0_u8; count];
    let mut in_cycle = vec![false; count];
    let mut path = Vec::new();
    for start in 0..count {
        path.clear();
        let mut current = start;
        loop {
            match state.get(current) {
                Some(1) => {
                    if let Some(at) = path.iter().position(|&root| root == current) {
                        for &root in path.get(at..).unwrap_or_default() {
                            if let Some(flag) = in_cycle.get_mut(root) {
                                *flag = true;
                            }
                        }
                    }
                    break;
                }
                Some(0) => {}
                _ => break,
            }
            if let Some(seen) = state.get_mut(current) {
                *seen = 1;
            }
            path.push(current);
            match next(current) {
                Some(tree) => current = tree,
                None => break,
            }
        }
        for &root in &path {
            if let Some(seen) = state.get_mut(root) {
                *seen = 2;
            }
        }
    }
    for outcome in outcomes.iter_mut() {
        if let Ok(root) = *outcome {
            if in_cycle.get(root).copied().unwrap_or(false) {
                *outcome = Err(Unresolved::Cycle);
            }
        }
    }
}

/// Adds the elements of every root's tree, read whole and sorted by their
/// place in document order, to `document`, each with everything read for
/// it, its metadata and preserved content included: depth first from the
/// roots, so a parent comes before its children, and the children of each
/// element in their order, a root a proxy moved at the proxy's place
/// (SACM23-XMI-030). Everything is checked, so the inserts succeed; a
/// failure would still be a refusal, never a panic.
fn hold(text: &str, document: &mut Document, read: Vec<Pending>) -> Result<(), ReadError> {
    let index_of = |order: usize| {
        read.binary_search_by_key(&order, |element| element.order)
            .ok()
    };
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); read.len()];
    let mut roots = Vec::new();
    for (index, element) in read.iter().enumerate() {
        match element.parent_order.and_then(index_of) {
            Some(parent) => children[parent].push(index),
            None => roots.push(index),
        }
    }
    let place = |index: &usize| read.get(*index).map_or(0, |element| element.place);
    roots.sort_by_key(place);
    for held in &mut children {
        held.sort_by_key(place);
    }
    let mut read: Vec<Option<Pending>> = read.into_iter().map(Some).collect();
    let mut pending: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(index) = pending.pop() {
        if let Some(element) = read.get_mut(index).and_then(Option::take) {
            hold_element(text, document, element)?;
        }
        if let Some(held) = children.get_mut(index) {
            pending.extend(std::mem::take(held).into_iter().rev());
        }
    }
    Ok(())
}

/// Adds `element`, read whole, to `document`, under its parent, which is
/// held already, with everything read for it.
fn hold_element(
    text: &str,
    document: &mut Document,
    mut element: Pending,
) -> Result<(), ReadError> {
    // What is held is taken out of `element`, which names the refusal
    // if an insert fails.
    let id = element.id.clone();
    let held = document
        .insert_node(
            element.parent.as_ref(),
            element.kind,
            id.clone(),
            element.name.take(),
        )
        .map_err(|error| error.to_string());
    let declared = held.and_then(|()| match element.declaration {
        Some(declaration) => document
            .set_declaration(&id, declaration)
            .map_err(|error| error.to_string()),
        None => Ok(()),
    });
    let named = declared.and_then(|()| match element.name_id.take() {
        Some(name_id) => document.set_name_id(&id, name_id),
        None => Ok(()),
    });
    let named = named.and_then(|()| match element.name_metadata.take() {
        Some(metadata) => document.set_metadata(&id, true, metadata),
        None => Ok(()),
    });
    let identified = named.and_then(|()| match element.gid.take() {
        Some(gid) => document.set_gid(&id, Some(gid)),
        None => Ok(()),
    });
    let valued = identified.and_then(|()| {
        std::mem::take(&mut element.strings)
            .into_iter()
            .filter_map(|(attribute, value)| value.map(|value| (attribute, value)))
            .try_for_each(|(attribute, value)| document.set_string(&id, attribute, Some(value)))
    });
    let texts = [
        (TextRole::Content, element.content.take()),
        (TextRole::Key, element.key.take()),
        (TextRole::Location, element.location.take()),
    ];
    let held = valued.and_then(|()| {
        texts.into_iter().try_for_each(|(role, held)| match held {
            Some(held) => document.set_text(&id, role, held),
            None => Ok(()),
        })
    });
    let flagged = held.and_then(|()| {
        std::mem::take(&mut element.flags)
            .into_iter()
            .try_for_each(|(flag, value)| document.set_flag(&id, flag, value))
    });
    let referenced = flagged.and_then(|()| {
        std::mem::take(&mut element.references)
            .into_iter()
            .try_for_each(|(end, targets)| document.set_references(&id, end, targets))
    });
    let referenced = referenced.and_then(|()| {
        std::mem::take(&mut element.hrefs)
            .into_iter()
            .try_for_each(|(end, hrefs)| document.set_hrefs(&id, end, hrefs))
    });
    let held = referenced.and_then(|()| {
        let holder = || ElementHolder {
            kind: element.kind,
            id: id.clone(),
        };
        let mut kept = std::mem::replace(&mut element.kept, Kept::new(holder())).into_preserved();
        let mut metadata =
            std::mem::replace(&mut element.metadata, Kept::new(holder())).into_preserved();
        // A kept proxy is no typed child: what is placed after it moves up
        // (SACM23-XMI-030).
        if let Some(slots) = element.slots.take() {
            kept.shift(&slots);
            metadata.shift(&slots);
        }
        document
            .set_metadata(&id, false, metadata)
            .and_then(|()| document.set_preserved(&id, kept))
    });
    held.map_err(|error| unsupported(text, element.at, Unread::Unheld, unheld(&element, &error)))?;
    Ok(())
}

/// The details of the refusal of `element`, which the document could not
/// hold because of `error`: SACM-SEC-002, naming the root element or the
/// element (LIB-API-017). Checked input never gives one.
fn unheld(element: &Pending, error: &str) -> Details {
    let template = if element.parent.is_none() {
        &template::SEC_002_UNHELD_ROOT
    } else {
        &template::SEC_002_UNHELD_ELEMENT
    };
    text_details(
        template,
        &[("element", element.written.as_str()), ("error", error)],
    )
}

/// Whether `attribute` of an object (a root, a child, a `name`, a
/// `content`, a `key`, a `location` or a `value`) is XMI metadata
/// (SACM23-XMI-019): `xmi:uuid`, `xmi:label`, `xsi:schemaLocation` or
/// `xsi:noNamespaceSchemaLocation`, written with a prefix the writer can
/// write back. Its `href` and `xmi:idref` are not: on a definition they do
/// not conform (production 2a), and on a role element they make it a proxy.
fn is_object_metadata(text: &str, attribute: &Attribute<'_, '_>) -> bool {
    let is_metadata = ["uuid", "label"]
        .iter()
        .any(|local| is_xmi(attribute, local))
        || is_schema_location(attribute);
    is_metadata && is_writable_prefix(text, attribute)
}

/// Whether `attribute` of the XMI element is its XMI metadata (XMI.xsd
/// `XMI`, SACM23-XMI-019): `xmi:id`, `xmi:label`, `xmi:uuid`, `href`,
/// `xmi:idref`, `xmi:type`, `xsi:schemaLocation` or
/// `xsi:noNamespaceSchemaLocation`, written with a prefix the writer can
/// write back.
fn is_wrapper_metadata(text: &str, attribute: &Attribute<'_, '_>) -> bool {
    let is_metadata = ["id", "label", "uuid", "idref", "type"]
        .iter()
        .any(|local| is_xmi(attribute, local))
        || is_unqualified(attribute, "href")
        || is_schema_location(attribute);
    is_metadata && is_writable_prefix(text, attribute)
}

/// Whether the prefix `attribute` is written with can be written back on
/// its element as it stands: not `sacm` or `xmi` bound to another namespace
/// than the writer binds them to. One that cannot is preserved content
/// instead, which renames it (ADR 0035).
fn is_writable_prefix(text: &str, attribute: &Attribute<'_, '_>) -> bool {
    let written = attribute_written(text, attribute);
    match written.split_once(':').map(|(prefix, _)| prefix) {
        Some(SACM_PREFIX) => attribute.namespace() == Some(SACM_NS),
        Some(XMI_PREFIX) => attribute.namespace() == Some(XMI_NS),
        _ => true,
    }
}

/// Whether `node` is an `xmi:extension` or `xmi:Extension` element, which
/// any object may hold as XMI metadata (XMI 2.5.1 §7.5.4, §7.8.3, rule 4c;
/// SACM23-XMI-019).
fn is_extension(node: Node<'_, '_>) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(XMI_NS)
        && matches!(node.tag_name().name(), "extension" | "Extension")
}

/// Whether `attribute` is `xsi:schemaLocation` or
/// `xsi:noNamespaceSchemaLocation`.
fn is_schema_location(attribute: &Attribute<'_, '_>) -> bool {
    attribute.namespace() == Some(XSI_NS)
        && matches!(
            attribute.name(),
            "schemaLocation" | "noNamespaceSchemaLocation"
        )
}

/// The element `node` of `kind` with the id `id`, written as `written`,
/// before anything but its start tag is read.
fn pending(
    found: &mut Findings<'_, '_>,
    node: Node<'_, '_>,
    parent: Option<&Pending>,
    kind: ElementKind,
    id: ElementId,
    written: String,
) -> Pending {
    let holder = || ElementHolder {
        kind,
        id: id.clone(),
    };
    let (kept, metadata) = (Kept::new(holder()), Kept::new(holder()));
    let order = found.next_order();
    Pending {
        order,
        place: order,
        slots: None,
        parent: parent.map(|parent| parent.id.clone()),
        parent_order: parent.map(|parent| parent.order),
        kind,
        id,
        gid: None,
        has_gid: false,
        name: None,
        name_id: None,
        content: None,
        shorthand: Shorthand::default(),
        key: None,
        location: None,
        strings: Vec::new(),
        declaration: None,
        flags: Vec::new(),
        references: Vec::new(),
        hrefs: Vec::new(),
        given: Vec::new(),
        written,
        at: node.range().start,
        kept,
        metadata,
        name_metadata: None,
    }
}

/// Checks the start tag of `root`: its namespace, kind, id, type and other
/// attributes, in that order. Every package kind is read as a root (ADR 0035
/// amendment 1).
fn root_start<'a>(found: &mut Findings<'_, 'a>, root: Node<'a, '_>) -> Result<Pending, ReadError> {
    let text = found.text;
    let at_root = root.range().start;
    let name = written_name(text, root);

    // The kind, then the namespace (ADR 0016 D5; ADR 0041 D4). Pass 1 has
    // refused a root of any other name, in a namespace that is not SACM, in
    // SACM 2.4's, and in strict mode in another SACM namespace. The root's
    // family is its SACM namespace for types (ADR 0041 D5).
    let Some(kind) = package_kind(&root) else {
        return Err(invalid_root(text, root));
    };
    let namespace = root.tag_name().namespace().unwrap_or_default();
    // Pass 1 classified this namespace value.
    let class = found.classes.of(Some(namespace));
    let Some(family) = class.family() else {
        return Err(invalid_root(text, root));
    };
    found.types = Types::new(family);
    found.revision = match class {
        RootNamespace::OtherRevision { revision, .. } => Some(revision),
        _ => None,
    };

    let id = read_id(found, root, kind)?;
    let mut element = pending(found, root, None, kind, id, name);
    let id = element.id.clone();
    let id_text = id.as_str();
    found.root_namespace(class, namespace, at_root, kind, id_text)?;

    // A type naming the root's own class is read, an `xsi:type` reported;
    // one naming another class does not conform, and is preserved (ADR
    // 0033, D-ROOT-NAME). Like the kind and the id, it is checked first, at
    // the root.
    for attribute in root.attributes().filter(is_type) {
        if names_class(&found.types, root, attribute.value(), kind.name()) {
            if is_xsi_type(&attribute) {
                let arguments = named(kind, id_text);
                found.xsi_type(&attribute, &template::XMI_010_XSI_TYPE, arguments, id_text)?;
            }
        } else {
            element
                .kept
                .preserve_attribute(found, root, &attribute, at_root)?;
        }
    }

    // Besides `xmi:id`, `xmi:version`, the type and `gid`, only the
    // SACMElement flags and references are read, in document order (ADR
    // 0016 D4, ADR 0026), and the C++ `name` attribute (LIB-COMPAT-008).
    // XMI metadata is kept as written (SACM23-XMI-019, ADR 0042); anything
    // else, `href` and `xmi:idref` included (production 2a), does not
    // conform, and is preserved (ADR 0033).
    let mut start = StartTag::default();
    for attribute in root.attributes() {
        if is_type(&attribute) || is_xmi(&attribute, "id") {
            continue;
        }
        if is_xmi(&attribute, "version") {
            let version = &template::XMI_011_VERSION_ON_ROOT;
            found.xmi_version(&attribute, version, named(kind, id_text), Some(id_text))?;
            continue;
        }
        if is_unqualified(&attribute, "name") {
            element.name = Some(read_name_attribute(found, &attribute, kind, &id)?);
            continue;
        }
        if is_unqualified(&attribute, GID) {
            element.gid = Some(attribute.value().to_owned());
            element.has_gid = true;
            continue;
        }
        match start.read(found, &attribute, kind, &id) {
            Ok(Given::Read) => continue,
            Ok(Given::NoId) => {}
            Ok(Given::Other) => {
                if is_object_metadata(text, &attribute) {
                    element
                        .metadata
                        .take_metadata_attribute(found, root, &attribute)?;
                    continue;
                }
            }
            Err(fault) => {
                element.keep_attribute(found, root, &attribute, fault)?;
                continue;
            }
        }
        element
            .kept
            .preserve_attribute(found, root, &attribute, attribute.range_qname().start)?;
    }
    element.flags = start.flags;
    element.references = start.references;
    Ok(element)
}

/// What [`child_start`] makes of a child element.
enum Child {
    /// An element, whose content is read next.
    Element(Box<Pending>),
    /// A same-file proxy (SACM23-XMI-030), resolved once every root is read.
    Proxy(ProxyLink),
    /// It does not conform as a whole: the caller keeps it whole (ADR 0033).
    Preserve,
}

/// Checks the start tag of `node`, a child element of `parent`: its role,
/// type and kind, its id, and its other attributes, in that order, reading
/// an Assertion's `assertionDeclaration` and the flags and references, and
/// keeping its XMI metadata (SACM23-XMI-019). [`Child::Proxy`] for a
/// same-file proxy, and [`Child::Preserve`] if the child does not conform
/// as a whole (its role, namespace or type), so the caller preserves it
/// whole (ADR 0033). [`Fault::Keep`] if a tolerant read keeps it whole where
/// a strict read refuses it: a child [`child_kind`] keeps, and a role
/// element in reference form with an `href` to another document (ADR 0035
/// D1). An attribute a strict read
/// refuses, a tolerant read keeps as preserved content of the child. A role
/// or type spelling a tolerant read reads (SACM23-XMI-009) is one
/// SACM-XMI-013 Warning at the start tag, affecting the parent for the role
/// and the child for the type.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over a child's start tag, each attribute in turn"
)]
fn child_start<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    parent: &Pending,
) -> Result<Child, Fault> {
    let text = found.text;
    let (kind, spelled) = match child_kind(found, node, parent)? {
        ChildKind::Read { kind, spelled } => (kind, spelled),
        ChildKind::Proxy(link) => return Ok(Child::Proxy(link)),
        ChildKind::Preserve => return Ok(Child::Preserve),
    };
    let written = written_name(text, node);
    let id = read_id(found, node, kind)?;
    let mut element = pending(found, node, Some(parent), kind, id, written);
    let id = element.id.clone();
    if let Some(spelled) = spelled {
        found.spelled_child(node, spelled, &parent.id, &id)?;
    }
    // Besides `xmi:id` and the type (ADR 0016 D4), only an unqualified
    // `gid`, an Assertion's unqualified `assertionDeclaration`, the
    // SACMElement flags and references (ADR 0026), the unqualified
    // string-valued attributes of the kind (`version`, `date`, `startTime`,
    // `endTime`; D-DATE), and the C++ shapes
    // (LIB-COMPAT-008): `name` on a ModelElement, and `lang`, `content` and,
    // on a TaggedValue, `key` and `value` on a UtilityElement, in document
    // order, so a refusal after them lands on the later attribute. XMI
    // metadata is kept as written (SACM23-XMI-019); anything else does not
    // conform, and is preserved (ADR 0033).
    let is_assertion = kind.is_a(AbstractClass::Assertion);
    let is_utility = kind.is_a(AbstractClass::UtilityElement);
    let is_tagged_value = kind == ElementKind::TaggedValue;
    let mut start = StartTag::default();
    for attribute in node.attributes() {
        if is_xsi_type(&attribute) {
            let (xsi, arguments) = (&template::XMI_010_XSI_TYPE, named(kind, id.as_str()));
            found.xsi_type(&attribute, xsi, arguments, id.as_str())?;
            continue;
        }
        if is_xmi(&attribute, "id") || is_type(&attribute) {
            continue;
        }
        if is_unqualified(&attribute, GID) {
            element.gid = Some(attribute.value().to_owned());
            element.has_gid = true;
            continue;
        }
        let at = attribute.range_qname().start;
        if is_assertion && is_unqualified(&attribute, DECLARATION) {
            // One that is no literal is kept in tolerant mode, and the
            // Assertion holds the default (ADR 0035).
            match parse_declaration(found, attribute.value(), at, &id) {
                Ok(declaration) => element.declaration = Some(declaration),
                Err(fault) => {
                    element.declaration = Some(AssertionDeclaration::Asserted);
                    element.keep_attribute(found, node, &attribute, fault)?;
                }
            }
            continue;
        }
        match start.read(found, &attribute, kind, &id) {
            Ok(Given::Read) => continue,
            Ok(Given::NoId) => {
                element
                    .kept
                    .preserve_attribute(found, node, &attribute, at)?;
                continue;
            }
            Ok(Given::Other) => {}
            Err(fault) => {
                element.keep_attribute(found, node, &attribute, fault)?;
                continue;
            }
        }
        if let Some(string) = string_attribute_named(kind, attribute.name())
            .filter(|string| is_unqualified(&attribute, string.name()))
        {
            // Held exactly as written (D-DATE); attributes are unique in a
            // start tag (XML 1.0 §3.1), so this is its first occurrence.
            element
                .strings
                .push((string, Some(attribute.value().to_owned())));
            continue;
        }
        if element.read_spelled_attribute(found, node, &attribute, &mut start)? {
            continue;
        }
        if element.read_older_date(found, node, &attribute)? {
            continue;
        }
        if !is_utility && is_unqualified(&attribute, "name") {
            element.name = Some(read_name_attribute(found, &attribute, kind, &id)?);
            continue;
        }
        // The C++ attribute forms of a UtilityElement's content and of a
        // TaggedValue's key and value.
        let gives_content = is_unqualified(&attribute, "lang")
            || is_unqualified(&attribute, "content")
            || (is_tagged_value && is_unqualified(&attribute, "value"));
        if is_utility && gives_content {
            element.read_content_attribute(found, node, &attribute)?;
            continue;
        }
        if is_tagged_value && is_unqualified(&attribute, "key") {
            element.read_key_attribute(found, &attribute)?;
            continue;
        }
        if is_object_metadata(text, &attribute) {
            element
                .metadata
                .take_metadata_attribute(found, node, &attribute)?;
            continue;
        }
        if is_unqualified(&attribute, "href") {
            // In reference form, the role element is no object but a proxy
            // of one in another document, which SACM23-XMI-029 reads: it is
            // kept whole (XMI 2.5.1 §7.10.1, §7.10.2; ADR 0035 D1). A
            // same-file one is a proxy ([`child_kind`]).
            let arguments = attribute_arguments(text, &attribute, &element.written);
            return Err(found.unread_fault(
                at,
                Unread::CrossDocumentHref,
                ElementReason::ExternalAttribute,
                arguments,
            ));
        }
        element
            .kept
            .preserve_attribute(found, node, &attribute, at)?;
    }
    element.flags = start.flags;
    element.references = start.references;
    Ok(Child::Element(Box::new(element)))
}

/// The name that `attribute`, an unqualified `name` on the `ModelElement`
/// `id` of `kind`, gives in the C++ shape (LIB-COMPAT-008): a `LangString`
/// whose `content` is the value, exactly, and whose `lang` is absent,
/// reported at the attribute's name.
fn read_name_attribute(
    found: &mut Findings<'_, '_>,
    attribute: &Attribute<'_, '_>,
    kind: ElementKind,
    id: &ElementId,
) -> Result<LangString, ReadError> {
    found.legacy_shape(
        attribute.range_qname().start,
        id,
        Shape::NameAttribute,
        named(kind, id.as_str()),
    )?;
    let mut name = LangString::default();
    name.set_content(Some(attribute.value().to_owned()));
    Ok(name)
}

/// The id of `node`, an element of `kind` (XMI 2.5.1 §7.6.1): the value of
/// its `xmi:id`, recorded in `found`, or, without one, a generated id
/// (SACM23-XMI-020). An empty `xmi:id` is `InvalidId` at `node`
/// (SACM-ID-002), and one whose value an earlier element or name carries is
/// `DuplicateId` (SACM-ID-001), giving the start tag of every element in the
/// input that carries the value (ADR 0015 amendment 2). An id with XML white
/// space around an `NCName` is read trimmed, with SACM-ID-004
/// (SACM23-XMI-025). Any other id is read as written; validation reports one
/// that is not an `NCName` (SACM23-VAL-020). Only `xmi:id`, in the XMI
/// namespace, is an id (XMI 2.5.1 rule 2f): an `id` in another namespace or
/// in none is an attribute that is not read.
fn read_id<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    kind: ElementKind,
) -> Result<ElementId, ReadError> {
    let Some(attribute) = node.attributes().find(|attribute| is_xmi(attribute, "id")) else {
        return found.generate_id(node, kind);
    };
    if attribute.value().is_empty() && found.mode == Mode::Tolerant {
        // Read as if it had no `xmi:id`, with SACM-ID-002 in place of
        // SACM-XMI-004: the empty value identifies nothing, and is not kept
        // (ADR 0035 D1).
        let id = found.next_id(kind);
        let arguments = named(kind, id.as_str());
        found.empty_id(
            node,
            &template::ID_002_EMPTY_ID,
            arguments,
            Some(id.clone()),
        )?;
        return Ok(id);
    }
    let value = checked_id(found, node, &attribute)?;
    if value != attribute.value() {
        let mut arguments = vec![Argument::text("written", attribute.value())];
        arguments.extend(named(kind, value));
        found.trimmed_id(&attribute, &template::ID_004_TRIMMED_ID, arguments, value)?;
    }
    Ok(ElementId::new(value))
}

/// What holds an `xmi:id` that is no element's, as its diagnostics name it:
/// the `name` of an element, its `content`, `key` or `location` element, or
/// a `value` in one (SACM23-XMI-022; ADR 0024, ADR 0025).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdOwner {
    Name,
    Text(TextRole),
    Value(TextRole),
}

impl IdOwner {
    /// The templates of its diagnostics: an empty id (SACM-ID-002), and an
    /// id read trimmed (SACM-ID-004).
    fn templates(self) -> (&'static Template, &'static Template) {
        match self {
            Self::Name => (
                &template::ID_002_EMPTY_ID_OF_NAME,
                &template::ID_004_TRIMMED_ID_OF_NAME,
            ),
            Self::Text(_) => (
                &template::ID_002_EMPTY_ID_OF_TEXT,
                &template::ID_004_TRIMMED_ID_OF_TEXT,
            ),
            Self::Value(_) => (
                &template::ID_002_EMPTY_ID_OF_TEXT_VALUE,
                &template::ID_004_TRIMMED_ID_OF_TEXT_VALUE,
            ),
        }
    }

    /// The arguments that name it, of the element `id` of `kind`: the role
    /// of a text or value, then the element's class and id.
    fn arguments(self, kind: ElementKind, id: &str) -> Vec<Argument> {
        let mut arguments = match self {
            Self::Name => Vec::new(),
            Self::Text(role) | Self::Value(role) => vec![Argument::text("role", role.name())],
        };
        arguments.extend(named(kind, id));
        arguments
    }
}

/// The `xmi:id` of `node`, the `name`, `content`, `key`, `location` or
/// `value` element `part` says, of the element `owner` of `kind`, if it has
/// one (SACM23-XMI-022; XMI 2.5.1 §7.8.3; ADR 0024, ADR 0025). It is in the
/// elements' id space, under [`read_id`]'s rules, except that none is
/// generated. A trimmed one is reported on the owner.
fn read_owned_id<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    part: IdOwner,
    owner: &ElementId,
    kind: ElementKind,
) -> Result<Option<ElementId>, ReadError> {
    let Some(attribute) = node.attributes().find(|attribute| is_xmi(attribute, "id")) else {
        return Ok(None);
    };
    let (empty, trimmed) = part.templates();
    if attribute.value().is_empty() && found.mode == Mode::Tolerant {
        // Read without an id (ADR 0035 D1).
        let arguments = part.arguments(kind, owner.as_str());
        found.empty_id(node, empty, arguments, Some(owner.clone()))?;
        return Ok(None);
    }
    let value = checked_id(found, node, &attribute)?;
    if value != attribute.value() {
        let mut arguments = vec![Argument::text("written", attribute.value())];
        arguments.extend(part.arguments(kind, owner.as_str()));
        arguments.push(Argument::text("value", value));
        found.trimmed_id(&attribute, trimmed, arguments, owner.as_str())?;
    }
    Ok(Some(ElementId::new(value)))
}

/// The value of the `xmi:id` `attribute` of `node`, an element or a name, as
/// [`id_value`] gives it, recorded in `found`. Empty is `InvalidId` at
/// `node`, and a value read before is `DuplicateId`.
fn checked_id<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    attribute: &Attribute<'a, '_>,
) -> Result<&'a str, ReadError> {
    let raw = attribute.value();
    if raw.is_empty() {
        return Err(ReadError::invalid_id(
            String::new(),
            locate(found.text, node.range().start),
        ));
    }
    let value = id_value(raw);
    if !found.insert_id(value) {
        return Err(duplicate_id(found.text, node, value));
    }
    Ok(value)
}

/// The `DuplicateId` refusal of `id`, which `node` repeats: the start tag of
/// every element in the input whose `xmi:id` has that value, found in one
/// walk in document order and located in one pass. No `xmi:id` holds a
/// generated id, so an element without one is never among them.
fn duplicate_id(text: &str, node: Node<'_, '_>, id: &str) -> ReadError {
    let mut locator = Locator::new(text);
    let locations = node
        .document()
        .root_element()
        .descendants()
        .filter(|element| {
            element
                .attributes()
                .any(|attribute| is_xmi(&attribute, "id") && id_value(attribute.value()) == id)
        })
        .filter_map(|element| locator.locate(element.range().start))
        .collect();
    ReadError::duplicate_id(ElementId::new(id), locations)
}

/// What [`child_kind`] makes of a child element.
enum ChildKind {
    /// It is read, as an element of `kind`, and `spelled` is the spelling a
    /// tolerant read read in its role or type, which its SACM-XMI-013
    /// Warning names (SACM23-XMI-009).
    Read {
        kind: ElementKind,
        spelled: Option<Spelled>,
    },
    /// It is a same-file proxy (SACM23-XMI-030), resolved once every root is
    /// read.
    Proxy(ProxyLink),
    /// It does not conform as a whole, by its namespace, role or type: the
    /// caller keeps it as preserved content (ADR 0033).
    Preserve,
}

/// What a same-file proxy says (SACM23-XMI-030; XMI 2.5.1 §7.10.1): the
/// type its role declares, the class its type attribute names, if it has
/// one, the id it names, and the spelling a tolerant read read in its role
/// or type (SACM23-XMI-009).
struct ProxyLink {
    declared: DeclaredType,
    class: Option<ElementKind>,
    target: String,
    spelled: Option<Spelled>,
}

/// The kind of the child element `node` of a `parent` element, which
/// `parent_what` names (SACM23-XMI-016; XMI 2.5.1 §7.6.3, §7.8.6).
///
/// The child is a role element: in no namespace, and named as a composite
/// role of the parent's class. Its kind is the class its type attributes
/// name, which must agree; without one, it is the role's declared type, if
/// that is concrete. The kind must conform to the role (ADR 0016 D5, D6).
/// A child that is none of this is [`ChildKind::Preserve`] (ADR 0033),
/// except an element in the XMI namespace, which is XMI metadata
/// (SACM23-XMI-019), and two spellings (SACM23-XMI-009): the §11.4 text
/// spelling `argumentationElement` of the role `argumentElement`, and the
/// type `ArtifactAssertedRelationship`, the model's spelling of a kind the
/// role admits. A tolerant read reads each as the resolved name, and
/// returns it to be reported; a strict read keeps the child whole with a
/// SACM-XMI-013 Error. Without a type under a role of an
/// abstract type, it is `MissingType` (SACM-XMI-005, LIB-COMPAT-010),
/// unless it carries an `xmi:idref` or an `href`, the reference form of a
/// composite, which is refused by its entry ([`proxy_link`]); a typed one,
/// or one under a concrete role, is refused at that attribute
/// ([`role_metadata`]). A
/// conforming kind this release does not read is refused by its entry.
/// Every refusal is at the child's start tag. A tolerant read keeps each of
/// these children whole instead, with its Warning: [`Fault::Keep`] (ADR 0035
/// D1).
#[expect(
    clippy::too_many_lines,
    reason = "one walk through a child's namespace, role, type and kind, each with its refusal"
)]
fn child_kind(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    parent_element: &Pending,
) -> Result<ChildKind, Fault> {
    let text = found.text;
    let written = written_name(text, node);
    let at = node.range().start;
    let parent = parent_element.kind;
    // Which of a pair of templates names the parent: the root element's, or
    // an element's.
    let in_root = parent_element.parent.is_none();
    let container = parent_element.written.as_str();
    // An XMI extension is XMI metadata, which the caller takes first; any
    // other qualified child is no role element (ADR 0033).
    if node
        .tag_name()
        .namespace()
        .is_some_and(|uri| !uri.is_empty())
    {
        return Ok(ChildKind::Preserve);
    }
    let local = node.tag_name().name();
    let mut spelled = None;
    let role = if let Some(role) = role_named(parent, local) {
        role
    } else {
        // The text's spelling of the role `argumentElement` (§11.4) or
        // `terminologyElement` (§10.4): read as that role in tolerant mode,
        // kept whole in strict mode (SACM23-XMI-009, SACM23-XMI-028).
        let Some((spelling, role)) = spelled_role(parent, local) else {
            return Ok(ChildKind::Preserve);
        };
        let reason = Spelled::of_child(spelling, &written, container, in_root);
        spelled = Some(child_spelling(found, at, reason)?);
        role
    };
    let mut typed = None;
    let mut type_spelled = false;
    for attribute in node.attributes().filter(is_type) {
        let kind = match type_kind(&found.types, node, attribute.value()) {
            Some(kind) => kind,
            // The model's spelling of ArtifactAssetRelationship, read as that
            // kind (SACM23-XMI-009, ADR 0034 amendment 2). Under a role that
            // does not admit the kind, the check below preserves the child,
            // as for any type the role does not admit, and it is no
            // spelling.
            None if names_class(&found.types, node, attribute.value(), MODEL_SPELLING) => {
                type_spelled = true;
                ElementKind::ArtifactAssetRelationship
            }
            None => return Ok(ChildKind::Preserve),
        };
        match typed {
            Some(earlier) if earlier != kind => return Ok(ChildKind::Preserve),
            _ => typed = Some(kind),
        }
    }
    // A role element carrying an `xmi:idref`, or an `href` to an id of this
    // document, is a proxy (SACM23-XMI-030; XMI 2.5.1 §7.10.1), which names
    // its object rather than leaving its class out: of a root, which moves
    // into its place once every root is read. One of a type the role does
    // not admit is no proxy of the role; one with anything on or in it
    // besides its link and a type does not conform (ADR 0033).
    if is_same_file_proxy(node) {
        if typed.is_some_and(|kind| !role.declared.admits(kind)) {
            return Ok(ChildKind::Preserve);
        }
        let Some(target) = proxy_target(text, node) else {
            return Err(Fault::Defect);
        };
        if type_spelled {
            let reason = Spelled::of_child(Spelling::Type, &written, container, in_root);
            spelled = Some(child_spelling(found, at, reason)?);
        }
        return Ok(ChildKind::Proxy(ProxyLink {
            declared: role.declared,
            class: typed,
            target,
            spelled,
        }));
    }
    let kind = match (typed, role.declared) {
        (Some(kind), _) | (None, DeclaredType::Kind(kind)) => kind,
        // Named after its role, so no class can be inferred (LIB-COMPAT-010),
        // unless it is the reference form of a composite, a proxy of an
        // element of another document, which names its object rather than
        // leaving its class out (XMI 2.5.1 §7.10.1): not read yet
        // (SACM23-XMI-029).
        (None, DeclaredType::Abstract(class)) => {
            if has_href(node) {
                let reason = if in_root {
                    ElementReason::ExternalProxyChildOfRoot
                } else {
                    ElementReason::ExternalProxyChild
                };
                let arguments = text_arguments(&[
                    ("element", &written),
                    ("container", container),
                    ("class", class.name()),
                ]);
                return Err(found.unread_fault(at, Unread::CrossDocumentHref, reason, arguments));
            }
            return Err(match found.mode {
                Mode::Strict => Fault::Refused(ReadError::missing_type(
                    role.name.to_owned(),
                    class.name().to_owned(),
                    locate(text, at),
                )),
                Mode::Tolerant => {
                    let template = if in_root {
                        &template::XMI_005_MISSING_TYPE_IN_ROOT_KEPT
                    } else {
                        &template::XMI_005_MISSING_TYPE_KEPT
                    };
                    let arguments = text_arguments(&[
                        ("element", &written),
                        ("container", container),
                        ("role", role.name),
                        ("type", class.name()),
                    ]);
                    Fault::Keep(Keep::held(
                        at,
                        TOLERANT_ROW,
                        Some("7.6.3"),
                        template,
                        arguments,
                    ))
                }
            });
        }
    };
    if !role.declared.admits(kind) {
        return Ok(ChildKind::Preserve);
    }
    // The readable children: every kind a role of a readable class admits.
    if type_spelled {
        let reason = Spelled::of_child(Spelling::Type, &written, container, in_root);
        spelled = Some(child_spelling(found, at, reason)?);
    }
    Ok(ChildKind::Read { kind, spelled })
}

/// The role of a `parent` element that the text spells `local`, with the
/// spelling: `argumentationElement` for an ArgumentPackage's
/// `argumentElement` (§11.4, SACM23-XMI-009) and `TerminologyElement` for a
/// TerminologyPackage's `terminologyElement` (§10.4, SACM23-XMI-028); `None`
/// when `local` is neither, or `parent` has no such role.
fn spelled_role(parent: ElementKind, local: &str) -> Option<(Spelling, Role)> {
    [Spelling::Role, Spelling::TerminologyRole]
        .into_iter()
        .find(|spelling| spelling.written() == local)
        .and_then(|spelling| role_named(parent, spelling.resolved()).map(|role| (spelling, role)))
}

/// What a read does with a child whose role or type is `spelled`, at byte
/// `at` (SACM23-XMI-009): a tolerant read reads it, and returns it for the
/// caller to report once the child's id is known; a strict read keeps the
/// child whole with the Error ([`Fault::Keep`]).
fn child_spelling(found: &Findings<'_, '_>, at: usize, spelled: Spelled) -> Result<Spelled, Fault> {
    match found.mode {
        Mode::Strict => Err(Fault::Keep(Keep::spelling(at, spelled))),
        Mode::Tolerant => Ok(spelled),
    }
}

/// Whether the role element `node` carries an unqualified `href`: under an
/// abstract role with no type, the reference form of a composite in another
/// document, which SACM23-XMI-029 reads (LIB-COMPAT-010).
fn has_href(node: Node<'_, '_>) -> bool {
    node.attributes()
        .any(|attribute| is_unqualified(&attribute, "href"))
}

/// Whether the role element `node` is a same-file proxy as XMI 2.5.1 rule 2l
/// writes it (§7.10.1, SACM23-XMI-030): it carries an `xmi:idref`, or an
/// unqualified `href` whose value, trimmed, is `#id`. Whether it carries
/// nothing else is [`proxy_target`]'s check.
fn is_same_file_proxy(node: Node<'_, '_>) -> bool {
    node.attributes().any(|attribute| {
        is_xmi(&attribute, "idref")
            || (is_unqualified(&attribute, "href")
                && matches!(
                    classify_href(attribute.value().trim_matches(is_xml_space)),
                    HrefForm::Id(_)
                ))
    })
}

/// The id the same-file proxy `node` names, if it is one as §7.10.1 allows
/// (SACM23-XMI-030): exactly one link attribute, an `xmi:idref` holding one
/// token or an `href="#id"`, at most one type attribute (`xmi:type` or
/// `xsi:type`), no other attribute, and only white space, comments and
/// processing instructions in it ("Proxies … contain no nested elements";
/// "XML attributes may be defined, but not contents"). `None` otherwise.
fn proxy_target(text: &str, node: Node<'_, '_>) -> Option<String> {
    let mut target = None;
    let mut links = 0_usize;
    let mut types = 0_usize;
    for attribute in node.attributes() {
        let value = attribute.value().trim_matches(is_xml_space);
        if is_xmi(&attribute, "idref") {
            links = links.saturating_add(1);
            if value.is_empty() || value.contains(is_xml_space) {
                return None;
            }
            target = Some(value);
        } else if is_unqualified(&attribute, "href") {
            links = links.saturating_add(1);
            let HrefForm::Id(id) = classify_href(value) else {
                return None;
            };
            target = Some(id);
        } else if is_type(&attribute) {
            types = types.saturating_add(1);
        } else {
            return None;
        }
    }
    let holds_content = node
        .children()
        .any(|child| child.is_element() || text_offset(text, child).is_some());
    if links != 1 || types > 1 || holds_content {
        return None;
    }
    target.map(str::to_owned)
}

/// The `assertionDeclaration` that `value` names, of the element `element`,
/// given by the attribute or the value element at byte `offset` of `text`:
/// the attribute's name or the element's start tag (SACM23-XMI-017,
/// SACM23-XMI-014; §11.10; ADR 0021). The value, as XML gives it, must be one
/// of the six literals exactly: nothing is trimmed or case-folded (XMI 2.5.1
/// §8.2.1 rule 8). Any other value is `InvalidLiteral` at `offset`
/// (SACM-ENUM-001, SACM23-VAL-006; ADR 0021 amendment 2), and never read as
/// `asserted`; a tolerant read keeps it as preserved content instead, and
/// the Assertion holds `asserted` (ADR 0035).
fn parse_declaration(
    found: &Findings<'_, '_>,
    value: &str,
    offset: usize,
    element: &ElementId,
) -> Result<AssertionDeclaration, Fault> {
    value
        .parse()
        .map_err(|_| literal_fault(found.text, found.mode, offset, element, DECLARATION, value))
}

/// The value of the flag `flag` of the element `element`, given as `value`
/// by the attribute or the value element at byte `offset` of `text`
/// (SACM23-BASE-007, SACM23-BASE-008; ADR 0026). It is an `xsd:boolean`,
/// whose white space is collapsed (XML Schema 1.0 Part 2 §3.2.2.1, §4.3.6):
/// `true` and `1` are true, `false` and `0` are false, with XML white space
/// around them. Any other value, the empty one included, is `InvalidLiteral`
/// at `offset`, holding the value as read (SACM-ENUM-001), and never read as
/// false; a tolerant read keeps it as preserved content instead, and the
/// flag holds false (ADR 0035).
fn parse_flag(
    found: &Findings<'_, '_>,
    value: &str,
    offset: usize,
    element: &ElementId,
    flag: Flag,
) -> Result<bool, Fault> {
    // A literal holds no white space, so collapsing a value that reads is
    // trimming it.
    match value.trim_matches(is_xml_space) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(literal_fault(
            found.text,
            found.mode,
            offset,
            element,
            flag.name(),
            value,
        )),
    }
}

/// The target of the reference element `node` (SACM23-XMI-015; XMI 2.5.1
/// production 2c, rules 2l and 2n, §7.10.1; ADR 0026): in no namespace and
/// named after its end, checked by the caller. Its start tag carries exactly
/// one link attribute, `xmi:idref="id"` or an unqualified `href="#id"`, and
/// nothing else; its content is only white space, comments and processing
/// instructions.
///
/// Each attribute is checked in document order, then the link, then the
/// content. An `XLink` attribute or an `XPointer` `href` is a
/// cross-document link not read yet (SACM23-XMI-029): each is refused at
/// the attribute. Anything else that does not conform is
/// [`Fault::Defect`]: an identity attribute or a type, which production 2c
/// and rule 4e do not give a reference element (SACM23-XMI-019), a second
/// link attribute, any other attribute, an
/// `xmi:idref` that is empty or holds white space, an `href` that names no
/// element, no link, a child
/// element, or text. The caller then keeps the whole element as preserved
/// content (ADR 0033). A tolerant read keeps the whole element, with the
/// Warning of a refusal's entry (ADR 0035 D1).
///
/// An `href` is classified by [`classify_href`]: `#id` names the id `id` of
/// this document; `document#fragment` is a cross-document reference
/// (SACM23-XMI-027, ADR 0040 D1), read where `cross_document` is true (every
/// element end) and refused as not read yet where it is false (the derived
/// `expression`, SACM23-XMI-029); an `XPointer` fragment is refused as not
/// read yet; and an `href` that names no element is a defect.
fn read_reference(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    cross_document: bool,
) -> Result<Link, Fault> {
    let text = found.text;
    let element = written_name(text, node);
    let mut link = None;
    for attribute in node.attributes() {
        let is_idref = is_xmi(&attribute, "idref");
        let at = attribute.range_qname().start;
        if !(is_idref || is_unqualified(&attribute, "href")) {
            // An XLink attribute is SACM23-XMI-029's (§7.6.2). Anything else,
            // identity attributes and a type included, keeps the element
            // whole: production 2c and rule 4e give a reference element only
            // its link (SACM23-XMI-019).
            if attribute.namespace() != Some(XLINK_NS) {
                return Err(Fault::Defect);
            }
            let arguments = attribute_arguments(text, &attribute, &element);
            return Err(found.unread_fault(
                at,
                Unread::CrossDocumentHref,
                ElementReason::ExternalAttribute,
                arguments,
            ));
        }
        if link.is_some() {
            return Err(Fault::Defect);
        }
        let value = attribute.value().trim_matches(is_xml_space);
        if is_idref {
            // An `xsd:IDREF` is one token: white space inside it would be
            // read as one id here but written as an IDREFS attribute of
            // several.
            if value.is_empty() || value.contains(is_xml_space) {
                return Err(Fault::Defect);
            }
            link = Some(Link::Id(ElementId::new(value)));
            continue;
        }
        let not_read = || {
            let arguments = text_arguments(&[("href", attribute.value()), ("element", &element)]);
            found.unread_fault(
                at,
                Unread::CrossDocumentHref,
                ElementReason::Href,
                arguments,
            )
        };
        link = Some(match classify_href(value) {
            HrefForm::Id(id) => Link::Id(ElementId::new(id)),
            HrefForm::Other if cross_document => Link::Href(Href::new_unchecked(value)),
            HrefForm::Other | HrefForm::Pointer => return Err(not_read()),
            HrefForm::NoElement => return Err(Fault::Defect),
        });
    }
    let Some(target) = link else {
        return Err(Fault::Defect);
    };
    if node
        .children()
        .any(|child| child.is_element() || text_offset(text, child).is_some())
    {
        return Err(Fault::Defect);
    }
    Ok(target)
}

/// What a reference element names: an id of this document, or an element of
/// another document by its `href` (ADR 0040 D2).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Link {
    /// An `xmi:idref`, or an `href="#id"`: the id.
    Id(ElementId),
    /// An `href` with a document part, as trimmed.
    Href(Href),
}

/// Appends `value` to the list of `end` in `ends`, a sparse list of ends in
/// the order they are first given.
fn push_value<T>(ends: &mut Vec<(ReferenceEnd, Vec<T>)>, end: ReferenceEnd, value: T) {
    match ends.iter_mut().find(|(held, _)| *held == end) {
        Some((_, values)) => values.push(value),
        None => ends.push((end, vec![value])),
    }
}

/// The form of a trimmed `href` value (XMI 2.5.1 §7.10.2 `URI#id_value`,
/// rule 2n; ADR 0040 D1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HrefForm<'v> {
    /// `#id`: the id of an element of this document.
    Id(&'v str),
    /// `document#fragment`: an element of another document by its id.
    Other,
    /// A fragment holding `(` or `)`: an `XPointer`, which SACM23-XMI-029 is to
    /// read.
    Pointer,
    /// No `#`, an empty fragment, a `#` in the fragment, or XML white space
    /// anywhere: it names no element.
    NoElement,
}

/// The form of the trimmed `href` value `value`, held exactly, with no
/// percent-decoding. The checks run in this order: XML white space, no `#`,
/// an empty fragment or a `#` in it name no element; then `(` or `)` in the
/// fragment is an `XPointer`; then an empty document part is `#id`, and any
/// other is a cross-document reference.
fn classify_href(value: &str) -> HrefForm<'_> {
    let Some((document, fragment)) = value.split_once('#') else {
        return HrefForm::NoElement;
    };
    if value.contains(is_xml_space) || fragment.is_empty() || fragment.contains('#') {
        return HrefForm::NoElement;
    }
    if fragment.contains(['(', ')']) {
        return HrefForm::Pointer;
    }
    if document.is_empty() {
        HrefForm::Id(fragment)
    } else {
        HrefForm::Other
    }
}

/// Whether `node` is the value element of the property `local`: an element
/// in no namespace named `local` (XMI 2.5.1 §7.8.4, production 2b). Under
/// `xmlns=""` the parser reports the empty string, which is no namespace
/// name (Namespaces 1.0 §6.2).
fn is_value_element(node: Node<'_, '_>, local: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace().is_none_or(str::is_empty)
        && node.tag_name().name() == local
}

/// The value that the value element `node` of the element `owner` holds,
/// or `None` if it is nil, as [`read_value_element`] reads it.
fn read_value(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    owner: &ElementId,
) -> Result<Option<String>, Fault> {
    read_value_element(found, node, owner, false).map(|(value, _)| value)
}

/// The `lang` attribute of a `content` in a `name`, the C++ shape
/// (LIB-COMPAT-008): its value and the byte offset of its name.
type LangAttribute = (String, usize);

/// What the value element `node` of the element `owner` holds: its value,
/// or `None` if it is nil, and, when `takes_lang`, the value and byte offset
/// of a `lang` attribute on it, the C++ shape of a `content` in a `name`
/// (LIB-COMPAT-008).
///
/// The value is its character content, joined in document order, exactly
/// (SACM23-XMI-014; ADR 0021 amendment 1). CDATA sections and references
/// give their characters, comments and processing instructions are skipped,
/// and nothing is trimmed; the parser has already ended every line with LF
/// (XML 1.0 §2.11). An empty element is the empty string.
///
/// Its `xsi:nil` is an `xsd:boolean` (SACM23-XMI-026; XMI 2.5.1 §9.4.1;
/// XML Schema 1.0 Part 1 §3.3.4): `true` or `1` makes it nil, read as the
/// property not written, and `false` or `0` reads it as usual, white space
/// around them collapsed; any other value is `InvalidLiteral` at the
/// attribute's name. A nil element that holds characters, even white
/// space (an empty CDATA section holds none), or an element, any other
/// attribute, and an element child do not
/// conform: [`Fault::Defect`] (ADR 0033).
fn read_value_element(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    owner: &ElementId,
    takes_lang: bool,
) -> Result<(Option<String>, Option<LangAttribute>), Fault> {
    let mut nil = false;
    let mut lang = None;
    for attribute in node.attributes() {
        if is_xsi_nil(&attribute) {
            nil = parse_nil(found, &attribute, owner)?;
        } else if takes_lang && is_unqualified(&attribute, "lang") {
            let at = attribute.range_qname().start;
            lang = Some((attribute.value().to_owned(), at));
        } else {
            return Err(Fault::Defect);
        }
    }
    let mut value = String::new();
    for child in node.children() {
        // An empty text node, such as an empty CDATA section, holds no
        // characters (SACM23-XMI-026).
        let has_characters = child
            .text()
            .is_some_and(|characters| !characters.is_empty());
        if child.is_element() || (nil && child.is_text() && has_characters) {
            return Err(Fault::Defect);
        }
        if child.is_text() {
            value.push_str(child.text().unwrap_or_default());
        }
    }
    Ok((if nil { None } else { Some(value) }, lang))
}

/// Whether the `xsi:nil` `attribute` of a value element of the element
/// `owner` is true: an `xsd:boolean`, `true` or `1`, and `false` or `0`,
/// with XML white space around them (XML Schema 1.0 Part 2 §3.2.2.1;
/// SACM23-XMI-026). Any other value, the empty one included, is
/// `InvalidLiteral` at the attribute's name, holding the value as read; a
/// tolerant read keeps the value element whole instead (ADR 0035).
fn parse_nil(
    found: &Findings<'_, '_>,
    attribute: &Attribute<'_, '_>,
    owner: &ElementId,
) -> Result<bool, Fault> {
    match attribute.value().trim_matches(is_xml_space) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(literal_fault(
            found.text,
            found.mode,
            attribute.range_qname().start,
            owner,
            "xsi:nil",
            attribute.value(),
        )),
    }
}

/// The `RepeatedValue` refusal of the property `property` of the element
/// `owner`, of `kind`, given at the byte `offsets` of `text`, in document
/// order, located in one pass (SACM-MULT-001; XMI 2.5.1 §9.5.2 rule 2h).
fn repeated(
    text: &str,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
    offsets: impl IntoIterator<Item = usize>,
) -> ReadError {
    let mut locator = Locator::new(text);
    ReadError::repeated_value(
        owner.clone(),
        kind,
        property.to_owned(),
        offsets
            .into_iter()
            .filter_map(|offset| locator.locate(offset))
            .collect(),
    )
}

/// The `RepeatedValue` refusal of the property `property` of the element
/// `owner`, given more than once in `holder`. It gives every occurrence,
/// found in one pass over `holder` in document order: the attribute `local`
/// in no namespace, if it is written, then every value element `local`. For
/// `name`, that is the C++ `name` attribute (LIB-COMPAT-008) and every
/// `name` child in no namespace (SACM23-XMI-021). What `kept` holds as
/// preserved content is no occurrence (ADR 0033).
fn repeated_value(
    text: &str,
    holder: Node<'_, '_>,
    local: &str,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
    kept: &Kept<ElementHolder>,
) -> ReadError {
    let attribute = holder
        .attributes()
        .find(|attribute| is_unqualified(attribute, local))
        .map(|attribute| attribute.range_qname().start)
        .filter(|&at| !kept.holds(at));
    let elements = holder
        .children()
        .filter(|child| is_value_element(*child, local))
        .map(|child| child.range().start)
        .filter(|&at| !kept.holds(at));
    repeated(
        text,
        owner,
        kind,
        property,
        attribute.into_iter().chain(elements),
    )
}

/// The `RepeatedValue` refusal of the `content` of the `UtilityElement`
/// `owner`, whose node is `holder`: the C++ shape on the element itself, at
/// the location of `shorthand` if it is given (LIB-COMPAT-008), and every
/// `content` element that `kept` does not hold as preserved content, in
/// document order.
fn repeated_content(
    text: &str,
    holder: Node<'_, '_>,
    owner: &ElementId,
    kind: ElementKind,
    shorthand: &Shorthand,
    kept: &Kept<ElementHolder>,
) -> ReadError {
    let elements = holder
        .children()
        .filter(|child| is_value_element(*child, TextRole::Content.name()))
        .map(|child| child.range().start)
        .filter(|&at| !kept.holds(at));
    let mut offsets: Vec<usize> = shorthand.at().into_iter().chain(elements).collect();
    offsets.sort_unstable();
    repeated(text, owner, kind, TextRole::Content.name(), offsets)
}

/// The `RepeatedValue` refusal of the field `field`, `lang` or `content`, of
/// the `LangString` that `holder`, a `name` or a `value` of the element
/// `owner`, holds; `property` names the `LangString` (`name`,
/// `content.value`, `key.value`). It gives every occurrence in document
/// order: the attribute, each value element and, for `content`, the text, at
/// its first character that is not literal white space; and in a `name` the
/// other C++ shapes (LIB-COMPAT-008), the `lang` attribute of a `content`
/// element and each `value` element that gives the field. In tolerant mode,
/// the Warning for the second occurrence, with which the whole of `holder`
/// is kept (ADR 0035 D1).
fn repeated_field(
    text: &str,
    mode: Mode,
    holder: Node<'_, '_>,
    field: &str,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
) -> Fault {
    let is_name = property == "name";
    let mut offsets = Vec::new();
    if let Some(attribute) = holder
        .attributes()
        .find(|attribute| is_unqualified(attribute, field))
    {
        offsets.push(attribute.range_qname().start);
    }
    let mut has_text = false;
    for child in holder.children() {
        if !child.is_element() {
            if field == "content" && !has_text {
                if let Some(first) = text_offset(text, child) {
                    offsets.push(first);
                    has_text = true;
                }
            }
            continue;
        }
        let lang_on_content = child
            .attributes()
            .find(|attribute| is_unqualified(attribute, "lang"))
            .filter(|_| is_name && field == "lang" && is_value_element(child, "content"));
        if let Some(attribute) = lang_on_content {
            offsets.push(attribute.range_qname().start);
        } else if is_value_element(child, field)
            || (is_name && is_value_element(child, "value") && gives_field(text, child, field))
        {
            offsets.push(child.range().start);
        }
    }
    let property = format!("{property}.{field}");
    let second = offsets
        .get(1)
        .copied()
        .unwrap_or_else(|| holder.range().start);
    fault(repeat_or_refuse(mode, second, owner, &property, || {
        repeated(text, owner, kind, &property, offsets)
    }))
}

/// Whether `value`, a `value` element in a `name`, gives the field `field`:
/// as an attribute, a value element or, for `content`, text.
fn gives_field(text: &str, value: Node<'_, '_>, field: &str) -> bool {
    value
        .attributes()
        .any(|attribute| is_unqualified(&attribute, field))
        || value.children().any(|child| {
            is_value_element(child, field)
                || (field == "content" && text_offset(text, child).is_some())
        })
}

/// The `LangString` that the `name` element `node` holds (SACM23-XMI-011,
/// SACM23-XMI-012, SACM23-XMI-014; ADR 0021), with its `xmi:id`, as
/// [`read_lang_string`] reads it. The `name` belongs to the element `owner`,
/// of `kind`.
fn read_name<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    kind: ElementKind,
    owner: &ElementId,
) -> Result<(LangString, Option<ElementId>, Preserved), Fault> {
    read_lang_string(found, node, LangPart::Name, owner, kind, "name")
}

/// What a `LangString` element is to the element that holds it, as its
/// diagnostics name it: its `name`, the `value` in its `name` (a C++ shape),
/// or a `value` of its `content`, `key` or `location`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LangPart {
    Name,
    NameValue,
    Value(TextRole),
}

impl LangPart {
    /// What its `xmi:id` is to the element, if it may have one: a `value` in
    /// a `name` has none, since the name has one id, on the `name`
    /// ([`read_name_value`]).
    fn id_owner(self) -> Option<IdOwner> {
        match self {
            Self::Name => Some(IdOwner::Name),
            Self::NameValue => None,
            Self::Value(role) => Some(IdOwner::Value(role)),
        }
    }

    /// The template of the SACM-XMI-010 diagnostic of an `xsi:type` on it.
    fn xsi_type(self) -> &'static Template {
        match self {
            Self::Name => &template::XMI_010_XSI_TYPE_ON_NAME,
            Self::NameValue => &template::XMI_010_XSI_TYPE_ON_NAME_VALUE,
            Self::Value(_) => &template::XMI_010_XSI_TYPE_ON_TEXT_VALUE,
        }
    }

    /// The C++ shape of its content given as text (LIB-COMPAT-008).
    fn text_shape(self) -> Shape {
        match self {
            Self::Name => Shape::NameText,
            Self::NameValue => Shape::NameValueText,
            Self::Value(_) => Shape::ValueText,
        }
    }

    /// The arguments that name it, of the element `id` of `kind`: the role
    /// of a value, then the element's class and id.
    fn arguments(self, kind: ElementKind, id: &str) -> Vec<Argument> {
        let mut arguments = match self {
            Self::Name | Self::NameValue => Vec::new(),
            Self::Value(role) => vec![Argument::text("role", role.name())],
        };
        arguments.extend(named(kind, id));
        arguments
    }
}

/// The `MultiLangString` that the `content`, `key` or `location` element
/// `node` holds, in `role`, with its ids (SACM23-BASE-004, SACM23-BASE-005,
/// SACM23-ART-005; XMI 2.5.1 §7.8.3, §7.8.6; ADR 0025, ADR 0034 amendment
/// 1). It belongs to the element `owner`, of `kind`.
///
/// It carries only `xmi:id` and a type naming `MultiLangString`, checked
/// at the start tag first, and holds only `value` elements in no namespace,
/// white space, comments and processing instructions. Each `value` is read
/// as a `name` is ([`read_lang_string`]), so values are kept in order, with
/// duplicates. With no `value`, it is an empty `MultiLangString`.
///
/// Without a `value`, `lang` and `content` attributes on a `content` or a
/// `key`, and text in it, are the C++ single-language shorthand
/// (LIB-COMPAT-008), which a `location` has not: one value,
/// whose content is the text, all of it, or the attribute, never both. It
/// is reported at its first attribute, else at its text. An `xsi:type`, and
/// an `xmi:id` read trimmed, are reported on the owner. XMI metadata on it,
/// and an XMI extension in it, placed by value, are kept as written
/// (SACM23-XMI-019). Anything else, such as either shorthand beside a
/// `value`, a `lang` alone, an `href`, an `xmi:idref` or another XMI
/// element, does not conform: [`Fault::Defect`] (ADR 0033).
fn read_text<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    role: TextRole,
    kind: ElementKind,
    owner: &ElementId,
) -> Result<Text, Fault> {
    let text = found.text;
    if node.attributes().any(|attribute| {
        is_type(&attribute)
            && !names_class(&found.types, node, attribute.value(), "MultiLangString")
    }) {
        return Err(Fault::Defect);
    }
    let mut held = Text::default();
    held.set_id(read_owned_id(
        found,
        node,
        IdOwner::Text(role),
        owner,
        kind,
    )?);
    // Its XMI metadata, its children placed by value (SACM23-XMI-019).
    let mut metadata = Kept::new(ElementHolder {
        kind,
        id: owner.clone(),
    });
    let has_values = node
        .children()
        .any(|child| is_value_element(child, "value"));
    // The single-language shorthand is the C++ shape of a `content` or a
    // `key` (LIB-COMPAT-008); a `location` has none (ADR 0034 amendment 1).
    let no_shorthand = has_values || role == TextRole::Location;
    let mut shorthand = Shorthand::default();
    for attribute in node.attributes() {
        if is_unqualified(&attribute, "lang") || is_unqualified(&attribute, "content") {
            if no_shorthand {
                return Err(Fault::Defect);
            }
            // Attributes are unique in a start tag (XML 1.0 §3.1), so the
            // content is given once here and this never refuses.
            let _ = shorthand.read_attribute(text, &attribute);
        } else if is_xsi_type(&attribute) {
            found.xsi_type(
                &attribute,
                &template::XMI_010_XSI_TYPE_ON_TEXT,
                IdOwner::Text(role).arguments(kind, owner.as_str()),
                owner.as_str(),
            )?;
        } else if is_object_metadata(text, &attribute) {
            metadata.take_metadata_attribute(found, node, &attribute)?;
        } else if !(is_type(&attribute) || is_xmi(&attribute, "id")) {
            return Err(Fault::Defect);
        }
    }
    let property = format!("{}.value", role.name());
    for child in node.children() {
        if !child.is_element() {
            if no_shorthand {
                if text_offset(text, child).is_some() {
                    return Err(Fault::Defect);
                }
            } else if let Some(first) = shorthand.add_text(text, child) {
                if let Some(attribute) = shorthand.content_at {
                    let property = format!("{property}.content");
                    return Err(fault(repeat_or_refuse(
                        found.mode,
                        first,
                        owner,
                        &property,
                        || repeated(text, owner, kind, &property, [attribute, first]),
                    )));
                }
            }
        } else if is_value_element(child, "value") {
            let part = LangPart::Value(role);
            let (value, id, value_metadata) =
                read_lang_string(found, child, part, owner, kind, &property)?;
            held.push(value, id, value_metadata);
        } else if is_extension(child) {
            metadata.typed = held.value().len();
            metadata.take_element(found, child)?;
        } else {
            // Another XMI element is no metadata here; anything else does
            // not conform either (ADR 0033).
            return Err(Fault::Defect);
        }
    }
    match shorthand.into_lang_string() {
        Ok(Some((value, at))) => {
            let shape = IdOwner::Text(role).arguments(kind, owner.as_str());
            found.legacy_shape(at, owner, Shape::Shorthand, shape)?;
            held.push(value, None, Preserved::default());
        }
        Ok(None) => {}
        Err(_) => return Err(Fault::Defect),
    }
    held.set_metadata(metadata.into_preserved());
    Ok(held)
}

/// The `LangString` that `node`, a `name` or a `value` of a `content` or
/// `key`, holds (SACM23-XMI-011, SACM23-XMI-012, SACM23-XMI-014; ADR 0021,
/// ADR 0025). `part` says what it is in diagnostics, and `property` in a
/// `RepeatedValue` (`name`, `content.value`, `key.value`).
///
/// It must be in no namespace, carry only `lang`, `content` and a type
/// naming `LangString`, and hold only the value elements `lang` and
/// `content`, white space, comments and processing instructions. Each field
/// is given at most once, as an attribute or a value element; a second is
/// `RepeatedValue`. The values are kept as the parser gives them, untrimmed.
/// Its `xmi:id`, if any, is returned with it, after the namespace and the
/// type are checked and before the other attributes (SACM23-XMI-022). An
/// `xsi:type` is reported on the element `owner`. A nil value element gives
/// its field as not written, and still counts as given (SACM23-XMI-026).
///
/// The C++ shapes are read too, each reported (LIB-COMPAT-008): text, all
/// of it, as the `content`, at its first character that is not literal
/// white space; and in a `name`, one `value` (at its `<`) or one `content`
/// carrying `lang` (at that attribute), as the name's fields.
///
/// Typed `ExpressionLangString` (SACM23-BASE-009, ADR 0038 D2), it may
/// also carry an `expression` attribute in no namespace, one IDREF, or hold
/// one `expression` reference element (SACM23-XMI-015); two ids, or the
/// attribute and the element, are `RepeatedValue` of
/// `<property>.expression`. With neither, it reads with no expression.
///
/// XMI metadata on it, and an XMI extension in it, are kept as written
/// (SACM23-XMI-019). Anything else does not conform, such as an `href`, an
/// `xmi:idref`, another XMI element in it, a second
/// `value` or `content` carrying `lang` in a `name`, which holds one
/// `LangString`, an `expression` on a plain `LangString`, an empty
/// `expression`, and a `value` typed `ExpressionLangString` in a `name`:
/// [`Fault::Defect`] (ADR 0033).
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the attributes and one over the children, each field in turn"
)]
fn read_lang_string<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    part: LangPart,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
) -> Result<(LangString, Option<ElementId>, Preserved), Fault> {
    let text = found.text;
    let mode = found.mode;
    let is_name = property == "name";
    let is_expression = check_lang_string_tag(&found.types, node)?;
    // A `value` in a `name` is the C++ shape of the name's one plain
    // LangString (LIB-COMPAT-008): typed ExpressionLangString, it does not
    // conform (SACM23-BASE-009).
    if is_expression && property == "name.value" {
        return Err(Fault::Defect);
    }
    let name_id = match part.id_owner() {
        Some(id_owner) => read_owned_id(found, node, id_owner, owner, kind)?,
        None => None,
    };
    let repeated = |field: &str| repeated_field(text, mode, node, field, owner, kind, property);
    let mut fields = Fields::default();
    // Its XMI metadata, every child of which is placed at index 0
    // (SACM23-XMI-019).
    let mut metadata = Kept::new(ElementHolder {
        kind,
        id: owner.clone(),
    });
    // The `expression` of an ExpressionLangString (§8.4), once it is given.
    let mut expression: Option<ElementId> = None;
    for attribute in node.attributes() {
        let value = || Some(attribute.value().to_owned());
        let given = match (attribute.namespace(), attribute.name()) {
            (None, "lang") => fields.lang(value()),
            (None, "content") => fields.content(value()),
            (None, "expression") if is_expression => {
                expression = Some(read_expression_attribute(
                    found, node, &attribute, owner, kind, property,
                )?);
                Ok(())
            }
            _ if is_xsi_type(&attribute) => {
                let arguments = part.arguments(kind, owner.as_str());
                found.xsi_type(&attribute, part.xsi_type(), arguments, owner.as_str())?;
                Ok(())
            }
            _ if is_type(&attribute) || is_xmi(&attribute, "id") => Ok(()),
            _ if is_object_metadata(text, &attribute) => {
                metadata.take_metadata_attribute(found, node, &attribute)?;
                Ok(())
            }
            _ => return Err(Fault::Defect),
        };
        // Attributes are unique in a start tag (XML 1.0 §3.1), so neither
        // field is given twice here.
        given.map_err(repeated)?;
    }
    // The text, all of it, and where it first is not white space.
    let mut characters = String::new();
    let mut text_at = None;
    // Whether a `value`, or a `content` carrying `lang`, gave the name.
    let mut has_entry = false;
    for child in node.children() {
        if !child.is_element() {
            if child.is_text() {
                characters.push_str(child.text().unwrap_or_default());
            }
            if text_at.is_none() {
                if let Some(first) = text_offset(text, child) {
                    fields.content(None).map_err(repeated)?;
                    text_at = Some(first);
                    let arguments = part.arguments(kind, owner.as_str());
                    found.legacy_shape(first, owner, part.text_shape(), arguments)?;
                }
            }
            continue;
        }
        if is_value_element(child, "lang") {
            // Checked whole first, then against an earlier occurrence.
            let value = read_value(found, child, owner)?;
            fields.lang(value).map_err(repeated)?;
        } else if is_value_element(child, "content") {
            let (value, lang) = read_value_element(found, child, owner, is_name)?;
            if let Some((lang, at)) = lang {
                // A name holds one LangString: a second entry merges
                // entries, which LIB-COMPAT-012 is to read.
                if std::mem::replace(&mut has_entry, true) {
                    return Err(Fault::Defect);
                }
                fields.lang(Some(lang)).map_err(repeated)?;
                let shape = named(kind, owner.as_str());
                found.legacy_shape(at, owner, Shape::NameContentLang, shape)?;
            }
            fields.content(value).map_err(repeated)?;
        } else if is_expression && is_value_element(child, "expression") {
            // One reference element (SACM23-XMI-015), checked whole first,
            // then against an earlier occurrence (XMI 2.5.1 rule 2h).
            // An `href` with a document part is SACM23-XMI-029's here
            // (ADR 0040 D1): the end is derived from the name.
            let Link::Id(target) = read_reference(found, child, false)? else {
                return Err(Fault::Defect);
            };
            if expression.is_some() {
                return Err(repeated_expression(
                    found, node, child, owner, kind, property,
                ));
            }
            expression = Some(target);
        } else if is_name && is_value_element(child, "value") {
            // The name's one LangString: the name holds nothing else, and
            // an ExpressionLangString name holds no such shape.
            if is_expression || holds_beside(text, node, child) {
                return Err(Fault::Defect);
            }
            let value = read_name_value(found, child, owner, kind)?;
            has_entry = true;
            let shape = named(kind, owner.as_str());
            found.legacy_shape(child.range().start, owner, Shape::NameValue, shape)?;
            if let Some(lang) = value.lang() {
                fields.lang(Some(lang.to_owned())).map_err(repeated)?;
            }
            if let Some(content) = value.content() {
                fields.content(Some(content.to_owned())).map_err(repeated)?;
            }
        } else if is_extension(child) {
            metadata.take_element(found, child)?;
        } else {
            // Another XMI element is no metadata here; anything else does
            // not conform either (ADR 0033).
            return Err(Fault::Defect);
        }
    }
    let mut name = fields.value;
    if text_at.is_some() {
        name.set_content(Some(characters));
    }
    if is_expression {
        name = match expression {
            Some(expression) => name.with_expression(expression),
            None => name.with_no_expression(),
        };
    }
    Ok((name, name_id, metadata.into_preserved()))
}

/// The one id of the `expression` attribute `attribute` of `node`, an
/// `ExpressionLangString` `name` or `value` of the element `owner`
/// (SACM23-BASE-009, ADR 0038 D2): an IDREF (XMI 2.5.1 rule 4i), so one
/// token. With none, the empty value or white space, it does not conform
/// ([`Fault::Defect`]); two or more are a repeat of the single-valued
/// `<property>.expression` (SACM-MULT-001), at the second token in tolerant
/// mode.
fn read_expression_attribute(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    attribute: &Attribute<'_, '_>,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
) -> Result<ElementId, Fault> {
    let mut tokens = id_tokens(attribute.value());
    let Some(first) = tokens.next() else {
        return Err(Fault::Defect);
    };
    if tokens.next().is_none() {
        return Ok(ElementId::new(first));
    }
    let offsets = expression_occurrences(found.text, node);
    let second = offsets
        .get(1)
        .copied()
        .unwrap_or_else(|| attribute.range_qname().start);
    let property = format!("{property}.expression");
    Err(fault(repeat_or_refuse(
        found.mode,
        second,
        owner,
        &property,
        || repeated(found.text, owner, kind, &property, offsets),
    )))
}

/// The repeat of the `expression` of `node`, an `ExpressionLangString`
/// `name` or `value` of the element `owner`, given again by the reference
/// element `again` (SACM-MULT-001; XMI 2.5.1 rule 2h): every occurrence in
/// strict mode, the Warning at `again` in tolerant mode, with which the
/// whole of `node` is kept (ADR 0035 D1).
fn repeated_expression(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    again: Node<'_, '_>,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
) -> Fault {
    let property = format!("{property}.expression");
    fault(repeat_or_refuse(
        found.mode,
        again.range().start,
        owner,
        &property,
        || {
            repeated(
                found.text,
                owner,
                kind,
                &property,
                expression_occurrences(found.text, node),
            )
        },
    ))
}

/// Every occurrence of the `expression` of `node`, in document order: each
/// token of its `expression` attribute (its name when it holds one token),
/// then each `expression` reference element.
fn expression_occurrences(text: &str, node: Node<'_, '_>) -> Vec<usize> {
    let mut offsets = Vec::new();
    if let Some(attribute) = node
        .attributes()
        .find(|attribute| is_unqualified(attribute, "expression"))
    {
        let at = attribute.range_qname().start;
        let count = id_tokens(attribute.value()).count();
        if count > 1 {
            offsets.extend(
                token_starts(text, &attribute)
                    .unwrap_or_else(|| std::iter::repeat_n(at, count).collect()),
            );
        } else {
            offsets.push(at);
        }
    }
    offsets.extend(
        node.children()
            .filter(|child| is_value_element(*child, "expression"))
            .map(|child| child.range().start),
    );
    offsets
}

/// Checks the start tag of `node`, a `name` or `value`, and returns whether
/// it is an `ExpressionLangString` (SACM23-BASE-009): a type, `xmi:type` or
/// `xsi:type`, naming `LangString` or `ExpressionLangString`. A namespace, a
/// type naming another class, and two type attributes naming different
/// classes do not conform: [`Fault::Defect`] (ADR 0033).
fn check_lang_string_tag(types: &Types<'_>, node: Node<'_, '_>) -> Result<bool, Fault> {
    if node
        .tag_name()
        .namespace()
        .is_some_and(|uri| !uri.is_empty())
    {
        return Err(Fault::Defect);
    }
    let mut named = None;
    for attribute in node.attributes().filter(is_type) {
        let Some(class @ ("LangString" | "ExpressionLangString")) =
            sacm_class(types, node, attribute.value())
        else {
            return Err(Fault::Defect);
        };
        if named.is_some_and(|earlier| earlier != class) {
            return Err(Fault::Defect);
        }
        named = Some(class);
    }
    Ok(named == Some("ExpressionLangString"))
}

/// The fields of a `LangString` as they are read, each with whether it is
/// given, a nil one included (SACM23-XMI-026).
#[derive(Default)]
struct Fields {
    value: LangString,
    has_lang: bool,
    has_content: bool,
}

impl Fields {
    /// Gives the `lang`, or returns `"lang"` if it is given already.
    fn lang(&mut self, lang: Option<String>) -> Result<(), &'static str> {
        if std::mem::replace(&mut self.has_lang, true) {
            return Err("lang");
        }
        self.value.set_lang(lang);
        Ok(())
    }

    /// Gives the `content`, or returns `"content"` if it is given already.
    fn content(&mut self, content: Option<String>) -> Result<(), &'static str> {
        if std::mem::replace(&mut self.has_content, true) {
            return Err("content");
        }
        self.value.set_content(content);
        Ok(())
    }
}

/// The `LangString` that `node`, a `value` in the `name` of the element
/// `owner` of `kind`, holds: the C++ shape of a name's one `LangString`
/// (LIB-COMPAT-008), read as a `value` of a `content` is. The name is
/// written back as one `name` element, so what only a `value` can carry does
/// not conform: an `xmi:id` (a name has one id, on the `name`) and XMI
/// metadata, which could not be written where it stood (SACM23-XMI-019):
/// [`Fault::Defect`], which keeps the whole name.
fn read_name_value<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    owner: &ElementId,
    kind: ElementKind,
) -> Result<LangString, Fault> {
    if node.attributes().any(|attribute| is_xmi(&attribute, "id")) {
        return Err(Fault::Defect);
    }
    let (value, _, metadata) =
        read_lang_string(found, node, LangPart::NameValue, owner, kind, "name.value")?;
    if !metadata.is_empty() {
        return Err(Fault::Defect);
    }
    Ok(value)
}

/// Whether `name`, a `name` element, holds a field besides its `value`
/// child `value`: a `lang` or `content` attribute, a value element, another
/// `value`, or text that is not all white space.
fn holds_beside(text: &str, name: Node<'_, '_>, value: Node<'_, '_>) -> bool {
    name.attributes().any(|attribute| {
        is_unqualified(&attribute, "lang") || is_unqualified(&attribute, "content")
    }) || name.children().any(|child| {
        child != value
            && (["lang", "content", "value"]
                .iter()
                .any(|local| is_value_element(child, local))
                || text_offset(text, child).is_some())
    })
}

/// Where the refusal of `node` is, if it is text whose value is not all XML
/// white space: its first character in the input that is not literal white
/// space (see [`check_text`]).
fn text_offset(text: &str, node: Node<'_, '_>) -> Option<usize> {
    let content = node.text().filter(|_| node.is_text())?;
    if content.chars().all(is_xml_space) {
        return None;
    }
    // roxmltree gives the range of the text's first piece only, so the
    // first character that is not literal white space is looked for in the
    // input.
    let start = node.range().start;
    Some(
        text.get(start..)
            .and_then(|rest| rest.find(|c: char| !is_xml_space(c)))
            .map_or(start, |skipped| start + skipped),
    )
}

/// Whether `attribute` is `local`, in no namespace.
fn is_unqualified(attribute: &Attribute<'_, '_>, local: &str) -> bool {
    attribute.namespace().is_none() && attribute.name() == local
}

/// Whether `attribute` is `xmi:<local>`, in the XMI namespace.
fn is_xmi(attribute: &Attribute<'_, '_>, local: &str) -> bool {
    attribute.namespace() == Some(XMI_NS) && attribute.name() == local
}

/// Whether `attribute` is `xmi:type` or `xsi:type`.
fn is_type(attribute: &Attribute<'_, '_>) -> bool {
    is_xmi(attribute, "type") || is_xsi_type(attribute)
}

/// Whether `attribute` is `xsi:type`, in the XML Schema instance namespace.
fn is_xsi_type(attribute: &Attribute<'_, '_>) -> bool {
    attribute.namespace() == Some(XSI_NS) && attribute.name() == "type"
}

/// Whether `attribute` is `xsi:nil`, in the XML Schema instance namespace.
fn is_xsi_nil(attribute: &Attribute<'_, '_>) -> bool {
    attribute.namespace() == Some(XSI_NS) && attribute.name() == "nil"
}

/// Whether the type `value`, a `QName` resolved in the scope of `node`, names
/// the SACM class `class`, in a namespace of `family`, the root's (ADR 0041
/// D5). The value's leading and trailing white space is removed first, as a
/// `QName`'s is collapsed (XML Schema 1.0 Part 2 §4.3.6); white space inside
/// it stays, so it names no class.
fn names_class(types: &Types<'_>, node: Node<'_, '_>, value: &str, class: &str) -> bool {
    sacm_class(types, node, value) == Some(class)
}

/// The concrete SACM class that the type `value` names, resolved as by
/// [`names_class`].
fn type_kind(types: &Types<'_>, node: Node<'_, '_>, value: &str) -> Option<ElementKind> {
    let class = sacm_class(types, node, value)?;
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name() == class)
}

/// The local part of the type `value`, a `QName` resolved in the scope of
/// `node` without its leading and trailing white space, if it is in a SACM
/// namespace of `family`.
fn sacm_class<'v>(types: &Types<'_>, node: Node<'_, '_>, value: &'v str) -> Option<&'v str> {
    let value = value.trim_matches(is_xml_space);
    let (prefix, local) = match value.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, value),
    };
    node.lookup_namespace_uri(prefix)
        .is_some_and(|uri| types.contains(uri))
        .then_some(local)
}

/// The reference end that `local`, the §11.2 text spelling
/// `argumentationElement`, stands for on an element of `kind`: an
/// `ArgumentGroup`'s `argumentElement` (SACM23-XMI-009). `None` for any
/// other name, and on a kind without that end, where the spelling is
/// ordinary content.
fn spelled_end(kind: ElementKind, local: &str) -> Option<ReferenceEnd> {
    Some(ReferenceEnd::GroupMember).filter(|end| local == TEXT_SPELLING && end.applies_to(kind))
}

/// The string-valued attribute that `local`, the model's spelling
/// `occurece`, stands for on an element of `kind`: an Event's `date` (§12.9,
/// spec defect D17; SACM23-XMI-009). `None` for any other name, and on
/// another kind, an Artifact included, where it is ordinary content.
fn spelled_string(kind: ElementKind, local: &str) -> Option<StringAttribute> {
    (local == EVENT_SPELLING && kind == ElementKind::Event).then_some(StringAttribute::Date)
}

/// Whether `holder` gives the property `local` under that name: as an
/// unqualified attribute or as an element in no namespace. A spelling of it
/// is then the repeat, and the name is read instead (SACM23-XMI-009).
fn gives(holder: Node<'_, '_>, local: &str) -> bool {
    holder
        .attributes()
        .any(|attribute| is_unqualified(&attribute, local))
        || holder
            .children()
            .any(|child| is_value_element(child, local))
}

/// The string-valued attribute named `local` that an element of `kind`
/// holds, if there is one (D-DATE).
fn string_attribute_named(kind: ElementKind, local: &str) -> Option<StringAttribute> {
    StringAttribute::ALL
        .into_iter()
        .find(|attribute| attribute.name() == local && attribute.is_held_by(kind))
}

/// The name of `attribute` as written: `prefix:local`, or `local`.
fn attribute_written<'t>(text: &'t str, attribute: &Attribute<'t, '_>) -> &'t str {
    text.get(attribute.range_qname())
        .unwrap_or(attribute.name())
}

/// The arguments of the refusal of `attribute` of the element written as
/// `element`, which this release does not read: the attribute as written,
/// then the element.
fn attribute_arguments(text: &str, attribute: &Attribute<'_, '_>, element: &str) -> Vec<Argument> {
    text_arguments(&[
        ("attribute", attribute_written(text, attribute)),
        ("element", element),
    ])
}

/// The name of `element` as written in its start tag: `prefix:Local`, or
/// `Local`.
fn written_name(text: &str, element: Node<'_, '_>) -> String {
    text.get(element.range().start + 1..)
        .and_then(|tag| {
            let end = tag.find(|c: char| is_xml_space(c) || matches!(c, '/' | '>'))?;
            tag.get(..end)
        })
        .unwrap_or(element.tag_name().name())
        .to_owned()
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::{SourceLocation, has_errors};
    use crate::model::{Document, ElementId, MultiLangString, SacmElement};
    use crate::xmi::{Mode, ReadError, ReadOptions, WriteOptions, read, write};

    /// An element `id` of `kind`, written `written`, under `parent`, with
    /// nothing read.
    fn pending_of(
        parent: Option<ElementId>,
        kind: crate::model::ElementKind,
        id: &str,
        written: &str,
    ) -> super::Pending {
        let holder = || super::ElementHolder {
            kind,
            id: ElementId::new(id),
        };
        super::Pending {
            order: 0,
            place: 0,
            parent_order: parent.as_ref().map(|_| 0),
            parent,
            kind,
            id: ElementId::new(id),
            gid: None,
            has_gid: false,
            name: None,
            name_id: None,
            content: None,
            shorthand: super::Shorthand::default(),
            key: None,
            location: None,
            strings: Vec::new(),
            declaration: None,
            flags: Vec::new(),
            references: Vec::new(),
            hrefs: Vec::new(),
            given: Vec::new(),
            written: written.to_owned(),
            at: 0,
            kept: super::Kept::new(holder()),
            metadata: super::Kept::new(holder()),
            name_metadata: None,
            slots: None,
        }
    }

    /// The (reason, holder) table of the kept forms gives each cell: the
    /// refusal of each reason and the diagnostic a tolerant read gives
    /// instead, in an element and in the document; and each C++ shape its
    /// refusal and its `-read` form (LIB-API-018).
    #[test]
    fn the_reason_and_holder_mapping_gives_each_cell_of_the_table() {
        use super::{DocumentReason, ElementReason, Shape};
        use crate::diagnostic::template::{Carrier, Template};

        let ids = |refusal: &Template, kept: &Template| (refusal.id, kept.id);
        let in_element: Vec<_> = ElementReason::ALL
            .iter()
            .map(|reason| ids(reason.refusal(), reason.kept()))
            .collect();
        assert_eq!(
            in_element,
            [
                (
                    "SACM-XMI-007/unread-attribute",
                    "SACM-XMI-007/unread-attribute-kept"
                ),
                ("SACM-XMI-007/proxy-child", "SACM-XMI-007/proxy-child-kept"),
                (
                    "SACM-XMI-007/proxy-child-of-root",
                    "SACM-XMI-007/proxy-child-of-root-kept"
                ),
                ("SACM-XMI-007/href", "SACM-XMI-007/href-kept"),
            ],
            "LIB-API-018: the element column"
        );
        let in_document: Vec<_> = DocumentReason::ALL
            .iter()
            .map(|reason| ids(reason.refusal(), reason.kept()))
            .collect();
        assert_eq!(
            in_document,
            [
                (
                    "SACM-XMI-003/unread-attribute",
                    "SACM-XMI-003/wrapper-attribute-kept"
                ),
                (
                    "SACM-XMI-003/wrapper-text",
                    "SACM-XMI-003/wrapper-text-kept"
                ),
                (
                    "SACM-XMI-003/wrapper-element",
                    "SACM-XMI-003/wrapper-element-kept"
                ),
            ],
            "LIB-API-018: the document column"
        );
        let shapes: Vec<_> = Shape::ALL
            .iter()
            .map(|shape| ids(shape.refusal(), shape.read()))
            .collect();
        for (refusal, read) in &shapes {
            assert_eq!(
                format!("{refusal}-read"),
                *read,
                "LIB-API-018: {refusal}: the -read form"
            );
        }
        assert_eq!(shapes.len(), 9, "LIB-API-018: the shapes");
        let pairs = ElementReason::ALL
            .iter()
            .map(|reason| (reason.refusal(), reason.kept()))
            .chain(
                DocumentReason::ALL
                    .iter()
                    .map(|reason| (reason.refusal(), reason.kept())),
            )
            .chain(
                Shape::ALL
                    .iter()
                    .map(|shape| (shape.refusal(), shape.read())),
            );
        for (refusal, kept) in pairs {
            assert_eq!(
                (refusal.code, kept.carrier),
                (kept.code, Carrier::Diagnostic),
                "LIB-API-018: {} and {}: one code, and a diagnostic's form",
                refusal.id,
                kept.id
            );
            assert_ne!(
                refusal.carrier,
                Carrier::Diagnostic,
                "LIB-API-018: {}: a refusal's template",
                refusal.id
            );
        }
    }

    /// The refusal of an element the document could not hold names a root
    /// as the root element and a child as an element (LIB-API-017,
    /// acceptance 4): the templates no input reaches.
    #[test]
    fn an_unheld_root_and_child_have_their_templates() {
        use crate::diagnostic::{Argument, template};
        use crate::model::ElementKind;

        let arguments = |element: &str| {
            vec![
                Argument::text("element", element),
                Argument::text("error", "e"),
            ]
        };
        let root = pending_of(
            None,
            ElementKind::ArgumentPackage,
            "p",
            "sacm:ArgumentPackage",
        );
        let details = super::unheld(&root, "e");
        assert_eq!(
            (details.template(), details.arguments()),
            (
                template::SEC_002_UNHELD_ROOT.id,
                arguments("sacm:ArgumentPackage").as_slice()
            ),
            "LIB-API-017: a root"
        );
        assert_eq!(
            details.render(),
            "the root element \"sacm:ArgumentPackage\" cannot be held in the document: e"
        );
        let child = pending_of(
            Some(ElementId::new("p")),
            ElementKind::Claim,
            "c1",
            "argumentElement",
        );
        let details = super::unheld(&child, "e");
        assert_eq!(
            (details.template(), details.arguments()),
            (
                template::SEC_002_UNHELD_ELEMENT.id,
                arguments("argumentElement").as_slice()
            ),
            "LIB-API-017: a child"
        );
        assert_eq!(
            details.render(),
            "the element \"argumentElement\" cannot be held in the document: e"
        );
    }

    const OPEN: &str = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n"
    );

    /// The strict refusal of `content` in the root `p`, on line 2.
    fn refusal(content: &str) -> Option<(Option<SourceLocation>, String)> {
        let input = format!("{OPEN}{content}</sacm:ArgumentPackage>");
        match read(input.as_bytes(), &strict()) {
            Err(ReadError::Unsupported {
                location, reason, ..
            }) => Some((location, reason)),
            _ => None,
        }
    }

    /// Options that read in strict mode.
    fn strict() -> ReadOptions {
        ReadOptions::default().with_mode(Mode::Strict)
    }

    /// The locations of the diagnostics of `content` in the root `p`, read.
    fn locations(content: &str) -> Option<Vec<Option<SourceLocation>>> {
        let input = format!("{OPEN}{content}</sacm:ArgumentPackage>");
        let output = read(input.as_bytes(), &ReadOptions::default()).ok()?;
        Some(
            output
                .diagnostics()
                .iter()
                .map(crate::diagnostic::Diagnostic::location)
                .collect(),
        )
    }

    #[test]
    fn an_href_is_an_id_a_cross_document_reference_a_pointer_or_names_no_element() {
        use super::HrefForm::{Id, NoElement, Other, Pointer};
        for (value, expected) in [
            ("#c2", Id("c2")),
            ("#1c", Id("1c")),
            ("#a%20b", Id("a%20b")),
            ("other.xmi#c2", Other),
            ("a%20b.xmi#//@argumentElement.0", Other),
            ("#xpointer(id('c2'))", Pointer),
            ("o.xmi#xpointer(id('c2'))", Pointer),
            ("o.xmi#a)", Pointer),
            ("", NoElement),
            ("#", NoElement),
            ("o.xmi#", NoElement),
            ("c2", NoElement),
            ("#a#b", NoElement),
            ("o.xmi#a#b", NoElement),
            ("#a b", NoElement),
            ("o.xmi#a b", NoElement),
            ("o xmi#a", NoElement),
            ("#a\tb", NoElement),
            ("#a\nb", NoElement),
            ("#a\rb", NoElement),
            ("#x(a b)", NoElement),
        ] {
            assert_eq!(super::classify_href(value), expected, "{value:?}");
        }
    }

    #[test]
    fn a_character_reference_stands_for_its_character() {
        for (name, character) in [
            ("lt", Some('<')),
            ("gt", Some('>')),
            ("amp", Some('&')),
            ("apos", Some('\'')),
            ("quot", Some('"')),
            ("#32", Some(' ')),
            ("#x9", Some('\t')),
            ("#x1F600", Some('\u{1F600}')),
            ("#xD800", None),
            ("nbsp", None),
            ("#", None),
        ] {
            assert_eq!(super::character_reference(name), character, "{name}");
        }
    }

    #[test]
    fn id_tokens_split_on_xml_white_space_only() {
        let tokens: Vec<&str> = super::id_tokens(" a\tb\n\rc  d\u{A0}e ").collect();
        assert_eq!(tokens, ["a", "b", "c", "d\u{A0}e"]);
        assert_eq!(super::id_tokens(" \t ").count(), 0);
    }

    #[test]
    fn a_reference_or_cdata_section_that_stands_for_white_space_is_white_space() {
        for content in [
            "  &#x20;",
            "&#9;&#10;&#13;&#x20;",
            "  <![CDATA[ ]]>",
            "<![CDATA[	
]]>&#xD; ",
            " <!-- c --> &#9;",
            " 	<!-- c -->
 ",
        ] {
            assert_eq!(locations(content), Some(Vec::new()), "{content:?}");
        }
    }

    #[test]
    fn other_text_is_preserved_at_its_first_character_that_is_not_literal_white_space() {
        for (content, column) in [
            ("  x", 3),
            ("  &#xA0;", 3),
            ("  <![CDATA[x]]>", 3),
            (" <![CDATA[ ]]>&#x20;x", 2),
            (" <!-- c --> &#9;x", 13),
        ] {
            assert_eq!(
                locations(content),
                Some(vec![SourceLocation::at(2, column)]),
                "{content:?}"
            );
        }
    }

    /// An `xsi:type` on a `content` or a `value` is read, and reported on the
    /// `UtilityElement`, as on a `name` (SACM-XMI-010).
    #[test]
    fn an_xsi_type_on_a_content_or_a_value_is_reported_on_the_owner() {
        let input = format!(
            "{OPEN}<note xmi:id=\"n\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
             <content xsi:type=\"sacm:MultiLangString\">\
             <value xsi:type=\"sacm:LangString\" content=\"x\"/></content></note>\
             </sacm:ArgumentPackage>"
        );
        let output = read(input.as_bytes(), &ReadOptions::default());
        let Ok(output) = output else {
            panic!("{output:?}");
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code().as_str(),
                    found.affected()[0].as_str(),
                    found.message().contains("content"),
                    found.location().map(SourceLocation::column),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("SACM-XMI-010", "n", true, Some(81)),
                ("SACM-XMI-010", "n", true, Some(120)),
            ]
        );
        let note = output.document().get("n");
        assert_eq!(
            note.and_then(SacmElement::content)
                .map(MultiLangString::len),
            Some(1)
        );
    }

    #[test]
    fn refusals_name_the_child_element_and_its_entry() {
        // An XMI extension in a child is XMI metadata, read in both modes
        // (SACM23-XMI-019).
        let extension = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><xmi:Extension/></argumentElement>";
        assert_eq!(refusal(extension), None, "{extension:?}");
        let cases = [(
            "<argumentElement href=\"o.xmi#c\"/>",
            "the element \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is in reference form, a proxy of an element of the class ArgumentationElement, which is not read in this release (a limit of this version; SACM23-XMI-029 reads it)",
        )];
        for (content, reason) in cases {
            assert_eq!(
                refusal(content).map(|(_, reason)| reason).as_deref(),
                Some(reason),
                "{content:?}"
            );
        }
        // Every child kind its role admits is read: a
        // TerminologyPackageInterface in an AssuranceCasePackage, refused
        // until slice 0067 (SACM23-TERM-007).
        let input = concat!(
            "<sacm:AssuranceCasePackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
            " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\">\n",
            "<terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"t\"/>",
            "</sacm:AssuranceCasePackage>"
        );
        assert!(read(input.as_bytes(), &strict()).is_ok(), "{input:?}");
        // The text's spelling of the role is kept, not refused
        // (SACM23-XMI-009).
        let spelled = "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>";
        assert_eq!(refusal(spelled), None, "{spelled:?}");
    }

    /// The model's spelling `ArtifactAssertedRelationship` is a spelling only
    /// where a role admits the kind it spells, an `ArtifactPackage`'s
    /// `artifactElement`: kept with a SACM-XMI-013 Error by a strict read and
    /// read by a tolerant one. Elsewhere it is a type the role does not
    /// admit, preserved as any such type is (ADR 0033, ADR 0034 amendment 2).
    #[test]
    fn the_model_spelling_is_a_spelling_only_where_its_kind_is_admitted() {
        let typed =
            "<argumentElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r\"/>";
        assert_eq!(refusal(typed), None, "in an ArgumentPackage");
        assert_eq!(locations(typed).map(|found| found.len()), Some(1));
        let input = format!("{OPEN}{typed}</sacm:ArgumentPackage>");
        let output = read(input.as_bytes(), &strict()).unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(
            output
                .diagnostics()
                .iter()
                .map(|found| found.code().as_str())
                .collect::<Vec<_>>(),
            ["SACM-XMI-003"]
        );

        let input = concat!(
            "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
            " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n",
            "<artifactElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r\"/>",
            "</sacm:ArtifactPackage>"
        );
        let reason = "the element \"artifactElement\" in the root element \
                      \"sacm:ArtifactPackage\" is typed ArtifactAssertedRelationship, the \
                      machine-readable model's spelling of ArtifactAssetRelationship";
        for (mode, affected, ending, read_as) in [
            (
                Mode::Strict,
                "p",
                "; it is kept as preserved content, and a tolerant read reads the name as \
                 ArtifactAssetRelationship",
                None,
            ),
            (
                Mode::Tolerant,
                "r",
                "; it is read as ArtifactAssetRelationship, which write writes",
                Some(crate::model::ElementKind::ArtifactAssetRelationship),
            ),
        ] {
            let output = read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
                .unwrap_or_else(|error| panic!("{mode:?}: {error:?}"));
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .map(|found| {
                    (
                        found.code().as_str(),
                        found.message().to_owned(),
                        found.affected()[0].as_str(),
                        found.location(),
                    )
                })
                .collect();
            assert_eq!(
                found,
                [(
                    "SACM-XMI-013",
                    format!("{reason}{ending}"),
                    affected,
                    SourceLocation::new(2, 1)
                )],
                "{mode:?}"
            );
            assert_eq!(
                output.document().get("r").map(SacmElement::kind),
                read_as,
                "{mode:?}"
            );
        }
    }

    /// A diagnostic as its code, affected ids and `(line, column)`.
    type Finding = (String, Vec<String>, Option<(u32, u32)>);

    /// What a tolerant read of `input` reports: each diagnostic's code,
    /// severity, affected ids and location, and the document. The read must
    /// return one, report no Error, and write back to a document that reads
    /// equal and writes the same bytes again (ADR 0035 D1).
    fn kept(input: &str) -> (Vec<Finding>, Document) {
        let output = match read(input.as_bytes(), &ReadOptions::default()) {
            Ok(output) => output,
            Err(error) => panic!("refused: {error:?}\n{input}"),
        };
        assert!(!has_errors(output.diagnostics()), "{output:?}");
        let found = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code().as_str().to_owned(),
                    found
                        .affected()
                        .iter()
                        .map(|id| id.as_str().to_owned())
                        .collect(),
                    found.location().map(|at| (at.line(), at.column())),
                )
            })
            .collect();
        let document = output.into_document();
        let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
        let text = write(&document, &tolerant).unwrap_or_else(|error| panic!("{error:?}"));
        let back = read(text.as_bytes(), &ReadOptions::default())
            .unwrap_or_else(|error| panic!("{error:?}\n{text}"))
            .into_document();
        assert!(back == document, "{text}");
        assert_eq!(write(&back, &tolerant).ok().as_deref(), Some(text.as_str()));
        (found, document)
    }

    /// `content` in the root `p`, on line 2.
    fn in_p(content: &str) -> String {
        format!("{OPEN}{content}</sacm:ArgumentPackage>")
    }

    /// The finding `(code, affected, (line, column))`.
    fn finding(code: &str, affected: &[&str], at: (u32, u32)) -> Finding {
        (
            code.to_owned(),
            affected.iter().map(|&id| id.to_owned()).collect(),
            Some(at),
        )
    }

    /// The renamed prefix skips one bound at the holder and one the holder's
    /// other preserved attributes use, and is shared by every attribute of
    /// its namespace (ADR 0035).
    #[test]
    fn a_rebound_prefix_is_renamed_once_per_namespace_past_taken_prefixes() {
        let input = in_p(
            "<argumentElement xmlns:s=\"http://www.omg.org/spec/SACM/20220301\" \
             xmlns:sacm=\"urn:o\" xmlns:ns1=\"urn:t\" xmi:type=\"s:Claim\" xmi:id=\"c\" \
             sacm:a=\"1\" sacm:b=\"2\"/>",
        );
        let (found, document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(codes, ["SACM-XMI-003", "SACM-XMI-003"]);
        let Some(c) = document.get("c").and_then(SacmElement::preserved) else {
            panic!("c holds nothing");
        };
        assert_eq!(c.namespaces, [("ns2".to_owned(), "urn:o".to_owned())]);
        let prefixes: Vec<Option<&str>> = c
            .attributes
            .iter()
            .map(|attr| attr.name.prefix.as_deref())
            .collect();
        assert_eq!(prefixes, [Some("ns2"), Some("ns2")]);
        // On the wrapper, too, the prefix is renamed.
        let wrapped = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:s=\"http://www.omg.org/spec/SACM/20220301\" xmlns:sacm=\"urn:o\" sacm:w=\"1\">\
             <s:ArgumentPackage xmi:id=\"p\"/></xmi:XMI>";
        let (found, document) = kept(wrapped);
        assert_eq!(found, [finding("SACM-XMI-003", &[], (1, 126))]);
        let outside = document.document_preserved();
        assert_eq!(
            outside.map(|outside| outside.namespaces.clone()),
            Some(vec![("ns1".to_owned(), "urn:o".to_owned())])
        );
    }

    /// Text that repeats a `UtilityElement`'s content is kept, once per run of
    /// text, and the content read stays the first (ADR 0035 D1).
    #[test]
    fn text_repeating_a_utility_content_is_kept_with_one_warning() {
        let input =
            in_p("<note xmi:id=\"n\"><content><value content=\"a\"/></content>x<!--c-->y</note>");
        let (found, document) = kept(&input);
        assert_eq!(found, [finding("SACM-MULT-001", &["n"], (2, 57))]);
        let note = document.get("n");
        assert_eq!(
            note.and_then(SacmElement::content)
                .map(|content| content.values()[0].content().map(str::to_owned)),
            Some(Some("a".to_owned()))
        );
        assert!(note.is_some_and(SacmElement::has_preserved_content));

        // `content=` and then text: the text is kept, the attribute read.
        let input = in_p("<note xmi:id=\"n\" content=\"a\">b</note>");
        let (found, document) = kept(&input);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[1], finding("SACM-MULT-001", &["n"], (2, 30)));
        assert_eq!(
            document
                .get("n")
                .and_then(SacmElement::content)
                .and_then(|content| content.values()[0].content().map(str::to_owned)),
            Some("a".to_owned())
        );

        // On a TaggedValue, `value=` after `content=` is kept.
        let input = in_p("<taggedValue xmi:id=\"t\" key=\"k\" content=\"a\" value=\"b\"/>");
        let (found, document) = kept(&input);
        let repeats: Vec<_> = found
            .iter()
            .filter(|(code, _, _)| code == "SACM-MULT-001")
            .collect();
        assert_eq!(repeats, [&finding("SACM-MULT-001", &["t"], (2, 45))]);
        // Both attributes are kept, and the shape gives no content.
        let t = document.get("t");
        assert!(t.is_some_and(|t| t.content().is_none()));
        assert_eq!(
            t.and_then(SacmElement::preserved)
                .map(|kept| kept.attributes.len()),
            Some(2)
        );
    }

    /// A part kept for a value that is no literal counts as the property's
    /// occurrence, so the next is a repeat; one kept for another reason does
    /// not (ADR 0033, ADR 0035).
    #[test]
    fn a_kept_bad_literal_counts_as_an_occurrence_and_other_kept_parts_do_not() {
        let input = in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">\
             <name><content xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xsi:nil=\"x\"/></name><name content=\"B\"/></argumentElement>",
        );
        let (found, document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(codes, ["SACM-ENUM-001", "SACM-MULT-001"]);
        assert!(document.get("c").is_some_and(|c| c.name().is_none()));

        let input = in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">\
             <name href=\"#c\" content=\"A\"/><name content=\"B\"/>\
             </argumentElement>",
        );
        let (found, document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(codes, ["SACM-XMI-003"]);
        assert_eq!(
            document
                .get("c")
                .and_then(SacmElement::name)
                .and_then(|name| name.content()),
            Some("B")
        );

        // A bad xsi:nil on a gid counts too, and so does a bad declaration
        // element before a good one.
        let input = in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><gid xsi:nil=\"x\"/>\
             <gid>g</gid><assertionDeclaration>no</assertionDeclaration>\
             <assertionDeclaration>assumed</assertionDeclaration><content>x</content>\
             </argumentElement>",
        );
        let (found, document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(
            codes,
            [
                "SACM-ENUM-001",
                "SACM-MULT-001",
                "SACM-ENUM-001",
                "SACM-MULT-001",
                "SACM-XMI-003"
            ]
        );
        let c = document.get("c");
        assert_eq!(c.and_then(SacmElement::gid), None);
        assert_eq!(
            c.and_then(SacmElement::assertion_declaration),
            Some(crate::model::AssertionDeclaration::Asserted)
        );
    }

    /// Two ids on a single-valued end are kept with the attribute, and count
    /// as its occurrence; a set of the end releases what they named.
    #[test]
    fn a_kept_two_id_end_counts_as_its_occurrence_and_a_set_replaces_it() {
        let input = in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\
             <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i\" source=\"c1\" \
             target=\"c1 c2\"><target xmi:idref=\"c1\"/></argumentElement>",
        );
        let (found, mut document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(codes, ["SACM-MULT-001", "SACM-MULT-001"]);
        assert!(document.get("i").is_some_and(|i| i.target().is_none()));
        assert!(document.is_referenced("c2"));
        let set = crate::edit::SetReferences::new("i", crate::model::ReferenceEnd::Target, ["c1"]);
        assert!(crate::edit::apply(&mut document, set).is_ok());
        assert!(
            !document.is_referenced("c2"),
            "the kept attribute's c2 is released"
        );
        let i = document.get("i");
        assert_eq!(
            i.and_then(SacmElement::target).map(ElementId::as_str),
            Some("c1")
        );
    }

    /// Removing a kept attribute releases exactly the references it counted:
    /// none for a flag, the ids of a reference end, each once per time it was
    /// counted, and never another id the holder's preserved content names.
    #[test]
    fn replacing_a_kept_attribute_releases_only_its_own_references() {
        let input = in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"/>\
             <argumentElement xmlns:v=\"urn:v\" xmi:type=\"sacm:AssertedInference\" \
             xmi:id=\"i\" source=\"c1\" target=\"c1 c1\" isCounter=\"c3\" v:note=\"c3\">\
             <v:x href=\"#c3\"/></argumentElement>",
        );
        let (_, mut document) = kept(&input);
        assert!(document.is_referenced("c3"), "the preserved href counts c3");
        // A flag named no reference: c3 stays referenced by the href.
        let flag = crate::edit::SetFlag::new("i", crate::model::Flag::IsCounter, false);
        assert!(crate::edit::apply(&mut document, flag).is_ok());
        assert!(
            document.is_referenced("c3"),
            "removing isCounter=\"c3\" released c3"
        );
        // The end's two c1 are released, and nothing else.
        let set = crate::edit::SetReferences::new("i", crate::model::ReferenceEnd::Target, ["c1"]);
        assert!(crate::edit::apply(&mut document, set).is_ok());
        assert!(
            document.is_referenced("c3"),
            "removing target=\"c1 c1\" released c3"
        );
        assert!(
            document
                .get("i")
                .is_some_and(SacmElement::has_preserved_content)
        );
        // Deleting the holder releases the href's c3.
        let delete = crate::edit::DeleteElement::new("i")
            .with_package_policy(crate::edit::PackageDeletePolicy::DeleteRecursively);
        assert!(crate::edit::apply(&mut document, delete).is_ok());
        assert!(
            !document.is_referenced("c3"),
            "the deleted href still counts c3"
        );
    }

    /// A repeat kept in tolerant mode leaves the first occurrence's value
    /// held; a part kept because it does not conform is no occurrence, so
    /// the next one is read; one kept for a value that is no literal is.
    #[test]
    fn a_kept_repeat_keeps_the_first_value_and_a_defective_part_is_no_occurrence() {
        let claim = |attributes: &str, body: &str| {
            in_p(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" \
                 xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"{attributes}>{body}\
                 </argumentElement>"
            ))
        };
        let codes = |found: &[Finding]| -> Vec<String> {
            found.iter().map(|(code, _, _)| code.clone()).collect()
        };
        // The first value stays: a declaration, a flag given as an
        // attribute and as an element, and a flag given twice as elements.
        let (found, document) = kept(&claim(
            " isCitation=\"true\"",
            "<assertionDeclaration>assumed</assertionDeclaration>\
             <assertionDeclaration>axiomatic</assertionDeclaration>\
             <isCitation>false</isCitation><isAbstract>true</isAbstract>\
             <isAbstract>false</isAbstract>",
        ));
        assert_eq!(codes(&found), ["SACM-MULT-001"; 3]);
        let c = document.get("c");
        assert_eq!(
            c.and_then(SacmElement::assertion_declaration),
            Some(crate::model::AssertionDeclaration::Assumed)
        );
        assert!(c.is_some_and(|c| c.is_citation() && c.is_abstract()));
        // A part that does not conform is no occurrence: the next is read.
        let (found, document) = kept(&claim(
            "",
            "<assertionDeclaration v=\"1\">assumed</assertionDeclaration>\
             <assertionDeclaration>axiomatic</assertionDeclaration>\
             <isCitation v=\"1\">false</isCitation><isCitation>true</isCitation>\
             <note xmi:id=\"n\"><content xmi:type=\"sacm:Claim\"/>\
             <content><value content=\"b\"/></content></note>",
        ));
        assert_eq!(codes(&found), ["SACM-XMI-003"; 3]);
        let c = document.get("c");
        assert_eq!(
            c.and_then(SacmElement::assertion_declaration),
            Some(crate::model::AssertionDeclaration::Axiomatic)
        );
        assert!(c.is_some_and(SacmElement::is_citation));
        assert!(document.get("n").and_then(SacmElement::content).is_some());
        // A content kept for a value that is no literal is an occurrence.
        let (found, document) = kept(&claim(
            "",
            "<note xmi:id=\"n\"><content><value><lang xsi:nil=\"x\"/></value></content>\
             <content><value content=\"b\"/></content></note>",
        ));
        assert_eq!(codes(&found), ["SACM-ENUM-001", "SACM-MULT-001"]);
        assert!(document.get("n").is_some_and(|n| n.content().is_none()));
    }

    /// The §11.2 spelling as an attribute is read on an `ArgumentGroup` as
    /// its members, with SACM-XMI-013; on any other kind it is an attribute
    /// the class lacks (SACM23-XMI-009).
    #[test]
    fn the_text_spelling_attribute_is_read_on_a_group_only() {
        let (found, document) = kept(&in_p(
            "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\" \
             argumentationElement=\"g\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" argumentationElement=\"g\"/>\
             <argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"h\" v=\"1\"/>",
        ));
        let codes: Vec<(&str, &str)> = found
            .iter()
            .map(|(code, affected, _)| (code.as_str(), affected[0].as_str()))
            .collect();
        assert_eq!(
            codes,
            [
                ("SACM-XMI-013", "g"),
                ("SACM-XMI-003", "c"),
                ("SACM-XMI-003", "h")
            ]
        );
        assert_eq!(
            document.get("g").map(|g| g
                .group_members()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>()),
            Some(vec!["g"])
        );
        assert!(
            document
                .get("c")
                .is_some_and(|c| c.group_members().is_empty())
        );
    }

    /// A kept construct's message names its holder, the element or the
    /// document, and ends with what reads it.
    #[test]
    fn a_kept_message_names_the_holder_and_the_row() {
        let message = |input: &str| -> String {
            let output = read(input.as_bytes(), &ReadOptions::default());
            output.map_or_else(
                |error| panic!("{error:?}"),
                |output| output.diagnostics()[0].message().to_owned(),
            )
        };
        let on_claim = message(&in_p(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" href=\"o.xmi#c\"/>",
        ));
        assert_eq!(
            on_claim,
            "the attribute \"href\" of the element \"argumentElement\" is not read in this \
             release, so it is kept as preserved content of the ArgumentPackage \"p\", which a \
             tolerant write writes back and a strict write refuses; SACM23-XMI-029 reads it"
        );
        let outside = message(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"><xmi:foo/>\
             <sacm:ArgumentPackage xmi:id=\"p\"/></xmi:XMI>",
        );
        assert!(
            outside.contains("kept as preserved content of the document,"),
            "{outside}"
        );
        // Content not read yet is LIB-API-011's, with no clause; a defect
        // kept on `xmi:XMI` is LIB-COMPAT-003's, clause 7.3.3.
        let first = |input: &str| {
            let output = read(input.as_bytes(), &ReadOptions::default());
            output.map_or_else(
                |error| panic!("{error:?}"),
                |output| {
                    let found = &output.diagnostics()[0];
                    (
                        found.requirement_id().to_owned(),
                        found.clause().map(str::to_owned),
                    )
                },
            )
        };
        assert_eq!(
            first(&in_p(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" href=\"o.xmi#c\"/>"
            )),
            ("LIB-API-011".to_owned(), None)
        );
        assert_eq!(
            first(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                 xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" w=\"1\">\
                 <sacm:ArgumentPackage xmi:id=\"p\"/></xmi:XMI>"
            ),
            ("LIB-COMPAT-003".to_owned(), Some("7.3.3".to_owned()))
        );
    }

    /// An empty `xmi:id` in XMI metadata the document holds outside its
    /// elements is kept, with SACM-ID-002 affecting no element
    /// (SACM23-XMI-019).
    #[test]
    fn an_empty_id_outside_every_element_affects_no_element() {
        let input = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\
             <xmi:Documentation xmi:id=\"\"/><sacm:ArgumentPackage xmi:id=\"p\"/></xmi:XMI>";
        let (found, document) = kept(input);
        assert_eq!(found, [finding("SACM-ID-002", &[], (1, 110))]);
        assert!(!document.has_preserved_content() && document.document_metadata().is_some());
        assert!(document.get("p").is_some());
    }

    /// An older revision's `occurence` beside a `date` is not read: the date
    /// is, and `occurence` is kept as preserved content (LIB-COMPAT-004).
    #[test]
    fn an_occurence_beside_a_date_is_kept() {
        let input = "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20210218\" \
                     xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arts\">\
                     <artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\" date=\"2024\" \
                     occurence=\"2021\"/></sacm:ArtifactPackage>";
        let Ok(output) = read(input.as_bytes(), &ReadOptions::default()) else {
            panic!("a tolerant read refuses an Event with a date and an occurence");
        };
        let codes: Vec<&str> = output
            .diagnostics()
            .iter()
            .map(|found| found.code().as_str())
            .collect();
        assert_eq!(codes, ["SACM-XMI-008", "SACM-XMI-003"]);
        let document = output.into_document();
        let e1 = document.get("e1");
        assert_eq!(e1.and_then(SacmElement::date), Some("2024"));
        assert!(e1.is_some_and(SacmElement::has_preserved_content));
    }

    /// The wrapper start tag binding `xmi` and `sacm`, with the extra
    /// attributes `attributes`, holding `content`.
    fn wrapper_with(attributes: &str, content: &str) -> String {
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"{attributes}>{content}</xmi:XMI>"
        )
    }

    /// The binding of `xsi`, as an attribute.
    const XSI: &str = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

    /// The root ArgumentPackage `id`, holding `body`.
    fn package(id: &str, body: &str) -> String {
        format!(
            "<sacm:ArgumentPackage xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</sacm:ArgumentPackage>"
        )
    }

    /// Reads `input` strictly, with no diagnostic, and writes it strictly.
    fn strict_clean(input: &str) -> (Document, String) {
        let output = read(input.as_bytes(), &strict()).unwrap_or_else(|error| panic!("{error:?}"));
        assert!(
            output.diagnostics().is_empty(),
            "{:?}\n{input}",
            output.diagnostics()
        );
        let document = output.into_document();
        assert!(!document.has_preserved_content(), "{input}");
        let text =
            write(&document, &WriteOptions::default()).unwrap_or_else(|error| panic!("{error:?}"));
        (document, text)
    }

    /// Every XMI element XMI.xsd declares under `xmi:XMI`, and every metadata
    /// attribute of it, is XMI metadata: read strictly with no diagnostic and
    /// written back by a strict save (SACM23-XMI-019).
    #[test]
    fn each_wrapper_element_and_attribute_of_xmi_xsd_is_metadata() {
        // XMI.xsd's global elements but `XMI` (XMI 2.5.1 §7.5.3 to §7.5.6),
        // listed here, not taken from the reader.
        for local in [
            "documentation",
            "Documentation",
            "difference",
            "Difference",
            "Add",
            "Replace",
            "Delete",
            "extension",
            "Extension",
        ] {
            let element = format!("<xmi:{local} n=\"1\"/>");
            let (document, text) =
                strict_clean(&wrapper_with("", &format!("{element}{}", package("p", ""))));
            assert!(document.document_metadata().is_some(), "{local}");
            assert!(
                text.contains(&format!("<xmi:{local} n=\"1\" />")),
                "{local}\n{text}"
            );
        }
        for attribute in [
            " xmi:id=\"w\"",
            " xmi:label=\"l\"",
            " xmi:uuid=\"u\"",
            " href=\"#p\"",
            " xmi:idref=\"p\"",
            " xmi:type=\"xmi:XMI\"",
            " xsi:schemaLocation=\"a b\"",
            " xsi:noNamespaceSchemaLocation=\"a\"",
        ] {
            let (_, text) = strict_clean(&wrapper_with(
                &format!("{XSI}{attribute}"),
                &package("p", ""),
            ));
            assert!(text.contains(attribute.trim_start()), "{attribute}\n{text}");
        }
    }

    /// `xmi:extension`, lower case, is XMI metadata in an element and in a
    /// part, as `xmi:Extension` is (XMI 2.5.1 rule 4c); the schema locations
    /// are metadata on an element and on a part (SACM23-XMI-019).
    #[test]
    fn the_lower_case_extension_and_the_schema_locations_are_metadata_everywhere() {
        let input = wrapper_with(
            "",
            &package(
                "p",
                &format!(
                    "<xmi:extension a=\"1\"/><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"{XSI} \
                     xsi:noNamespaceSchemaLocation=\"s\"><name content=\"C\" xsi:schemaLocation=\"a b\">\
                     <xmi:extension b=\"2\"/></name></argumentElement>"
                ),
            ),
        );
        let (document, text) = strict_clean(&input);
        assert!(document.get("p").and_then(SacmElement::metadata).is_some());
        assert!(text.contains("<xmi:extension a=\"1\" />"), "{text}");
        assert!(
            text.contains("xsi:noNamespaceSchemaLocation=\"s\""),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "<name content=\"C\"{XSI} xsi:schemaLocation=\"a b\">"
            )),
            "{text}"
        );
        assert!(
            text.contains(&format!("<xmi:extension{XSI} b=\"2\" />")),
            "{text}"
        );
    }

    /// A kept proxy stands where it was among its container's children: after
    /// what was placed before it, before what follows it, and no typed slot
    /// any more, so what follows it stays before the next typed child
    /// (SACM23-XMI-030).
    #[test]
    fn a_kept_proxy_keeps_its_place_and_gives_up_its_slot() {
        let input = wrapper_with(
            "",
            &package(
                "p",
                "<v:a xmlns:v=\"urn:v\"/><argumentElement xmi:idref=\"nope\"/><v:b xmlns:v=\"urn:v\"/>\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>",
            ),
        );
        let (found, document) = kept(&input);
        assert_eq!(found.len(), 3, "{found:?}");
        let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
        let text = write(&document, &tolerant).unwrap_or_else(|error| panic!("{error:?}"));
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}\n{text}"))
        };
        assert!(
            at("<v:a ") < at("<argumentElement xmi:idref=\"nope\" />")
                && at("<argumentElement xmi:idref=\"nope\" />") < at("<v:b ")
                && at("<v:b ") < at("xmi:id=\"c\""),
            "{text}"
        );
    }

    /// A root that a member of a cycle names, and that is in no cycle itself,
    /// is resolved: only the proxies of the cycle are kept (SACM23-XMI-030).
    #[test]
    fn a_root_that_a_cycle_names_is_moved_into_the_cycle() {
        let case = |id: &str, body: &str| {
            format!(
                "<sacm:AssuranceCasePackage xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</sacm:AssuranceCasePackage>"
            )
        };
        // `d` first, so the walk from it passes into the cycle before it
        // finds it.
        let input = wrapper_with(
            "",
            &[
                case("d", ""),
                case(
                    "a",
                    "<assuranceCasePackage xmi:idref=\"b\"/><assuranceCasePackage xmi:idref=\"d\"/>",
                ),
                case("b", "<assuranceCasePackage xmi:idref=\"a\"/>"),
            ]
            .concat(),
        );
        let (found, document) = kept(&input);
        assert_eq!(
            found
                .iter()
                .map(|(code, affected, _)| (code.as_str(), affected.clone()))
                .collect::<Vec<_>>(),
            [
                ("SACM-XMI-003", vec!["a".to_owned()]),
                ("SACM-XMI-003", vec!["b".to_owned()])
            ]
        );
        let roots: Vec<&str> = document.roots().map(|root| root.id().as_str()).collect();
        assert_eq!(roots, ["a", "b"]);
        assert_eq!(
            document
                .get("d")
                .and_then(SacmElement::parent)
                .map(|parent| parent.id().as_str()),
            Some("a")
        );
    }

    /// A root that comes before its proxy's container takes the proxy's
    /// place among the container's typed children, after the child read
    /// before the proxy (SACM23-XMI-030).
    #[test]
    fn a_moved_root_takes_the_proxys_place_among_the_typed_children() {
        let input = wrapper_with(
            "",
            &format!(
                "{}<sacm:AssuranceCasePackage xmi:id=\"case\"><name content=\"case\"/>\
                 <argumentPackage xmi:id=\"inline\"><name content=\"inline\"/></argumentPackage>\
                 <argumentPackage xmi:idref=\"args\"/></sacm:AssuranceCasePackage>",
                package("args", "")
            ),
        );
        let (document, _) = strict_clean(&input);
        let children: Vec<&str> = document
            .get("case")
            .map(|case| case.children().map(|child| child.id().as_str()).collect())
            .unwrap_or_default();
        assert_eq!(children, ["inline", "args"]);
    }

    /// Content the document holds outside its elements, placed before a root
    /// that stays, keeps its place before it once a root before it moves
    /// (SACM23-XMI-030, `Preserved::shift`).
    #[test]
    fn document_content_keeps_its_place_before_a_remaining_root() {
        let input = wrapper_with(
            "",
            &format!(
                "{}<xmi:Documentation n=\"1\"/><sacm:AssuranceCasePackage xmi:id=\"case\">\
                 <name content=\"case\"/><argumentPackage xmi:idref=\"args\"/>\
                 </sacm:AssuranceCasePackage>",
                package("args", "")
            ),
        );
        let (_, text) = strict_clean(&input);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}\n{text}"))
        };
        assert!(
            at("<xmi:Documentation n=\"1\" />") < at("xmi:id=\"case\""),
            "{text}"
        );
    }

    /// An extension in a `content` is placed among its values, where it
    /// stood (SACM23-XMI-019).
    #[test]
    fn an_extension_in_a_content_stays_between_its_values() {
        let input = wrapper_with(
            "",
            &package(
                "p",
                "<note xmi:id=\"n\"><content><value content=\"a\"/><xmi:Extension e=\"1\"/>\
                 <value content=\"b\"/></content></note>",
            ),
        );
        let (_, text) = strict_clean(&input);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}\n{text}"))
        };
        assert!(
            at("<value content=\"a\" />") < at("<xmi:Extension e=\"1\" />")
                && at("<xmi:Extension e=\"1\" />") < at("<value content=\"b\" />"),
            "{text}"
        );
    }
}
