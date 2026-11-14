//! The interchange roots of an XMI document, their names and the package
//! content under them (SACM23-XMI-001, SACM23-XMI-005, SACM23-XMI-012,
//! SACM23-XMI-014, SACM23-XMI-015, SACM23-XMI-016, SACM23-XMI-017,
//! SACM23-BASE-004, -005, -006, -007, -008; formal/23-05-08 §2.2, §2.4,
//! §8.2-§8.12, §11.10; XMI 2.5.1 §7.5.3, §7.6.3, §7.8.4, §7.8.6, §7.10; ADR
//! 0015, ADR 0016, ADR 0021, ADR 0025, ADR 0026).
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
//!    (SACM23-XMI-016); then its attributes, of which a Claim's
//!    `assertionDeclaration` is read (SACM23-XMI-017), and its content. The
//!    first thing this release cannot read is `Unsupported`, at its location;
//!    nothing is dropped (ADR 0015, D-PRESERVE).
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
//! SACM23-BASE-005; ADR 0025). The single-language shorthand is refused
//! until LIB-COMPAT-008.
//!
//! A name's `lang` and `content` (and those of a `value`), every element's
//! `gid`, `isCitation` and `isAbstract`, and a Claim's `assertionDeclaration`
//! are read from the attribute or from a value element: an element in no
//! namespace named after the property, whose character content is the value
//! (SACM23-XMI-014, SACM23-BASE-006, SACM23-BASE-007, SACM23-BASE-008; XMI
//! 2.5.1 §7.8.4; ADR 0021 amendment 1). A flag is an `xsd:boolean`. Every
//! element's `citedElement` and `abstractForm` are references, read from the
//! attribute, whose value is the target ids separated by XML white space
//! (SACM23-XMI-005), or from a reference element with one `xmi:idref` or
//! `href="#id"` (SACM23-XMI-015), and held as ids, resolved or not (ADR
//! 0026). The id pre-pass also collects every id a reference may name, so
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
//! processing instructions. An `xmi:version`, here or on a root, is ignored
//! with any value: the XMI namespace identifies the version (XMI 2.5.1
//! §7.5.3), and the writer does not write it (SACM23-XMI-024).
//!
//! Two attributes are read and reported (LIB-COMPAT-001, ADR 0022), one
//! diagnostic per attribute at its name: an `xsi:type` is SACM-XMI-010, an
//! Error in strict mode and a Warning in tolerant mode, and an `xmi:version`
//! is SACM-XMI-011, Info. Both modes read and refuse the same inputs. A
//! message names the element by its kind and id, never by the name or prefix
//! written, so the choice of prefix does not change it (SACM23-XMI-002).

use std::collections::BTreeSet;

use roxmltree::{Attribute, Node};

use super::input::{Locator, is_xml_space, locate};
use super::namespaces::{SACM_NS, XMI_NS};
use super::{Mode, ReadError};
use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, DeclaredType, Document, ElementId, ElementKind, Flag,
    IdGenerator, LangString, ReferenceEnd, Text, TextRole, is_ncname, reference_end_named,
    role_named,
};

/// The XML Schema instance namespace, of `xsi:type`, which is read but never
/// written (ADR 0017 T1).
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The requirement the read diagnostics of `xsi:type` and `xmi:version`
/// enforce.
const MODES_ROW: &str = "LIB-COMPAT-001";

/// The requirement of the Info for an element without an `xmi:id`.
const MISSING_ID_ROW: &str = "SACM23-XMI-020";

/// The requirement of the Info for an `xmi:id` read trimmed.
const TRIMMED_ID_ROW: &str = "SACM23-XMI-025";

/// The Assertion property read as an attribute or a value element
/// (formal/23-05-08 §11.10).
const DECLARATION: &str = "assertionDeclaration";

/// The `SACMElement` property read as an attribute or a value element
/// (formal/23-05-08 §8.2).
const GID: &str = "gid";

