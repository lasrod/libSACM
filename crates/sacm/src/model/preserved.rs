//! Preserved content: what an element read from XMI, or the document outside
//! its elements, holds as written instead of read: content that is not SACM
//! 2.3 where it stands (LIB-COMPAT-003, ADR 0033), or, in tolerant mode, a
//! construct a strict read refuses, such as a value that is no literal or an
//! element of a kind not read yet (ADR 0035).
//!
//! Only [`xmi::read`](crate::xmi::read) makes it; the public API sees it only
//! as [`SacmElement::has_preserved_content`](super::SacmElement::has_preserved_content)
//! and [`Document::has_preserved_content`](super::Document::has_preserved_content),
//! a strict write's refusal and a comparison's difference. It is held parsed,
//! not as XML text, so it compares exactly and can declare the namespaces it
//! needs wherever it is written.
//!
//! A fragment is a flat list of pre-order events, so cloning, comparing,
//! dropping and writing one never recurses, however deep it is.

use super::{ElementId, ReferenceEnd};

/// The preserved content of one element, its holder, or of the document
/// outside its elements: attributes after the typed ones (for the document,
/// on `xmi:XMI`), in input order, and children placed among its typed
/// children (for the document, among the roots).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Preserved {
    /// The namespace bindings the holder's start tag declares for
    /// `attributes`: each prefix they use, other than `xml`, and `sacm` and
    /// `xmi` bound to the SACM 2.3 and XMI 2.5.1 namespaces, which the
    /// document element declares. Sorted by prefix.
    pub(crate) namespaces: Vec<(String, String)>,
    /// The attributes, in input order.
    pub(crate) attributes: Vec<Attr>,
    /// The children, in input order.
    pub(crate) children: Vec<Placed>,
    /// Derived from the rest: the `xmi:id` value of every element of the
    /// fragments, which stays in the document's id space.
    pub(crate) ids: Vec<ElementId>,
    /// Derived from the rest: every id an `xmi:idref`, an `href="#id"` or an
    /// attribute named after a reference end may name, so that no id is
    /// generated that it would then resolve to.
    pub(crate) references: Vec<ElementId>,
}

/// A preserved child, with its place among the holder's typed children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    /// Written before the typed child at this index of the holder's
    /// children; at the number of typed children, after the last.
    pub(crate) index: usize,
    /// The child.
    pub(crate) item: Item,
}

/// A preserved child: text or an element with its subtree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// Text in element-only content, trimmed of XML white space; adjacent
    /// pieces joined with LF.
    Text(String),
    /// An element with its subtree.
    Element(Fragment),
}

/// An element with its subtree, as pre-order events: a `Start`, its
/// content, and the matching `End`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Fragment {
    /// The events, in document order.
    pub(crate) events: Vec<Event>,
}

/// One event of a [`Fragment`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// A start tag.
    Start {
        /// The element's name.
        name: QName,
        /// The namespace bindings it declares, the default one (`None`)
        /// first, then by prefix. The outermost element declares every
        /// binding in scope that the written document might not.
        namespaces: Vec<(Option<String>, String)>,
        /// Its attributes, in input order.
        attributes: Vec<Attr>,
    },
    /// Character data, exactly as XML gives it; adjacent pieces joined.
    Text(String),
    /// The end tag of the innermost open element.
    End,
}

/// A name as written: its namespace URI, its prefix and its local part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QName {
    /// The namespace URI; `None` for no namespace.
    pub(crate) namespace: Option<String>,
    /// The prefix written; `None` for none.
    pub(crate) prefix: Option<String>,
    /// The local part.
    pub(crate) local: String,
}

/// An attribute: its name and its value after XML normalisation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Attr {
    /// The attribute's name.
    pub(crate) name: QName,
    /// Its value.
    pub(crate) value: String,
}

impl Preserved {
    /// Whether it holds nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.attributes.is_empty() && self.children.is_empty()
    }

    /// Whether it holds an attribute `local` in no namespace.
    pub(crate) fn has_unqualified(&self, local: &str) -> bool {
        self.attributes
            .iter()
            .any(|attr| attr.name.namespace.is_none() && attr.name.local == local)
    }

    /// Removes the attribute `local` in no namespace, if it holds one, and
    /// returns the ids it named as a reference, each once per time
    /// [`references`](Self::references) counted it, which it no longer
    /// counts.
    ///
    /// An edit that sets the property of that name removes it: a reference
    /// end, which the reader keeps as an attribute when it holds no id, or a
    /// single-valued end given two ids in a tolerant read (`target="c2
    /// c3"`), or a flag or `assertionDeclaration` whose value is no literal
    /// (ADR 0033, ADR 0035).
    pub(crate) fn remove_unqualified(&mut self, local: &str) -> Vec<ElementId> {
        let is_end = ReferenceEnd::ALL.iter().any(|end| end.name() == local);
        let mut tokens = Vec::new();
        self.attributes.retain(|attr| {
            let removed = attr.name.namespace.is_none() && attr.name.local == local;
            if removed && is_end {
                tokens.extend(
                    attr.value
                        .split([' ', '\t', '\n', '\r'])
                        .filter(|token| !token.is_empty())
                        .map(ElementId::new),
                );
            }
            !removed
        });
        let mut released = Vec::new();
        for id in tokens {
            if let Some(position) = self.references.iter().position(|held| *held == id) {
                self.references.remove(position);
                released.push(id);
            }
        }
        released
    }

    /// The number of preserved child elements: text and attributes do not
    /// count (the maintainer, 2026-11-15).
    pub(crate) fn element_count(&self) -> usize {
        self.children
            .iter()
            .filter(|placed| matches!(placed.item, Item::Element(_)))
            .count()
    }

    /// Places the children again after the typed children of their holder
    /// changed from `before` to those of `before` that `kept` says survive:
    /// each index becomes the number of surviving typed children before it,
    /// and text that is now next to text at the same place is joined with LF,
    /// as a read of the written output would join it.
    pub(crate) fn shift(&mut self, kept: &[bool]) {
        let mut survivors = Vec::with_capacity(kept.len().saturating_add(1));
        let mut count = 0_usize;
        survivors.push(0);
        for &alive in kept {
            if alive {
                count = count.saturating_add(1);
            }
            survivors.push(count);
        }
        let children = std::mem::take(&mut self.children);
        for mut placed in children {
            placed.index = survivors.get(placed.index).copied().unwrap_or(count);
            if let (
                Item::Text(text),
                Some(Placed {
                    index,
                    item: Item::Text(last),
                }),
            ) = (&placed.item, self.children.last_mut())
            {
                if *index == placed.index {
                    last.push('\n');
                    last.push_str(text);
                    continue;
                }
            }
            self.children.push(placed);
        }
    }
}

impl QName {
    /// The name as written: `prefix:local`, or `local`.
    pub(crate) fn written(&self) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}:{}", self.local),
            None => self.local.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, Placed, Preserved};

    fn text(index: usize, text: &str) -> Placed {
        Placed {
            index,
            item: Item::Text(text.to_owned()),
        }
    }

    #[test]
    fn a_shift_counts_the_surviving_children_before_each_index_and_joins_text() {
        let mut preserved = Preserved {
            children: vec![text(0, "a"), text(1, "b"), text(2, "c"), text(3, "d")],
            ..Preserved::default()
        };
        // Of three typed children, the second is deleted.
        preserved.shift(&[true, false, true]);
        assert_eq!(
            preserved.children,
            [text(0, "a"), text(1, "b\nc"), text(2, "d")]
        );
        assert_eq!(preserved.element_count(), 0);
    }
}
