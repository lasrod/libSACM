//! Preserved content as the reader takes it from the parsed tree
//! (LIB-COMPAT-003, ADR 0033): an element with its subtree becomes a
//! [`Fragment`] of flat pre-order events, and an attribute an [`Attr`],
//! with namespace URIs, prefixes, local names and values exactly as written.
//!
//! Nothing here reads SACM: the caller decides what is preserved and checks
//! the `xmi:id`s inside it. The walk uses a stack, not recursion, so a deep
//! subtree cannot exhaust the call stack.
//!
//! Each also gives the namespace text it keeps, which a read counts against
//! its size limit (LIB-SEC-003, ADR 0041 D2): the bytes of each namespace URI
//! a kept name holds, and `prefix + URI + 10` for each binding a kept start
//! tag declares, about what a written declaration ` xmlns:p="u"` takes.

use roxmltree::{Attribute, Node};

use super::namespaces::{SACM_NS, SACM_PREFIX, XMI_NS, XMI_PREFIX};
use crate::model::{Attr, Event, Fragment, QName};

/// `node`, an element, with its subtree, as a fragment written where the
/// holder is: its start tag declares every binding in scope except those
/// the written document has there anyway (`xml`, and `sacm` and `xmi` bound
/// to SACM 2.3 and XMI 2.5.1), and each element inside declares the
/// bindings it declares itself. Character data is kept exactly, adjacent
/// pieces joined, CDATA sections and references giving their characters;
/// comments and processing instructions are dropped. Returns it with the
/// namespace text it keeps.
pub(super) fn fragment(text: &str, node: Node<'_, '_>) -> (Fragment, u64) {
    let mut events = Vec::new();
    let mut kept = 0;
    events.push(start(text, node, in_scope(node), &mut kept));
    // The children still to visit of each open element, innermost last.
    let mut open = vec![node.children()];
    while let Some(children) = open.last_mut() {
        let Some(child) = children.next() else {
            open.pop();
            events.push(Event::End);
            continue;
        };
        if child.is_element() {
            events.push(start(text, child, declared(child), &mut kept));
            open.push(child.children());
        } else if child.is_text() {
            let piece = child.text().unwrap_or_default();
            match events.last_mut() {
                Some(Event::Text(joined)) => joined.push_str(piece),
                _ => events.push(Event::Text(piece.to_owned())),
            }
        }
    }
    (Fragment { events }, kept)
}

/// The bytes a binding of `prefix` to `uri` keeps: `prefix + URI + 10`.
pub(super) fn binding_bytes(prefix: Option<&str>, uri: &str) -> u64 {
    let length = prefix.map_or(0, str::len) + uri.len() + 10;
    u64::try_from(length).unwrap_or(u64::MAX)
}

/// The bytes of the namespace URI `name` keeps.
fn name_bytes(name: &QName) -> u64 {
    name.namespace
        .as_deref()
        .map_or(0, |uri| u64::try_from(uri.len()).unwrap_or(u64::MAX))
}

/// The start event of the element `node`, declaring `namespaces`, adding
/// the namespace text it keeps to `kept`.
fn start(
    text: &str,
    node: Node<'_, '_>,
    namespaces: Vec<(Option<String>, String)>,
    kept: &mut u64,
) -> Event {
    let name = element_name(text, node);
    let mut bytes = name_bytes(&name);
    for (prefix, uri) in &namespaces {
        bytes = bytes.saturating_add(binding_bytes(prefix.as_deref(), uri));
    }
    let attributes = node
        .attributes()
        .map(|attribute| {
            let (attr, uri) = attr(text, &attribute);
            bytes = bytes.saturating_add(uri);
            attr
        })
        .collect();
    *kept = kept.saturating_add(bytes);
    Event::Start {
        name,
        namespaces,
        attributes,
    }
}

/// The bindings in scope at `node` that a written document may lack where
/// it is written: all but `xml`, `sacm` bound to SACM 2.3, `xmi` bound to
/// XMI 2.5.1, and a default namespace undeclared, which is how the writer
/// leaves it. The default first, then by prefix.
fn in_scope(node: Node<'_, '_>) -> Vec<(Option<String>, String)> {
    let mut bindings: Vec<(Option<String>, String)> = node
        .namespaces()
        .filter(|namespace| {
            !matches!(
                (namespace.name(), namespace.uri()),
                (None, "") | (Some(SACM_PREFIX), SACM_NS) | (Some(XMI_PREFIX), XMI_NS)
            )
        })
        .map(|namespace| {
            (
                namespace.name().map(str::to_owned),
                namespace.uri().to_owned(),
            )
        })
        .collect();
    bindings.sort();
    bindings
}

