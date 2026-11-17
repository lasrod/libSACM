//! The copy operation (LIB-EDIT-018; ADR 0036 D2).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use super::EditError;
use super::plan::{Copied, Plan};
use crate::model::{
    CapturedElement, Document, ElementId, ElementKind, ReferenceEnd, SacmElement, Snapshot, admits,
    is_ncname,
};

/// Copies elements of a source [`Document`], each with everything it
/// contains, under a parent of the target ([`new`](Self::new)) or as roots
/// ([`root`](Self::root)), after the last child or root, in the order given:
/// so that an application can accept a draft built in another document.
///
/// The source elements are captured when the operation is built, so the
/// source may change or go afterwards. An element listed twice, or inside
/// another listed element, is copied once.
///
/// Apply it with [`edit::apply`](super::apply) to the target, which may be
/// the source itself. Each copy keeps its source id when that is an XML
/// `NCName` free in the target (no element, name, `content`, `key`,
/// `location`, `value` or preserved content holds it, and no reference
/// names it); otherwise it gets a generated id, such as `claim_3`, that
/// avoids every id the copy keeps or names. A part (a name, `content`,
/// `key`, `location` or `value`) keeps its `xmi:id` when it is an `NCName`
/// free in the target, and is copied without one otherwise. A reference to
/// a copied element names its copy; every other reference is kept as
/// written. Everything else is copied exactly, preserved content and the
/// gid included; a copy within one document therefore repeats each gid,
/// which [`validate`](crate::validation::validate) reports (SACM-ID-003).
/// The [`Mutation`](super::Mutation) lists the created ids in
/// [`source_ids`](Self::source_ids) order, and changes and deletes nothing.
/// The refusals are in the
/// [module docs](super#moving-and-copying-elements).
///
/// A `CopyElements` is the one operation that is not serialized: with the
/// `serde` feature, serializing it, or anything that holds one, is an error
/// (ADR 0030 amendment 2). A JSON client builds a draft with creates instead.
///
/// # Examples
///
/// Accept a draft of a claim and its inference, built in a scratch document,
/// into a package whose ids clash:
///
/// ```
/// use sacm::edit::{self, CopyElements, CreateArgumentPackage, CreateAssertedInference, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let name = LangString::new;
/// let mut target = Document::new();
/// edit::apply(&mut target, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut target, CreateClaim::new("p", name("The system is safe")).with_id("top"))?;
/// edit::apply(&mut target, CreateClaim::new("p", name("Taken")).with_id("h1"))?;
///
/// let mut draft = Document::new();
/// edit::apply(&mut draft, CreateArgumentPackage::root(name("Draft")).with_id("d"))?;
/// edit::apply(&mut draft, CreateClaim::new("d", name("H1 is mitigated")).with_id("h1"))?;
/// edit::apply(&mut draft, CreateClaim::new("d", name("Placeholder")).with_id("top"))?;
/// edit::apply(&mut draft, CreateAssertedInference::new("d", name("I1"), ["h1"], "top").with_id("i1"))?;
///
/// // `top` is not copied, so the inference's target is the target's `top`.
/// let copy = CopyElements::new(&draft, ["h1", "i1"], "p")?;
/// let mutation = edit::apply(&mut target, copy.clone())?;
/// let mapping: Vec<(&str, &str)> = copy
///     .source_ids()
///     .iter()
///     .zip(mutation.created_ids())
///     .map(|(from, to)| (from.as_str(), to.as_str()))
///     .collect();
/// assert_eq!(mapping, [("h1", "claim_1"), ("i1", "i1")]);
/// let inference = target.get("i1").expect("copied");
/// assert_eq!(inference.sources()[0].as_str(), "claim_1");
/// assert_eq!(inference.target().map(|id| id.as_str()), Some("top"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct CopyElements {
    parent: Option<ElementId>,
    copied: Arc<Snapshot>,
}

