//! What an operation's checks computed, and the one change that applies it
//! (LIB-EDIT-013, ADR 0019 amendment 3).
//!
//! [`check`](super::check), [`preview`](super::preview) and
//! [`apply`](super::apply) share one code path: each operation's `plan` runs
//! every check and builds every refusal, and returns a [`Plan`], which also
//! knows the ids the change creates, changes and deletes ([`Plan::outcome`])
//! and, for a delete, the references it leaves dangling. `check` drops it;
//! `preview` reports it; `apply` hands it to [`Plan::commit`], which cannot
//! fail. So `preview` and `apply` agree by construction (ADR 0031). A `Plan`
//! borrows the operation, never the document, so `commit` can take the
//! document mutably.

use super::DanglingReference;
use super::set::ASSERTION_DECLARATION;
use crate::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString, MultiLangString,
    ReferenceEnd, Text, TextRole,
};

/// The ids a change creates, changes and deletes, in the order a
/// [`Mutation`](super::Mutation) reports them.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    /// The ids created, in creation order.
    pub(super) created: Vec<ElementId>,
    /// The ids changed, in pre-order after the change.
    pub(super) changed: Vec<ElementId>,
    /// The ids deleted, in pre-order before the change.
    pub(super) deleted: Vec<ElementId>,
}

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
        /// Whether the order changes: the ids `order` names are not in this
        /// order now.
        changes: bool,
    },
    /// A delete (ADR 0031).
    Delete(Removal),
}

impl Plan<'_> {
    /// The ids the change creates, changes and deletes, as `commit` will
    /// make them: what [`Mutation`](super::Mutation) reports.
    pub(super) fn outcome(&self) -> Outcome {
        match self {
            Self::Create(new) => Outcome {
                created: vec![new.id.clone()],
                ..Outcome::default()
            },
            Self::Unchanged => Outcome::default(),
            Self::Set { element, .. } => Outcome {
                changed: vec![(*element).clone()],
                ..Outcome::default()
            },
            // The roots' order is the document's, not an element's.
            Self::Reorder {
                parent, changes, ..
            } => Outcome {
                changed: parent.filter(|_| *changes).into_iter().cloned().collect(),
                ..Outcome::default()
            },
            Self::Delete(removal) => Outcome {
                changed: removal.changed.clone(),
                deleted: removal.deleted.clone(),
                ..Outcome::default()
            },
        }
    }

    /// The references the change leaves dangling, which it removes: only a
    /// delete has any.
    pub(super) fn dangling_references(&self) -> &[DanglingReference] {
        match self {
            Self::Delete(removal) => &removal.dangling,
            _ => &[],
        }
    }

    /// Makes the change.
    ///
    /// It cannot fail: everything it relies on, the plan's checks
    /// established against the same document, which nothing has changed
    /// since.
    pub(super) fn commit(self, document: &mut Document) {
        match self {
            Self::Create(new) => new.commit(document),
            Self::Unchanged => {}
            Self::Set { element, value } => value.commit(document, element),
            Self::Reorder {
                parent,
                order,
                changes,
            } => {
                let changed = document.reorder(parent, order);
                debug_assert_eq!(
                    changed, changes,
                    "the plan computed whether the order changes"
                );
            }
            Self::Delete(removal) => document.remove_elements(&removal.deleted, &removal.changed),
        }
    }
}

/// What a delete removes, once its checks have passed (ADR 0031).
#[derive(Debug)]
pub(super) struct Removal {
    /// Every element to delete, in pre-order: closed under containment.
    pub(super) deleted: Vec<ElementId>,
    /// Every surviving element that holds the id of a deleted one, in
    /// pre-order: each loses those ids.
    pub(super) changed: Vec<ElementId>,
    /// The references held outside the deleted element's subtree that name
    /// a deleted element, in the order of ADR 0031.
    pub(super) dangling: Vec<DanglingReference>,
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
    /// Adds the element, and advances the counter for a generated id.
    fn commit(self, document: &mut Document) {
        let Self {
            kind,
            parent,
            id,
            generated,
            features,
        } = self;
        let inserted = match features {
            Features::Name(name) => {
                document.insert_element(parent, kind, id, Some(name.clone()), None, None)
            }
            Features::Texts { content, key } => {
                document.insert_element(parent, kind, id, None, Some(content.clone()), key.cloned())
            }
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
                document.insert_with_references(parent, kind, id, Some(name.clone()), &flags, held)
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
            Self::Flag(flag, value) => {
                document
                    .set_flag(element, flag, value)
                    .expect("invariant: the plan found the element, and its kind has the flag");
                // The flag is given now: it replaces an attribute of its name
                // kept as preserved content, whose value was no literal (ADR
                // 0035).
                document.replace_preserved_attribute(element, flag.name());
            }
            Self::Declaration(declaration) => {
                document
                    .set_declaration(element, declaration)
                    .expect("invariant: the plan found the element, and it is an Assertion");
                document.replace_preserved_attribute(element, ASSERTION_DECLARATION);
            }
            Self::Text(role, Some(text)) => document.set_text(element, role, text).expect(
                "invariant: the plan found the element, its kind holds the role, and the text \
                 keeps only the role's own ids",
            ),
            Self::Text(role, None) => document.remove_text(element, role),
            Self::References(end, ids) => {
                let sets = !ids.is_empty();
                document.set_references(element, end, ids).expect(
                    "invariant: the plan found the element, its kind owns the end, and the ids \
                     are within its bounds and not empty",
                );
                // The end is given now: it replaces an attribute of its name
                // kept as preserved content, which held no id (ADR 0033).
                if sets {
                    document.replace_preserved_attribute(element, end.name());
                }
            }
        }
    }
}
