//! The interchange roots of an XMI document, their names and the package
//! content under them (SACM23-XMI-001, SACM23-XMI-012, SACM23-XMI-016,
//! SACM23-XMI-017; formal/23-05-08 §2.2, §2.4, §11.10; XMI 2.5.1 §7.5.3,
//! §7.6.3, §7.8.6; ADR 0015, ADR 0016, ADR 0017 amendments 1 and 2).
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

use super::input::{is_xml_space, locate};
use super::namespaces::{SACM_NS, XMI_NS};
use super::{Mode, ReadError};
use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, DeclaredType, Document, ElementId, ElementKind,
    LangString, role_named,
};

/// The XML Schema instance namespace, of `xsi:type`, which is read but never
/// written (ADR 0017 T1).
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The requirement the read diagnostics enforce.
const MODES_ROW: &str = "LIB-COMPAT-001";

/// The document whose roots `tree` holds, read in `mode`, and the
/// diagnostics found, unsorted. `text` is the text `tree` was parsed from.
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
        diagnostics: Vec::new(),
    };
    if is_wrapper {
        read_wrapper(&mut found, element, &mut document)?;
    } else {
        read_root(&mut found, element, &mut document)?;
    }
    Ok((document, found.diagnostics))
}

/// What pass 2 reads with: the input text, the mode, and the diagnostics
/// found so far.
struct Findings<'t> {
    text: &'t str,
    mode: Mode,
    diagnostics: Vec<Diagnostic>,
}

impl Findings<'_> {
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
             version, and write does not write it",
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
        let diagnostic = match locate(self.text, attribute.range_qname().start) {
            Some(location) => diagnostic.set_location(location),
            None => diagnostic,
        };
        self.diagnostics.push(diagnostic);
    }
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
fn read_wrapper(
    found: &mut Findings<'_>,
    wrapper: Node<'_, '_>,
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
struct Pending<'a> {
    /// The element's place in document order within the tree.
    order: usize,
    /// The id of the element's parent; `None` for the root.
    parent: Option<&'a str>,
    kind: ElementKind,
    id: &'a str,
    name: Option<LangString>,
    /// The `assertionDeclaration` read, if the attribute is written.
    declaration: Option<AssertionDeclaration>,
    /// The element's name as written in its start tag.
    written: String,
    /// The byte offset of its start tag.
    at: usize,
}

impl Pending<'_> {
    /// How refusals name the element.
    fn what(&self) -> String {
        let element = if self.parent.is_none() {
            "root element"
        } else {
            "element"
        };
        format!("the {element} {:?}", self.written)
    }
}

