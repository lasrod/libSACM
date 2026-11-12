//! The XMI writer (SACM23-XMI-003, -004, -006, -007, -013, LIB-API-010;
//! ADR 0017).
//!
//! Two passes. [`plan`] checks every element and resolves what it is written
//! as; only if that succeeds does [`write`] build the `String` from the plan,
//! so a refusal never leaves partial output, and the output pass has no
//! lookup that could fail. The output form is pinned byte for byte: UTF-8
//! without a BOM, the XML declaration, LF line endings, two spaces per depth,
//! one tag per line and `<x … />` for an element with no children. Neither
//! pass recurses, so deep nesting is safe.

use super::WriteError;
use super::namespaces::{SACM_NS, SACM_PREFIX, XMI_NS, XMI_PREFIX, XMI_VERSION};
use crate::model::{AssertionDeclaration, DeclaredType, Document, ElementKind, SacmElement, role};

/// Line 1 of every output.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// `document` written as XMI, or the first reason it cannot be.
pub(super) fn write(document: &Document) -> Result<String, WriteError> {
    let plan = plan(document)?;
    let wrapped = document.roots().nth(1).is_some();
    let mut out = String::new();
    out.push_str(DECLARATION);
    out.push('\n');
    let base = if wrapped {
        out.push('<');
        push_wrapper_name(&mut out);
        push_document_attributes(&mut out);
        out.push_str(">\n");
        1
    } else {
        0
    };

    // The elements whose start tag is written and whose end tag is not, the
    // innermost last. The plan is in pre-order, so an element's parent is
    // on this stack when the element comes.
    let mut open: Vec<&Planned<'_>> = Vec::new();
    for planned in &plan {
        let parent = planned.element.parent().map(SacmElement::id);
        while let Some(&top) = open.last() {
            if Some(top.element.id()) == parent {
                break;
            }
            open.pop();
            push_end_tag(&mut out, base + open.len(), &top.tag);
        }
        let depth = base + open.len();
        indent(&mut out, depth);
        out.push('<');
        push_tag(&mut out, &planned.tag);
        if depth == 0 {
            push_document_attributes(&mut out);
        }
        if let Some(kind) = planned.written_type {
            push_attribute_name(&mut out, "type");
            out.push_str(SACM_PREFIX);
            out.push(':');
            out.push_str(kind.name());
            out.push('"');
        }
        push_attribute_name(&mut out, "id");
        escape_into(&mut out, planned.element.id().as_str());
        out.push('"');
        if planned.element.children().next().is_some() {
            out.push_str(">\n");
            open.push(planned);
        } else {
            out.push_str(" />\n");
        }
    }
    while let Some(top) = open.pop() {
        push_end_tag(&mut out, base + open.len(), &top.tag);
    }

    if wrapped {
        out.push_str("</");
        push_wrapper_name(&mut out);
        out.push_str(">\n");
    }
    Ok(out)
}

/// One element, checked, with what it is written as.
struct Planned<'a> {
    /// The element.
    element: SacmElement<'a>,
    /// Its element name.
    tag: Tag,
    /// The class written as its `xmi:type`, if one is.
    written_type: Option<ElementKind>,
}