/// The bindings `node` declares itself: those in scope at it that are not
/// in scope at its parent with the same URI. The default first, then by
/// prefix.
fn declared(node: Node<'_, '_>) -> Vec<(Option<String>, String)> {
    let parent = node.parent_element();
    let mut bindings: Vec<(Option<String>, String)> = node
        .namespaces()
        .filter(|namespace| {
            parent.is_none_or(|parent| {
                parent.lookup_namespace_uri(namespace.name()) != Some(namespace.uri())
            })
        })
        .map(|namespace| {
            (
                namespace.name().map(str::to_owned),
                namespace.uri().to_owned(),
            )
        })
        .collect();
    bindings.sort();
    bindings
}

/// The name of the element `node` as written.
fn element_name(text: &str, node: Node<'_, '_>) -> QName {
    let start = node.range().start.saturating_add(1);
    let written = text
        .get(start..)
        .and_then(|tag| {
            let end =
                tag.find(|c: char| super::input::is_xml_space(c) || matches!(c, '/' | '>'))?;
            tag.get(..end)
        })
        .unwrap_or(node.tag_name().name());
    qname(node.tag_name().namespace(), written, node.tag_name().name())
}

/// `attribute` as written, with its value after XML normalisation, and the
/// bytes of the namespace URI its name keeps.
pub(super) fn attr(text: &str, attribute: &Attribute<'_, '_>) -> (Attr, u64) {
    let written = text
        .get(attribute.range_qname())
        .unwrap_or(attribute.name());
    let name = qname(attribute.namespace(), written, attribute.name());
    let bytes = name_bytes(&name);
    (
        Attr {
            name,
            value: attribute.value().to_owned(),
        },
        bytes,
    )
}

/// The name written as `written` in the namespace `namespace`, whose local
/// part is `local`. Under `xmlns=""` the parser reports the empty string,
/// which is no namespace name (Namespaces 1.0 §2.2).
fn qname(namespace: Option<&str>, written: &str, local: &str) -> QName {
    let prefix = written.split_once(':').map(|(prefix, _)| prefix.to_owned());
    QName {
        namespace: namespace.filter(|uri| !uri.is_empty()).map(str::to_owned),
        prefix,
        local: local.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::fragment;
    use crate::model::{Event, QName};

    #[test]
    fn a_fragment_is_its_events_in_document_order_with_text_joined() {
        let text = "<r xmlns:v=\"urn:v\"><v:a v:b=\"1\">x<!--c-->y<![CDATA[<z>]]><c xmlns=\"urn:d\"/></v:a></r>";
        let Ok(tree) = roxmltree::Document::parse(text) else {
            panic!("the test input is well-formed");
        };
        let Some(a) = tree.root_element().first_element_child() else {
            panic!("the test input has a child");
        };
        let (fragment, kept) = fragment(text, a);
        // `v` bound (1 + 5 + 10), the names `v:a` and `v:b` (5 + 5), and the
        // default binding of `c` (0 + 5 + 10) and its name (5).
        assert_eq!(kept, 16 + 10 + 15 + 5);
        let events = fragment.events;
        let name = |prefix: Option<&str>, namespace: &str, local: &str| QName {
            namespace: Some(namespace.to_owned()),
            prefix: prefix.map(str::to_owned),
            local: local.to_owned(),
        };
        assert_eq!(events.len(), 5);
        assert!(matches!(
            &events[0],
            Event::Start { name: n, namespaces, attributes }
                if *n == name(Some("v"), "urn:v", "a")
                    && namespaces == &[(Some("v".to_owned()), "urn:v".to_owned())]
                    && attributes.len() == 1
        ));
        assert_eq!(events[1], Event::Text("xy<z>".to_owned()));
        assert!(matches!(
            &events[2],
            Event::Start { name: n, namespaces, .. }
                if *n == name(None, "urn:d", "c")
                    && namespaces == &[(None, "urn:d".to_owned())]
        ));
        assert_eq!(events[3], Event::End);
        assert_eq!(events[4], Event::End);
    }
}
