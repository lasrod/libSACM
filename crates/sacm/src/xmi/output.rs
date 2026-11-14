//! The XMI writer (SACM23-XMI-003, -004, -005, -006, -007, -011, -012, -013,
//! -017, SACM23-BASE-004, -005, -006, -007, -008, LIB-API-010,
//! LIB-COMPAT-007; ADR 0017, ADR 0022, ADR 0025, ADR 0026).
//!
//! Two passes. [`plan`] checks the document (in strict mode, validating it)
//! and every element, and resolves what each is written as; only if that succeeds does [`write`] build the `String` from the plan,
//! so a refusal never leaves partial output, and the output pass has no
//! lookup that could fail. The output form is pinned byte for byte: UTF-8
//! without a BOM, the XML declaration, LF line endings, two spaces per depth,
//! one tag per line and `<x … />` for an element with no children. Neither
//! pass recurses, so deep nesting is safe.

use super::namespaces::{SACM_NS, SACM_PREFIX, XMI_NS, XMI_PREFIX};
use super::{Mode, WriteAttribute, WriteError};
use crate::diagnostic::Severity;
use crate::model::{
    AssertionDeclaration, DeclaredType, Document, ElementId, ElementKind, Flag, LangString,
    ReferenceEnd, SacmElement, Text, TextRole,
};
use crate::validation;

