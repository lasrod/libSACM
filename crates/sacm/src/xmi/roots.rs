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
//! roots, and a root of a kind not read, is kept as the document's own
//! preserved content, placed among the roots (ADR 0035 D3).
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
use super::namespaces::{SACM_NS, SACM_PREFIX, XMI_NS, XMI_PREFIX};
use super::preserve;
use super::refusal::{Tolerant, Unread, unsupported};
use super::{Mode, ReadError};
use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, Attr, DeclaredType, Document, ElementId, ElementKind,
    Flag, IdGenerator, Item, LangString, Placed, Preserved, ReferenceEnd, Role, StringAttribute,
    Text, TextRole, flag_named, is_ncname, reference_end_named, role_named,
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

    /// The end of the message of its diagnostic in `mode`.
    fn tail(self, mode: Mode) -> String {
        match mode {
            Mode::Tolerant => format!("; it is read as {}, which write writes", self.resolved()),
            Mode::Strict => format!(
                "; it is kept as preserved content, and a tolerant read reads the name as {}",
                self.resolved()
            ),
        }
    }
}

/// The `XLink` namespace, whose attributes on a reference element are a
/// cross-document link (SACM23-XMI-027).
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

/// The Warning a tolerant read gives for a construct it keeps as preserved
/// content where a strict read refuses it (ADR 0035 D1, D2): at the strict
/// refusal's byte offset, with its code. Or the Error a strict read gives
/// for a spelling it keeps (SACM-XMI-013, SACM23-XMI-009).
struct Keep {
    /// Where the strict refusal is.
    offset: usize,
    code: Code,
    /// A Warning, but an Error for a spelling a strict read keeps.
    severity: Severity,
    /// The matrix row the Warning enforces.
    requirement: &'static str,
    clause: Option<&'static str>,
    /// What the construct is and why it is not read: the start of the
    /// message.
    reason: String,
    /// The end of the message, such as the row that reads it.
    tail: String,
    /// Whether the construct is a value that is no literal, which counts as
    /// an occurrence of its property (ADR 0035, SACM23-VAL-006).
    is_literal: bool,
    /// Whether `reason` and `tail` are the whole message, which then does
    /// not name the holder: a spelling's (SACM23-XMI-009).
    whole: bool,
}

impl Keep {
    /// The Warning of `unread`, kept with `code` (`Unread::tolerant`), at
    /// byte `offset`, whose reason is the strict refusal's `reason`.
    fn unread(offset: usize, unread: Unread, code: Code, reason: &str) -> Self {
        let tail = unread
            .refusal()
            .row()
            .map(|row| format!("; {row} reads it"))
            .unwrap_or_default();
        let (requirement, clause) = if code == Code::XMI_UNKNOWN_CONTENT {
            (PRESERVED_ROW, Some("7.3.3"))
        } else {
            (UNREAD_ROW, None)
        };
        Self {
            offset,
            code,
            severity: Severity::Warning,
            requirement,
            clause,
            reason: reason.to_owned(),
            tail,
            is_literal: false,
            whole: false,
        }
    }

    /// The Warning of the value `value` of `property`, given at byte
    /// `offset`, that is no literal (SACM-ENUM-001, SACM23-VAL-006).
    fn literal(offset: usize, property: &str, value: &str) -> Self {
        Self {
            offset,
            code: Code::ENUM_INVALID_LITERAL,
            severity: Severity::Warning,
            requirement: "SACM23-VAL-006",
            clause: None,
            reason: format!(
                "the {property} {value:?} is no literal: {}",
                super::literals_reason(property)
            ),
            tail: ", and the property is read as not written".to_owned(),
            is_literal: true,
            whole: false,
        }
    }

    /// The Warning of an occurrence of the property `property` of the
    /// element `owner` at byte `offset`, after its first (SACM-MULT-001;
    /// XMI 2.5.1 §9.5.2 rule 2h).
    fn repeat(offset: usize, owner: &ElementId, property: &str) -> Self {
        Self {
            offset,
            code: Code::MULTIPLICITY_VIOLATION,
            severity: Severity::Warning,
            requirement: TOLERANT_ROW,
            clause: Some("9.5.2"),
            reason: format!(
                "the {property} of the element {:?} is given more than once, which XMI 2.5.1 \
                 §9.5.2 rule 2h does not allow, and this is not its first occurrence",
                owner.as_str()
            ),
            tail: "; the first occurrence is read".to_owned(),
            is_literal: false,
            whole: false,
        }
    }

    /// The Error of a strict read for `spelling` at byte `offset`, which it
    /// keeps as preserved content, whose message begins with `reason`
    /// (SACM-XMI-013, SACM23-XMI-009; ADR 0034 amendment 2).
    fn spelling(offset: usize, spelling: Spelling, reason: String) -> Self {
        Self {
            offset,
            code: Code::XMI_SPELLING_VARIANT,
            severity: Severity::Error,
            requirement: spelling.row(),
            clause: Some(spelling.clause()),
            reason,
            tail: spelling.tail(Mode::Strict),
            is_literal: false,
            whole: true,
        }
    }

    /// The Warning, for the holder `holder`, with the byte offset it is to
    /// be located at.
    fn into_diagnostic(self, holder: &Holder) -> (usize, Diagnostic) {
        let Self {
            offset,
            code,
            severity,
            requirement,
            clause,
            reason,
            tail,
            is_literal: _,
            whole,
        } = self;
        let message = if whole {
            format!("{reason}{tail}")
        } else {
            format!(
                "{reason}, so it is kept as preserved content of {}, which a tolerant write \
                 writes back and a strict write refuses{tail}",
                holder.subject()
            )
        };
        let mut diagnostic = Diagnostic::create(code, severity, requirement, message)
            .set_affected(holder.affected());
        if let Some(clause) = clause {
            diagnostic = diagnostic.set_clause(clause);
        }
        (offset, diagnostic)
    }
}

/// What holds preserved content: an element, or the document outside every
/// element (ADR 0035 D3).
enum Holder {
    /// The element `id` of `kind`.
    Element { kind: ElementKind, id: ElementId },
    /// The document, on or under the `xmi:XMI` element.
    Document,
}

impl Holder {
    /// How a diagnostic names it.
    fn subject(&self) -> String {
        match self {
            Self::Element { kind, id } => subject(*kind, id.as_str()),
            Self::Document => "the document".to_owned(),
        }
    }

