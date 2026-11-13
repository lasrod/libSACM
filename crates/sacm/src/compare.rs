//! Comparing documents by their SACM content: [`semantic_compare`] lists
//! every [`Difference`] between two documents, and `==` on two
//! [`Document`]s is true exactly when there is none.
//!
//! Elements are matched by id across the whole document, so an element that
//! moved to another parent is one [`Difference::Parent`], not a removal and an
//! addition. For each element both documents hold, the comparison checks its
//! kind, parent, name, the `xmi:id` of its name
//! ([`SacmElement::name_id`](crate::model::SacmElement::name_id)) and
//! [`assertionDeclaration`](crate::model::AssertionDeclaration) (SACM 2.3
//! §11.10), and the relative order of its children; it also checks the
//! relative order of the roots. Values are compared exactly: a missing name
//! differs from an empty one.
//!
//! Order is compared because equal documents must write equal bytes, though
//! SACM 2.3 orders no composition. To compare without order, drop the two
//! order variants:
//!
//! ```
//! use sacm::compare::{Difference, semantic_compare};
//! use sacm::edit::{self, CreateArgumentPackage};
//! use sacm::model::{Document, LangString};
//!
//! let mut left = Document::new();
//! edit::apply(&mut left, CreateArgumentPackage::root(LangString::new("A")).with_id("a"))?;
//! edit::apply(&mut left, CreateArgumentPackage::root(LangString::new("B")).with_id("b"))?;
//! let mut right = Document::new();
//! edit::apply(&mut right, CreateArgumentPackage::root(LangString::new("B")).with_id("b"))?;
//! edit::apply(&mut right, CreateArgumentPackage::root(LangString::new("A")).with_id("a"))?;
//! assert_ne!(left, right);
//!
//! let mut differences = semantic_compare(&left, &right);
//! differences.retain(|d| !matches!(d, Difference::RootOrder { .. } | Difference::ChildOrder { .. }));
//! let same_ignoring_order = differences.is_empty();
//! assert!(same_ignoring_order);
//! # Ok::<(), sacm::edit::EditError>(())
//! ```
//!
//! Not compared: the per-kind counters [`edit::apply`](crate::edit::apply)
//! uses to generate ids such as `claim_1`. They are not SACM content, and XMI
//! does not carry them. Formatting, attribute order and
//! namespace prefixes are not held by a [`Document`] at all, so two files that
//! differ only in them read into equal documents.
//!
//! # Examples
//!
//! ```
//! use sacm::compare::{Difference, semantic_compare};
//! use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
//! use sacm::model::{Document, ElementKind, LangString};
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//!
//! let mut built = Document::new();
//! let arguments = CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments");
//! edit::apply(&mut built, arguments)?;
//!
//! // Write it and read it back: the same SACM content.
//! let text = xmi::write(&built, &WriteOptions::default())?;
//! let mut reloaded = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
//! assert_eq!(semantic_compare(&built, &reloaded), []);
//! assert_eq!(built, reloaded);
//!
//! // Add a Claim to one side: it is the one difference.
//! edit::apply(&mut reloaded, CreateClaim::new("arguments", LangString::new("C1")).with_id("c1"))?;
//! let differences = semantic_compare(&built, &reloaded);
//! assert_eq!(
//!     differences,
//!     [Difference::RightOnly { id: "c1".into(), kind: ElementKind::Claim }]
//! );
//! // Prints: element "c1" (Claim) is only on the right
//! println!("{}", differences[0]);
//! assert_ne!(built, reloaded);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;
use std::ops::ControlFlow;

use crate::model::{
    AssertionDeclaration, Compared, Document, ElementId, ElementKind, LangString, SacmElement,
};