/// Why the single-language shorthand of a `MultiLangString` is refused.
const SHORTHAND: &str = "the single-language shorthand of a MultiLangString is not read \
                         until LIB-COMPAT-008; write a `value` child per language";

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
        reserved: reserved_ids(element),
        generator: IdGenerator::default(),
        order: 0,
        diagnostics: Vec::new(),
    };
    if is_wrapper {
        read_wrapper(&mut found, element, &mut document)?;
    } else {
        read_root(&mut found, element, &mut document)?;
    }
    let Findings {
        generator,
        diagnostics,
        ..
    } = found;
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
    /// every root's tree (SACM23-VAL-001, SACM23-XMI-022).
    ids: BTreeSet<&'a str>,
    /// The value of every `xmi:id` in the input, collected before anything
    /// is read, so that no generated id is one, even one that comes later
    /// (SACM23-XMI-020).
    reserved: BTreeSet<&'a str>,
    /// The ids generated for elements without an `xmi:id`.
    generator: IdGenerator,
    /// The number of elements met so far: the next one's place in document
    /// order.
    order: usize,
    /// The diagnostics found so far, in reading order, each with its byte
    /// offset in `text`.
    diagnostics: Vec<(usize, Diagnostic)>,
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
        let value = attribute.value();
        if is_xmi(&attribute, "id") {
            reserved.insert(id_value(value));
        } else if is_xmi(&attribute, "idref") {
            reserved.insert(value.trim_matches(is_xml_space));
        } else if is_unqualified(&attribute, "href") {
            if let Some(fragment) = value.trim_matches(is_xml_space).strip_prefix('#') {
                reserved.insert(fragment);
            }
        } else if attribute.namespace().is_none()
            && ReferenceEnd::ALL
                .iter()
                .any(|end| end.name() == attribute.name())
        {
            reserved.extend(id_tokens(value));
        }
    }
    reserved
}