/// Pass 2 for one root: checks it and its tree in document order, then adds
/// it to `document` as a parentless element, with its tree under it in
/// document order (ADR 0010 D2). The tree is walked with a stack, not by
/// recursion, so a deep document cannot exhaust the call stack.
fn read_root(
    found: &mut Findings<'_>,
    root: Node<'_, '_>,
    document: &mut Document,
) -> Result<(), ReadError> {
    let text = found.text;
    // The ids read so far in this root's tree; earlier roots' trees are in
    // `document`.
    let mut ids = BTreeSet::new();
    let first = root_start(found, root, document)?;
    ids.insert(first.id);
    // The elements whose content is being read, innermost last, each with
    // the rest of its content, and the elements read whole. Content is one
    // `name` (SACM23-XMI-012), readable children (SACM23-XMI-016), white
    // space, comments and processing instructions.
    let mut open = vec![(first, root.children())];
    let mut read = Vec::new();
    while let Some((element, content)) = open.last_mut() {
        let Some(node) = content.next() else {
            read.extend(open.pop().map(|(element, _)| element));
            continue;
        };
        if !node.is_element() {
            check_text(text, node, &element.written)?;
        } else if node.tag_name().name() == "name" {
            if element.name.is_some() {
                return Err(unsupported(
                    text,
                    node.range().start,
                    format!("{} has a second \"name\"", element.what()),
                ));
            }
            element.name = Some(read_name(found, node, element.kind, element.id)?);
        } else {
            let child = child_start(found, node, element, document, &ids)?;
            ids.insert(child.id);
            open.push((child, node.children()));
        }
    }

    // Everything is checked, so the inserts succeed; a failure would still
    // be a refusal, never a panic. In document order, a parent comes before
    // its children.
    read.sort_by_key(|element| element.order);
    for element in read {
        let what = element.what();
        let id = ElementId::new(element.id);
        let held = document
            .insert_node(
                element.parent.map(ElementId::new).as_ref(),
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
        declared.map_err(|error| {
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
fn root_start<'a>(
    found: &mut Findings<'_>,
    root: Node<'a, '_>,
    document: &Document,
) -> Result<Pending<'a>, ReadError> {
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
    let id = read_id(text, root, &what, document, &BTreeSet::new())?;
    for attribute in root.attributes() {
        if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject(kind, id), id);
        } else if is_xmi(&attribute, "version") {
            found.xmi_version(&attribute, &subject(kind, id), Some(id));
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

    // Any other attribute than `xmi:id`, `xmi:version` and the type is not
    // read yet (ADR 0016 D4).
    if let Some(attribute) = root.attributes().find(|attribute| {
        !(is_xmi(attribute, "id") || is_xmi(attribute, "version") || is_type(attribute))
    }) {
        return Err(unread_attribute(text, &attribute, &name));
    }

    Ok(Pending {
        order: 0,
        parent: None,
        kind,
        id,
        name: None,
        declaration: None,
        written: name,
        at: at_root,
    })
}

/// Checks the start tag of `node`, a child element of `parent`: its role,
/// type and kind, its id, and its other attributes, in that order, reading
/// an Assertion's `assertionDeclaration`. `ids` holds the ids read so far in
/// the root's tree.
fn child_start<'a>(
    found: &mut Findings<'_>,
    node: Node<'a, '_>,
    parent: &Pending<'a>,
    document: &Document,
    ids: &BTreeSet<&str>,
) -> Result<Pending<'a>, ReadError> {
    let text = found.text;
    let kind = child_kind(text, node, parent.kind, &parent.what())?;
    let written = written_name(text, node);
    let what = format!("the element {written:?}");
    let id = read_id(text, node, &what, document, ids)?;
    // Besides `xmi:id` and the type (ADR 0016 D4), only an Assertion's
    // unqualified `assertionDeclaration`, in document order, so a refusal
    // after it lands on the later attribute.
    let is_assertion = kind.is_a(AbstractClass::Assertion);
    let mut declaration = None;
    for attribute in node.attributes() {
        if is_xsi_type(&attribute) {
            found.xsi_type(&attribute, &subject(kind, id), id);
            continue;
        }
        if is_xmi(&attribute, "id") || is_type(&attribute) {
            continue;
        }
        if is_assertion
            && attribute.namespace().is_none()
            && attribute.name() == "assertionDeclaration"
        {
            declaration = Some(read_declaration(text, &attribute, &written)?);
            continue;
        }
        return Err(unread_attribute(text, &attribute, &written));
    }
    Ok(Pending {
        // Each element read before it has added one id.
        order: ids.len(),
        parent: Some(parent.id),
        kind,
        id,
        name: None,
        declaration,
        written,
        at: node.range().start,
    })
}

/// The `xmi:id` of `node` (XMI 2.5.1 §7.6.1). It must be non-empty and
/// unlike every id read before it, in `document` or in `ids`. Refusals are
/// at `node`, which `what` names.
fn read_id<'a>(
    text: &str,
    node: Node<'a, '_>,
    what: &str,
    document: &Document,
    ids: &BTreeSet<&str>,
) -> Result<&'a str, ReadError> {
    let not_read = |reason: String| unsupported(text, node.range().start, reason);
    match node.attributes().find(|attribute| is_xmi(attribute, "id")) {
        None => Err(not_read(format!("{what} has no \"xmi:id\""))),
        Some(id) if id.value().is_empty() => {
            Err(not_read(format!("{what} has an empty \"xmi:id\"")))
        }
        Some(id) if document.contains(id.value()) || ids.contains(id.value()) => {
            Err(not_read(format!(
                "{what} has the \"xmi:id\" {:?} of an earlier element",
                id.value()
            )))
        }
        Some(id) => Ok(id.value()),
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
    // The readable children. At most one role of a class admits a kind, so
    // the parent and the kind decide the role.
    if !matches!(
        (parent, kind),
        (AssuranceCasePackage, AssuranceCasePackage | ArgumentPackage)
            | (ArgumentPackage, ArgumentPackage | Claim)
    ) {
        return Err(not_read(format!(
            "the {kind} {written:?} in {parent_what} is not read in this release"
        )));
    }
    Ok(kind)
}