/// One way in which two documents differ in SACM content, as
/// [`semantic_compare`] reports it.
///
/// "Left" and "right" are the first and the second document given to
/// [`semantic_compare`]. Each variant names one feature of one element, except
/// [`RootOrder`](Self::RootOrder), which is about the document.
/// [`element_id`](Self::element_id) gives the id to look up with
/// [`Document::get`](crate::model::Document::get) on either side.
///
/// New variants are added as the model holds more SACM features, so a `match`
/// needs a wildcard arm.
///
/// `Display` writes one line for people to read. Its wording is not API.
///
/// # Examples
///
/// ```
/// use sacm::compare::Difference;
/// use sacm::model::{ElementId, LangString};
///
/// let difference = Difference::Name {
///     id: ElementId::from("c1"),
///     left: None,
///     right: Some(LangString::new("C1")),
/// };
/// assert_eq!(difference.element_id(), Some(&ElementId::from("c1")));
/// // Prints: element "c1" differs in name: no name on the left, [no lang, content "C1"] on the right
/// println!("{difference}");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Difference {
    /// The roots both documents hold are in a different relative order.
    ///
    /// Only roots on both sides count, so adding or removing a root is not an
    /// order difference.
    RootOrder {
        /// The common roots, in the left document's order.
        left: Vec<ElementId>,
        /// The common roots, in the right document's order.
        right: Vec<ElementId>,
    },
    /// Only the left document has an element with this id.
    ///
    /// Every element of a one-side subtree is reported, not just its top.
    LeftOnly {
        /// The element's id.
        id: ElementId,
        /// The element's kind in the left document.
        kind: ElementKind,
    },
    /// Only the right document has an element with this id.
    ///
    /// Every element of a one-side subtree is reported, not just its top.
    RightOnly {
        /// The element's id.
        id: ElementId,
        /// The element's kind in the right document.
        kind: ElementKind,
    },
    /// The element has a different kind on each side.
    ///
    /// Its other features are still compared.
    Kind {
        /// The element's id.
        id: ElementId,
        /// The kind in the left document.
        left: ElementKind,
        /// The kind in the right document.
        right: ElementKind,
    },
    /// The element has a different parent on each side; `None` is a root.
    Parent {
        /// The element's id.
        id: ElementId,
        /// The parent's id in the left document, or `None` for a root.
        left: Option<ElementId>,
        /// The parent's id in the right document, or `None` for a root.
        right: Option<ElementId>,
    },
    /// The element has a different `name` (SACM 2.3 §8.6) on each side;
    /// `None` is a missing name.
    ///
    /// Names are compared exactly: a missing name, [`LangString::default()`]
    /// and a name with an empty `content` are three different values.
    Name {
        /// The element's id.
        id: ElementId,
        /// The name in the left document, or `None` when missing.
        left: Option<LangString>,
        /// The name in the right document, or `None` when missing.
        right: Option<LangString>,
    },
    /// The `xmi:id` of the element's name
    /// ([`SacmElement::name_id`](crate::model::SacmElement::name_id)) differs
    /// on each side; `None` is a name without an id, or no name.
    ///
    /// A name's id is compared because equal documents must write equal
    /// bytes, and `write` writes it (ADR 0024).
    NameId {
        /// The id of the element that owns the name.
        id: ElementId,
        /// The name's id in the left document, or `None` when it has none.
        left: Option<ElementId>,
        /// The name's id in the right document, or `None` when it has none.
        right: Option<ElementId>,
    },
    /// The element has a different `assertionDeclaration` (SACM 2.3 §11.10)
    /// on each side.
    ///
    /// `None` means the kind on that side is not an Assertion (an Assertion
    /// is a Claim or one of the five asserted-relationship kinds). So a
    /// `None` side always
    /// comes with a [`Kind`](Self::Kind) difference for the same id.
    AssertionDeclaration {
        /// The element's id.
        id: ElementId,
        /// The declaration in the left document, or `None` for a kind that
        /// is not an Assertion.
        left: Option<AssertionDeclaration>,
        /// The declaration in the right document, or `None` for a kind that
        /// is not an Assertion.
        right: Option<AssertionDeclaration>,
    },
    /// The children the element holds on both sides are in a different
    /// relative order.
    ///
    /// Children are one ordered list, whichever composition holds them (for
    /// example, a package's `argumentElement` children and its utility
    /// children).
    ///
    /// Only children with this parent on both sides count, so adding,
    /// removing or moving a child is not an order difference.
    ChildOrder {
        /// The parent's id.
        id: ElementId,
        /// The common children, in the left document's order.
        left: Vec<ElementId>,
        /// The common children, in the right document's order.
        right: Vec<ElementId>,
    },
}

