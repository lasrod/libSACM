//! The interchange roots of an XMI document, their names and the package
//! content under them (SACM23-XMI-001, SACM23-XMI-005, SACM23-XMI-012,
//! SACM23-XMI-014, SACM23-XMI-015, SACM23-XMI-016, SACM23-XMI-017,
//! SACM23-BASE-004, -005, -006, -007, -008, SACM23-ARG-004 to -013;
//! formal/23-05-08 §2.2, §2.4, §8.2-§8.12, §11.2-§11.18; XMI 2.5.1 §7.5.3,
//! §7.6.3, §7.8.4, §7.8.6, §7.10; ADR 0015, ADR 0016, ADR 0021, ADR 0025,
//! ADR 0026, ADR 0027, ADR 0028).
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
//!    preserved after all. The first conforming thing this release does not
//!    read is `Unsupported`, at its location, labelled by its entry of
//!    `refusal::Unread` (LIB-API-011); nothing is dropped (ADR 0015,
//!    D-PRESERVE).
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
//! `referencedArtifactElement`, a group's `argumentElement`) holds every
//! token of the attribute, or one id per element, in document order. A flag
//! or end the kind lacks is not read. An `argumentElement` element is a
//! reference element under an `ArgumentGroup` and a child under an
//! `ArgumentPackage`: the parent's kind decides, and no class has both
//! (ADR 0028), so an element nested in a group's `argumentElement` is
//! refused as a reference element would be, never read as a child. The id pre-pass also collects every id a reference may name, so
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
//! processing instructions; it is no element of the document, so anything
//! else on it has nowhere to be kept, and is refused (a defect, or XMI
//! metadata not read yet). An `xmi:version`, here or on a root, is ignored
//! with any value: the XMI namespace identifies the version (XMI 2.5.1
//! §7.5.3), and the writer does not write it (SACM23-XMI-024).
//!
//! Two attributes are read and reported (LIB-COMPAT-001, ADR 0022), one
//! diagnostic per attribute at its name: an `xsi:type` is SACM-XMI-010, an
//! Error in strict mode and a Warning in tolerant mode, and an `xmi:version`
//! is SACM-XMI-011, Info. Both modes read and refuse the same inputs, apart
//! from the C++ shapes above, which a strict read refuses. A
//! message names the element by its kind and id, never by the name or prefix
//! written, so the choice of prefix does not change it (SACM23-XMI-002).

use std::collections::BTreeSet;

use roxmltree::{Attribute, Node};

use super::input::{Locator, is_xml_space, locate};
use super::namespaces::{SACM_NS, SACM_PREFIX, XMI_NS, XMI_PREFIX};
use super::preserve;
use super::refusal::{Unread, unsupported};
use super::{Mode, ReadError};
use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, Attr, DeclaredType, Document, ElementId, ElementKind,
    Flag, IdGenerator, Item, LangString, Placed, Preserved, ReferenceEnd, Text, TextRole,
    flag_named, is_ncname, reference_end_named, role_named,
};

/// The §11.4 text spelling of `argumentElement`, which SACM23-XMI-009 is to
/// read in tolerant mode, as the role of an ArgumentPackage and the
/// reference end of an `ArgumentGroup`.
const TEXT_SPELLING: &str = "argumentationElement";

/// The `XLink` namespace, whose attributes on a reference element are a
/// cross-document link (SACM23-XMI-027).
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// The requirement of the diagnostics for preserved content.
const PRESERVED_ROW: &str = "LIB-COMPAT-003";

/// Why a part of the input is not read: a refusal, or content that does not
/// conform, which the caller keeps as preserved content (ADR 0033).
enum Fault {
    /// The input is refused.
    Refused(ReadError),
    /// The part does not conform: the caller preserves the whole of it.
    Defect,
}