    /// The ids a diagnostic about its preserved content affects: the
    /// element's, or none for the document.
    fn affected(&self) -> Option<ElementId> {
        match self {
            Self::Element { id, .. } => Some(id.clone()),
            Self::Document => None,
        }
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

/// The fault of `unread` at byte `offset` in `mode`: its refusal, as
/// `reason` says, in strict mode and when a tolerant read refuses it too;
/// otherwise the Warning a tolerant read keeps it with ([`Unread::tolerant`],
/// ADR 0035 D1, D2).
fn unread_fault(text: &str, mode: Mode, offset: usize, unread: Unread, reason: &str) -> Fault {
    fault(keep_or_refuse(text, mode, offset, unread, reason))
}

/// What a read in `mode` does with `unread` at byte `offset`: the Warning a
/// tolerant read keeps it with, or, in strict mode and for an entry a
/// tolerant read refuses, its refusal, as `reason` says.
fn keep_or_refuse(
    text: &str,
    mode: Mode,
    offset: usize,
    unread: Unread,
    reason: &str,
) -> Result<Keep, ReadError> {
    match (mode, unread.tolerant()) {
        (Mode::Tolerant, Tolerant::Kept(code)) => Ok(Keep::unread(offset, unread, code, reason)),
        _ => Err(unsupported(text, offset, unread, reason)),
    }
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
        Mode::Strict => Fault::Refused(ReadError::InvalidLiteral {
            element: element.clone(),
            property: property.to_owned(),
            value: value.to_owned(),
            location: locate(text, offset),
        }),
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
) -> Result<(Document, Vec<Diagnostic>), ReadError> {
    let element = tree.root_element();
    let is_wrapper = classify(text, element)?;
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
    };
    // What the document holds outside every element (ADR 0035 D3).
    let mut outside = Kept::new(Holder::Document);
    if is_wrapper {
        read_wrapper(&mut found, element, &mut document, &mut outside)?;
    } else {
        read_root(&mut found, element, &mut document, &mut outside)?;
    }
    document
        .set_document_preserved(outside.into_preserved())
        .map_err(|error| {
            unsupported(
                text,
                element.range().start,
                Unread::Unheld,
                &format!(
                    "the content outside every element cannot be held in the document: {error}"
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
        return Err(ReadError::LegacyShape {
            element,
            shape,
            location: locate(text, offset),
        });
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
    legacy: Option<(usize, ElementId, String)>,
}

/// What [`Findings`] holds before a part is read, so that a part that turns
/// out not to conform can be preserved as if it had not been read.
struct Checkpoint {
    /// The number of diagnostics found.
    diagnostics: usize,
    /// The first C++ shape found in strict mode.
    legacy: Option<(usize, ElementId, String)>,
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

    /// Reports the construct `what` at byte `offset`, preserved in `holder`
    /// (SACM-XMI-003, LIB-COMPAT-003; XMI 2.5.1 §7.3.3): an Error in strict
    /// mode and a Warning in tolerant mode.
    fn unknown(&mut self, offset: usize, what: &str, holder: &Holder) {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        let message = format!(
            "{what} is not SACM 2.3 where it stands, so it is kept as written as preserved \
             content of {}, which a tolerant write writes back and a strict write refuses",
            holder.subject()
        );
        let diagnostic =
            Diagnostic::create(Code::XMI_UNKNOWN_CONTENT, severity, PRESERVED_ROW, message)
                .set_clause("7.3.3")
                .set_affected(holder.affected());
        self.push_at(diagnostic, offset);
    }
}

impl Findings<'_, '_> {
    /// What this read does with `unread` at byte `offset`: the Warning a
    /// tolerant read keeps it with, or its refusal, as `reason` says
    /// ([`keep_or_refuse`]). Every refusal site that a tolerant read may
    /// keep asks this.
    fn refuse_or_keep(
        &self,
        offset: usize,
        unread: Unread,
        reason: &str,
    ) -> Result<Keep, ReadError> {
        keep_or_refuse(self.text, self.mode, offset, unread, reason)
    }

    /// [`refuse_or_keep`](Self::refuse_or_keep) as a [`Fault`].
    fn unread_fault(&self, offset: usize, unread: Unread, reason: &str) -> Fault {
        unread_fault(self.text, self.mode, offset, unread, reason)
    }

    /// Reports `keep`, a construct kept as preserved content of `holder`: one
    /// Warning at the strict refusal's location, affecting the holder (ADR
    /// 0035 D1).
    fn keep(&mut self, keep: Keep, holder: &Holder) {
        let (offset, diagnostic) = keep.into_diagnostic(holder);
        self.push_at(diagnostic, offset);
    }

    /// Reports `spelling`, which a tolerant read read at byte `offset` as
    /// its resolved name: one SACM-XMI-013 Warning affecting `affected`, the
    /// element the spelling belongs to, whose message begins with `reason`
    /// (SACM23-XMI-009; ADR 0034 amendment 2).
    fn spelled(&mut self, offset: usize, spelling: Spelling, reason: &str, affected: &ElementId) {
        let message = format!("{reason}{}", spelling.tail(Mode::Tolerant));
        let diagnostic = Diagnostic::create(
            Code::XMI_SPELLING_VARIANT,
            Severity::Warning,
            spelling.row(),
            message,
        )
        .set_clause(spelling.clause())
        .set_affected([affected.clone()]);
        self.push_at(diagnostic, offset);
    }

    /// Reports the role or type `spelling` of the child `node`, the element
    /// `child` of `parent`, which a tolerant read read, at its start tag, as
    /// [`spelled`](Self::spelled) does: a role belongs to the parent, a type
    /// to the child (SACM23-XMI-009).
    fn spelled_child(
        &mut self,
        node: Node<'_, '_>,
        spelling: Spelling,
        reason: &str,
        parent: &ElementId,
        child: &ElementId,
    ) {
        let affected = if spelling.is_role() { parent } else { child };
        self.spelled(node.range().start, spelling, reason, affected);
    }

    /// Reports the empty `xmi:id` of `node`, which `subject` names, read as
    /// `read_as` says, affecting `affected`: one SACM-ID-002 Warning at the
    /// start tag of `node` in a tolerant read, which reads on (ADR 0035 D1).
    fn empty_id(
        &mut self,
        node: Node<'_, '_>,
        subject: &str,
        read_as: &str,
        affected: Option<ElementId>,
    ) {
        let message = format!(
            "the xmi:id of {subject} is empty, which is no xsd:ID (XMI 2.5.1 §7.6.1), so {read_as}"
        );
        let diagnostic =
            Diagnostic::create(Code::ID_INVALID, Severity::Warning, TOLERANT_ROW, message)
                .set_clause("7.6.1")
                .set_affected(affected);
        self.push_at(diagnostic, node.range().start);
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
    fn generate_id(&mut self, node: Node<'_, '_>, kind: ElementKind) -> ElementId {
        let id = self.next_id(kind);
        let message = format!(
            "the {kind} has no xmi:id, which XMI 2.5.1 §7.6.1 makes optional, so it is read \
             with the generated id {:?}, which write writes",
            id.as_str()
        );
        let diagnostic = Diagnostic::create(
            Code::XMI_MISSING_ID,
            Severity::Info,
            MISSING_ID_ROW,
            message,
        )
        .set_clause("7.6.1")
        .set_affected([id.clone()]);
        self.push_at(diagnostic, node.range().start);
        id
    }

    /// The next generated id of `kind` that no `xmi:id` in the input holds
    /// and no reference names (ADR 0024).
    fn next_id(&mut self, kind: ElementKind) -> ElementId {
        let reserved = &self.reserved;
        self.generator
            .next(kind, |candidate| reserved.contains(candidate))
    }

    /// Reports the `xmi:id` `attribute` of `subject`, read as `value`
    /// without the white space around it, which affects the element
    /// `affected` (SACM-ID-004; XMI 2.5.1 §7.6.1; ADR 0024).
    fn trimmed_id(
        &mut self,
        attribute: &Attribute<'_, '_>,
        value: &str,
        subject: &str,
        affected: &str,
    ) {
        let message = format!(
            "the xmi:id {:?} of {subject} is read as {value:?}: an xmi:id is an xsd:ID, whose \
             white space is collapsed (XMI 2.5.1 §7.6.1), and write writes it trimmed",
            attribute.value()
        );
        let diagnostic =
            Diagnostic::create(Code::ID_TRIMMED, Severity::Info, TRIMMED_ID_ROW, message)
                .set_clause("7.6.1")
                .set_affected([ElementId::new(affected)]);
        self.push(diagnostic, attribute);
    }

    /// Reports the `xsi:type` `attribute` of `subject`, which affects the
    /// element `id` (SACM-XMI-010; XMI 2.5.1 §9.5.2 rule 2g).
    fn xsi_type(&mut self, attribute: &Attribute<'_, '_>, subject: &str, id: &str) {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        let message = format!(
            "{subject} is typed with xsi:type, which XMI 2.5.1 allows only when \
             org.omg.xmi.useSchemaExtensions is true, and SACM 2.3 does not set it; \
             it is read as xmi:type"
        );
        let diagnostic = Diagnostic::create(Code::XMI_XSI_TYPE, severity, MODES_ROW, message)
            .set_clause("9.5.2")
            .set_affected([ElementId::new(id)]);
        self.push(diagnostic, attribute);
    }

    /// Reports the `xmi:version` `attribute` of `subject`, which affects the
    /// root `id`, or nothing on the XMI element (SACM-XMI-011; XMI 2.5.1
    /// §7.5.3).
    fn xmi_version(&mut self, attribute: &Attribute<'_, '_>, subject: &str, id: Option<&str>) {
        let message = format!(
            "the xmi:version {:?} of {subject} is ignored: the XMI namespace identifies the \
             version (XMI 2.5.1 §7.5.3), and write does not write it",
            attribute.value()
        );
        let diagnostic = Diagnostic::create(
            Code::XMI_VERSION_IGNORED,
            Severity::Info,
            MODES_ROW,
            message,
        )
        .set_clause("7.5.3")
        .set_affected(id.map(ElementId::new));
        self.push(diagnostic, attribute);
    }

    /// Reports the C++ shape at byte `offset` that `shape` describes, which
    /// gives a name or a text of the element `owner`: one SACM-XMI-012
    /// Warning, and in strict mode the refusal [`read`] returns if this is
    /// the first shape (LIB-COMPAT-008; XMI 2.5.1 §7.8.6; ADR 0022
    /// amendment 1).
    fn legacy_shape(&mut self, offset: usize, owner: &ElementId, shape: String) {
        let message = format!(
            "{shape}, a shape of the assurance-forge C++ library that XMI 2.5.1 does not define; \
             it is read as its standard form, which write writes"
        );
        let diagnostic = Diagnostic::create(
            Code::XMI_LEGACY_SHAPE,
            Severity::Warning,
            LEGACY_ROW,
            message,
        )
        .set_clause("7.8.6")
        .set_affected([owner.clone()]);
        self.push_at(diagnostic, offset);
        let is_first = self
            .legacy
            .as_ref()
            .is_none_or(|(first, _, _)| offset < *first);
        if self.mode == Mode::Strict && is_first {
            self.legacy = Some((offset, owner.clone(), shape));
        }
    }

    /// Adds `diagnostic`, located at the name of `attribute`.
    fn push(&mut self, diagnostic: Diagnostic, attribute: &Attribute<'_, '_>) {
        self.push_at(diagnostic, attribute.range_qname().start);
    }

    /// Adds `diagnostic`, to be located at byte `offset` of the text.
    fn push_at(&mut self, diagnostic: Diagnostic, offset: usize) {
        self.diagnostics.push((offset, diagnostic));
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

/// How a read diagnostic names the element `id` of `kind`: by its kind and
/// id, never by the name written, which carries a prefix.
fn subject(kind: ElementKind, id: &str) -> String {
    format!("the {kind} {id:?}")
}

/// Pass 1: whether the document element is the XMI element, after checking
/// that every root is named as a package kind.
fn classify(text: &str, element: Node<'_, '_>) -> Result<bool, ReadError> {
    let name = element.tag_name();
    match (name.namespace(), name.name()) {
        (Some(XMI_NS), "XMI") => {
            let mut roots = element
                .children()
                .filter(|child| child.is_element() && child.tag_name().namespace() != Some(XMI_NS))
                .peekable();
            if roots.peek().is_none() {
                return Err(ReadError::NoPackage {
                    location: locate(text, element.range().start),
                });
            }
            match roots.find(|root| package_kind(root).is_none()) {
                Some(root) => Err(invalid_root(text, root)),
                None => Ok(true),
            }
        }
        // XMI document metadata, read by SACM23-XMI-019.
        (Some(XMI_NS), _) => Err(unsupported(
            text,
            element.range().start,
            Unread::XmiMetadata,
            &format!(
                "the XMI element {:?} is not read as a root in this release",
                written_name(text, element)
            ),
        )),
        // An XMI element outside the XMI namespace (ADR 0016 D1), which
        // LIB-COMPAT-002 may read.
        (_, "XMI") => Err(unsupported(
            text,
            element.range().start,
            Unread::OtherXmiNamespace,
            &format!(
                "the element {:?} is not in the XMI namespace {XMI_NS:?}, so it is not read as the XMI element",
                written_name(text, element)
            ),
        )),
        _ => match package_kind(&element) {
            Some(_) => Ok(false),
            None => Err(invalid_root(text, element)),
        },
    }
}

/// The package kind that `node`'s local name names exactly, if any.
fn package_kind(node: &Node<'_, '_>) -> Option<ElementKind> {
    let local = node.tag_name().name();
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.is_package() && kind.name() == local)
}

/// The `InvalidRoot` refusal of `root`.
fn invalid_root(text: &str, root: Node<'_, '_>) -> ReadError {
    ReadError::InvalidRoot {
        location: locate(text, root.range().start),
        element_name: written_name(text, root),
    }
}

/// Pass 2 for the XMI element: checks its attributes, then reads its roots
/// and checks the content beside them, in document order.
fn read_wrapper<'a>(
    found: &mut Findings<'_, 'a>,
    wrapper: Node<'a, '_>,
    document: &mut Document,
    outside: &mut Kept,
) -> Result<(), ReadError> {
    let text = found.text;
    let name = written_name(text, wrapper);
    // The wrapper is no element of the document: what it carries besides
    // `xmi:version` a strict read refuses, and a tolerant read keeps as the
    // document's preserved content (ADR 0033, ADR 0035 D3).
    for attribute in wrapper.attributes() {
        if is_xmi(&attribute, "version") {
            found.xmi_version(&attribute, "the XMI element", None);
            continue;
        }
        let is_metadata = ["id", "type", "uuid", "label", "idref"]
            .iter()
            .any(|local| is_xmi(&attribute, local))
            || is_unqualified(&attribute, "href")
            || is_schema_location(&attribute);
        let unread = if is_metadata {
            Unread::XmiMetadata
        } else {
            Unread::WrapperContent
        };
        let at = attribute.range_qname().start;
        let reason = unread_attribute_reason(text, &attribute, &name);
        let mut keep = found.refuse_or_keep(at, unread, &reason)?;
        if !is_metadata {
            keep.reason = format!(
                "the attribute {:?} of the element {name:?} is not SACM 2.3 where it stands",
                attribute_written(text, &attribute)
            );
        }
        outside.keep_attribute(found, wrapper, &attribute, keep)?;
        // A kept `xmi:id` of the wrapper is in the id space, as any id of the
        // document's preserved content is; an empty one identifies nothing,
        // and is kept as written outside it, with SACM-ID-002 (ADR 0035).
        if is_xmi(&attribute, "id") {
            if attribute.value().is_empty() {
                let read_as = "it is kept as written, outside the id space";
                found.empty_id(wrapper, "the XMI element", read_as, None);
            } else {
                let value = checked_id(found, wrapper, &attribute)?;
                outside.ids.push(ElementId::new(value));
            }
        }
    }
    for child in wrapper.children() {
        if !child.is_element() {
            if let Some(first) = text_offset(text, child) {
                let mut keep = found.refuse_or_keep(
                    first,
                    Unread::WrapperContent,
                    &format!("the element {name:?} holds text, which has no element to be kept in"),
                )?;
                keep.reason =
                    format!("text in the element {name:?} is not SACM 2.3 where it stands");
                outside.keep_text(found, child, keep);
            }
        } else if child.tag_name().namespace() == Some(XMI_NS) {
            // XMI document metadata, read by SACM23-XMI-019.
            let keep = found.refuse_or_keep(
                child.range().start,
                Unread::XmiMetadata,
                &format!(
                    "the XMI element {:?} in {name:?} is not read in this release",
                    written_name(text, child)
                ),
            )?;
            outside.keep_element(found, child, keep)?;
        } else {
            read_root(found, child, document, outside)?;
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
    /// The `name`, `content` and `key` a tolerant read kept as preserved
    /// content for a value in it that is no literal: each counts as given
    /// (ADR 0035, SACM23-VAL-006).
    given: Vec<&'static str>,
    /// The element's name as written in its start tag.
    written: String,
    /// The byte offset of its start tag.
    at: usize,
    /// Its preserved content so far (LIB-COMPAT-003, ADR 0033).
    kept: Kept,
}

/// The preserved content of one element, or of the document outside every
/// element, as it is read (ADR 0033, ADR 0035 D3).
struct Kept {
    /// What holds it.
    holder: Holder,
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

impl Kept {
    /// Nothing yet, held by `holder`.
    fn new(holder: Holder) -> Self {
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
    /// returns the Warning that names it (ADR 0035; a defect).
    fn take_attribute<'a>(
        &mut self,
        found: &Findings<'_, 'a>,
        node: Node<'a, '_>,
        attribute: &Attribute<'a, '_>,
        refuse_at: usize,
    ) -> Result<Option<Keep>, ReadError> {
        let text = found.text;
        let at = attribute.range_qname().start;
        let mut attr = preserve::attr(text, attribute);
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
                    let reason = format!(
                        "the attribute {:?} of the element {:?} is not SACM 2.3, and cannot be \
                         kept: its prefix {prefix:?} is bound to {uri:?}, and the written \
                         element binds it to {pinned:?}",
                        attr.name.written(),
                        written_name(text, node)
                    );
                    let mut keep =
                        found.refuse_or_keep(refuse_at, Unread::UnwritablePrefix, &reason)?;
                    let new = self.rename(node, uri);
                    keep.reason = format!(
                        "the attribute {:?} of the element {:?} is not SACM 2.3 where it stands, \
                         and its prefix {prefix:?} is bound to {uri:?}, which the written element \
                         binds to {pinned:?}, so it is written with the prefix {new:?}",
                        attr.name.written(),
                        written_name(text, node)
                    );
                    attr.name.prefix = Some(new);
                    renamed = Some(keep);
                }
                Some(_) => {}
                None if prefix == "xml" => {}
                None => self.namespaces.push((prefix, uri.to_owned())),
            }
        }
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
            found.keep(renamed, &self.holder);
        } else {
            let what = format!(
                "the attribute {:?}",
                attribute_written(found.text, attribute)
            );
            found.unknown(attribute.range_qname().start, &what, &self.holder);
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
        found.keep(keep, &self.holder);
        Ok(())
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
                        let subject = format!(
                            "the element {:?} kept in {}",
                            written_name(text, element),
                            self.holder.subject()
                        );
                        found.empty_id(
                            element,
                            &subject,
                            "it is kept as written, outside the id space",
                            self.holder.affected(),
                        );
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
        self.offsets.insert(at);
        self.children.push(Placed {
            index: self.typed,
            item: Item::Element(preserve::fragment(text, node)),
        });
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
        let what = format!("the element {:?}", written_name(found.text, node));
        found.unknown(node.range().start, &what, &self.holder);
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
        found.keep(keep, &self.holder);
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

    /// Keeps `node`, a child of the holder, as preserved text if it is text
    /// that is not all XML white space, reported at its first character that
    /// is not literal white space with SACM-XMI-003 (ADR 0033); see
    /// [`take_text`](Self::take_text).
    fn preserve_text(&mut self, found: &mut Findings<'_, '_>, node: Node<'_, '_>) {
        if let Some((first, true)) = self.take_text(found.text, node) {
            found.unknown(first, "text", &self.holder);
        }
    }

    /// Keeps `node`, text, as preserved text, reported as `keep` says when
    /// it is a new piece, in place of a strict refusal (ADR 0035 D1); see
    /// [`take_text`](Self::take_text).
    fn keep_text(&mut self, found: &mut Findings<'_, '_>, node: Node<'_, '_>, keep: Keep) {
        if let Some((_, true)) = self.take_text(found.text, node) {
            found.keep(keep, &self.holder);
        }
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

impl Pending {
    /// How refusals name the element.
    fn what(&self) -> String {
        let element = if self.parent.is_none() {
            "root element"
        } else {
            "element"
        };
        format!("the {element} {:?}", self.written)
    }

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
        self.kept.keep_text(found, node, keep);
        Ok(())
    }

    /// Ends the element once its content is read: a `content` given in the
    /// C++ shape on a `UtilityElement` becomes its content, a
    /// `MultiLangString` of one value, reported at the shape
    /// (LIB-COMPAT-008). A `lang` attribute that gives no content is no
    /// shape: it is preserved (ADR 0033).
    fn finish(&mut self, found: &mut Findings<'_, '_>) {
        let mut shorthand = std::mem::take(&mut self.shorthand);
        let lang = shorthand.lang_attr.take();
        let (value, at) = match shorthand.into_lang_string() {
            Ok(Some(given)) => given,
            Ok(None) => return,
            Err(at) => {
                if let Some(attr) = lang {
                    let what = format!("the attribute {:?}", attr.name.written());
                    self.kept.offsets.insert(at);
                    self.kept.attributes.push((at, attr));
                    found.unknown(at, &what, &self.kept.holder);
                }
                return;
            }
        };
        let shape = format!(
            "the content of {} is given on the element itself",
            subject(self.kind, self.id.as_str())
        );
        found.legacy_shape(at, &self.id, shape);
        let mut held = Text::default();
        held.push(value, None);
        self.content = Some(held);
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
        let (name, name_id) = name?;
        if repeats {
            return Err(self.repeat(found, holder, node.range().start, "name", "name"));
        }
        self.name = Some(name);
        self.name_id = name_id;
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
        let target = read_reference(found, node)?;
        let Some(position) = self.references.iter().position(|(given, _)| *given == end) else {
            self.references.push((end, vec![target]));
            return Ok(());
        };
        let property = end.name();
        // The attribute of the end, or a spelling of it a tolerant read read
        // (SACM23-XMI-009); one kept as preserved content is none.
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
        self.references[position].1.push(target);
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
            "; the attribute that gave it first is kept with it, and the element has no \
             content from them"
                .clone_into(&mut keep.tail);
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
    fn read_key_attribute(&mut self, found: &mut Findings<'_, '_>, attribute: &Attribute<'_, '_>) {
        let shape = format!(
            "the key of {} is given as the attribute \"key\"",
            subject(self.kind, self.id.as_str())
        );
        found.legacy_shape(attribute.range_qname().start, &self.id, shape);
        let mut value = LangString::default();
        value.set_content(Some(attribute.value().to_owned()));
        let mut held = Text::default();
        held.push(value, None);
        self.key = Some(held);
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
        let (spelling, resolved, reason) = if spelled_string(self.kind, attribute.name()).is_some()
        {
            let resolved = StringAttribute::Date.name();
            (
                Spelling::Date,
                resolved,
                date_spelling_reason(&self.written),
            )
        } else if spelled_end(self.kind, attribute.name()).is_some() {
            (
                Spelling::End,
                TEXT_RESOLVED,
                end_spelling_reason(&self.written),
            )
        } else {
            return Ok(false);
        };
        let fault = if found.mode == Mode::Strict {
            Fault::Keep(Keep::spelling(at, spelling, reason))
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
                    found.spelled(at, spelling, &reason, &self.id);
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
                    ReadError::RepeatedValue {
                        element: id.clone(),
                        kind,
                        property: local.to_owned(),
                        locations: starts
                            .into_iter()
                            .filter_map(|start| locator.locate(start))
                            .collect(),
                    }
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
            self.lang_attr = Some(preserve::attr(text, attribute));
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

/// Pass 2 for one root: checks it and its tree in document order, then adds
/// it to `document` as a parentless element, with its tree under it in
/// document order (ADR 0010 D2). A root of a kind this release does not
/// read, a tolerant read keeps whole in `outside`, the document's preserved
/// content (ADR 0035 D3). The tree is walked with a stack, not by recursion,
/// so a deep document cannot exhaust the call stack.
fn read_root<'a>(
    found: &mut Findings<'_, 'a>,
    root: Node<'a, '_>,
    document: &mut Document,
    outside: &mut Kept,
) -> Result<(), ReadError> {
    let text = found.text;
    let Some(first) = root_start(found, root, outside)? else {
        return Ok(());
    };
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
    // (SACM23-XMI-016), white space, comments and processing instructions.
    let mut open = vec![(first, root, root.children())];
    let mut read = Vec::new();
    while let Some((element, holder, content)) = open.last_mut() {
        let Some(node) = content.next() else {
            if let Some((mut element, _, _)) = open.pop() {
                element.finish(found);
                read.push(element);
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
                element.kept.preserve_text(found, node);
            }
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
                Ok(Some(child)) => Ok(Some(child)),
                Ok(None) => Err(Fault::Defect),
                Err(fault) => Err(fault),
            },
            Err(fault) => Err(fault),
        };
        match outcome {
            Ok(None) => {}
            Ok(Some(child)) => {
                element.kept.typed = element.kept.typed.saturating_add(1);
                open.push((child, node, node.children()));
            }
            Err(Fault::Refused(error)) => return Err(error),
            Err(Fault::Defect) => {
                found.rollback(checkpoint);
                element.kept.preserve_element(found, node)?;
            }
            Err(Fault::Keep(keep)) => {
                found.rollback(checkpoint);
                element.kept.keep_element(found, node, keep)?;
            }
        }
    }
    hold(text, document, read)?;
    outside.typed = outside.typed.saturating_add(1);
    Ok(())
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
    if let Some(unread) = interface_end(element.kind).filter(|_| is_value_element(node, INTERFACE))
    {
        let reason = format!(
            "the reference element {:?} in the element {:?} is the {} end {INTERFACE:?}, which \
             is not read in this release",
            written_name(text, node),
            element.written,
            element.kind
        );
        return Err(found.unread_fault(at, unread, &reason));
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
    let (spelling, reason) = if spelled_end(element.kind, local).is_some() {
        (Spelling::End, end_spelling_reason(&element.written))
    } else if spelled_string(element.kind, local).is_some() {
        (Spelling::Date, date_spelling_reason(&element.written))
    } else {
        return Ok(false);
    };
    if found.mode == Mode::Strict {
        return Err(Fault::Keep(Keep::spelling(at, spelling, reason)));
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
    found.spelled(at, spelling, &reason, &element.id);
    Ok(true)
}

/// Adds the elements of one root's tree, read whole, to `document`, each
/// with everything read for it, its preserved content included, in document
/// order, so a parent comes before its children. Everything is checked, so
/// the inserts succeed; a failure would still be a refusal, never a panic.
fn hold(text: &str, document: &mut Document, mut read: Vec<Pending>) -> Result<(), ReadError> {
    read.sort_by_key(|element| element.order);
    for element in read {
        let what = element.what();
        let id = element.id;
        let held = document
            .insert_node(
                element.parent.as_ref(),
                element.kind,
                id.clone(),
                element.name,
            )
            .map_err(|error| error.to_string());
        let declared = held.and_then(|()| match element.declaration {
            Some(declaration) => document
                .set_declaration(&id, declaration)
                .map_err(|error| error.to_string()),
            None => Ok(()),
        });
        let named = declared.and_then(|()| match element.name_id {
            Some(name_id) => document.set_name_id(&id, name_id),
            None => Ok(()),
        });
        let identified = named.and_then(|()| match element.gid {
            Some(gid) => document.set_gid(&id, Some(gid)),
            None => Ok(()),
        });
        let valued = identified.and_then(|()| {
            element
                .strings
                .into_iter()
                .filter_map(|(attribute, value)| value.map(|value| (attribute, value)))
                .try_for_each(|(attribute, value)| document.set_string(&id, attribute, Some(value)))
        });
        let texts = [
            (TextRole::Content, element.content),
            (TextRole::Key, element.key),
            (TextRole::Location, element.location),
        ];
        let held = valued.and_then(|()| {
            texts.into_iter().try_for_each(|(role, held)| match held {
                Some(held) => document.set_text(&id, role, held),
                None => Ok(()),
            })
        });
        let flagged = held.and_then(|()| {
            element
                .flags
                .into_iter()
                .try_for_each(|(flag, value)| document.set_flag(&id, flag, value))
        });
        let referenced = flagged.and_then(|()| {
            element
                .references
                .into_iter()
                .try_for_each(|(end, targets)| document.set_references(&id, end, targets))
        });
        let held =
            referenced.and_then(|()| document.set_preserved(&id, element.kept.into_preserved()));
        held.map_err(|error| {
            unsupported(
                text,
                element.at,
                Unread::Unheld,
                &format!("{what} cannot be held in the document: {error}"),
            )
        })?;
    }
    Ok(())
}

/// The reference end `interface` of a package (ptc/22-03-13
/// `AssuranceCasePackage::interface`, `ArgumentPackage::interface`,
/// `ArtifactPackage::interface`), which conforms and is read with the package
/// interfaces.
const INTERFACE: &str = "interface";

/// The entry of the `interface` end of an element of `kind`, if its class
/// has one this release reads elements of: an AssuranceCasePackage's names
/// an `AssuranceCasePackageInterface` (SACM23-AC-002), an ArgumentPackage's
/// an `ArgumentPackageInterface` (SACM23-ARG-014), an ArtifactPackage's an
/// `ArtifactPackageInterface` (SACM23-ART-012), and a TerminologyPackage's a
/// `TerminologyPackageInterface` (SACM23-TERM-007). It conforms, so it is
/// refused, not preserved (ADR 0033, ADR 0034 and its amendment 3).
const fn interface_end(kind: ElementKind) -> Option<Unread> {
    match kind {
        ElementKind::AssuranceCasePackage => Some(Unread::AssuranceCasePackageInterface),
        ElementKind::ArgumentPackage => Some(Unread::ArgumentPackageInterface),
        ElementKind::ArtifactPackage => Some(Unread::ArtifactPackageInterface),
        ElementKind::TerminologyPackage => Some(Unread::TerminologyPackageInterface),
        _ => None,
    }
}

/// The entry of `attribute` of an element of `kind` if it is the
/// unqualified `interface` end of its class ([`interface_end`]).
fn interface_attribute(kind: ElementKind, attribute: &Attribute<'_, '_>) -> Option<Unread> {
    interface_end(kind).filter(|_| is_unqualified(attribute, INTERFACE))
}

/// The entry of `attribute` of an object (a root, a child, a `name`, a
/// `content`, a `key` or a `value`) if it is XMI metadata this version
/// does not read yet (SACM23-XMI-019): `xmi:uuid`, `xmi:label`,
/// `xmi:idref`, `href`, and `xsi:schemaLocation` or
/// `xsi:noNamespaceSchemaLocation`. They conform, so they are refused, not
/// preserved (ADR 0033). A role element's `href` is its reference form
/// instead ([`role_metadata`]).
fn object_metadata(attribute: &Attribute<'_, '_>) -> Option<Unread> {
    let is_metadata = ["uuid", "label", "idref"]
        .iter()
        .any(|local| is_xmi(attribute, local))
        || is_unqualified(attribute, "href")
        || is_schema_location(attribute);
    is_metadata.then_some(Unread::XmiMetadata)
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
    parent: Option<ElementId>,
    kind: ElementKind,
    id: ElementId,
    written: String,
) -> Pending {
    let kept = Kept::new(Holder::Element {
        kind,
        id: id.clone(),
    });
    Pending {
        order: found.next_order(),
        parent,
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
        given: Vec::new(),
        written,
        at: node.range().start,
        kept,
    }
}

/// Checks the start tag of `root`: its namespace, kind, id, type and other
/// attributes, in that order. `None` if a tolerant read keeps the root whole
/// in `outside`, the document's preserved content, as a root of a kind this
/// release does not read (ADR 0035 D3).
fn root_start<'a>(
    found: &mut Findings<'_, 'a>,
    root: Node<'a, '_>,
    outside: &mut Kept,
) -> Result<Option<Pending>, ReadError> {
    let text = found.text;
    let at_root = root.range().start;
    let name = written_name(text, root);

    // The namespace, then the kind (ADR 0016 D5, D6). Under `xmlns=""` the
    // parser reports the empty string, which is no namespace name
    // (Namespaces 1.0 §2.2, §6.2). Every package kind but the three read
    // has its entry; pass 1 has refused a root of any other name. A root
    // outside SACM 2.3 is refused in both modes (ADR 0035 D1).
    let namespace = root.tag_name().namespace().filter(|uri| !uri.is_empty());
    let kind = match (namespace, package_kind(&root)) {
        (Some(SACM_NS), Some(kind)) => match Unread::kind(kind) {
            None => kind,
            Some(unread) => {
                let reason = format!("the root element {name:?} is not read in this release");
                let keep = found.refuse_or_keep(at_root, unread, &reason)?;
                outside.keep_element(found, root, keep)?;
                return Ok(None);
            }
        },
        (Some(SACM_NS), None) => return Err(invalid_root(text, root)),
        (Some(namespace), _) => {
            return Err(unsupported(
                text,
                at_root,
                Unread::OtherSacmNamespace,
                &format!(
                    "the root element {name:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace {SACM_NS:?}"
                ),
            ));
        }
        (None, _) => {
            return Err(unsupported(
                text,
                at_root,
                Unread::OtherSacmNamespace,
                &format!(
                    "the root element {name:?} is in no namespace, not the SACM 2.3 namespace {SACM_NS:?}"
                ),
            ));
        }
    };

    let id = read_id(found, root, kind)?;
    let mut element = pending(found, root, None, kind, id, name);
    let id = element.id.clone();
    let id_text = id.as_str();

    // A type naming the root's own class is read, an `xsi:type` reported;
    // one naming another class does not conform, and is preserved (ADR
    // 0033, D-ROOT-NAME). Like the kind and the id, it is checked first, at
    // the root.
    for attribute in root.attributes().filter(is_type) {
        if names_class(root, attribute.value(), kind.name()) {
            if is_xsi_type(&attribute) {
                found.xsi_type(&attribute, &subject(kind, id_text), id_text);
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
    // XMI metadata is not read yet: a strict read refuses it, and a tolerant
    // read keeps it (ADR 0035 D2); anything else does not conform, and is
    // preserved (ADR 0033).
    let mut start = StartTag::default();
    for attribute in root.attributes() {
        if is_type(&attribute) || is_xmi(&attribute, "id") {
            continue;
        }
        if is_xmi(&attribute, "version") {
            found.xmi_version(&attribute, &subject(kind, id_text), Some(id_text));
            continue;
        }
        if is_unqualified(&attribute, "name") {
            element.name = Some(read_name_attribute(found, &attribute, kind, &id));
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
                let unread =
                    object_metadata(&attribute).or_else(|| interface_attribute(kind, &attribute));
                if let Some(unread) = unread {
                    let at = attribute.range_qname().start;
                    let reason = unread_attribute_reason(text, &attribute, &element.written);
                    let keep = found.refuse_or_keep(at, unread, &reason)?;
                    element.kept.keep_attribute(found, root, &attribute, keep)?;
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
    Ok(Some(element))
}

/// Checks the start tag of `node`, a child element of `parent`: its role,
/// type and kind, its id, and its other attributes, in that order, reading
/// an Assertion's `assertionDeclaration` and the flags and references.
/// `None` if the child does not conform as a whole (its role, namespace or
/// type), so the caller preserves it whole (ADR 0033). [`Fault::Keep`] if a
/// tolerant read keeps it whole where a strict read refuses it: a child
/// [`child_kind`] keeps, and a role element in reference form, with an
/// `xmi:idref` or an `href` (ADR 0035 D1). An attribute a strict read
/// refuses, a tolerant read keeps as preserved content of the child. A role
/// or type spelling a tolerant read reads (SACM23-XMI-009) is one
/// SACM-XMI-013 Warning at the start tag, affecting the parent for the role
/// and the child for the type.
fn child_start<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    parent: &Pending,
) -> Result<Option<Pending>, Fault> {
    let text = found.text;
    let ChildKind::Read { kind, spelled } = child_kind(found, node, parent.kind, &parent.what())?
    else {
        return Ok(None);
    };
    let written = written_name(text, node);
    let id = read_id(found, node, kind)?;
    let mut element = pending(found, node, Some(parent.id.clone()), kind, id, written);
    let id = element.id.clone();
    if let Some((spelling, reason)) = spelled {
        found.spelled_child(node, spelling, &reason, &parent.id, &id);
    }
    // Besides `xmi:id` and the type (ADR 0016 D4), only an unqualified
    // `gid`, an Assertion's unqualified `assertionDeclaration`, the
    // SACMElement flags and references (ADR 0026), the unqualified
    // string-valued attributes of the kind (`version`, `date`, `startTime`,
    // `endTime`; D-DATE), and the C++ shapes
    // (LIB-COMPAT-008): `name` on a ModelElement, and `lang`, `content` and,
    // on a TaggedValue, `key` and `value` on a UtilityElement, in document
    // order, so a refusal after them lands on the later attribute. XMI
    // metadata is not read yet; anything else does not conform, and is
    // preserved (ADR 0033).
    let is_assertion = kind.is_a(AbstractClass::Assertion);
    let is_utility = kind.is_a(AbstractClass::UtilityElement);
    let is_tagged_value = kind == ElementKind::TaggedValue;
    let mut start = StartTag::default();
    for attribute in node.attributes() {
        if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject(kind, id.as_str()), id.as_str());
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
        if !is_utility && is_unqualified(&attribute, "name") {
            element.name = Some(read_name_attribute(found, &attribute, kind, &id));
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
            element.read_key_attribute(found, &attribute);
            continue;
        }
        let unread = role_metadata(&attribute).or_else(|| interface_attribute(kind, &attribute));
        if let Some(unread) = unread {
            let reason = unread_attribute_reason(text, &attribute, &element.written);
            let fault = found.unread_fault(at, unread, &reason);
            // In reference form, the role element is no object but a
            // reference to one: it is kept whole (ADR 0035 D1).
            if is_reference_form(&attribute) {
                return Err(fault);
            }
            element.keep_attribute(found, node, &attribute, fault)?;
            continue;
        }
        element
            .kept
            .preserve_attribute(found, node, &attribute, at)?;
    }
    element.flags = start.flags;
    element.references = start.references;
    Ok(Some(element))
}

/// Whether `attribute` of a role element makes it the reference form of a
/// composite (XMI 2.5.1 §7.10.1): an `xmi:idref`, or an unqualified `href`.
fn is_reference_form(attribute: &Attribute<'_, '_>) -> bool {
    is_xmi(attribute, "idref") || is_unqualified(attribute, "href")
}

/// The entry of `attribute` of a role element of a composite, typed or
/// under a concrete role, if this version does not read it yet: an
/// unqualified `href` makes the element the reference form of a composite
/// held in another document, a proxy (XMI 2.5.1 §7.10.1, §7.10.2), which
/// SACM23-XMI-027 reads; anything else as on any object
/// ([`object_metadata`]), so an `xmi:idref` names SACM23-XMI-019. A
/// qualified `xlink:href` or `xmi:href` is not `href` (XMI.xsd
/// `LinkAttribs`).
fn role_metadata(attribute: &Attribute<'_, '_>) -> Option<Unread> {
    if is_unqualified(attribute, "href") {
        Some(Unread::CrossDocumentHref)
    } else {
        object_metadata(attribute)
    }
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
) -> LangString {
    let shape = format!(
        "the name of {} is given as the attribute \"name\"",
        subject(kind, id.as_str())
    );
    found.legacy_shape(attribute.range_qname().start, id, shape);
    let mut name = LangString::default();
    name.set_content(Some(attribute.value().to_owned()));
    name
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
        return Ok(found.generate_id(node, kind));
    };
    if attribute.value().is_empty() && found.mode == Mode::Tolerant {
        // Read as if it had no `xmi:id`, with SACM-ID-002 in place of
        // SACM-XMI-004: the empty value identifies nothing, and is not kept
        // (ADR 0035 D1).
        let id = found.next_id(kind);
        let read_as = format!(
            "it is read with the generated id {:?}, which write writes",
            id.as_str()
        );
        found.empty_id(node, &format!("the {kind}"), &read_as, Some(id.clone()));
        return Ok(id);
    }
    let value = checked_id(found, node, &attribute)?;
    if value != attribute.value() {
        found.trimmed_id(&attribute, value, &subject(kind, value), value);
    }
    Ok(ElementId::new(value))
}

/// The `xmi:id` of `node`, a `name`, `content`, `key`, `location` or `value` element
/// that `subject` names, of the element `owner`, if it has one
/// (SACM23-XMI-022; XMI 2.5.1 §7.8.3; ADR 0024, ADR 0025). It is in the
/// elements' id space, under [`read_id`]'s rules, except that none is
/// generated. A trimmed one is reported on the owner.
fn read_owned_id<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    subject: &str,
    owner: &ElementId,
) -> Result<Option<ElementId>, ReadError> {
    let Some(attribute) = node.attributes().find(|attribute| is_xmi(attribute, "id")) else {
        return Ok(None);
    };
    if attribute.value().is_empty() && found.mode == Mode::Tolerant {
        // Read without an id (ADR 0035 D1).
        found.empty_id(node, subject, "it is read without one", Some(owner.clone()));
        return Ok(None);
    }
    let value = checked_id(found, node, &attribute)?;
    if value != attribute.value() {
        found.trimmed_id(&attribute, value, subject, owner.as_str());
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
        return Err(ReadError::InvalidId {
            id: String::new(),
            location: locate(found.text, node.range().start),
        });
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
    ReadError::DuplicateId {
        id: ElementId::new(id),
        locations,
    }
}

/// What [`child_kind`] makes of a child element.
enum ChildKind {
    /// It is read, as an element of `kind`, and `spelled` is the spelling a
    /// tolerant read read in its role or type, with the reason its
    /// SACM-XMI-013 Warning begins with (SACM23-XMI-009).
    Read {
        kind: ElementKind,
        spelled: Option<(Spelling, String)>,
    },
    /// It does not conform as a whole, by its namespace, role or type: the
    /// caller keeps it as preserved content (ADR 0033).
    Preserve,
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
fn child_kind(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    parent: ElementKind,
    parent_what: &str,
) -> Result<ChildKind, Fault> {
    let text = found.text;
    let written = written_name(text, node);
    let at = node.range().start;
    match node.tag_name().namespace().filter(|uri| !uri.is_empty()) {
        Some(XMI_NS) => {
            let reason =
                format!("the XMI element {written:?} in {parent_what} is not read in this release");
            return Err(found.unread_fault(at, Unread::XmiMetadata, &reason));
        }
        Some(_) => return Ok(ChildKind::Preserve),
        None => {}
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
        let reason = format!(
            "the element {written:?} in {parent_what} is spelled as in the text of §{}, and the \
             role is {}",
            spelling.clause(),
            spelling.resolved()
        );
        spelled = Some(child_spelling(found, at, spelling, reason)?);
        role
    };
    let mut typed = None;
    let mut type_spelled = false;
    for attribute in node.attributes().filter(is_type) {
        let kind = match type_kind(node, attribute.value()) {
            Some(kind) => kind,
            // The model's spelling of ArtifactAssetRelationship, read as that
            // kind (SACM23-XMI-009, ADR 0034 amendment 2). Under a role that
            // does not admit the kind, the check below preserves the child,
            // as for any type the role does not admit, and it is no
            // spelling.
            None if names_class(node, attribute.value(), MODEL_SPELLING) => {
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
    let kind = match (typed, role.declared) {
        (Some(kind), _) | (None, DeclaredType::Kind(kind)) => kind,
        // Named after its role, so no class can be inferred (LIB-COMPAT-010),
        // unless it is the reference form of a composite, a proxy, which
        // names its object rather than leaving its class out (XMI 2.5.1
        // §7.10.1): not read yet.
        (None, DeclaredType::Abstract(class)) => {
            if let Some(unread) = proxy_link(node) {
                let reason = format!(
                    "the element {written:?} in {parent_what} is in reference form, a proxy of an \
                     element of the class {class}, which is not read in this release"
                );
                return Err(found.unread_fault(at, unread, &reason));
            }
            return Err(match found.mode {
                Mode::Strict => Fault::Refused(ReadError::MissingType {
                    role: role.name.to_owned(),
                    declared_type: class.name().to_owned(),
                    location: locate(text, at),
                }),
                Mode::Tolerant => Fault::Keep(Keep {
                    offset: at,
                    code: Code::XMI_MISSING_TYPE,
                    severity: Severity::Warning,
                    requirement: TOLERANT_ROW,
                    clause: Some("7.6.3"),
                    reason: format!(
                        "the element {written:?} in {parent_what} has no type, and its role {} \
                         has the abstract type {class}, so its class is not known (XMI 2.5.1 \
                         §7.6.3)",
                        role.name
                    ),
                    tail: String::new(),
                    is_literal: false,
                    whole: false,
                }),
            });
        }
    };
    if !role.declared.admits(kind) {
        return Ok(ChildKind::Preserve);
    }
    // The readable children: every kind a role of a readable class admits,
    // but the kinds this release does not read.
    if let Some(unread) = Unread::kind(kind) {
        let reason = format!("the {kind} {written:?} in {parent_what} is not read in this release");
        return Err(found.unread_fault(at, unread, &reason));
    }
    if type_spelled {
        let reason = format!(
            "the element {written:?} in {parent_what} is typed {MODEL_SPELLING}, the \
             machine-readable model's spelling of {}",
            Spelling::Type.resolved()
        );
        spelled = Some(child_spelling(found, at, Spelling::Type, reason)?);
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

/// What a read does with a child whose role or type is `spelling`, at byte
/// `at`, whose SACM-XMI-013 message begins with `reason` (SACM23-XMI-009): a
/// tolerant read reads it, and returns the two for the caller to report once
/// the child's id is known; a strict read keeps the child whole with the
/// Error ([`Fault::Keep`]).
fn child_spelling(
    found: &Findings<'_, '_>,
    at: usize,
    spelling: Spelling,
    reason: String,
) -> Result<(Spelling, String), Fault> {
    match found.mode {
        Mode::Strict => Err(Fault::Keep(Keep::spelling(at, spelling, reason))),
        Mode::Tolerant => Ok((spelling, reason)),
    }
}

/// The entry of the role element `node` if it is in the reference form of a
/// composite (XMI 2.5.1 §7.10.1): an `xmi:idref` (SACM23-XMI-019), else an
/// unqualified `href` (SACM23-XMI-027) (LIB-COMPAT-010).
fn proxy_link(node: Node<'_, '_>) -> Option<Unread> {
    let attributes = || node.attributes();
    if attributes().any(|attribute| is_xmi(&attribute, "idref")) {
        Some(Unread::XmiMetadata)
    } else if attributes().any(|attribute| is_unqualified(&attribute, "href")) {
        Some(Unread::CrossDocumentHref)
    } else {
        None
    }
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
/// content. The identity attributes `xmi:id`, `xmi:type` (or `xsi:type`),
/// `xmi:uuid` and `xmi:label` are XMI metadata not read yet
/// (SACM23-XMI-019), and an `XLink` attribute or an `href` that is not `#id`
/// a cross-document link not read yet (SACM23-XMI-027): each is refused at
/// the attribute. Anything else that does not conform is
/// [`Fault::Defect`]: a second link attribute, any other attribute, an
/// `xmi:idref` that is empty or holds white space, no link, a child
/// element, or text. The caller then keeps the whole element as preserved
/// content (ADR 0033). A tolerant read keeps the whole element, with the
/// Warning of a refusal's entry (ADR 0035 D1).
fn read_reference(found: &Findings<'_, '_>, node: Node<'_, '_>) -> Result<ElementId, Fault> {
    let text = found.text;
    let element = written_name(text, node);
    let mut link = None;
    for attribute in node.attributes() {
        let is_idref = is_xmi(&attribute, "idref");
        let at = attribute.range_qname().start;
        if !(is_idref || is_unqualified(&attribute, "href")) {
            let is_identity = ["id", "type", "uuid", "label"]
                .iter()
                .any(|local| is_xmi(&attribute, local))
                || is_xsi_type(&attribute);
            let unread = if is_identity {
                Unread::XmiMetadata
            } else if attribute.namespace() == Some(XLINK_NS) {
                Unread::CrossDocumentHref
            } else {
                return Err(Fault::Defect);
            };
            let reason = unread_attribute_reason(text, &attribute, &element);
            return Err(found.unread_fault(at, unread, &reason));
        }
        if link.is_some() {
            return Err(Fault::Defect);
        }
        let value = attribute.value().trim_matches(is_xml_space);
        let target = if is_idref {
            value
        } else {
            let Some(fragment) = same_document_fragment(value) else {
                let reason = format!(
                    "the href {:?} of the reference element {element:?} names no element of this \
                     document by its id; only href=\"#id\" is read in this release",
                    attribute.value()
                );
                return Err(found.unread_fault(at, Unread::CrossDocumentHref, &reason));
            };
            fragment
        };
        // An `xsd:IDREF` is one token: white space inside it would be read
        // as one id here but written as an IDREFS attribute of several.
        if target.is_empty() || target.contains(is_xml_space) {
            return Err(Fault::Defect);
        }
        link = Some(ElementId::new(target));
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

/// The id that the trimmed `href` value `value` names in the same document,
/// if it is `#id`: a bare fragment, not empty and not a pointer (XMI 2.5.1
/// rule 2n). `None` for a value with a document part, such as `other.xmi#c`
/// or `c`, the empty value, `#` and a fragment holding `#`, `(`, `)` or white
/// space, such as `#xpointer(id('c'))`. The fragment is held exactly, with no
/// percent-decoding.
fn same_document_fragment(value: &str) -> Option<&str> {
    let fragment = value.strip_prefix('#')?;
    let is_bare = !fragment.is_empty()
        && !fragment.contains(|c: char| matches!(c, '#' | '(' | ')') || is_xml_space(c));
    is_bare.then_some(fragment)
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
    ReadError::RepeatedValue {
        element: owner.clone(),
        kind,
        property: property.to_owned(),
        locations: offsets
            .into_iter()
            .filter_map(|offset| locator.locate(offset))
            .collect(),
    }
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
    kept: &Kept,
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
    kept: &Kept,
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
) -> Result<(LangString, Option<ElementId>), Fault> {
    let subject = format!("the name of {}", subject(kind, owner.as_str()));
    read_lang_string(found, node, &subject, owner, kind, "name")
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
/// or an XMI element in it, is refused (SACM23-XMI-019). Anything else,
/// such as either shorthand beside a `value` or a `lang` alone, does not
/// conform: [`Fault::Defect`] (ADR 0033).
fn read_text<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    role: TextRole,
    kind: ElementKind,
    owner: &ElementId,
) -> Result<Text, Fault> {
    let text = found.text;
    let element = written_name(text, node);
    if node.attributes().any(|attribute| {
        is_type(&attribute) && !names_class(node, attribute.value(), "MultiLangString")
    }) {
        return Err(Fault::Defect);
    }
    let subject = format!("the {} of {}", role.name(), subject(kind, owner.as_str()));
    let mut held = Text::default();
    held.set_id(read_owned_id(found, node, &subject, owner)?);
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
            found.xsi_type(&attribute, &subject, owner.as_str());
        } else if let Some(unread) = object_metadata(&attribute) {
            let reason = unread_attribute_reason(text, &attribute, &element);
            return Err(found.unread_fault(attribute.range_qname().start, unread, &reason));
        } else if !(is_type(&attribute) || is_xmi(&attribute, "id")) {
            return Err(Fault::Defect);
        }
    }
    let value_subject = format!("a value of {subject}");
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
            let (value, id) =
                read_lang_string(found, child, &value_subject, owner, kind, &property)?;
            held.push(value, id);
        } else {
            return Err(xmi_element_in(found, child, &element));
        }
    }
    match shorthand.into_lang_string() {
        Ok(Some((value, at))) => {
            let shape = format!("{subject} is given in the single-language shorthand");
            found.legacy_shape(at, owner, shape);
            held.push(value, None);
        }
        Ok(None) => {}
        Err(_) => return Err(Fault::Defect),
    }
    Ok(held)
}

/// What an element `child` that `element`, a `name`, `content`, `key`, `location` or
/// `value`, does not read makes it: refused if it is in the XMI namespace,
/// which is XMI metadata not read yet (SACM23-XMI-019), and otherwise
/// [`Fault::Defect`] (ADR 0033).
fn xmi_element_in(found: &Findings<'_, '_>, child: Node<'_, '_>, element: &str) -> Fault {
    if child.tag_name().namespace() != Some(XMI_NS) {
        return Fault::Defect;
    }
    let reason = format!(
        "the XMI element {:?} in {element:?} is not read in this release",
        written_name(found.text, child)
    );
    found.unread_fault(child.range().start, Unread::XmiMetadata, &reason)
}

/// The `LangString` that `node`, a `name` or a `value` of a `content` or
/// `key`, holds (SACM23-XMI-011, SACM23-XMI-012, SACM23-XMI-014; ADR 0021,
/// ADR 0025). `subject` names it in diagnostics, and `property` in a
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
/// XMI metadata on it, an XMI element in it, and an `ExpressionLangString`
/// type are conforming and not read yet, so refused. Anything else does
/// not conform, such as a second `value` or `content` carrying `lang` in a
/// `name`, which holds one `LangString`: [`Fault::Defect`] (ADR 0033).
fn read_lang_string<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    subject: &str,
    owner: &ElementId,
    kind: ElementKind,
    property: &str,
) -> Result<(LangString, Option<ElementId>), Fault> {
    let text = found.text;
    let mode = found.mode;
    let element = written_name(text, node);
    let is_name = property == "name";
    check_lang_string_tag(found, node, &element)?;
    let name_id = read_owned_id(found, node, subject, owner)?;
    let repeated = |field: &str| repeated_field(text, mode, node, field, owner, kind, property);
    let mut fields = Fields::default();
    for attribute in node.attributes() {
        let value = || Some(attribute.value().to_owned());
        let given = match (attribute.namespace(), attribute.name()) {
            (None, "lang") => fields.lang(value()),
            (None, "content") => fields.content(value()),
            _ if is_xsi_type(&attribute) => {
                found.xsi_type(&attribute, subject, owner.as_str());
                Ok(())
            }
            _ if is_type(&attribute) || is_xmi(&attribute, "id") => Ok(()),
            _ => {
                return Err(match object_metadata(&attribute) {
                    Some(unread) => {
                        let reason = unread_attribute_reason(text, &attribute, &element);
                        found.unread_fault(attribute.range_qname().start, unread, &reason)
                    }
                    None => Fault::Defect,
                });
            }
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
                    let shape = format!("{subject} has its content as text");
                    found.legacy_shape(first, owner, shape);
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
                let shape = format!("{subject} holds a content carrying lang");
                found.legacy_shape(at, owner, shape);
            }
            fields.content(value).map_err(repeated)?;
        } else if is_name && is_value_element(child, "value") {
            // The name's one LangString: the name holds nothing else.
            if holds_beside(text, node, child) {
                return Err(Fault::Defect);
            }
            let value = read_name_value(found, child, subject, owner, kind)?;
            has_entry = true;
            let shape = format!("{subject} holds a value");
            found.legacy_shape(child.range().start, owner, shape);
            if let Some(lang) = value.lang() {
                fields.lang(Some(lang.to_owned())).map_err(repeated)?;
            }
            if let Some(content) = value.content() {
                fields.content(Some(content.to_owned())).map_err(repeated)?;
            }
        } else {
            return Err(xmi_element_in(found, child, &element));
        }
    }
    let mut name = fields.value;
    if text_at.is_some() {
        name.set_content(Some(characters));
    }
    Ok((name, name_id))
}

/// Checks the start tag of `node`, a `name` or `value` written as
/// `element`: an `ExpressionLangString` type is not read yet
/// (SACM23-BASE-009), and refused at the start tag; a namespace or a type
/// naming another class than `LangString` does not conform:
/// [`Fault::Defect`] (ADR 0033).
fn check_lang_string_tag(
    found: &Findings<'_, '_>,
    node: Node<'_, '_>,
    element: &str,
) -> Result<(), Fault> {
    if node
        .tag_name()
        .namespace()
        .is_some_and(|uri| !uri.is_empty())
    {
        return Err(Fault::Defect);
    }
    for attribute in node.attributes().filter(is_type) {
        match sacm_class(node, attribute.value()) {
            Some("LangString") => {}
            Some("ExpressionLangString") => {
                let reason = format!(
                    "the element {element:?} has the type {:?}, which is not read in this release",
                    attribute.value()
                );
                return Err(found.unread_fault(
                    node.range().start,
                    Unread::ExpressionLangString,
                    &reason,
                ));
            }
            _ => return Err(Fault::Defect),
        }
    }
    Ok(())
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

/// The `LangString` that `node`, a `value` in a `name` that `subject`
/// names, holds: the C++ shape of a name's one `LangString`
/// (LIB-COMPAT-008), read as a `value` of a `content` is. An `xmi:id` on it
/// is XMI metadata on a value that this version does not read, since a
/// name has one id, on the `name` (SACM23-XMI-019): refused at it.
fn read_name_value<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    subject: &str,
    owner: &ElementId,
    kind: ElementKind,
) -> Result<LangString, Fault> {
    let text = found.text;
    if let Some(attribute) = node.attributes().find(|attribute| is_xmi(attribute, "id")) {
        let element = written_name(text, node);
        let reason = unread_attribute_reason(text, &attribute, &element);
        let at = attribute.range_qname().start;
        return Err(found.unread_fault(at, Unread::XmiMetadata, &reason));
    }
    let subject = format!("the value in {subject}");
    read_lang_string(found, node, &subject, owner, kind, "name.value").map(|(value, _)| value)
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
/// the SACM class `class`. The value's leading and trailing white space is
/// removed first, as a `QName`'s is collapsed (XML Schema 1.0 Part 2 §4.3.6);
/// white space inside it stays, so it names no class.
fn names_class(node: Node<'_, '_>, value: &str, class: &str) -> bool {
    sacm_class(node, value) == Some(class)
}

/// The concrete SACM class that the type `value` names, resolved as by
/// [`names_class`].
fn type_kind(node: Node<'_, '_>, value: &str) -> Option<ElementKind> {
    let class = sacm_class(node, value)?;
    ElementKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.name() == class)
}

/// The local part of the type `value`, a `QName` resolved in the scope of
/// `node` without its leading and trailing white space, if it is in the
/// SACM namespace.
fn sacm_class<'v>(node: Node<'_, '_>, value: &'v str) -> Option<&'v str> {
    let value = value.trim_matches(is_xml_space);
    let (prefix, local) = match value.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, value),
    };
    (node.lookup_namespace_uri(prefix) == Some(SACM_NS)).then_some(local)
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

/// The reason, the start of its SACM-XMI-013 message, of the §11.2 text
/// spelling `argumentationElement` on the `ArgumentGroup` written as `group`,
/// whose end is `argumentElement` (SACM23-XMI-009).
fn end_spelling_reason(group: &str) -> String {
    format!(
        "{TEXT_SPELLING:?} in the element {group:?} is spelled as in the text of §11.2, and the end \
         of an ArgumentGroup is {TEXT_RESOLVED}"
    )
}

/// The reason, the start of its SACM-XMI-013 message, of the model's
/// spelling `occurece` on the `Event` written as `written`, whose attribute
/// the text spells `date` (§12.9; SACM23-XMI-009).
fn date_spelling_reason(written: &str) -> String {
    format!(
        "{EVENT_SPELLING:?} in the element {written:?} is the machine-readable model's spelling, \
         and the attribute of an Event is date (§12.9)"
    )
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

/// The reason of the refusal of `attribute` of the element written as
/// `element`, which this release does not read.
fn unread_attribute_reason(text: &str, attribute: &Attribute<'_, '_>, element: &str) -> String {
    format!(
        "the attribute {:?} of the element {element:?} is not read in this release",
        text.get(attribute.range_qname())
            .unwrap_or(attribute.name())
    )
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

    const OPEN: &str = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n"
    );

    /// The strict refusal of `content` in the root `p`, on line 2.
    fn refusal(content: &str) -> Option<(Option<SourceLocation>, String)> {
        let input = format!("{OPEN}{content}</sacm:ArgumentPackage>");
        match read(input.as_bytes(), &strict()) {
            Err(ReadError::Unsupported { location, reason }) => Some((location, reason)),
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
    fn only_a_bare_fragment_is_a_same_document_href() {
        for (value, expected) in [
            ("#c2", Some("c2")),
            ("#1c", Some("1c")),
            ("#a%20b", Some("a%20b")),
            ("", None),
            ("#", None),
            ("c2", None),
            ("other.xmi#c2", None),
            ("#xpointer(id('c2'))", None),
            ("#a#b", None),
            ("#a b", None),
            ("#a\tb", None),
            ("#a\nb", None),
            ("#a\rb", None),
        ] {
            assert_eq!(super::same_document_fragment(value), expected, "{value:?}");
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
        let cases = [
            (
                "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"c\"/>",
                "the ArgumentPackageInterface \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is not read in this release (a limit of this version; SACM23-ARG-014 reads it)",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"><xmi:Extension/></argumentElement>",
                "the XMI element \"xmi:Extension\" in the element \"argumentElement\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)",
            ),
        ];
        for (content, reason) in cases {
            assert_eq!(
                refusal(content).map(|(_, reason)| reason).as_deref(),
                Some(reason),
                "{content:?}"
            );
        }
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
             <name xmi:type=\"sacm:ExpressionLangString\" content=\"A\"/><name content=\"B\"/>\
             </argumentElement>",
        );
        let (found, document) = kept(&input);
        let codes: Vec<&str> = found.iter().map(|(code, _, _)| code.as_str()).collect();
        assert_eq!(codes, ["SACM-XMI-014"]);
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
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" xmi:uuid=\"u\"/>",
        ));
        assert_eq!(
            on_claim,
            "the attribute \"xmi:uuid\" of the element \"argumentElement\" is not read in this \
             release, so it is kept as preserved content of the Claim \"c\", which a tolerant \
             write writes back and a strict write refuses; SACM23-XMI-019 reads it"
        );
        let outside = message(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"><xmi:Documentation/>\
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
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" xmi:uuid=\"u\"/>"
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

    /// An empty `xmi:id` in content the document holds outside its elements
    /// is kept, with SACM-ID-002 affecting no element.
    #[test]
    fn an_empty_id_outside_every_element_affects_no_element() {
        let input = "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\
             <xmi:Documentation xmi:id=\"\"/><sacm:ArgumentPackage xmi:id=\"p\"/></xmi:XMI>";
        let (found, document) = kept(input);
        assert_eq!(
            found,
            [
                finding("SACM-ID-002", &[], (1, 110)),
                finding("SACM-XMI-014", &[], (1, 110)),
            ]
        );
        assert!(document.has_preserved_content());
        assert!(document.get("p").is_some());
    }
}