impl Difference {
    /// The element the difference is about: the parent for
    /// [`ChildOrder`](Self::ChildOrder), and `None` for
    /// [`RootOrder`](Self::RootOrder), which is about the document.
    ///
    /// ```
    /// use sacm::compare::Difference;
    /// use sacm::model::ElementId;
    ///
    /// let order = Difference::RootOrder {
    ///     left: vec![ElementId::from("a"), ElementId::from("b")],
    ///     right: vec![ElementId::from("b"), ElementId::from("a")],
    /// };
    /// assert_eq!(order.element_id(), None);
    /// ```
    #[must_use]
    pub fn element_id(&self) -> Option<&ElementId> {
        match self {
            Self::RootOrder { .. } => None,
            Self::LeftOnly { id, .. }
            | Self::RightOnly { id, .. }
            | Self::Kind { id, .. }
            | Self::Parent { id, .. }
            | Self::Name { id, .. }
            | Self::NameId { id, .. }
            | Self::AssertionDeclaration { id, .. }
            | Self::ChildOrder { id, .. } => Some(id),
        }
    }
}

impl fmt::Display for Difference {
    /// One line: the quoted id, the SACM feature and both values. Ids and
    /// names are quoted with quotes and line breaks escaped, so the line
    /// never breaks.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootOrder { left, right } => write!(
                f,
                "the root order differs: {} on the left, {} on the right",
                Ids(left),
                Ids(right)
            ),
            Self::LeftOnly { id, kind } => {
                write!(f, "element {:?} ({kind}) is only on the left", id.as_str())
            }
            Self::RightOnly { id, kind } => {
                write!(f, "element {:?} ({kind}) is only on the right", id.as_str())
            }
            Self::Kind { id, left, right } => write!(
                f,
                "element {:?} differs in kind: {left} on the left, {right} on the right",
                id.as_str()
            ),
            Self::Parent { id, left, right } => write!(
                f,
                "element {:?} differs in parent: {} on the left, {} on the right",
                id.as_str(),
                Parent(left.as_ref()),
                Parent(right.as_ref())
            ),
            Self::Name { id, left, right } => write!(
                f,
                "element {:?} differs in name: {} on the left, {} on the right",
                id.as_str(),
                Name(left.as_ref()),
                Name(right.as_ref())
            ),
            Self::NameId { id, left, right } => write!(
                f,
                "element {:?} differs in the xmi:id of its name: {} on the left, {} on the right",
                id.as_str(),
                NameId(left.as_ref()),
                NameId(right.as_ref())
            ),
            Self::AssertionDeclaration { id, left, right } => write!(
                f,
                "element {:?} differs in assertionDeclaration: {} on the left, {} on the right",
                id.as_str(),
                Declaration(*left),
                Declaration(*right)
            ),
            Self::ChildOrder { id, left, right } => write!(
                f,
                "element {:?} differs in the order of its children: {} on the left, {} on the right",
                id.as_str(),
                Ids(left),
                Ids(right)
            ),
        }
    }
}

/// A list of ids for `Display`: `["a", "b"]`.
struct Ids<'a>(&'a [ElementId]);

impl fmt::Display for Ids<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (position, id) in self.0.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{:?}", id.as_str())?;
        }
        f.write_str("]")
    }
}

/// A parent for `Display`: its quoted id, or `no parent` for a root.
struct Parent<'a>(Option<&'a ElementId>);

impl fmt::Display for Parent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(id) => write!(f, "{:?}", id.as_str()),
            None => f.write_str("no parent (a root)"),
        }
    }
}

/// A name for `Display`: `no name` when missing, otherwise both fields, each
/// quoted or written as absent, so absent and empty read differently.
struct Name<'a>(Option<&'a LangString>);

impl fmt::Display for Name<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(name) = self.0 else {
            return f.write_str("no name");
        };
        match name.lang() {
            Some(lang) => write!(f, "[lang {lang:?}, ")?,
            None => f.write_str("[no lang, ")?,
        }
        match name.content() {
            Some(content) => write!(f, "content {content:?}]"),
            None => f.write_str("no content]"),
        }
    }
}

/// A name's id for `Display`: quoted, or `no id`.
struct NameId<'a>(Option<&'a ElementId>);

impl fmt::Display for NameId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(id) => write!(f, "{:?}", id.as_str()),
            None => f.write_str("no id"),
        }
    }
}

/// A declaration for `Display`: its SACM literal, or a note that the kind
/// has none.
struct Declaration(Option<AssertionDeclaration>);

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(declaration) => write!(f, "{declaration}"),
            None => f.write_str("none (not an Assertion)"),
        }
    }
}