impl From<ReadError> for Fault {
    fn from(error: ReadError) -> Self {
        Self::Refused(error)
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
    if is_wrapper {
        read_wrapper(&mut found, element, &mut document)?;
    } else {
        read_root(&mut found, element, &mut document)?;
    }
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
    fn unknown(&mut self, offset: usize, what: &str, holder: &Pending) {
        let severity = match self.mode {
            Mode::Strict => Severity::Error,
            Mode::Tolerant => Severity::Warning,
        };
        let message = format!(
            "{what} is not SACM 2.3 where it stands, so it is kept as written as preserved \
             content of {}, which a tolerant write writes back and a strict write refuses",
            subject(holder.kind, holder.id.as_str())
        );
        let diagnostic =
            Diagnostic::create(Code::XMI_UNKNOWN_CONTENT, severity, PRESERVED_ROW, message)
                .set_clause("7.3.3")
                .set_affected([holder.id.clone()]);
        self.push_at(diagnostic, offset);
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
        let reserved = &self.reserved;
        let id = self
            .generator
            .next(kind, |candidate| reserved.contains(candidate));
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
/// and each token of an attribute in no namespace named as a reference end.
/// None for any other attribute.
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
        && ReferenceEnd::ALL
            .iter()
            .any(|end| end.name() == attribute.name())
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
) -> Result<(), ReadError> {
    let text = found.text;
    let name = written_name(text, wrapper);
    // The wrapper is no element of the document, so what it carries besides
    // `xmi:version` and the XMI attributes XMI.xsd gives it has nowhere to
    // be kept (ADR 0033).
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
        return Err(unread_attribute(text, &attribute, &name, unread));
    }
    for child in wrapper.children() {
        if !child.is_element() {
            if let Some(first) = text_offset(text, child) {
                return Err(unsupported(
                    text,
                    first,
                    Unread::WrapperContent,
                    &format!("the element {name:?} holds text, which has no element to be kept in"),
                ));
            }
        } else if child.tag_name().namespace() == Some(XMI_NS) {
            // XMI document metadata, read by SACM23-XMI-019.
            return Err(unsupported(
                text,
                child.range().start,
                Unread::XmiMetadata,
                &format!(
                    "the XMI element {:?} in {name:?} is not read in this release",
                    written_name(text, child)
                ),
            ));
        } else {
            read_root(found, child, document)?;
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
    /// The `assertionDeclaration` read, if it is written, as the attribute
    /// or as a value element.
    declaration: Option<AssertionDeclaration>,
    /// Each flag given, as the attribute or as a value element, with its
    /// value (SACM23-BASE-007, SACM23-BASE-008).
    flags: Vec<(Flag, bool)>,
    /// Each reference end given, in the attribute or the element form, with
    /// its targets in document order (SACM23-XMI-005, SACM23-XMI-015).
    references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    /// The element's name as written in its start tag.
    written: String,
    /// The byte offset of its start tag.
    at: usize,
    /// The number of its children read so far: the index of the next.
    children: usize,
    /// Its preserved content so far (LIB-COMPAT-003, ADR 0033).
    kept: Kept,
}

/// The preserved content of one element as it is read (ADR 0033).
#[derive(Default)]
struct Kept {
    /// The attributes, each with the byte offset of its name: sorted by it
    /// when the element is held, so they keep their input order.
    attributes: Vec<(usize, Attr)>,
    /// The bindings their prefixes need.
    namespaces: Vec<(String, String)>,
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
    /// Whether the construct at byte `offset` is preserved.
    fn holds(&self, offset: usize) -> bool {
        self.offsets.contains(&offset)
    }

    /// The preserved content, its attributes in input order and its
    /// bindings by prefix.
    fn into_preserved(self) -> Preserved {
        let Self {
            mut attributes,
            mut namespaces,
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
        }
    }

    /// Whether `role` is given already: by its role element, or, for the
    /// `content`, in the C++ shape on the element itself.
    fn has_text(&self, role: TextRole) -> bool {
        match role {
            TextRole::Content => self.content.is_some() || self.shorthand.is_given(),
            TextRole::Key => self.key.is_some(),
        }
    }

    /// Reads `node`, text in `holder`, this element's node, a
    /// `UtilityElement`: the C++ shape of its `content` (LIB-COMPAT-008),
    /// with any `lang=` and `content=` it carries. Text that is all white
    /// space is no shape but joins one. A `content` element given as well is
    /// `RepeatedValue`, and so is `content=` with text.
    fn read_utility_text(
        &mut self,
        text: &str,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
    ) -> Result<(), ReadError> {
        let Some(first) = self.shorthand.add_text(text, node) else {
            return Ok(());
        };
        if self.content.is_some() {
            return Err(repeated_content(
                text,
                holder,
                &self.id,
                &self.shorthand,
                &self.kept,
            ));
        }
        if let Some(attribute) = self.shorthand.content_at {
            return Err(repeated(
                text,
                &self.id,
                "content.value.content",
                [attribute, first],
            ));
        }
        Ok(())
    }

    /// Keeps `attribute` of this element as preserved content, reported at
    /// its name (ADR 0033). An attribute whose prefix `sacm` or `xmi` is
    /// bound to another namespace than SACM 2.3 or XMI 2.5.1 cannot be
    /// written back on the element's start tag, where the writer binds them
    /// so, and is refused at byte `refuse_at` instead (a defect).
    fn preserve_attribute<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        attribute: &Attribute<'a, '_>,
        refuse_at: usize,
    ) -> Result<(), ReadError> {
        let text = found.text;
        let at = attribute.range_qname().start;
        let attr = preserve::attr(text, attribute);
        let written = attr.name.written();
        if let Some(prefix) = attr.name.prefix.as_deref() {
            let uri = attribute.namespace().unwrap_or_default();
            let pinned = match prefix {
                SACM_PREFIX => Some(SACM_NS),
                XMI_PREFIX => Some(XMI_NS),
                _ => None,
            };
            match pinned {
                Some(pinned) if pinned != uri => {
                    return Err(unsupported(
                        text,
                        refuse_at,
                        Unread::UnwritablePrefix,
                        &format!(
                            "the attribute {written:?} of the element {:?} is not SACM 2.3, and \
                             cannot be kept: its prefix {prefix:?} is bound to {uri:?}, and the \
                             written element binds it to {pinned:?}",
                            self.written
                        ),
                    ));
                }
                Some(_) => {}
                None if prefix == "xml" => {}
                None => self
                    .kept
                    .namespaces
                    .push((prefix.to_owned(), uri.to_owned())),
            }
        }
        self.kept.references.extend(
            reference_tokens(attribute)
                .into_iter()
                .filter(|token| !token.is_empty())
                .map(ElementId::new),
        );
        self.kept.offsets.insert(at);
        self.kept.attributes.push((at, attr));
        found.unknown(at, &format!("the attribute {written:?}"), self);
        Ok(())
    }

    /// Keeps `node`, a child element of this element, with its subtree, as
    /// preserved content placed before the next typed child, reported at its
    /// start tag (ADR 0033). Nothing inside it is read or reported, but each
    /// `xmi:id` in it is checked: an empty one is `InvalidId`, and one read
    /// before `DuplicateId`.
    fn preserve_element<'a>(
        &mut self,
        found: &mut Findings<'_, 'a>,
        node: Node<'a, '_>,
    ) -> Result<(), ReadError> {
        let text = found.text;
        for element in node.descendants().filter(Node::is_element) {
            for attribute in element.attributes() {
                if is_xmi(&attribute, "id") {
                    let value = checked_id(found, element, &attribute)?;
                    self.kept.ids.push(ElementId::new(value));
                } else {
                    self.kept.references.extend(
                        reference_tokens(&attribute)
                            .into_iter()
                            .filter(|token| !token.is_empty())
                            .map(ElementId::new),
                    );
                }
            }
        }
        let at = node.range().start;
        self.kept.offsets.insert(at);
        self.kept.children.push(Placed {
            index: self.children,
            item: Item::Element(preserve::fragment(text, node)),
        });
        let what = format!("the element {:?}", written_name(text, node));
        found.unknown(at, &what, self);
        Ok(())
    }

    /// Keeps `node`, a child of this element, as preserved text if it is
    /// text that is not all XML white space, trimmed of it, reported at its
    /// first character that is not literal white space (ADR 0033). Text
    /// right after preserved text, with no child between, joins it after an
    /// LF, as a read of the written output gives it.
    fn preserve_text(&mut self, found: &mut Findings<'_, '_>, node: Node<'_, '_>) {
        let Some(first) = text_offset(found.text, node) else {
            return;
        };
        let piece = node.text().unwrap_or_default().trim_matches(is_xml_space);
        if let Some(Placed {
            index,
            item: Item::Text(joined),
        }) = self.kept.children.last_mut()
        {
            if *index == self.children {
                joined.push('\n');
                joined.push_str(piece);
                return;
            }
        }
        self.kept.children.push(Placed {
            index: self.children,
            item: Item::Text(piece.to_owned()),
        });
        found.unknown(first, "text", self);
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
                    found.unknown(at, &what, self);
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

    /// Reads `node`, the value element of the flag `flag` in `holder`, this
    /// element's node (SACM23-BASE-007, SACM23-BASE-008): checked whole
    /// first, then against an earlier occurrence, the attribute included.
    fn read_flag_element(
        &mut self,
        text: &str,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        flag: Flag,
    ) -> Result<(), Fault> {
        // A nil flag reads as the flag not written: false (SACM23-XMI-026).
        let value = match read_value(text, node, &self.id)? {
            Some(value) => parse_flag(text, &value, node.range().start, &self.id, flag)?,
            None => false,
        };
        if self.flags.iter().any(|(given, _)| *given == flag) {
            let property = flag.name();
            return Err(self.repeated(text, holder, property, property).into());
        }
        self.flags.push((flag, value));
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
        text: &str,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        end: ReferenceEnd,
    ) -> Result<(), Fault> {
        let target = read_reference(text, node)?;
        let Some(position) = self.references.iter().position(|(given, _)| *given == end) else {
            self.references.push((end, vec![target]));
            return Ok(());
        };
        let property = end.name();
        let in_attribute = holder.attributes().any(|attribute| {
            is_unqualified(&attribute, property) && !self.kept.holds(attribute.range_qname().start)
        });
        if !end.is_many() || in_attribute {
            return Err(self.repeated(text, holder, property, property).into());
        }
        self.references[position].1.push(target);
        Ok(())
    }

    /// The `RepeatedValue` refusal of the property `property` of this
    /// element, whose node is `holder`, given more than once as the
    /// attribute or the value element `local`, as [`repeated_value`] finds
    /// it, without the occurrences kept as preserved content, which are
    /// none (ADR 0033).
    fn repeated(&self, text: &str, holder: Node<'_, '_>, local: &str, property: &str) -> ReadError {
        repeated_value(text, holder, local, &self.id, property, &self.kept)
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
    fn read(
        &mut self,
        text: &str,
        attribute: &Attribute<'_, '_>,
        kind: ElementKind,
        id: &ElementId,
    ) -> Result<Given, ReadError> {
        if attribute.namespace().is_some() {
            return Ok(Given::Other);
        }
        let at = attribute.range_qname().start;
        let local = attribute.name();
        if let Some(flag) = flag_named(kind, local) {
            let value = parse_flag(text, attribute.value(), at, id, flag)?;
            self.flags.push((flag, value));
            return Ok(Given::Read);
        }
        let Some(end) = reference_end_named(kind, local) else {
            return Ok(Given::Other);
        };
        let value = attribute.value();
        let count = id_tokens(value).count();
        if count == 0 {
            return Ok(Given::NoId);
        }
        if count > 1 && !end.is_many() {
            let starts = token_starts(text, attribute)
                .unwrap_or_else(|| std::iter::repeat_n(at, count).collect());
            let mut locator = Locator::new(text);
            return Err(ReadError::RepeatedValue {
                element: id.clone(),
                property: local.to_owned(),
                locations: starts
                    .into_iter()
                    .filter_map(|start| locator.locate(start))
                    .collect(),
            });
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
/// document order (ADR 0010 D2). The tree is walked with a stack, not by
/// recursion, so a deep document cannot exhaust the call stack.
fn read_root<'a>(
    found: &mut Findings<'_, 'a>,
    root: Node<'a, '_>,
    document: &mut Document,
) -> Result<(), ReadError> {
    let text = found.text;
    let first = root_start(found, root)?;
    // The elements whose content is being read, innermost last, each with
    // its node and the rest of its content, and the elements read whole.
    // Content is, on a ModelElement, one `name` (SACM23-XMI-012) and on an
    // Assertion one `assertionDeclaration` value element (SACM23-XMI-014);
    // on a UtilityElement one `content`, and on a TaggedValue one `key`
    // (SACM23-BASE-004, SACM23-BASE-005); on every element one `gid` value
    // element (SACM23-BASE-006), one value element per flag of its kind, one
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
                element.read_utility_text(text, holder, node)?;
            } else {
                // Text in element-only content does not conform (ADR 0033).
                element.preserve_text(found, node);
            }
            continue;
        }
        // A property part is read whole, and kept as preserved content
        // instead, as if it had not been read, if it turns out not to
        // conform (ADR 0033).
        let checkpoint = found.checkpoint();
        match read_part(found, element, holder, node) {
            Ok(true) => {}
            Ok(false) => match child_start(found, node, element)? {
                Some(child) => {
                    element.children = element.children.saturating_add(1);
                    open.push((child, node, node.children()));
                }
                None => element.preserve_element(found, node)?,
            },
            Err(Fault::Refused(error)) => return Err(error),
            Err(Fault::Defect) => {
                found.rollback(checkpoint);
                element.preserve_element(found, node)?;
            }
        }
    }
    hold(text, document, read)
}

/// Reads `node`, an element in `holder`, the node of `element`, if it is a
/// property part of the element, and returns whether it was one. A part is
/// a `name` on a `ModelElement` (SACM23-XMI-012), an `assertionDeclaration`
/// value element on an Assertion (SACM23-XMI-014), a `content` on a
/// `UtilityElement` and a `key` on a `TaggedValue` (SACM23-BASE-004,
/// SACM23-BASE-005), a `gid` value element on every element
/// (SACM23-BASE-006), a value element per flag of its kind, and a
/// reference element per end of its kind, one per single-valued end and
/// any number per multi-valued end (ADR 0026, ADR 0027). Each is checked
/// whole first, then against the occurrences before it that were read, so a
/// second is `RepeatedValue` (SACM23-XMI-021; ADR 0021 amendment 2).
///
/// A part that does not conform, and a `name` on a `UtilityElement`, is
/// [`Fault::Defect`]: the caller keeps it whole as preserved content, and
/// the property is not given (ADR 0033).
fn read_part<'a>(
    found: &mut Findings<'_, 'a>,
    element: &mut Pending,
    holder: Node<'a, '_>,
    node: Node<'a, '_>,
) -> Result<bool, Fault> {
    let text = found.text;
    let local = node.tag_name().name();
    if element.kind == ElementKind::ArgumentGroup && is_value_element(node, TEXT_SPELLING) {
        return Err(text_spelling(text, node.range().start, &element.written).into());
    }
    if let Some(unread) = interface_end(element.kind).filter(|_| is_value_element(node, INTERFACE))
    {
        return Err(unsupported(
            text,
            node.range().start,
            unread,
            &format!(
                "the reference element {:?} in the element {:?} is the {} end {INTERFACE:?}, \
                 which is not read in this release",
                written_name(text, node),
                element.written,
                element.kind
            ),
        )
        .into());
    }
    if is_value_element(node, "name") {
        if element.kind.is_a(AbstractClass::UtilityElement) {
            // A UtilityElement has no name.
            return Err(Fault::Defect);
        }
        let (name, name_id) = read_name(found, node, element.kind, &element.id)?;
        if element.name.is_some() {
            return Err(element.repeated(text, holder, "name", "name").into());
        }
        element.name = Some(name);
        element.name_id = name_id;
    } else if is_value_element(node, GID) {
        let value = read_value(text, node, &element.id)?;
        if std::mem::replace(&mut element.has_gid, true) {
            return Err(element.repeated(text, holder, GID, GID).into());
        }
        element.gid = value;
    } else if let Some(role) = [TextRole::Content, TextRole::Key]
        .into_iter()
        .find(|role| role.is_held_by(element.kind) && is_value_element(node, role.name()))
    {
        let held = read_text(found, node, role, element.kind, &element.id)?;
        if element.has_text(role) {
            return Err(match role {
                TextRole::Content => {
                    repeated_content(text, holder, &element.id, &element.shorthand, &element.kept)
                }
                TextRole::Key => element.repeated(text, holder, "key", "key"),
            }
            .into());
        }
        *element.text_mut(role) = Some(held);
    } else if element.kind.is_a(AbstractClass::Assertion) && is_value_element(node, DECLARATION) {
        // A nil declaration reads as one not written: `asserted`
        // (SACM23-XMI-026).
        let declaration = match read_value(text, node, &element.id)? {
            Some(value) => parse_declaration(text, &value, node.range().start, &element.id)?,
            None => AssertionDeclaration::Asserted,
        };
        if element.declaration.is_some() {
            return Err(element
                .repeated(text, holder, DECLARATION, DECLARATION)
                .into());
        }
        element.declaration = Some(declaration);
    } else if let Some(flag) =
        flag_named(element.kind, local).filter(|flag| is_value_element(node, flag.name()))
    {
        element.read_flag_element(text, holder, node, flag)?;
    } else if let Some(end) =
        reference_end_named(element.kind, local).filter(|end| is_value_element(node, end.name()))
    {
        element.read_reference_element(text, holder, node, end)?;
    } else {
        return Ok(false);
    }
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
        let texts = [
            (TextRole::Content, element.content),
            (TextRole::Key, element.key),
        ];
        let held = identified.and_then(|()| {
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
/// `AssuranceCasePackage::interface`, `ArgumentPackage::interface`), which
/// conforms and is read with the package interfaces.
const INTERFACE: &str = "interface";

/// The entry of the `interface` end of an element of `kind`, if its class
/// has one this release reads elements of: an AssuranceCasePackage's names
/// an `AssuranceCasePackageInterface` (SACM23-AC-002), and an
/// ArgumentPackage's an `ArgumentPackageInterface` (SACM23-ARG-014). It
/// conforms, so it is refused, not preserved (ADR 0033).
const fn interface_end(kind: ElementKind) -> Option<Unread> {
    match kind {
        ElementKind::AssuranceCasePackage => Some(Unread::AssuranceCasePackageInterface),
        ElementKind::ArgumentPackage => Some(Unread::ArgumentPackageInterface),
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
/// preserved (ADR 0033).
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
        declaration: None,
        flags: Vec::new(),
        references: Vec::new(),
        written,
        at: node.range().start,
        children: 0,
        kept: Kept::default(),
    }
}

/// Checks the start tag of `root`: its namespace, kind, id, type and other
/// attributes, in that order.
fn root_start<'a>(found: &mut Findings<'_, 'a>, root: Node<'a, '_>) -> Result<Pending, ReadError> {
    let text = found.text;
    let at_root = root.range().start;
    let name = written_name(text, root);

    // The namespace, then the kind (ADR 0016 D5, D6). Under `xmlns=""` the
    // parser reports the empty string, which is no namespace name
    // (Namespaces 1.0 §2.2, §6.2). Every package kind but the two read has
    // its entry; pass 1 has refused a root of any other name.
    let namespace = root.tag_name().namespace().filter(|uri| !uri.is_empty());
    let kind = match (namespace, package_kind(&root)) {
        (Some(SACM_NS), Some(kind)) => match Unread::kind(kind) {
            None => kind,
            Some(unread) => {
                return Err(unsupported(
                    text,
                    at_root,
                    unread,
                    &format!("the root element {name:?} is not read in this release"),
                ));
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
            element.preserve_attribute(found, &attribute, at_root)?;
        }
    }

    // Besides `xmi:id`, `xmi:version`, the type and `gid`, only the
    // SACMElement flags and references are read, in document order (ADR
    // 0016 D4, ADR 0026), and the C++ `name` attribute (LIB-COMPAT-008).
    // XMI metadata is not read yet; anything else does not conform, and is
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
        match start.read(text, &attribute, kind, &id)? {
            Given::Read => continue,
            Given::NoId => {}
            Given::Other => {
                let unread =
                    object_metadata(&attribute).or_else(|| interface_attribute(kind, &attribute));
                if let Some(unread) = unread {
                    return Err(unread_attribute(text, &attribute, &element.written, unread));
                }
            }
        }
        element.preserve_attribute(found, &attribute, attribute.range_qname().start)?;
    }
    element.flags = start.flags;
    element.references = start.references;
    Ok(element)
}

/// Checks the start tag of `node`, a child element of `parent`: its role,
/// type and kind, its id, and its other attributes, in that order, reading
/// an Assertion's `assertionDeclaration` and the flags and references.
/// `None` if the child does not conform as a whole (its role, namespace or
/// type), so the caller preserves it whole (ADR 0033).
fn child_start<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    parent: &Pending,
) -> Result<Option<Pending>, ReadError> {
    let text = found.text;
    let ChildKind::Read(kind) = child_kind(text, node, parent.kind, &parent.what())? else {
        return Ok(None);
    };
    let written = written_name(text, node);
    let id = read_id(found, node, kind)?;
    let mut element = pending(found, node, Some(parent.id.clone()), kind, id, written);
    let id = element.id.clone();
    // Besides `xmi:id` and the type (ADR 0016 D4), only an unqualified
    // `gid`, an Assertion's unqualified `assertionDeclaration`, and the
    // SACMElement flags and references (ADR 0026), and the C++ shapes
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
        if is_assertion && is_unqualified(&attribute, DECLARATION) {
            let at = attribute.range_qname().start;
            element.declaration = Some(parse_declaration(text, attribute.value(), at, &id)?);
            continue;
        }
        match start.read(text, &attribute, kind, &id)? {
            Given::Read => continue,
            Given::NoId => {
                element.preserve_attribute(found, &attribute, attribute.range_qname().start)?;
                continue;
            }
            Given::Other => {}
        }
        if kind == ElementKind::ArgumentGroup && is_unqualified(&attribute, TEXT_SPELLING) {
            let at = attribute.range_qname().start;
            return Err(text_spelling(text, at, &element.written));
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
            if let Err(earlier) = element.shorthand.read_attribute(text, &attribute) {
                let at = attribute.range_qname().start;
                return Err(repeated(text, &id, "content.value.content", [earlier, at]));
            }
            continue;
        }
        if is_tagged_value && is_unqualified(&attribute, "key") {
            let shape = format!(
                "the key of {} is given as the attribute \"key\"",
                subject(kind, id.as_str())
            );
            found.legacy_shape(attribute.range_qname().start, &id, shape);
            let mut value = LangString::default();
            value.set_content(Some(attribute.value().to_owned()));
            let mut held = Text::default();
            held.push(value, None);
            element.key = Some(held);
            continue;
        }
        let unread = object_metadata(&attribute).or_else(|| interface_attribute(kind, &attribute));
        if let Some(unread) = unread {
            return Err(unread_attribute(text, &attribute, &element.written, unread));
        }
        element.preserve_attribute(found, &attribute, attribute.range_qname().start)?;
    }
    element.flags = start.flags;
    element.references = start.references;
    Ok(Some(element))
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
    let value = checked_id(found, node, &attribute)?;
    if value != attribute.value() {
        found.trimmed_id(&attribute, value, &subject(kind, value), value);
    }
    Ok(ElementId::new(value))
}

/// The `xmi:id` of `node`, a `name`, `content`, `key` or `value` element
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
    /// It is read, as an element of this kind.
    Read(ElementKind),
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
/// except two refusals: an element in the XMI namespace, XMI metadata
/// (SACM23-XMI-019), and the §11.4 text spelling `argumentationElement` in
/// an ArgumentPackage (SACM23-XMI-009). Without a type under a role of an
/// abstract type, it is `MissingType` (SACM-XMI-005, LIB-COMPAT-010),
/// unless it carries an `xmi:idref` or an `href`, the reference form of a
/// composite, which is refused by its entry ([`proxy_link`]). A
/// conforming kind this release does not read is refused by its entry.
/// Every refusal is at the child's start tag.
fn child_kind(
    text: &str,
    node: Node<'_, '_>,
    parent: ElementKind,
    parent_what: &str,
) -> Result<ChildKind, ReadError> {
    let written = written_name(text, node);
    let at = node.range().start;
    match node.tag_name().namespace().filter(|uri| !uri.is_empty()) {
        Some(XMI_NS) => {
            return Err(unsupported(
                text,
                at,
                Unread::XmiMetadata,
                &format!(
                    "the XMI element {written:?} in {parent_what} is not read in this release"
                ),
            ));
        }
        Some(_) => return Ok(ChildKind::Preserve),
        None => {}
    }
    let local = node.tag_name().name();
    let Some(role) = role_named(parent, local) else {
        if parent == ElementKind::ArgumentPackage && local == TEXT_SPELLING {
            return Err(unsupported(
                text,
                at,
                Unread::TextSpelling,
                &format!(
                    "the element {written:?} in {parent_what} is spelled as in the text of §11.4, \
                     and the role is argumentElement"
                ),
            ));
        }
        return Ok(ChildKind::Preserve);
    };
    let mut typed = None;
    for attribute in node.attributes().filter(is_type) {
        let Some(kind) = type_kind(node, attribute.value()) else {
            return Ok(ChildKind::Preserve);
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
                return Err(unsupported(
                    text,
                    at,
                    unread,
                    &format!(
                        "the element {written:?} in {parent_what} is the reference form of a \
                         {class}, which is not read in this release"
                    ),
                ));
            }
            return Err(ReadError::MissingType {
                role: role.name.to_owned(),
                declared_type: class.name().to_owned(),
                location: locate(text, at),
            });
        }
    };
    if !role.declared.admits(kind) {
        return Ok(ChildKind::Preserve);
    }
    // The readable children: every kind a role of a readable class admits,
    // but the package kinds this release does not read.
    if let Some(unread) = Unread::kind(kind) {
        return Err(unsupported(
            text,
            at,
            unread,
            &format!("the {kind} {written:?} in {parent_what} is not read in this release"),
        ));
    }
    Ok(ChildKind::Read(kind))
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
/// `asserted`.
fn parse_declaration(
    text: &str,
    value: &str,
    offset: usize,
    element: &ElementId,
) -> Result<AssertionDeclaration, ReadError> {
    value.parse().map_err(|_| ReadError::InvalidLiteral {
        element: element.clone(),
        property: DECLARATION.to_owned(),
        value: value.to_owned(),
        location: locate(text, offset),
    })
}

/// The value of the flag `flag` of the element `element`, given as `value`
/// by the attribute or the value element at byte `offset` of `text`
/// (SACM23-BASE-007, SACM23-BASE-008; ADR 0026). It is an `xsd:boolean`,
/// whose white space is collapsed (XML Schema 1.0 Part 2 §3.2.2.1, §4.3.6):
/// `true` and `1` are true, `false` and `0` are false, with XML white space
/// around them. Any other value, the empty one included, is `InvalidLiteral`
/// at `offset`, holding the value as read (SACM-ENUM-001), and never read as
/// false.
fn parse_flag(
    text: &str,
    value: &str,
    offset: usize,
    element: &ElementId,
    flag: Flag,
) -> Result<bool, ReadError> {
    // A literal holds no white space, so collapsing a value that reads is
    // trimming it.
    match value.trim_matches(is_xml_space) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(ReadError::InvalidLiteral {
            element: element.clone(),
            property: flag.name().to_owned(),
            value: value.to_owned(),
            location: locate(text, offset),
        }),
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
/// content (ADR 0033).
fn read_reference(text: &str, node: Node<'_, '_>) -> Result<ElementId, Fault> {
    let element = written_name(text, node);
    let mut link = None;
    for attribute in node.attributes() {
        let is_idref = is_xmi(&attribute, "idref");
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
            return Err(unread_attribute(text, &attribute, &element, unread).into());
        }
        if link.is_some() {
            return Err(Fault::Defect);
        }
        let value = attribute.value().trim_matches(is_xml_space);
        let target = if is_idref {
            value
        } else {
            let Some(fragment) = same_document_fragment(value) else {
                return Err(unsupported(
                    text,
                    attribute.range_qname().start,
                    Unread::CrossDocumentHref,
                    &format!(
                        "the href {:?} of the reference element {element:?} names no element of \
                         this document by its id; only href=\"#id\" is read in this release",
                        attribute.value()
                    ),
                )
                .into());
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
fn read_value(text: &str, node: Node<'_, '_>, owner: &ElementId) -> Result<Option<String>, Fault> {
    read_value_element(text, node, owner, false).map(|(value, _)| value)
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
    text: &str,
    node: Node<'_, '_>,
    owner: &ElementId,
    takes_lang: bool,
) -> Result<(Option<String>, Option<LangAttribute>), Fault> {
    let mut nil = false;
    let mut lang = None;
    for attribute in node.attributes() {
        if is_xsi_nil(&attribute) {
            nil = parse_nil(text, &attribute, owner)?;
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
/// `InvalidLiteral` at the attribute's name, holding the value as read.
fn parse_nil(
    text: &str,
    attribute: &Attribute<'_, '_>,
    owner: &ElementId,
) -> Result<bool, ReadError> {
    match attribute.value().trim_matches(is_xml_space) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(ReadError::InvalidLiteral {
            element: owner.clone(),
            property: "xsi:nil".to_owned(),
            value: attribute.value().to_owned(),
            location: locate(text, attribute.range_qname().start),
        }),
    }
}

/// The `RepeatedValue` refusal of the property `property` of the element
/// `owner`, given at the byte `offsets` of `text`, in document order,
/// located in one pass (SACM-MULT-001; XMI 2.5.1 §9.5.2 rule 2h).
fn repeated(
    text: &str,
    owner: &ElementId,
    property: &str,
    offsets: impl IntoIterator<Item = usize>,
) -> ReadError {
    let mut locator = Locator::new(text);
    ReadError::RepeatedValue {
        element: owner.clone(),
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
    repeated(text, owner, property, attribute.into_iter().chain(elements))
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
    repeated(text, owner, TextRole::Content.name(), offsets)
}

/// The `RepeatedValue` refusal of the field `field`, `lang` or `content`, of
/// the `LangString` that `holder`, a `name` or a `value` of the element
/// `owner`, holds; `property` names the `LangString` (`name`,
/// `content.value`, `key.value`). It gives every occurrence in document
/// order: the attribute, each value element and, for `content`, the text, at
/// its first character that is not literal white space; and in a `name` the
/// other C++ shapes (LIB-COMPAT-008), the `lang` attribute of a `content`
/// element and each `value` element that gives the field.
fn repeated_field(
    text: &str,
    holder: Node<'_, '_>,
    field: &str,
    owner: &ElementId,
    property: &str,
) -> ReadError {
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
    repeated(text, owner, &format!("{property}.{field}"), offsets)
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
    read_lang_string(found, node, &subject, owner, "name")
}

/// The `MultiLangString` that the `content` or `key` element `node` holds,
/// in `role`, with its ids (SACM23-BASE-004, SACM23-BASE-005; XMI 2.5.1
/// §7.8.3, §7.8.6; ADR 0025). It belongs to the element `owner`, of `kind`.
///
/// It carries only `xmi:id` and a type naming `MultiLangString`, checked
/// at the start tag first, and holds only `value` elements in no namespace,
/// white space, comments and processing instructions. Each `value` is read
/// as a `name` is ([`read_lang_string`]), so values are kept in order, with
/// duplicates. With no `value`, it is an empty `MultiLangString`.
///
/// Without a `value`, `lang` and `content` attributes on it, and text in
/// it, are the C++ single-language shorthand (LIB-COMPAT-008): one value,
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
    let mut shorthand = Shorthand::default();
    for attribute in node.attributes() {
        if is_unqualified(&attribute, "lang") || is_unqualified(&attribute, "content") {
            if has_values {
                return Err(Fault::Defect);
            }
            // Attributes are unique in a start tag (XML 1.0 §3.1), so the
            // content is given once here and this never refuses.
            let _ = shorthand.read_attribute(text, &attribute);
        } else if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject, owner.as_str());
        } else if let Some(unread) = object_metadata(&attribute) {
            return Err(unread_attribute(text, &attribute, &element, unread).into());
        } else if !(is_type(&attribute) || is_xmi(&attribute, "id")) {
            return Err(Fault::Defect);
        }
    }
    let value_subject = format!("a value of {subject}");
    let property = format!("{}.value", role.name());
    for child in node.children() {
        if !child.is_element() {
            if has_values {
                if text_offset(text, child).is_some() {
                    return Err(Fault::Defect);
                }
            } else if let Some(first) = shorthand.add_text(text, child) {
                if let Some(attribute) = shorthand.content_at {
                    let property = format!("{property}.content");
                    return Err(repeated(text, owner, &property, [attribute, first]).into());
                }
            }
        } else if is_value_element(child, "value") {
            let (value, id) = read_lang_string(found, child, &value_subject, owner, &property)?;
            held.push(value, id);
        } else {
            return Err(xmi_element_in(text, child, &element));
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

/// What an element `child` that `element`, a `name`, `content`, `key` or
/// `value`, does not read makes it: refused if it is in the XMI namespace,
/// which is XMI metadata not read yet (SACM23-XMI-019), and otherwise
/// [`Fault::Defect`] (ADR 0033).
fn xmi_element_in(text: &str, child: Node<'_, '_>, element: &str) -> Fault {
    if child.tag_name().namespace() != Some(XMI_NS) {
        return Fault::Defect;
    }
    Fault::Refused(unsupported(
        text,
        child.range().start,
        Unread::XmiMetadata,
        &format!(
            "the XMI element {:?} in {element:?} is not read in this release",
            written_name(text, child)
        ),
    ))
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
    property: &str,
) -> Result<(LangString, Option<ElementId>), Fault> {
    let text = found.text;
    let element = written_name(text, node);
    let is_name = property == "name";
    check_lang_string_tag(text, node, &element)?;
    let name_id = read_owned_id(found, node, subject, owner)?;
    let repeated = |field: &str| repeated_field(text, node, field, owner, property);
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
                    Some(unread) => unread_attribute(text, &attribute, &element, unread).into(),
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
            let value = read_value(text, child, owner)?;
            fields.lang(value).map_err(repeated)?;
        } else if is_value_element(child, "content") {
            let (value, lang) = read_value_element(text, child, owner, is_name)?;
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
            let value = read_name_value(found, child, subject, owner)?;
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
            return Err(xmi_element_in(text, child, &element));
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
fn check_lang_string_tag(text: &str, node: Node<'_, '_>, element: &str) -> Result<(), Fault> {
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
                return Err(unsupported(
                    text,
                    node.range().start,
                    Unread::ExpressionLangString,
                    &format!(
                        "the element {element:?} has the type {:?}, which is not read in this \
                         release",
                        attribute.value()
                    ),
                )
                .into());
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
) -> Result<LangString, Fault> {
    let text = found.text;
    if let Some(attribute) = node.attributes().find(|attribute| is_xmi(attribute, "id")) {
        let element = written_name(text, node);
        return Err(unread_attribute(text, &attribute, &element, Unread::XmiMetadata).into());
    }
    let subject = format!("the value in {subject}");
    read_lang_string(found, node, &subject, owner, "name.value").map(|(value, _)| value)
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

/// The refusal of the §11.4 text spelling `argumentationElement` at byte
/// `offset`, an attribute or element of the `ArgumentGroup` written as
/// `group`, whose end is spelled `argumentElement` (SACM23-XMI-009).
fn text_spelling(text: &str, offset: usize, group: &str) -> ReadError {
    unsupported(
        text,
        offset,
        Unread::TextSpelling,
        &format!(
            "{TEXT_SPELLING:?} in the element {group:?} is spelled as in the text of §11.4, and the \
             end of an ArgumentGroup is argumentElement"
        ),
    )
}

/// The refusal of `attribute` of the element written as `element`, at its
/// name, as the entry `unread` labels it.
fn unread_attribute(
    text: &str,
    attribute: &Attribute<'_, '_>,
    element: &str,
    unread: Unread,
) -> ReadError {
    let range = attribute.range_qname();
    let name = text.get(range.clone()).unwrap_or(attribute.name());
    unsupported(
        text,
        range.start,
        unread,
        &format!("the attribute {name:?} of the element {element:?} is not read in this release"),
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
    use crate::diagnostic::SourceLocation;
    use crate::model::{MultiLangString, SacmElement};
    use crate::xmi::{ReadError, ReadOptions, read};

    const OPEN: &str = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n"
    );

    /// The refusal of `content` in the root `p`, on line 2.
    fn refusal(content: &str) -> Option<(Option<SourceLocation>, String)> {
        let input = format!("{OPEN}{content}</sacm:ArgumentPackage>");
        match read(input.as_bytes(), &ReadOptions::default()) {
            Err(ReadError::Unsupported { location, reason }) => Some((location, reason)),
            _ => None,
        }
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
                "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>",
                "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" is spelled as in the text of §11.4, and the role is argumentElement (a defect in the input; SACM23-XMI-009 is to read it in tolerant mode)",
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
    }
}
