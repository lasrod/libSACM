//! What an operation's checks computed, and the one change that applies it
//! (LIB-EDIT-013, ADR 0019 amendment 3).
//!
//! [`check`](super::check) and [`apply`](super::apply) share one code path:
//! each operation's `plan` runs every check and builds every refusal, and
//! returns a [`Plan`]. `check` drops it; `apply` hands it to
//! [`Plan::commit`], which cannot fail. A `Plan` borrows the operation,
//! never the document, so `commit` can take the document mutably.

use crate::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd, Text, TextRole,
};

/// What every check of one operation computed: what [`commit`](Self::commit)
/// changes. Only a `plan` builds one, once every check has passed.
#[derive(Debug)]
pub(super) enum Plan<'o> {
    /// A create: the element to add.
    Create(NewElement<'o>),
    /// An operation that changes an element and sets the value held: it
    /// changes nothing.
    Unchanged,
    /// An operation that changes a feature of `element` to `value`.
    Set {
        /// The element to change.
        element: &'o ElementId,
        /// The new value of the feature.
        value: Value<'o>,
    },
    /// A reorder of the children of `parent`, or of the roots for `None`.
    Reorder {
        /// The parent, or `None` for the roots.
        parent: Option<&'o ElementId>,
        /// The ids to move, in their new order: each once, each a child of
        /// `parent` (or a root).
        order: &'o [ElementId],
    },
}

impl Plan<'_> {
    /// Makes the change, and returns the ids created and the ids changed.
    ///
    /// It cannot fail: everything it relies on, the plan's checks
    /// established against the same document, which nothing has changed
    /// since.
    pub(super) fn commit(self, document: &mut Document) -> (Vec<ElementId>, Vec<ElementId>) {
        match self {
            Self::Create(new) => (vec![new.commit(document)], Vec::new()),
            Self::Unchanged => (Vec::new(), Vec::new()),
            Self::Set { element, value } => {
                value.commit(document, element);
                (Vec::new(), vec![element.clone()])
            }
            Self::Reorder { parent, order } => {
                let changed = document.reorder(parent, order);
                // The roots' order is the document's, not an element's.
                let changed = parent.filter(|_| changed).into_iter().cloned().collect();
                (Vec::new(), changed)
            }
        }
    }
}

/// A new element, once the create's checks have passed: its kind, place,
/// id and features.
#[derive(Debug)]
pub(super) struct NewElement<'o> {
    /// Its kind.
    pub(super) kind: ElementKind,
    /// Its parent, which holds a role for `kind`, or `None` for a root of a
    /// package kind.
    pub(super) parent: Option<&'o ElementId>,
    /// Its id: an `NCName` not in use, given or generated.
    pub(super) id: ElementId,
    /// The `n` of a generated id, which advances the kind's counter.
    pub(super) generated: Option<u64>,
    /// What it is given besides its id and place.
    pub(super) features: Features<'o>,
}

impl NewElement<'_> {
    /// Adds the element, advances the counter for a generated id, and
    /// returns the element's id.
    fn commit(self, document: &mut Document) -> ElementId {
        let Self {
            kind,
            parent,
            id,
            generated,
            features,
        } = self;
        let inserted = match features {
            Features::Name(name) => {
                document.insert_element(parent, kind, id.clone(), Some(name.clone()), None, None)
            }
            Features::Texts { content, key } => document.insert_element(
                parent,
                kind,
                id.clone(),
                None,
                Some(content.clone()),
                key.cloned(),
            ),
            Features::Referencing {
                name,
                flags,
                references,
            } => {
                let held = references
                    .into_iter()
                    .filter(|(_, targets)| !targets.is_empty())
                    .map(|(end, targets)| (end, targets.to_vec()))
                    .collect();
                document.insert_with_references(
                    parent,
                    kind,
                    id.clone(),
                    Some(name.clone()),
                    &flags,
                    held,
                )
            }
        };
        inserted.expect(
            "invariant: the plan found the parent, whose class holds the kind, or made a root of a \
             package kind, gave a name only to a ModelElement kind, chose an NCName id not in use, \
             and checked each reference end's bounds",
        );
        if let Some(n) = generated {
            document.commit_generated_id(kind, n);
        }
        id
    }
}

/// What a create operation gives its new element besides its id and place.
#[derive(Debug)]
pub(super) enum Features<'o> {
    /// A `ModelElement`'s name.
    Name(&'o LangString),
    /// A `UtilityElement`'s `content`, and a `TaggedValue`'s `key`.
    Texts {
        /// The `content`.
        content: &'o MultiLangString,
        /// The `key`, given exactly for a `TaggedValue`.
        key: Option<&'o MultiLangString>,
    },
    /// The name, flags and reference ends of an argumentation kind that
    /// references other elements.
    Referencing {
        /// The name.
        name: &'o LangString,
        /// The flags to set, each one the kind has.
        flags: Vec<(Flag, bool)>,
        /// Each end, one the kind owns, with the ids it holds, within its
        /// bounds; an end with none holds nothing.
        references: Vec<(ReferenceEnd, &'o [ElementId])>,
    },
}

/// The new value of one feature of an element, once the operation's checks
/// have passed and found it differs from the value held.
#[derive(Debug)]
pub(super) enum Value<'o> {
    /// The name of a `ModelElement`.
    Name(&'o LangString),
    /// The gid, or `None` to remove it.
    Gid(Option<&'o str>),
    /// A flag the element's kind has.
    Flag(Flag, bool),
    /// The `assertionDeclaration` of an Assertion.
    Declaration(AssertionDeclaration),
    /// A role the element's kind holds, with the ids it keeps, or `None` to
    /// remove it.
    Text(TextRole, Option<Text>),
    /// The ids an end the element's kind owns is to hold, within its bounds.
    References(ReferenceEnd, Vec<ElementId>),
}

impl Value<'_> {
    /// Sets the feature of `element`, which the plan found in the document.
    fn commit(self, document: &mut Document, element: &ElementId) {
        match self {
            Self::Name(name) => document.replace_name(element, name.clone()),
            Self::Gid(gid) => document
                .set_gid(element, gid.map(str::to_owned))
                .expect("invariant: the plan found the element"),
            Self::Flag(flag, value) => document
                .set_flag(element, flag, value)
                .expect("invariant: the plan found the element, and its kind has the flag"),
            Self::Declaration(declaration) => document
                .set_declaration(element, declaration)
                .expect("invariant: the plan found the element, and it is an Assertion"),
            Self::Text(role, Some(text)) => document.set_text(element, role, text).expect(
                "invariant: the plan found the element, its kind holds the role, and the text \
                 keeps only the role's own ids",
            ),
            Self::Text(role, None) => document.remove_text(element, role),
            Self::References(end, ids) => document.set_references(element, end, ids).expect(
                "invariant: the plan found the element, its kind owns the end, and the ids are \
                 within its bounds and not empty",
            ),
        }
    }
}