/// Every difference in SACM content between `left` and `right`, in a fixed
/// order; an empty result means the documents are equal.
///
/// Elements are matched by id across the whole document. For an element both
/// documents hold, its kind, parent, name, its name's `xmi:id`,
/// `assertionDeclaration` and the
/// relative order of its children are compared, even when its kind differs.
/// The relative order of the roots is compared too. The per-kind counters
/// that generate ids are not compared.
///
/// Order is compared because equal documents must write equal bytes, though
/// SACM 2.3 orders no composition. To compare without order, drop the two
/// order variants (the [module example](crate::compare) shows two documents
/// that differ only in order):
///
/// ```
/// # use sacm::compare::{Difference, semantic_compare};
/// # let (left, right) = (sacm::model::Document::new(), sacm::model::Document::new());
/// let mut differences = semantic_compare(&left, &right);
/// differences.retain(|d| !matches!(d, Difference::RootOrder { .. } | Difference::ChildOrder { .. }));
/// let same_ignoring_order = differences.is_empty();
/// # assert!(same_ignoring_order);
/// ```
///
/// The order of the result:
/// 1. [`Difference::RootOrder`];
/// 2. for each element of `left`, in pre-order: [`Difference::LeftOnly`], or
///    else [`Kind`](Difference::Kind), [`Parent`](Difference::Parent),
///    [`Name`](Difference::Name), [`NameId`](Difference::NameId),
///    [`AssertionDeclaration`](Difference::AssertionDeclaration) and
///    [`ChildOrder`](Difference::ChildOrder), in that order;
/// 3. [`Difference::RightOnly`] for each element only `right` holds, in
///    `right`'s pre-order.
///
/// Swapping the arguments gives the same differences with left and right
/// swapped, though possibly in another order. The comparison does not recurse and takes time linear in the
/// number of elements, so it is safe on arbitrarily deep documents.
///
/// # Examples
///
/// ```
/// use sacm::compare::{Difference, semantic_compare};
/// use sacm::edit::{self, CreateArgumentPackage};
/// use sacm::model::{Document, LangString};
///
/// let mut left = Document::new();
/// edit::apply(&mut left, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// let mut right = Document::new();
/// edit::apply(&mut right, CreateArgumentPackage::root(LangString::new("Evidence")).with_id("p"))?;
/// assert_eq!(
///     semantic_compare(&left, &right),
///     [Difference::Name {
///         id: "p".into(),
///         left: Some(LangString::new("Arguments")),
///         right: Some(LangString::new("Evidence")),
///     }]
/// );
/// assert_eq!(semantic_compare(&left, &left.clone()), []);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[must_use]
pub fn semantic_compare(left: &Document, right: &Document) -> Vec<Difference> {
    let mut differences = Vec::new();
    // The sink never breaks, so the walk always runs to the end.
    let _ = walk(left, right, |found| {
        differences.push(found.into_difference(left, right));
        ControlFlow::Continue(())
    });
    differences
}

