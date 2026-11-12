//! The interchange roots of an XMI document (SACM23-XMI-001; formal/23-05-08
//! §2.2, §2.4; XMI 2.5.1 §7.5.3; ADR 0015, ADR 0016).
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
//!    document. A root's namespace and kind, its `xmi:id` and its type are
//!    checked first, all located at the root, then its other attributes, its
//!    content, and whether an earlier root has its id. The first thing this
//!    release cannot read is `Unsupported`, at its location; nothing is
//!    dropped (ADR 0015, D-PRESERVE).
//!
//! Names are matched by namespace URI and local name, and a type value is
//! resolved as a `QName` against the namespaces in scope, so the choice of
//! prefix does not matter (SACM23-XMI-002). An element under `xmlns=""` is
//! in no namespace.
//!
//! The XMI element itself may carry only namespace declarations and
//! `xmi:version`, and hold, besides its roots, only white space, comments and
//! processing instructions.

use roxmltree::{Attribute, Node};

use super::ReadError;
use super::input::{is_xml_space, locate};
use super::namespaces::{SACM_NS, XMI_NS};
use crate::model::{Document, ElementId, ElementKind, InsertError};

/// The XML Schema instance namespace, of `xsi:type`, which is read but never
/// written (ADR 0017 T1).
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// The document whose roots `tree` holds. `text` is the text `tree` was
/// parsed from.
pub(super) fn read(text: &str, tree: &roxmltree::Document<'_>) -> Result<Document, ReadError> {
    let element = tree.root_element();
    let is_wrapper = classify(text, element)?;
    let mut document = Document::new();
    if is_wrapper {
        read_wrapper(text, element, &mut document)?;
    } else {
        read_root(text, element, &mut document)?;
    }
    Ok(document)
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
    text: &str,
    wrapper: Node<'_, '_>,
    document: &mut Document,
) -> Result<(), ReadError> {
    let name = written_name(text, wrapper);
    if let Some(attribute) = wrapper
        .attributes()
        .find(|attribute| !is_xmi(attribute, "version"))
    {
        return Err(unread_attribute(text, &attribute, &name));
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
            read_root(text, child, document)?;
        }
    }
    Ok(())
}

/// Pass 2 for one root: checks it, then adds it to `document` as a
/// parentless element.
fn read_root(text: &str, root: Node<'_, '_>, document: &mut Document) -> Result<(), ReadError> {
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

    // The id (XMI 2.5.1 §7.6.1).
    let id = match root.attributes().find(|attribute| is_xmi(attribute, "id")) {
        Some(id) if !id.value().is_empty() => id.value(),
        Some(_) => {
            return Err(not_read(format!(
                "the root element {name:?} has an empty \"xmi:id\""
            )));
        }
        None => {
            return Err(not_read(format!(
                "the root element {name:?} has no \"xmi:id\""
            )));
        }
    };

    // A type must name the root's own class (ADR 0016 D2). Like the kind and
    // the id, it is refused at the root.
    if let Some(attribute) = root
        .attributes()
        .find(|attribute| is_type(attribute) && !names_class(root, attribute.value(), kind))
    {
        return Err(not_read(format!(
            "the root element {name:?} has the type {:?}, which names another class",
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

    // The content: white space, comments and processing instructions only.
    for child in root.children() {
        if child.is_element() {
            return Err(unsupported(
                text,
                child.range().start,
                format!(
                    "the element {:?} in the root element {name:?} is not read in this release",
                    written_name(text, child)
                ),
            ));
        }
        check_text(text, child, &name)?;
    }

    document
        .insert_node(None, kind, ElementId::new(id), None)
        .map_err(|error| match error {
            InsertError::DuplicateId(id) => not_read(format!(
                "the root element {name:?} has the \"xmi:id\" {:?} of an earlier root",
                id.as_str()
            )),
            other => not_read(format!(
                "the root element {name:?} cannot be held in the document: {other}"
            )),
        })
}

/// Refuses `node` if it is text that is not all white space, at its first
/// character in the input that is not literal white space. A CDATA section
/// or character reference counts as such a character even when it stands for
/// white space, so `<![CDATA[ ]]>x` is refused at the `<![CDATA[`, and
/// `&#x20;x` at the `&`.
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
    is_xmi(attribute, "type") || attribute.namespace() == Some(XSI_NS) && attribute.name() == "type"
}

/// Whether the type `value`, a `QName` resolved in the scope of `root`, names
/// the SACM class of `kind`. The value's leading and trailing white space is
/// removed first, as a `QName`'s is collapsed (XML Schema 1.0 Part 2 §4.3.6);
/// white space inside it stays, so it names no class.
fn names_class(root: Node<'_, '_>, value: &str, kind: ElementKind) -> bool {
    let value = value.trim_matches(is_xml_space);
    let (prefix, local) = match value.split_once(':') {
        Some((prefix, local)) => (Some(prefix), local),
        None => (None, value),
    };
    local == kind.name() && root.lookup_namespace_uri(prefix) == Some(SACM_NS)
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