/// The element name an element is written under.
enum Tag {
    /// A root: `sacm:<Kind>`.
    Root(ElementKind),
    /// A child: the name of the role of its parent's class that holds it.
    Role(&'static str),
}

/// Checks `document` in ADR 0017's order, a root first, then each element in
/// document order (its id, its name, its `assertionDeclaration`, its role),
/// and returns every element in that order with what it is written as.
fn plan(document: &Document) -> Result<Vec<Planned<'_>>, WriteError> {
    if document.roots().next().is_none() {
        return Err(WriteError::NoRoot);
    }
    let mut plan = Vec::new();
    for element in document.elements() {
        let id = element.id();
        let unsupported = |reason: String| WriteError::Unsupported {
            element: id.clone(),
            reason,
        };
        if let Some(character) = id.as_str().chars().find(|&c| !is_xml_char(c)) {
            return Err(WriteError::UnrepresentableCharacter {
                element: id.clone(),
                character,
            });
        }
        // Names are written by SACM23-XMI-012.
        if element.name().is_some() {
            return Err(unsupported("its name is not written yet".to_owned()));
        }
        // Declarations are written by SACM23-XMI-017.
        if let Some(declaration) = element
            .assertion_declaration()
            .filter(|&declaration| declaration != AssertionDeclaration::default())
        {
            return Err(unsupported(format!(
                "its assertionDeclaration {:?} is not written yet",
                declaration.as_str()
            )));
        }
        let kind = element.kind();
        let (tag, written_type) = match element.parent() {
            None => (Tag::Root(kind), None),
            // A document holds a child only in a role that admits it, so
            // this refusal is not reached; it stands in for a panic.
            Some(parent) => match role(parent.kind(), kind) {
                None => {
                    return Err(unsupported(
                        "it is held in no role of its parent's class".to_owned(),
                    ));
                }
                Some(role) => {
                    let written_type = match role.declared {
                        DeclaredType::Kind(declared) if declared == kind => None,
                        DeclaredType::Kind(_) | DeclaredType::Abstract(_) => Some(kind),
                    };
                    (Tag::Role(role.name), written_type)
                }
            },
        };
        plan.push(Planned {
            element,
            tag,
            written_type,
        });
    }
    Ok(plan)
}

/// Whether `c` is an XML 1.0 `Char` (§2.2). A Rust `char` is never a
/// surrogate.
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | ' '..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// Writes the element name `tag`.
fn push_tag(out: &mut String, tag: &Tag) {
    match tag {
        Tag::Root(kind) => {
            out.push_str(SACM_PREFIX);
            out.push(':');
            out.push_str(kind.name());
        }
        Tag::Role(name) => out.push_str(name),
    }
}

/// Writes the end tag of `tag` on its own line at `depth`.
fn push_end_tag(out: &mut String, depth: usize, tag: &Tag) {
    indent(out, depth);
    out.push_str("</");
    push_tag(out, tag);
    out.push_str(">\n");
}

/// Writes `xmi:XMI`.
fn push_wrapper_name(out: &mut String) {
    out.push_str(XMI_PREFIX);
    out.push_str(":XMI");
}

/// Writes the namespace declarations and `xmi:version` of the document
/// element, each after a space.
fn push_document_attributes(out: &mut String) {
    for (prefix, uri) in [(SACM_PREFIX, SACM_NS), (XMI_PREFIX, XMI_NS)] {
        out.push_str(" xmlns:");
        out.push_str(prefix);
        out.push_str("=\"");
        out.push_str(uri);
        out.push('"');
    }
    push_attribute_name(out, "version");
    out.push_str(XMI_VERSION);
    out.push('"');
}

/// Writes ` xmi:<local>="`, up to the value.
fn push_attribute_name(out: &mut String, local: &str) {
    out.push(' ');
    out.push_str(XMI_PREFIX);
    out.push(':');
    out.push_str(local);
    out.push_str("=\"");
}

/// Writes two spaces per `depth`.
fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// Writes `value` as the content of a double-quoted attribute value: `&`,
/// `<`, `>`, `"`, TAB, LF and CR escaped, and everything else as it is.
fn escape_into(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_into, is_xml_char};

    #[test]
    fn xml_char_bounds() {
        for c in [
            '\t',
            '\n',
            '\r',
            ' ',
            '\u{D7FF}',
            '\u{E000}',
            '\u{FFFD}',
            '\u{10000}',
            '\u{10FFFF}',
        ] {
            assert!(is_xml_char(c), "U+{:04X}", u32::from(c));
        }
        for c in [
            '\0', '\u{8}', '\u{B}', '\u{C}', '\u{1F}', '\u{FFFE}', '\u{FFFF}',
        ] {
            assert!(!is_xml_char(c), "U+{:04X}", u32::from(c));
        }
    }

    #[test]
    fn escaping_covers_the_pinned_characters_only() {
        let mut out = String::new();
        escape_into(&mut out, "&<>\"'\t\n\r\u{e9}");
        assert_eq!(out, "&amp;&lt;&gt;&quot;'&#9;&#10;&#13;\u{e9}");
    }
}