/// One difference as [`walk`] finds it: borrowed from the two documents, so
/// finding it allocates nothing. [`Found::into_difference`] makes the owned
/// [`Difference`].
enum Found<'a> {
    /// The common roots are in a different relative order.
    RootOrder,
    /// Only the left document holds this element.
    LeftOnly(Compared<'a>),
    /// The element's kind differs; the left and the right features.
    Kind(Compared<'a>, Compared<'a>),
    /// The element's parent differs.
    Parent(Compared<'a>, Compared<'a>),
    /// The element's name differs.
    Name(Compared<'a>, Compared<'a>),
    /// The `xmi:id` of the element's name differs.
    NameId(Compared<'a>, Compared<'a>),
    /// The element's `assertionDeclaration` differs.
    AssertionDeclaration(Compared<'a>, Compared<'a>),
    /// The element's common children are in a different relative order; the
    /// element in the left and in the right document.
    ChildOrder(SacmElement<'a>, SacmElement<'a>),
    /// Only the right document holds this element.
    RightOnly(SacmElement<'a>),
}

impl Found<'_> {
    /// The owned [`Difference`], for the documents the walk compared.
    fn into_difference(self, left: &Document, right: &Document) -> Difference {
        match self {
            Self::RootOrder => Difference::RootOrder {
                left: common_roots(left, right).cloned().collect(),
                right: common_roots(right, left).cloned().collect(),
            },
            Self::LeftOnly(ours) => Difference::LeftOnly {
                id: ours.id.clone(),
                kind: ours.kind,
            },
            Self::Kind(ours, theirs) => Difference::Kind {
                id: ours.id.clone(),
                left: ours.kind,
                right: theirs.kind,
            },
            Self::Parent(ours, theirs) => Difference::Parent {
                id: ours.id.clone(),
                left: ours.parent.cloned(),
                right: theirs.parent.cloned(),
            },
            Self::Name(ours, theirs) => Difference::Name {
                id: ours.id.clone(),
                left: ours.name.cloned(),
                right: theirs.name.cloned(),
            },
            Self::NameId(ours, theirs) => Difference::NameId {
                id: ours.id.clone(),
                left: ours.name_id.cloned(),
                right: theirs.name_id.cloned(),
            },
            Self::AssertionDeclaration(ours, theirs) => Difference::AssertionDeclaration {
                id: ours.id.clone(),
                left: ours.assertion_declaration,
                right: theirs.assertion_declaration,
            },
            Self::ChildOrder(ours, theirs) => Difference::ChildOrder {
                id: ours.id().clone(),
                left: common_children(ours, right).cloned().collect(),
                right: common_children(theirs, left).cloned().collect(),
            },
            Self::RightOnly(element) => Difference::RightOnly {
                id: element.id().clone(),
                kind: element.kind(),
            },
        }
    }
}

/// The one comparison walk behind [`semantic_compare`] and `==`: gives each
/// difference to `sink`, in [`semantic_compare`]'s order, until `sink`
/// breaks. Iterative and linear in the number of elements.
fn walk<'a>(
    left: &'a Document,
    right: &'a Document,
    mut sink: impl FnMut(Found<'a>) -> ControlFlow<()>,
) -> ControlFlow<()> {
    if common_roots(left, right).ne(common_roots(right, left)) {
        sink(Found::RootOrder)?;
    }

    for element in left.elements() {
        let ours = element.compared();
        let Some(other) = right.get(ours.id) else {
            sink(Found::LeftOnly(ours))?;
            continue;
        };
        let theirs = other.compared();
        if ours.kind != theirs.kind {
            sink(Found::Kind(ours, theirs))?;
        }
        if ours.parent != theirs.parent {
            sink(Found::Parent(ours, theirs))?;
        }
        if ours.name != theirs.name {
            sink(Found::Name(ours, theirs))?;
        }
        if ours.name_id != theirs.name_id {
            sink(Found::NameId(ours, theirs))?;
        }
        if ours.assertion_declaration != theirs.assertion_declaration {
            sink(Found::AssertionDeclaration(ours, theirs))?;
        }
        if common_children(element, right).ne(common_children(other, left)) {
            sink(Found::ChildOrder(element, other))?;
        }
    }

    for element in right.elements() {
        if !left.contains(element.id()) {
            sink(Found::RightOnly(element))?;
        }
    }
    ControlFlow::Continue(())
}

/// The ids of the roots of `document` that are roots of `other` too, in
/// `document`'s order.
fn common_roots<'a>(
    document: &'a Document,
    other: &'a Document,
) -> impl Iterator<Item = &'a ElementId> {
    document
        .compared_roots()
        .filter(|root| {
            other
                .get(root.id())
                .is_some_and(|theirs| theirs.compared().parent.is_none())
        })
        .map(SacmElement::id)
}

/// The ids of the children of `element` whose parent in `other` is an
/// element with the same id, in `element`'s order.
fn common_children<'a>(
    element: SacmElement<'a>,
    other: &'a Document,
) -> impl Iterator<Item = &'a ElementId> {
    let parent = element.id();
    element
        .children()
        .filter(move |child| {
            other
                .get(child.id())
                .is_some_and(|theirs| theirs.compared().parent == Some(parent))
        })
        .map(SacmElement::id)
}

impl PartialEq for Document {
    /// Whether the documents hold the same SACM content: whether
    /// [`semantic_compare`] finds no difference. It stops at the first
    /// difference and builds no [`Difference`]. The per-kind counters that
    /// generate ids are not compared.
    fn eq(&self, other: &Self) -> bool {
        walk(self, other, |_| ControlFlow::Break(())).is_continue()
    }
}

impl Eq for Document {}