/// The ids of an `xsd:IDREFS` value: its tokens between XML white space,
/// each exactly as written (XML Schema 1.0 Part 2 §3.3.10, §4.3.6; ADR 0026).
fn id_tokens(value: &str) -> impl Iterator<Item = &str> {
    value.split(is_xml_space).filter(|token| !token.is_empty())
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
            format!(
                "the XMI element {:?} is not read as a root in this release",
                written_name(text, element)
            ),
        )),
        // An XMI element outside the XMI namespace (ADR 0016 D1), which
        // LIB-COMPAT-002 may read.
        (_, "XMI") => Err(unsupported(
            text,
            element.range().start,
            format!(
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
    for attribute in wrapper.attributes() {
        if !is_xmi(&attribute, "version") {
            return Err(unread_attribute(text, &attribute, &name));
        }
        found.xmi_version(&attribute, "the XMI element", None);
    }
    for child in wrapper.children() {
        if !child.is_element() {
            check_text(text, child, &name)?;
        } else if child.tag_name().namespace() == Some(XMI_NS) {
            // XMI document metadata, read by SACM23-XMI-019.
            return Err(unsupported(
                text,
                child.range().start,
                format!(
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
    name: Option<LangString>,
    /// The `xmi:id` of the name, if it has one (SACM23-XMI-022).
    name_id: Option<ElementId>,
    /// The `content` of a `UtilityElement`, with its ids.
    content: Option<Text>,
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

    /// Reads `node`, the value element of the flag `flag` in `holder`, this
    /// element's node (SACM23-BASE-007, SACM23-BASE-008): checked whole
    /// first, then against an earlier occurrence, the attribute included.
    fn read_flag_element(
        &mut self,
        text: &str,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        flag: Flag,
    ) -> Result<(), ReadError> {
        let value = read_value(text, node)?;
        let value = parse_flag(text, &value, node.range().start, &self.id, flag)?;
        if self.flags.iter().any(|(given, _)| *given == flag) {
            let property = flag.name();
            return Err(repeated_value(text, holder, property, &self.id, property));
        }
        self.flags.push((flag, value));
        Ok(())
    }

    /// Reads `node`, a reference element of the end `end` in `holder`, this
    /// element's node (SACM23-XMI-015): checked whole first
    /// ([`read_reference`]), then against the attribute (XMI 2.5.1 rule 2h)
    /// and, on a single-valued end, an earlier element.
    fn read_reference_element(
        &mut self,
        text: &str,
        holder: Node<'_, '_>,
        node: Node<'_, '_>,
        end: ReferenceEnd,
    ) -> Result<(), ReadError> {
        let target = read_reference(text, node)?;
        let property = end.name();
        let has_attribute = holder
            .attributes()
            .any(|attribute| is_unqualified(&attribute, property));
        let given = self
            .references
            .iter_mut()
            .find(|(held, _)| *held == end)
            .map(|(_, targets)| targets);
        match given {
            Some(targets) if end.is_many() && !has_attribute => targets.push(target),
            Some(_) => {
                return Err(repeated_value(text, holder, property, &self.id, property));
            }
            None => self.references.push((end, vec![target])),
        }
        Ok(())
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

impl StartTag {
    /// Reads `attribute` of the element `id`, of `kind`, if it is a flag or
    /// a reference end of the element in no namespace, and returns whether
    /// it was one. Attributes are unique in a start tag (XML 1.0 §3.1), so
    /// none repeats here.
    ///
    /// A flag is an `xsd:boolean` ([`parse_flag`]). A reference holds the
    /// ids its value lists, each token exactly as written
    /// (SACM23-XMI-005): none is `Unsupported`, since that would read as no
    /// reference, and two or more on a single-valued end is `RepeatedValue`,
    /// with one location per token, each at the attribute's name.
    fn read(
        &mut self,
        text: &str,
        attribute: &Attribute<'_, '_>,
        kind: ElementKind,
        id: &ElementId,
        element: &str,
    ) -> Result<bool, ReadError> {
        if attribute.namespace().is_some() {
            return Ok(false);
        }
        let at = attribute.range_qname().start;
        let local = attribute.name();
        if let Some(flag) = Flag::ALL.into_iter().find(|flag| flag.name() == local) {
            let value = parse_flag(text, attribute.value(), at, id, flag)?;
            self.flags.push((flag, value));
            return Ok(true);
        }
        let Some(end) = reference_end_named(kind, local) else {
            return Ok(false);
        };
        let value = attribute.value();
        let count = id_tokens(value).count();
        if count == 0 {
            return Err(unsupported(
                text,
                at,
                format!(
                    "the attribute {local:?} of the element {element:?} holds no id, so it \
                     cannot be read as a {local}; leave it out for none"
                ),
            ));
        }
        if count > 1 && !end.is_many() {
            return Err(ReadError::RepeatedValue {
                element: id.clone(),
                property: local.to_owned(),
                locations: locate(text, at).map_or_else(Vec::new, |at| vec![at; count]),
            });
        }
        let targets = id_tokens(value).map(ElementId::new).collect();
        self.references.push((end, targets));
        Ok(true)
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
    // element (SACM23-BASE-006), one value element per flag and one
    // reference element per single-valued end (ADR 0026); readable children
    // (SACM23-XMI-016), white space, comments and processing instructions.
    let mut open = vec![(first, root, root.children())];
    let mut read = Vec::new();
    while let Some((element, holder, content)) = open.last_mut() {
        let Some(node) = content.next() else {
            read.extend(open.pop().map(|(element, _, _)| element));
            continue;
        };
        let is_utility = element.kind.is_a(AbstractClass::UtilityElement);
        if !node.is_element() {
            if is_utility {
                // Text on a UtilityElement is the C++ form of its content.
                check_shorthand_text(text, node, &element.written)?;
            } else {
                check_text(text, node, &element.written)?;
            }
        } else if node.tag_name().name() == "name" {
            if is_utility {
                return Err(unsupported(
                    text,
                    node.range().start,
                    format!(
                        "the element {:?} in {} is not read: {} is a UtilityElement, which \
                         has no name",
                        written_name(text, node),
                        element.what(),
                        subject(element.kind, element.id.as_str())
                    ),
                ));
            }
            // Checked whole first, its namespace included, so that only a
            // `name` in no namespace is an occurrence; then against an
            // earlier name (SACM23-XMI-021; ADR 0021 amendment 2).
            let (name, name_id) = read_name(found, node, element.kind, &element.id)?;
            if element.name.is_some() {
                return Err(repeated_value(text, *holder, "name", &element.id, "name"));
            }
            element.name = Some(name);
            element.name_id = name_id;
        } else if is_value_element(node, GID) {
            // Checked whole first, then against an earlier occurrence.
            let value = read_value(text, node)?;
            if element.gid.is_some() {
                return Err(repeated_value(text, *holder, GID, &element.id, GID));
            }
            element.gid = Some(value);
        } else if let Some(role) = [TextRole::Content, TextRole::Key]
            .into_iter()
            .find(|role| role.is_held_by(element.kind) && is_value_element(node, role.name()))
        {
            // Checked whole first, then against an earlier occurrence.
            let held = read_text(found, node, role, element.kind, &element.id)?;
            if element.text_mut(role).is_some() {
                let property = role.name();
                return Err(repeated_value(
                    text,
                    *holder,
                    property,
                    &element.id,
                    property,
                ));
            }
            *element.text_mut(role) = Some(held);
        } else if element.kind.is_a(AbstractClass::Assertion) && is_value_element(node, DECLARATION)
        {
            // Checked whole first, then against an earlier occurrence.
            let value = read_value(text, node)?;
            let declaration = parse_declaration(text, &value, node.range().start, &element.id)?;
            if element.declaration.is_some() {
                return Err(repeated_value(
                    text,
                    *holder,
                    DECLARATION,
                    &element.id,
                    DECLARATION,
                ));
            }
            element.declaration = Some(declaration);
        } else if let Some(flag) = Flag::ALL
            .into_iter()
            .find(|flag| is_value_element(node, flag.name()))
        {
            element.read_flag_element(text, *holder, node, flag)?;
        } else if let Some(end) = reference_end_named(element.kind, node.tag_name().name())
            .filter(|end| is_value_element(node, end.name()))
        {
            element.read_reference_element(text, *holder, node, end)?;
        } else {
            let child = child_start(found, node, element)?;
            open.push((child, node, node.children()));
        }
    }
    hold(text, document, read)
}

/// Adds the elements of one root's tree, read whole, to `document`, each
/// with everything read for it, in document order, so a parent comes before
/// its children. Everything is checked, so the inserts succeed; a failure
/// would still be a refusal, never a panic.
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
        let held = flagged.and_then(|()| {
            element
                .references
                .into_iter()
                .try_for_each(|(end, targets)| document.set_references(&id, end, targets))
        });
        held.map_err(|error| {
            unsupported(
                text,
                element.at,
                format!("{what} cannot be held in the document: {error}"),
            )
        })?;
    }
    Ok(())
}

/// Checks the start tag of `root`: its namespace, kind, id, type and other
/// attributes, in that order.
fn root_start<'a>(found: &mut Findings<'_, 'a>, root: Node<'a, '_>) -> Result<Pending, ReadError> {
    let text = found.text;
    let at_root = root.range().start;
    let name = written_name(text, root);
    let not_read = |reason: String| unsupported(text, at_root, reason);

    // The namespace, then the kind (ADR 0016 D5, D6). Under `xmlns=""` the
    // parser reports the empty string, which is no namespace name
    // (Namespaces 1.0 §2.2, §6.2).
    let namespace = root.tag_name().namespace().filter(|uri| !uri.is_empty());
    let kind = match (namespace, package_kind(&root)) {
        (
            Some(SACM_NS),
            Some(kind @ (ElementKind::AssuranceCasePackage | ElementKind::ArgumentPackage)),
        ) => kind,
        (Some(SACM_NS), _) => {
            return Err(not_read(format!(
                "the root element {name:?} is not read in this release"
            )));
        }
        (Some(namespace), _) => {
            return Err(not_read(format!(
                "the root element {name:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace {SACM_NS:?}"
            )));
        }
        (None, _) => {
            return Err(not_read(format!(
                "the root element {name:?} is in no namespace, not the SACM 2.3 namespace {SACM_NS:?}"
            )));
        }
    };

    let what = format!("the root element {name:?}");
    let id = read_id(found, root, kind)?;
    let id_text = id.as_str();
    for attribute in root.attributes() {
        if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject(kind, id_text), id_text);
        } else if is_xmi(&attribute, "version") {
            found.xmi_version(&attribute, &subject(kind, id_text), Some(id_text));
        }
    }

    // A type must name the root's own class (ADR 0016 D2). Like the kind and
    // the id, it is refused at the root.
    if let Some(attribute) = root
        .attributes()
        .find(|attribute| is_type(attribute) && !names_class(root, attribute.value(), kind.name()))
    {
        return Err(not_read(format!(
            "{what} has the type {:?}, which names another class",
            attribute.value()
        )));
    }

    // Besides `xmi:id`, `xmi:version`, the type and `gid`, only the
    // SACMElement flags and references are read, in document order (ADR
    // 0016 D4, ADR 0026); any other attribute is not read yet.
    let mut start = StartTag::default();
    for attribute in root.attributes() {
        if is_xmi(&attribute, "id")
            || is_xmi(&attribute, "version")
            || is_type(&attribute)
            || is_unqualified(&attribute, GID)
            || start.read(text, &attribute, kind, &id, &name)?
        {
            continue;
        }
        return Err(unread_attribute(text, &attribute, &name));
    }

    Ok(Pending {
        order: found.next_order(),
        parent: None,
        kind,
        id,
        gid: root
            .attributes()
            .find(|attribute| is_unqualified(attribute, GID))
            .map(|attribute| attribute.value().to_owned()),
        name: None,
        name_id: None,
        content: None,
        key: None,
        declaration: None,
        flags: start.flags,
        references: start.references,
        written: name,
        at: at_root,
    })
}

/// Checks the start tag of `node`, a child element of `parent`: its role,
/// type and kind, its id, and its other attributes, in that order, reading
/// an Assertion's `assertionDeclaration` and the flags and references.
fn child_start<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    parent: &Pending,
) -> Result<Pending, ReadError> {
    let text = found.text;
    let kind = child_kind(text, node, parent.kind, &parent.what())?;
    let written = written_name(text, node);
    let id = read_id(found, node, kind)?;
    // Besides `xmi:id` and the type (ADR 0016 D4), only an unqualified
    // `gid`, an Assertion's unqualified `assertionDeclaration`, and the
    // SACMElement flags and references (ADR 0026), in document order, so a
    // refusal after them lands on the later attribute.
    let is_assertion = kind.is_a(AbstractClass::Assertion);
    let is_utility = kind.is_a(AbstractClass::UtilityElement);
    let is_tagged_value = kind == ElementKind::TaggedValue;
    let mut declaration = None;
    let mut gid = None;
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
            gid = Some(attribute.value().to_owned());
            continue;
        }
        if is_assertion && is_unqualified(&attribute, DECLARATION) {
            let at = attribute.range_qname().start;
            declaration = Some(parse_declaration(text, attribute.value(), at, &id)?);
            continue;
        }
        if start.read(text, &attribute, kind, &id, &written)? {
            continue;
        }
        // The C++ attribute forms of a UtilityElement's content and of a
        // TaggedValue's key and value (ADR 0025).
        let shorthand = (is_utility && is_unqualified(&attribute, "content"))
            || (is_tagged_value
                && (is_unqualified(&attribute, "key") || is_unqualified(&attribute, "value")));
        if shorthand {
            return Err(shorthand_attribute(text, &attribute, &written));
        }
        return Err(unread_attribute(text, &attribute, &written));
    }
    Ok(Pending {
        order: found.next_order(),
        parent: Some(parent.id.clone()),
        kind,
        id,
        gid,
        name: None,
        name_id: None,
        content: None,
        key: None,
        declaration,
        flags: start.flags,
        references: start.references,
        written,
        at: node.range().start,
    })
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
    if !found.ids.insert(value) {
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

/// The kind of the child element `node` of a `parent` element, which
/// `parent_what` names (SACM23-XMI-016; XMI 2.5.1 §7.6.3, §7.8.6).
///
/// The child is a role element: in no namespace, and named as a composite
/// role of the parent's class. Its kind is the class its type attributes
/// name, which must agree; without one, it is the role's declared type, if
/// that is concrete. The kind must conform to the role and be one this
/// release reads (ADR 0016 D5, D6). Every refusal is at the child's start
/// tag.
fn child_kind(
    text: &str,
    node: Node<'_, '_>,
    parent: ElementKind,
    parent_what: &str,
) -> Result<ElementKind, ReadError> {
    use ElementKind::{ArgumentPackage, AssuranceCasePackage, Claim};

    let written = written_name(text, node);
    let not_read = |reason: String| unsupported(text, node.range().start, reason);
    if let Some(namespace) = node.tag_name().namespace().filter(|uri| !uri.is_empty()) {
        return Err(not_read(format!(
            "the element {written:?} in {parent_what} is in the namespace {namespace:?}, so it is not read as a role"
        )));
    }
    let Some(role) = role_named(parent, node.tag_name().name()) else {
        return Err(not_read(format!(
            "the element {written:?} in {parent_what} is not a role of {parent}"
        )));
    };
    let mut typed = None;
    for attribute in node.attributes().filter(is_type) {
        let Some(kind) = type_kind(node, attribute.value()) else {
            return Err(not_read(format!(
                "the element {written:?} has the type {:?}, which names no concrete SACM class",
                attribute.value()
            )));
        };
        match typed {
            Some(earlier) if earlier != kind => {
                return Err(not_read(format!(
                    "the element {written:?} has two types, which name {earlier} and {kind}"
                )));
            }
            _ => typed = Some(kind),
        }
    }
    let kind = match (typed, role.declared) {
        (Some(kind), _) | (None, DeclaredType::Kind(kind)) => kind,
        (None, DeclaredType::Abstract(class)) => {
            return Err(not_read(format!(
                "the element {written:?} has no type, and its role has the abstract type {class}"
            )));
        }
    };
    if !role.declared.admits(kind) {
        return Err(not_read(format!(
            "the element {written:?} has the type {kind}, which its role {:?} does not admit",
            role.name
        )));
    }
    // The readable children: the package content above, and the
    // UtilityElements in their own roles, which every ModelElement has (a
    // role was found, so the parent is one). At most one role of a class
    // admits a kind, so the parent and the kind decide the role.
    let readable = kind.is_a(AbstractClass::UtilityElement)
        || matches!(
            (parent, kind),
            (AssuranceCasePackage, AssuranceCasePackage | ArgumentPackage)
                | (ArgumentPackage, ArgumentPackage | Claim)
        );
    if !readable {
        return Err(not_read(format!(
            "the {kind} {written:?} in {parent_what} is not read in this release"
        )));
    }
    Ok(kind)
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
/// content, and each refusal is `Unsupported` at its construct: a second link
/// attribute, any other attribute, a link value that names no element of
/// this document, no link (at the start tag), a child element, or text.
fn read_reference(text: &str, node: Node<'_, '_>) -> Result<ElementId, ReadError> {
    let element = written_name(text, node);
    let mut link = None;
    for attribute in node.attributes() {
        let range = attribute.range_qname();
        let name = text.get(range.clone()).unwrap_or(attribute.name());
        let not_read = |reason: String| unsupported(text, range.start, reason);
        let is_idref = is_xmi(&attribute, "idref");
        if !(is_idref || is_unqualified(&attribute, "href")) {
            return Err(not_read(format!(
                "the attribute {name:?} of the reference element {element:?} is not read: a \
                 reference element carries only xmi:idref or href"
            )));
        }
        if link.is_some() {
            return Err(not_read(format!(
                "the reference element {element:?} carries both xmi:idref and href; give one"
            )));
        }
        let value = attribute.value().trim_matches(is_xml_space);
        let target = if is_idref {
            value
        } else {
            same_document_fragment(value).ok_or_else(|| {
                not_read(format!(
                    "the href {:?} of the reference element {element:?} names no element of \
                     this document by its id; only href=\"#id\" is read, and cross-document \
                     links are not read until SACM23-XMI-027",
                    attribute.value()
                ))
            })?
        };
        if target.is_empty() {
            return Err(not_read(format!(
                "the xmi:idref of the reference element {element:?} is empty, so it names no element"
            )));
        }
        // An `xsd:IDREF` is one token: white space inside it would be read
        // as one id here but written as an IDREFS attribute of several.
        if target.contains(is_xml_space) {
            return Err(not_read(format!(
                "the xmi:idref {:?} of the reference element {element:?} holds white space, \
                 so it is not one id",
                attribute.value()
            )));
        }
        link = Some(ElementId::new(target));
    }
    let Some(target) = link else {
        return Err(unsupported(
            text,
            node.range().start,
            format!(
                "the reference element {element:?} carries neither xmi:idref nor href, so it \
                 names no element"
            ),
        ));
    };
    for child in node.children() {
        if child.is_element() {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in the reference element {element:?} is not read: a \
                     reference element holds no elements",
                    written_name(text, child)
                ),
            ));
        }
        if let Some(first) = text_offset(text, child) {
            return Err(unsupported(
                text,
                first,
                format!(
                    "the reference element {element:?} holds text, which is not read: the \
                     target is given by xmi:idref or href"
                ),
            ));
        }
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

/// The value that the value element `node` holds (SACM23-XMI-014; ADR 0021
/// amendment 1): its character content, joined in document order, exactly.
/// CDATA sections and references give their characters, comments and
/// processing instructions are skipped, and nothing is trimmed; the parser
/// has already ended every line with LF (XML 1.0 §2.11). An empty element is
/// the empty string. Any attribute is `Unsupported` at its name, and any
/// element child at its start tag.
fn read_value(text: &str, node: Node<'_, '_>) -> Result<String, ReadError> {
    let element = written_name(text, node);
    if let Some(attribute) = node.attributes().next() {
        return Err(unread_attribute(text, &attribute, &element));
    }
    let mut value = String::new();
    for child in node.children() {
        if child.is_element() {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in the value element {element:?} is not read: a value \
                     element holds only characters",
                    written_name(text, child)
                ),
            ));
        }
        if child.is_text() {
            value.push_str(child.text().unwrap_or_default());
        }
    }
    Ok(value)
}

/// The `RepeatedValue` refusal of the property `property` of the element
/// `owner`, given more than once in `holder` (SACM-MULT-001; XMI 2.5.1 §9.5.2
/// rule 2h). It gives every occurrence, found in one pass over `holder` in
/// document order and located in one pass over the text: the attribute
/// `local` in no namespace, if it is written, then every value element
/// `local`. For `name`, that is every `name` child in no namespace: a `name`
/// attribute is refused with the start tag, before the content is read, so
/// it is never an occurrence (SACM23-XMI-021).
fn repeated_value(
    text: &str,
    holder: Node<'_, '_>,
    local: &str,
    owner: &ElementId,
    property: &str,
) -> ReadError {
    let mut locator = Locator::new(text);
    let attribute = holder
        .attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == local)
        .map(|attribute| attribute.range_qname().start);
    let elements = holder
        .children()
        .filter(|child| is_value_element(*child, local))
        .map(|child| child.range().start);
    let locations = attribute
        .into_iter()
        .chain(elements)
        .filter_map(|offset| locator.locate(offset))
        .collect();
    ReadError::RepeatedValue {
        element: owner.clone(),
        property: property.to_owned(),
        locations,
    }
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
) -> Result<(LangString, Option<ElementId>), ReadError> {
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
/// duplicates. With no `value`, it is an empty `MultiLangString`. `lang` or
/// `content` on it, and text in it, are the single-language shorthand,
/// refused until LIB-COMPAT-008. An `xsi:type`, and an `xmi:id` read
/// trimmed, are reported on the owner.
fn read_text<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    role: TextRole,
    kind: ElementKind,
    owner: &ElementId,
) -> Result<Text, ReadError> {
    let text = found.text;
    let element = written_name(text, node);
    if let Some(attribute) = node.attributes().find(|attribute| {
        is_type(attribute) && !names_class(node, attribute.value(), "MultiLangString")
    }) {
        return Err(unsupported(
            text,
            node.range().start,
            format!(
                "the element {element:?} has the type {:?}, which names another class than \
                 MultiLangString",
                attribute.value()
            ),
        ));
    }
    let subject = format!("the {} of {}", role.name(), subject(kind, owner.as_str()));
    let mut held = Text::default();
    held.set_id(read_owned_id(found, node, &subject, owner)?);
    for attribute in node.attributes() {
        if is_unqualified(&attribute, "lang") || is_unqualified(&attribute, "content") {
            return Err(shorthand_attribute(text, &attribute, &element));
        } else if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject, owner.as_str());
        } else if !(is_type(&attribute) || is_xmi(&attribute, "id")) {
            return Err(unread_attribute(text, &attribute, &element));
        }
    }
    let value_subject = format!("a value of {subject}");
    let property = format!("{}.value", role.name());
    for child in node.children() {
        if !child.is_element() {
            check_shorthand_text(text, child, &element)?;
        } else if is_value_element(child, "value") {
            let (value, id) = read_lang_string(found, child, &value_subject, owner, &property)?;
            held.push(value, id);
        } else {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in {element:?} is not read: a {} holds only `value` \
                     elements",
                    written_name(text, child),
                    role.name()
                ),
            ));
        }
    }
    Ok(held)
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
/// `xsi:type` is reported on the element `owner`.
fn read_lang_string<'a>(
    found: &mut Findings<'_, 'a>,
    node: Node<'a, '_>,
    subject: &str,
    owner: &ElementId,
    property: &str,
) -> Result<(LangString, Option<ElementId>), ReadError> {
    let text = found.text;
    let at_name = node.range().start;
    let element = written_name(text, node);
    if let Some(namespace) = node.tag_name().namespace().filter(|uri| !uri.is_empty()) {
        return Err(unsupported(
            text,
            at_name,
            format!(
                "the element {element:?} is in the namespace {namespace:?}, so it is not read as a {}",
                node.tag_name().name()
            ),
        ));
    }
    // Like a root's, a type naming another class is refused at the start tag.
    if let Some(attribute) = node
        .attributes()
        .find(|attribute| is_type(attribute) && !names_class(node, attribute.value(), "LangString"))
    {
        return Err(unsupported(
            text,
            at_name,
            format!(
                "the element {element:?} has the type {:?}, which names another class than LangString",
                attribute.value()
            ),
        ));
    }
    let name_id = read_owned_id(found, node, subject, owner)?;
    let mut name = LangString::default();
    for attribute in node.attributes() {
        match (attribute.namespace(), attribute.name()) {
            (None, "lang") => name.set_lang(Some(attribute.value().to_owned())),
            (None, "content") => name.set_content(Some(attribute.value().to_owned())),
            _ if is_xsi_type(&attribute) => found.xsi_type(&attribute, subject, owner.as_str()),
            _ if is_type(&attribute) || is_xmi(&attribute, "id") => {}
            _ => return Err(unread_attribute(text, &attribute, &element)),
        }
    }
    for child in node.children() {
        if !child.is_element() {
            check_text(text, child, &element)?;
            continue;
        }
        let is_lang = is_value_element(child, "lang");
        if !(is_lang || is_value_element(child, "content")) {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in {element:?} is not read in this release",
                    written_name(text, child)
                ),
            ));
        }
        // Checked whole first, then against an earlier occurrence.
        let value = read_value(text, child)?;
        let local = child.tag_name().name();
        let held = if is_lang { name.lang() } else { name.content() };
        if held.is_some() {
            let property = format!("{property}.{local}");
            return Err(repeated_value(text, node, local, owner, &property));
        }
        if is_lang {
            name.set_lang(Some(value));
        } else {
            name.set_content(Some(value));
        }
    }
    Ok((name, name_id))
}