impl CopyElements {
    /// Copies `elements` of `source`, each with everything it contains, as
    /// the last children of `parent`, in the order given.
    ///
    /// # Errors
    ///
    /// [`EditError::ElementNotFound`] for the first id, in order, that names
    /// no element of `source`. The id of a name, `content`, `key`,
    /// `location` or `value` names no element.
    pub fn new(
        source: &Document,
        elements: impl IntoIterator<Item = impl Into<ElementId>>,
        parent: impl Into<ElementId>,
    ) -> Result<Self, EditError> {
        Self::captured(source, elements, Some(parent.into()))
    }

    /// Copies `elements` of `source`, each with everything it contains, as
    /// the last roots, in the order given. Each must be a package.
    ///
    /// # Errors
    ///
    /// [`EditError::ElementNotFound`] for the first id, in order, that names
    /// no element of `source`.
    pub fn root(
        source: &Document,
        elements: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Result<Self, EditError> {
        Self::captured(source, elements, None)
    }

    /// The copy of `elements` of `source` under `parent`.
    fn captured(
        source: &Document,
        elements: impl IntoIterator<Item = impl Into<ElementId>>,
        parent: Option<ElementId>,
    ) -> Result<Self, EditError> {
        let listed: Vec<ElementId> = elements.into_iter().map(Into::into).collect();
        let snapshot = source
            .snapshot(&listed)
            .map_err(|element| EditError::ElementNotFound { element })?;
        Ok(Self {
            parent,
            copied: Arc::new(snapshot),
        })
    }

    /// The parent's id, or `None` for the roots.
    #[must_use]
    pub fn parent(&self) -> Option<&ElementId> {
        self.parent.as_ref()
    }

    /// The ids of the elements copied, in the source: the elements listed
    /// in the order given, each once and none inside another, each followed
    /// by everything it contains, in pre-order. The i-th created id of the
    /// [`Mutation`](super::Mutation) is the copy of the i-th.
    #[must_use]
    pub fn source_ids(&self) -> &[ElementId] {
        self.copied.ids()
    }

    /// Runs every check of the operation, in the order of ADR 0036 D2, and
    /// returns what it adds.
    #[expect(
        clippy::too_many_lines,
        reason = "the checks of ADR 0036 D2 read best in their order, as one list"
    )]
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let snapshot: &Snapshot = &self.copied;
        // 1. The parent is in the target.
        let parent_kind = match &self.parent {
            Some(parent) => match document.get(parent) {
                Some(found) => Some(found.kind()),
                None => {
                    return Err(EditError::ParentNotFound {
                        parent: parent.clone(),
                    });
                }
            },
            None => None,
        };
        if snapshot.ids().is_empty() {
            return Ok(Plan::Unchanged);
        }
        // 2. Each top, in list order, can be held by the parent, or is a
        //    package for the roots.
        for top in snapshot.elements().filter(|element| element.is_top()) {
            match (&self.parent, parent_kind) {
                (Some(parent), Some(parent_kind)) => {
                    if !admits(parent_kind, top.kind()) {
                        return Err(EditError::InvalidParent {
                            parent: parent.clone(),
                            parent_kind,
                            kind: top.kind(),
                        });
                    }
                }
                _ => {
                    if !top.kind().is_package() {
                        return Err(EditError::InvalidRoot {
                            element: top.id().clone(),
                            kind: top.kind(),
                        });
                    }
                }
            }
        }
        // The ids the copy keeps: an element's or a part's id that is an
        // NCName and free in the target.
        let keeps = |id: &ElementId| is_ncname(id.as_str()) && document.is_free(id.as_str());
        let copied: BTreeMap<&ElementId, ElementKind> = snapshot
            .elements()
            .map(|element| (element.id(), element.kind()))
            .collect();
        let changes = |id: &ElementId| copied.contains_key(id) && !keeps(id);
        let mut kept: BTreeSet<ElementId> = BTreeSet::new();
        let mut dropped: BTreeSet<&ElementId> = BTreeSet::new();
        for part in snapshot.elements().flat_map(CapturedElement::part_ids) {
            if keeps(part) {
                kept.insert(part.clone());
            } else {
                dropped.insert(part);
            }
        }
        // 3. Preserved content is copied verbatim: its ids must be free, and
        //    it may not name an id the copy changes or drops.
        for element in snapshot.elements() {
            if let Some(id) = element
                .preserved_ids()
                .iter()
                .find(|id| !document.is_free(id.as_str()))
            {
                return Err(EditError::DuplicateId { id: id.clone() });
            }
            if let Some(id) = element
                .preserved_references()
                .iter()
                .find(|id| changes(id) || dropped.contains(id))
            {
                return Err(EditError::DuplicateId { id: id.clone() });
            }
        }
        // 4. Each end holds at least the ids it needs.
        for element in snapshot.elements() {
            for &end in ReferenceEnd::ALL {
                if let Some((required, _)) = end.bounds(element.kind()) {
                    let given = element.references(end).len();
                    if given < required {
                        return Err(EditError::TooFewReferences {
                            end: end.name(),
                            given,
                            required,
                        });
                    }
                }
            }
            // §8.4 `expression[1]`: an ExpressionLangString with no
            // expression is not copied (SACM23-BASE-009, ADR 0038 D3).
            if element.has_missing_expression() {
                return Err(EditError::TooFewReferences {
                    end: ReferenceEnd::Expression.name(),
                    given: 0,
                    required: 1,
                });
            }
        }
        // 5 and 6, each over every reference in turn, as a create's: every id
        // names an element of the target or a copied one, then every id
        // names an element of a kind its end admits. A typed end never holds
        // an id with white space, so no id is refused for it.
        let references = || {
            snapshot.elements().flat_map(|element| {
                ReferenceEnd::ALL
                    .iter()
                    .flat_map(move |&end| element.references(end).iter().map(move |id| (end, id)))
            })
        };
        for (end, id) in references() {
            if !copied.contains_key(id) && !document.contains(id) {
                return Err(EditError::ReferenceNotFound {
                    end: end.name(),
                    id: id.clone(),
                });
            }
        }
        for (end, id) in references() {
            let kind = match copied.get(id) {
                Some(&kind) => Some(kind),
                None => document.get(id).map(SacmElement::kind),
            };
            let declared = end.target();
            if let Some(kind) = kind.filter(|&kind| !declared.admits(kind)) {
                return Err(EditError::WrongReferenceType {
                    end: end.name(),
                    id: id.clone(),
                    kind,
                    expected: declared.name(),
                });
            }
        }
        // The ids: kept, or generated in pre-order, avoiding every id the
        // copy keeps or names.
        let mut avoid: BTreeSet<ElementId> = kept.clone();
        for element in snapshot.elements() {
            if keeps(element.id()) {
                avoid.insert(element.id().clone());
            }
            avoid.extend(element.preserved_ids().iter().cloned());
            avoid.extend(element.preserved_references().iter().cloned());
            for &end in ReferenceEnd::ALL {
                avoid.extend(element.references(end).iter().cloned());
            }
        }
        let mut last: BTreeMap<ElementKind, u64> = BTreeMap::new();
        let mut ids = Vec::with_capacity(snapshot.ids().len());
        for element in snapshot.elements() {
            if keeps(element.id()) {
                ids.push(element.id().clone());
            } else {
                let kind = element.kind();
                let (id, n) =
                    document.generated_id_avoiding(kind, last.get(&kind).copied(), &avoid);
                last.insert(kind, n);
                ids.push(id);
            }
        }
        Ok(Plan::Copy(Copied {
            parent: self.parent.as_ref(),
            snapshot,
            ids,
            kept,
            generated: last.into_iter().collect(),
        }))
    }
}

impl fmt::Debug for CopyElements {
    /// Shows the parent and the source ids, not the captured elements.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CopyElements")
            .field("parent", &self.parent)
            .field("source_ids", &self.source_ids())
            .finish_non_exhaustive()
    }
}