/// The `assertionDeclaration` that `attribute`, of the element written as
/// `element`, names (SACM23-XMI-017; §11.10; ADR 0017 amendment 2). Its value,
/// as XML attribute-value normalisation gives it, must be one of the six
/// literals exactly: nothing is trimmed or case-folded (XMI 2.5.1 §8.2.1 rule
/// 8). Any other value is refused at the attribute's name, never read as
/// `asserted` (ADR 0015, until SACM23-VAL-006).
fn read_declaration(
    text: &str,
    attribute: &Attribute<'_, '_>,
    element: &str,
) -> Result<AssertionDeclaration, ReadError> {
    let value = attribute.value();
    value.parse().map_err(|_| {
        unsupported(
            text,
            attribute.range_qname().start,
            format!(
                "the attribute \"assertionDeclaration\" of the element {element:?} has the value {value:?}, which is not an AssertionDeclaration literal"
            ),
        )
    })
}

/// The `LangString` that the `name` element `node` holds (SACM23-XMI-011,
/// SACM23-XMI-012; ADR 0017 amendment 1). It must be in no namespace, carry
/// only `lang`, `content` and a type naming `LangString`, and hold only white
/// space, comments and processing instructions. The values are kept as the
/// parser gives them, untrimmed. An `xsi:type` is reported on the owner: the
/// element `owner`, of `kind`.
fn read_name(
    found: &mut Findings<'_>,
    node: Node<'_, '_>,
    kind: ElementKind,
    owner: &str,
) -> Result<LangString, ReadError> {
    let text = found.text;
    let at_name = node.range().start;
    let element = written_name(text, node);
    if let Some(namespace) = node.tag_name().namespace().filter(|uri| !uri.is_empty()) {
        return Err(unsupported(
            text,
            at_name,
            format!(
                "the element {element:?} is in the namespace {namespace:?}, so it is not read as a name"
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
    let mut name = LangString::default();
    for attribute in node.attributes() {
        match (attribute.namespace(), attribute.name()) {
            (None, "lang") => name.set_lang(Some(attribute.value().to_owned())),
            (None, "content") => name.set_content(Some(attribute.value().to_owned())),
            _ if is_xsi_type(&attribute) => {
                let subject = format!("the name of {}", subject(kind, owner));
                found.xsi_type(&attribute, &subject, owner);
            }
            _ if is_type(&attribute) => {}
            _ => return Err(unread_attribute(text, &attribute, &element)),
        }
    }
    for child in node.children() {
        if child.is_element() {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in {element:?} is not read in this release",
                    written_name(text, child)
                ),
            ));
        }
        check_text(text, child, &element)?;
    }
    Ok(name)
}

/// Refuses `node` if it is text whose value is not all XML white space
/// (space, TAB, LF, CR), however it is written: a CDATA section or character
/// reference that stands for white space is white space (the XML Infoset).
/// The refusal is at the text's first character in the input that is not
/// literal white space; a CDATA section or reference there is located at its
/// start, so `<![CDATA[x]]>` is refused at the `<![CDATA[`, and `&#xA0;` at
/// the `&`.
fn check_text(text: &str, node: Node<'_, '_>, parent: &str) -> Result<(), ReadError> {
    let Some(content) = node.text().filter(|_| node.is_text()) else {
        return Ok(());
    };
    if content.chars().all(is_xml_space) {
        return Ok(());
    }
    // roxmltree gives the range of the text's first piece only, so the
    // first character that is not literal white space is looked for in the
    // input.
    let start = node.range().start;
    let first = text
        .get(start..)
        .and_then(|rest| rest.find(|c: char| !is_xml_space(c)))
        .map_or(start, |skipped| start + skipped);
    Err(unsupported(
        text,
        first,
        format!("the element {parent:?} holds text, which is not read in this release"),
    ))
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

    #[test]
    fn refusals_name_the_root_or_the_child_element() {
        let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">";
        let cases = [
            (
                "<name/><name/>".to_owned(),
                "the root element \"sacm:ArgumentPackage\" has a second \"name\"",
            ),
            (
                format!("{claim}<name/><name/></argumentElement>"),
                "the element \"argumentElement\" has a second \"name\"",
            ),
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
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"p\"/>".to_owned(),
                "the element \"argumentElement\" has the \"xmi:id\" \"p\" of an earlier element",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Assertion\" xmi:id=\"c\"/>".to_owned(),
                "the element \"argumentElement\" has the type \"sacm:Assertion\", which names no concrete SACM class",
            ),
            (
                "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"c\"/>".to_owned(),
                "the AssertedInference \"argumentElement\" in the root element \"sacm:ArgumentPackage\" is not read in this release",
            ),
            (
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\" assertionDeclaration=\"Assumed\"/>"
                    .to_owned(),
                "the attribute \"assertionDeclaration\" of the element \"argumentElement\" has the value \"Assumed\", which is not an AssertionDeclaration literal",
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