/// Refuses `node` if it is text whose value is not all XML white space
/// (space, TAB, LF, CR), however it is written: a CDATA section or character
/// reference that stands for white space is white space (the XML Infoset).
/// The refusal is at the text's first character in the input that is not
/// literal white space; a CDATA section or reference there is located at its
/// start, so `<![CDATA[x]]>` is refused at the `<![CDATA[`, and `&#xA0;` at
/// the `&`.
fn check_text(text: &str, node: Node<'_, '_>, parent: &str) -> Result<(), ReadError> {
    match text_offset(text, node) {
        Some(first) => Err(unsupported(
            text,
            first,
            format!("the element {parent:?} holds text, which is not read in this release"),
        )),
        None => Ok(()),
    }
}

/// Refuses `node` as [`check_text`] does, in a `UtilityElement` or in a
/// `content`, `key` or `value` of one, where text is the single-language
/// shorthand of a `MultiLangString` (ADR 0025).
fn check_shorthand_text(text: &str, node: Node<'_, '_>, parent: &str) -> Result<(), ReadError> {
    match text_offset(text, node) {
        Some(first) => Err(unsupported(
            text,
            first,
            format!("the element {parent:?} holds text: {SHORTHAND}"),
        )),
        None => Ok(()),
    }
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

/// The `Unsupported` refusal of an attribute that is not read, at its name.
fn unread_attribute(text: &str, attribute: &Attribute<'_, '_>, element: &str) -> ReadError {
    let range = attribute.range_qname();
    let name = text.get(range.clone()).unwrap_or(attribute.name());
    unsupported(
        text,
        range.start,
        format!("the attribute {name:?} of the element {element:?} is not read in this release"),
    )
}

/// The `Unsupported` refusal of an attribute that is the single-language
/// shorthand of a `MultiLangString`, at its name (ADR 0025).
fn shorthand_attribute(text: &str, attribute: &Attribute<'_, '_>, element: &str) -> ReadError {
    let range = attribute.range_qname();
    let name = text.get(range.clone()).unwrap_or(attribute.name());
    unsupported(
        text,
        range.start,
        format!("the attribute {name:?} of the element {element:?} is not read: {SHORTHAND}"),
    )
}

/// An `Unsupported` refusal at byte `offset` of `text`.
fn unsupported(text: &str, offset: usize, reason: String) -> ReadError {
    ReadError::Unsupported {
        location: locate(text, offset),
        reason,
    }
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
            let input = format!("{OPEN}{content}</sacm:ArgumentPackage>");
            let read = read(input.as_bytes(), &ReadOptions::default());
            assert!(read.is_ok(), "{content:?}: {read:?}");
        }
    }

    #[test]
    fn other_text_is_refused_at_its_first_character_that_is_not_literal_white_space() {
        for (content, column) in [
            ("  x", 3),
            ("  &#xA0;", 3),
            ("  <![CDATA[x]]>", 3),
            (" <![CDATA[ ]]>&#x20;x", 2),
            (" <!-- c --> &#9;x", 13),
        ] {
            assert_eq!(
                refusal(content).map(|(location, _)| location),
                Some(SourceLocation::at(2, column)),
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
    fn refusals_name_the_root_or_the_child_element() {
        let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">";
        let cases = [
            (
                format!("{claim}<foo/></argumentElement>"),
                "the element \"foo\" in the element \"argumentElement\" is not a role of Claim",
            ),
            (
                "<argumentElement xmi:id=\"c\"/>".to_owned(),
                "the element \"argumentElement\" has no type, and its role has the abstract type ArgumentationElement",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Note\" xmi:id=\"c\"/>".to_owned(),
                "the element \"argumentElement\" has the type Note, which its role \"argumentElement\" does not admit",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Term\" xsi:type=\"sacm:Claim\" xmi:id=\"c\" \
                 xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>"
                    .to_owned(),
                "the element \"argumentElement\" has two types, which name Term and Claim",
            ),
            (
                "<sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/>".to_owned(),
                "the element \"sacm:argumentElement\" in the root element \"sacm:ArgumentPackage\" is in the namespace \"http://www.omg.org/spec/SACM/20220301\", so it is not read as a role",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Assertion\" xmi:id=\"c\"/>".to_owned(),
                "the element \"argumentElement\" has the type \"sacm:Assertion\", which names no concrete SACM class",
            ),
            (
                "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"c\"/>".to_owned(),
                "the AssertedInference \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is not read in this release",
            ),
        ];
        for (content, reason) in cases {
            assert_eq!(
                refusal(&content).map(|(_, reason)| reason).as_deref(),
                Some(reason),
                "{content:?}"
            );
        }
    }
}