/// Line 1 of every output.
const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// `document` written as XMI in `mode`, or the first reason it cannot be.
pub(super) fn write(document: &Document, mode: Mode) -> Result<String, WriteError> {
    let plan = plan(document, mode)?;
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
        // A gid comes directly after the id (ADR 0025), escaped like it.
        if let Some(gid) = planned.element.gid() {
            out.push_str(" gid=\"");
            escape_into(&mut out, gid);
            out.push('"');
        }
        // Then the SACMElement flags and references, before the class's own
        // attributes (superclass first, Annex B.5.2).
        push_flags_and_references(&mut out, planned.element);
        if let Some(declaration) = planned.declaration {
            out.push_str(" assertionDeclaration=\"");
            out.push_str(declaration.as_str());
            out.push('"');
        }
        let name = planned.element.name();
        let texts =
            [TextRole::Content, TextRole::Key].map(|role| (role, planned.element.text(role)));
        let has_texts = texts.iter().any(|(_, text)| text.is_some());
        if name.is_some() || has_texts || planned.element.children().next().is_some() {
            out.push_str(">\n");
            if let Some(name) = name {
                push_lang_string(&mut out, depth + 1, "name", name, planned.element.name_id());
            }
            // `content` before `key`, superclass first (XMI 2.5.1 Annex
            // B.5.2).
            for (role, text) in texts {
                if let Some(text) = text {
                    push_text(&mut out, depth + 1, role, text);
                }
            }
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
    /// Its `assertionDeclaration`, if it is an Assertion not declared
    /// `asserted`, the default, which is not written (XMI 2.5.1 §9.4.1).
    declaration: Option<AssertionDeclaration>,
}

/// The element name an element is written under.
enum Tag {
    /// A root: `sacm:<Kind>`.
    Root(ElementKind),
    /// A child: the name of the role of its parent's class that holds it.
    Role(&'static str),
}

/// Checks `document` in ADR 0022's order, a root first, then, in strict
/// `mode`, validation, then each element in document order (see
/// [`unrepresentable`]), and returns every element in that order with what it
/// is written as. Everything a document holds is written, so these are the
/// only refusals.
fn plan(document: &Document, mode: Mode) -> Result<Vec<Planned<'_>>, WriteError> {
    if document.roots().next().is_none() {
        return Err(WriteError::NoRoot);
    }
    if mode == Mode::Strict {
        // Every Error, in `validate`'s order (ADR 0022).
        let diagnostics: Vec<_> = validation::validate(document)
            .into_iter()
            .filter(|found| found.severity() == Severity::Error)
            .collect();
        if !diagnostics.is_empty() {
            return Err(WriteError::Invalid { diagnostics });
        }
    }
    let mut plan = Vec::new();
    for element in document.elements() {
        if let Some((attribute, character)) = unrepresentable(element) {
            return Err(WriteError::UnrepresentableCharacter {
                element: element.id().clone(),
                character,
                attribute,
            });
        }
        let kind = element.kind();
        // A child keeps the role it was added in; a root has none.
        let (tag, written_type) = match element.role() {
            None => (Tag::Root(kind), None),
            Some(role) => {
                let written_type = match role.declared {
                    DeclaredType::Kind(declared) if declared == kind => None,
                    DeclaredType::Kind(_) | DeclaredType::Abstract(_) => Some(kind),
                };
                (Tag::Role(role.name), written_type)
            }
        };
        let declaration = element
            .assertion_declaration()
            .filter(|&declaration| declaration != AssertionDeclaration::default());
        plan.push(Planned {
            element,
            tag,
            written_type,
            declaration,
        });
    }
    Ok(plan)
}

/// The first value `element` holds that XML 1.0 cannot represent, as the
/// attribute that holds it and its first such character, checked in this
/// order: its id, its gid, its name's `lang` and `content`, then each value
/// of its `content` and then of its `key`, `lang` before `content`. The ids
/// of a name, `content`, `key` or `value`, and the ids a reference holds, are
/// not checked: only [`xmi::read`](crate::xmi::read) gives them, and what it
/// reads is representable.
fn unrepresentable(element: SacmElement<'_>) -> Option<(WriteAttribute, char)> {
    let name = element.name();
    let fields = [
        (WriteAttribute::Id, Some(element.id().as_str())),
        (WriteAttribute::Gid, element.gid()),
        (WriteAttribute::NameLang, name.and_then(LangString::lang)),
        (
            WriteAttribute::NameContent,
            name.and_then(LangString::content),
        ),
    ];
    let texts = [
        (
            TextRole::Content,
            WriteAttribute::ContentValueLang,
            WriteAttribute::ContentValueContent,
        ),
        (
            TextRole::Key,
            WriteAttribute::KeyValueLang,
            WriteAttribute::KeyValueContent,
        ),
    ];
    let values = texts.into_iter().flat_map(move |(role, lang, content)| {
        element
            .text(role)
            .map_or(&[][..], |text| text.value().values())
            .iter()
            .flat_map(move |value| [(lang, value.lang()), (content, value.content())])
    });
    fields
        .into_iter()
        .chain(values)
        .find_map(|(attribute, value)| {
            value
                .and_then(|value| value.chars().find(|&c| !is_xml_char(c)))
                .map(|character| (attribute, character))
        })
}

/// Whether `c` is an XML 1.0 `Char` (§2.2). A Rust `char` is never a
/// surrogate.
fn is_xml_char(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | ' '..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// Writes the `SACMElement` flags of `element` and its references as
/// attributes, each after a space, in table order (ADR 0026): a flag only
/// when true, as `"true"`, and a reference end only when it holds an id, as
/// its ids separated by one space, each escaped (`xsd:IDREFS`, XMI 2.5.1
/// rule 2j).
fn push_flags_and_references(out: &mut String, element: SacmElement<'_>) {
    for flag in Flag::ALL {
        if element.flag(flag) {
            out.push(' ');
            out.push_str(flag.name());
            out.push_str("=\"true\"");
        }
    }
    for end in ReferenceEnd::ALL {
        let targets = element.references(end);
        if targets.is_empty() {
            continue;
        }
        out.push(' ');
        out.push_str(end.name());
        out.push_str("=\"");
        for (position, target) in targets.iter().enumerate() {
            if position > 0 {
                out.push(' ');
            }
            escape_into(out, target.as_str());
        }
        out.push('"');
    }
}

/// Writes `value` as a `tag` element on its own line at `depth`: its `id`,
/// as `xmi:id`, then `lang`, then `content`, each only if present (ADR 0017,
/// ADR 0024): a `name`, or a `value` of a `content` or `key`.
fn push_lang_string(
    out: &mut String,
    depth: usize,
    tag: &str,
    value: &LangString,
    id: Option<&ElementId>,
) {
    indent(out, depth);
    out.push('<');
    out.push_str(tag);
    push_id(out, id);
    for (attribute, value) in [("lang", value.lang()), ("content", value.content())] {
        if let Some(value) = value {
            out.push(' ');
            out.push_str(attribute);
            out.push_str("=\"");
            escape_into(out, value);
            out.push('"');
        }
    }
    out.push_str(" />\n");
}

/// Writes `text` as its `role` element at `depth`, with its `xmi:id` first,
/// and one `value` element per value under it, in order; with no values,
/// one empty element (ADR 0025).
fn push_text(out: &mut String, depth: usize, role: TextRole, text: &Text) {
    indent(out, depth);
    out.push('<');
    out.push_str(role.name());
    push_id(out, text.id());
    let values = text.value().values();
    if values.is_empty() {
        out.push_str(" />\n");
        return;
    }
    out.push_str(">\n");
    for (value, id) in values.iter().zip(text.value_ids()) {
        push_lang_string(out, depth + 1, "value", value, id.as_ref());
    }
    indent(out, depth);
    out.push_str("</");
    out.push_str(role.name());
    out.push_str(">\n");
}

/// Writes ` xmi:id="<id>"`, escaped, if there is an id.
fn push_id(out: &mut String, id: Option<&ElementId>) {
    if let Some(id) = id {
        push_attribute_name(out, "id");
        escape_into(out, id.as_str());
        out.push('"');
    }
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

/// Writes the namespace declarations of the document element, each after a
/// space. No `xmi:version` is written: the XMI namespace identifies the
/// version (XMI 2.5.1 §7.5.3; ADR 0017 amendment 3).
fn push_document_attributes(out: &mut String) {
    for (prefix, uri) in [(SACM_PREFIX, SACM_NS), (XMI_PREFIX, XMI_NS)] {
        out.push_str(" xmlns:");
        out.push_str(prefix);
        out.push_str("=\"");
        out.push_str(uri);
        out.push('"');
    }
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
    use super::{escape_into, is_xml_char, write};
    use crate::model::{Document, ElementId, ElementKind, LangString, MultiLangString};
    use crate::xmi::{Mode, WriteAttribute, WriteError};

    /// The attribute and character a tolerant write of `document` refuses.
    fn refusal(document: &Document) -> Option<(WriteAttribute, char)> {
        match write(document, Mode::Tolerant) {
            Err(WriteError::UnrepresentableCharacter {
                attribute,
                character,
                ..
            }) => Some((attribute, character)),
            _ => None,
        }
    }

    /// A root `id` named `name` with the gid `gid`.
    fn root(id: &str, name: &str, gid: &str) -> Document {
        let mut document = Document::new();
        let id = ElementId::new(id);
        let named = Some(LangString::new(name));
        let inserted = document.insert_node(None, ElementKind::ArgumentPackage, id.clone(), named);
        assert_eq!(inserted, Ok(()));
        assert_eq!(document.set_gid(&id, Some(gid.to_owned())), Ok(()));
        document
    }

    /// No read input can hold a gid XML cannot represent, so the check is
    /// pinned here: after the id, before the name (ADR 0025).
    #[test]
    fn a_gid_xml_cannot_represent_is_refused_after_the_id_and_before_the_name() {
        assert_eq!(
            refusal(&root("p", "P", "g\u{1}")),
            Some((WriteAttribute::Gid, '\u{1}'))
        );
        assert_eq!(
            refusal(&root("p", "P\u{2}", "g\u{FFFE}")),
            Some((WriteAttribute::Gid, '\u{FFFE}'))
        );
        assert_eq!(
            refusal(&root("p\u{3}", "P", "g\u{1}")),
            Some((WriteAttribute::Id, '\u{3}'))
        );
        let written = write(&root("p", "P", "a\tb"), Mode::Strict);
        assert!(
            written
                .as_deref()
                .is_ok_and(|text| text.contains(" xmi:id=\"p\" gid=\"a&#9;b\">")),
            "{written:?}"
        );
    }

    /// A `TaggedValue`'s content is checked before its key, as it is written.
    #[test]
    fn a_content_value_is_checked_before_a_key_value() {
        let mut document = root("p", "P", "g");
        let bad = |content: &str| MultiLangString::from(LangString::new(content));
        let inserted = document.insert_element(
            Some(&ElementId::new("p")),
            ElementKind::TaggedValue,
            ElementId::new("t"),
            None,
            Some(bad("v\u{4}")),
            Some(bad("k\u{5}")),
        );
        assert_eq!(inserted, Ok(()));
        assert_eq!(
            refusal(&document),
            Some((WriteAttribute::ContentValueContent, '\u{4}'))
        );
    }

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
