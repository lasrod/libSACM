//! The document and its read-only element views.

use std::collections::HashMap;
use std::fmt;

use super::{ElementId, ElementKind};

/// A SACM 2.3 document: one interchange unit, owning its root packages and
/// every element under them.
///
/// A document is read through borrowing views. [`roots`](Self::roots) yields
/// the top-level packages, [`get`](Self::get) looks an element up by its
/// [`ElementId`], and [`elements`](Self::elements) visits every element in
/// document order. Each view is a [`SacmElement`], which navigates to its
/// [`parent`](SacmElement::parent) and [`children`](SacmElement::children).
///
/// The document's storage is private. Documents are filled by the library
/// itself; this release has no public way to build one beyond
/// [`Document::new`], which is empty.
///
/// Cloning a document copies all of it; the copy is independent of the
/// original. Cloning, dropping and traversal do not recurse, so arbitrarily
/// deep nesting is safe.
///
/// # Examples
///
/// ```
/// use sacm::model::Document;
///
/// let doc = Document::new();
/// assert_eq!(doc.element_count(), 0);
/// assert!(doc.roots().next().is_none());
/// assert!(doc.get("claim_top").is_none());
/// ```
#[derive(Clone, Default)]
pub struct Document {
    /// Every element, in insertion order. A node's position is its handle.
    nodes: Vec<Node>,
    /// The handles of the root elements, in document order.
    roots: Vec<usize>,
    /// The handle of each element by id. Only looked up, never iterated, so
    /// nothing observable depends on its order.
    index: HashMap<ElementId, usize>,
}

/// One element of a [`Document`].
#[derive(Clone, Debug)]
struct Node {
    id: ElementId,
    kind: ElementKind,
    /// The handle of the containing element; `None` for a root.
    parent: Option<usize>,
    /// The handles of the contained elements, in containment order.
    children: Vec<usize>,
}

impl Document {
    /// An empty document: no roots and no elements.
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// let doc = Document::new();
    /// assert_eq!(doc.elements().count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The root elements, in document order.
    ///
    /// A root is an element with no parent: the top-level packages of the
    /// interchange unit. They may be of any package kind.
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// # use sacm::model::{ElementId, ElementKind};
    /// let mut doc = Document::new();
    /// # doc.insert(None, ElementKind::AssuranceCasePackage, ElementId::from("safety_case")).unwrap();
    /// # doc.insert(None, ElementKind::ArtifactPackage, ElementId::from("evidence")).unwrap();
    /// let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    /// assert_eq!(roots, ["safety_case", "evidence"]);
    /// ```
    pub fn roots(&self) -> impl Iterator<Item = SacmElement<'_>> {
        self.roots.iter().map(|&node| self.view(node))
    }

    /// The element with the given id, or `None` if the document has none.
    ///
    /// Ids are compared exactly, byte for byte. The id may be given as a
    /// `&str`, a `&String` or an `&ElementId`.
    ///
    /// ```
    /// use sacm::model::{Document, ElementKind};
    ///
    /// # use sacm::model::ElementId;
    /// let mut doc = Document::new();
    /// # doc.insert(None, ElementKind::ArgumentPackage, ElementId::from("arguments")).unwrap();
    /// let arguments = doc.get("arguments").expect("the document has it");
    /// assert_eq!(arguments.kind(), ElementKind::ArgumentPackage);
    /// assert!(doc.get("Arguments").is_none());
    /// ```
    #[must_use]
    pub fn get<Q: AsRef<str> + ?Sized>(&self, id: &Q) -> Option<SacmElement<'_>> {
        self.index.get(id.as_ref()).map(|&node| self.view(node))
    }

    /// Whether the document has an element with the given id.
    ///
    /// Agrees with [`get`](Self::get): `contains(id)` is `get(id).is_some()`.
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// assert!(!Document::new().contains("claim_top"));
    /// ```
    #[must_use]
    pub fn contains<Q: AsRef<str> + ?Sized>(&self, id: &Q) -> bool {
        self.index.contains_key(id.as_ref())
    }

    /// The number of elements in the document, of every kind and at every depth.
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// assert_eq!(Document::new().element_count(), 0);
    /// ```
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.nodes.len()
    }

    /// Every element, in pre-order document order.
    ///
    /// Each root comes before its descendants, and each element before its
    /// children, which come in containment order. Every element is visited
    /// exactly once. The traversal does not recurse, so it is safe on
    /// arbitrarily deep documents.
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// # use sacm::model::{ElementId, ElementKind};
    /// let mut doc = Document::new();
    /// # let id = ElementId::from;
    /// # doc.insert(None, ElementKind::AssuranceCasePackage, id("case")).unwrap();
    /// # doc.insert(Some(&id("case")), ElementKind::ArgumentPackage, id("arguments")).unwrap();
    /// # doc.insert(Some(&id("case")), ElementKind::ArtifactPackage, id("artifacts")).unwrap();
    /// # doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim")).unwrap();
    /// let order: Vec<&str> = doc.elements().map(|element| element.id().as_str()).collect();
    /// assert_eq!(order, ["case", "arguments", "claim", "artifacts"]);
    /// ```
    pub fn elements(&self) -> impl Iterator<Item = SacmElement<'_>> {
        PreOrder {
            document: self,
            pending: self.roots.iter().rev().copied().collect(),
        }
    }

    /// Adds an element as the last child of `parent`, or as the last root
    /// when `parent` is `None`.
    ///
    /// **Not part of the stable API.** This is the test seam of the
    /// `test-support` feature: it builds documents for tests until the XMI
    /// reader and the edit operations exist. It may change or disappear in
    /// any release. It checks only identity and containment, not whether
    /// SACM allows `kind` under `parent`.
    ///
    /// # Errors
    ///
    /// Refuses, and leaves the document unchanged, when:
    /// - `id` is empty ([`InsertError::EmptyId`]);
    /// - the document already has an element with `id`
    ///   ([`InsertError::DuplicateId`]);
    /// - `parent` is not an element of the document
    ///   ([`InsertError::UnknownParent`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::model::{Document, ElementId, ElementKind, InsertError};
    ///
    /// let mut doc = Document::new();
    /// let case = ElementId::from("case");
    /// doc.insert(None, ElementKind::AssuranceCasePackage, case.clone())?;
    /// doc.insert(Some(&case), ElementKind::ArgumentPackage, ElementId::from("arguments"))?;
    ///
    /// let again = doc.insert(None, ElementKind::ArtifactPackage, case.clone());
    /// assert_eq!(again, Err(InsertError::DuplicateId(case)));
    /// assert_eq!(doc.element_count(), 2);
    /// # Ok::<(), InsertError>(())
    /// ```
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    pub fn insert(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
    ) -> Result<(), InsertError> {
        self.insert_node(parent, kind, id)
    }

    /// Adds an element as the last child of `parent`, or as the last root
    /// when `parent` is `None`, after checking everything: on an error the
    /// document is unchanged.
    ///
    /// The crate-internal form of the `test-support` seam `insert`, present
    /// with or without the feature.
    #[cfg_attr(
        not(feature = "test-support"),
        expect(
            dead_code,
            reason = "the XMI reader and edit operations will build documents through it"
        )
    )]
    pub(crate) fn insert_node(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
    ) -> Result<(), InsertError> {
        if id.as_str().is_empty() {
            return Err(InsertError::EmptyId);
        }
        if self.index.contains_key(&id) {
            return Err(InsertError::DuplicateId(id));
        }
        let parent = match parent {
            None => None,
            Some(parent_id) => match self.index.get(parent_id) {
                Some(&parent) => Some(parent),
                None => return Err(InsertError::UnknownParent(parent_id.clone())),
            },
        };

        let node = self.nodes.len();
        match parent {
            Some(parent) => self.nodes[parent].children.push(node),
            None => self.roots.push(node),
        }
        self.index.insert(id.clone(), node);
        self.nodes.push(Node {
            id,
            kind,
            parent,
            children: Vec::new(),
        });
        Ok(())
    }

    /// The view of the node with handle `node`.
    fn view(&self, node: usize) -> SacmElement<'_> {
        SacmElement {
            document: self,
            node,
        }
    }
}

impl fmt::Debug for Document {
    /// Lists the elements in pre-order document order, each with its kind and
    /// id. The output is deterministic and does not recurse on nesting depth.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Elements<'a>(&'a Document);
        impl fmt::Debug for Elements<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.elements()).finish()
            }
        }
        f.debug_struct("Document")
            .field("element_count", &self.element_count())
            .field("elements", &Elements(self))
            .finish_non_exhaustive()
    }
}

/// The pre-order traversal behind [`Document::elements`]: a stack of the
/// handles still to visit, the next one on top.
struct PreOrder<'a> {
    document: &'a Document,
    pending: Vec<usize>,
}

impl<'a> Iterator for PreOrder<'a> {
    type Item = SacmElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let element = self.document.view(self.pending.pop()?);
        self.pending
            .extend(element.node().children.iter().rev().copied());
        Some(element)
    }
}

/// A read-only view of one element of a [`Document`].
///
/// A view is a cheap, copyable handle that borrows the document. Everything
/// it returns, including the views it navigates to, borrows the document,
/// not the view, so it can outlive the view it came from.
///
/// # Examples
///
/// ```
/// use sacm::model::{Document, ElementKind, SacmElement};
///
/// /// The claims directly under `package`, in containment order.
/// fn claims<'d>(package: SacmElement<'d>) -> impl Iterator<Item = SacmElement<'d>> {
///     package.children().filter(|child| child.kind() == ElementKind::Claim)
/// }
///
/// # use sacm::model::ElementId;
/// let mut doc = Document::new();
/// # let id = ElementId::from;
/// # doc.insert(None, ElementKind::ArgumentPackage, id("arguments")).unwrap();
/// # doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_top")).unwrap();
/// # doc.insert(Some(&id("arguments")), ElementKind::ArgumentReasoning, id("reasoning")).unwrap();
/// # doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_sub")).unwrap();
/// let arguments = doc.get("arguments").expect("the document has it");
/// let ids: Vec<&str> = claims(arguments).map(|claim| claim.id().as_str()).collect();
/// assert_eq!(ids, ["claim_top", "claim_sub"]);
///
/// let claim = doc.get("claim_sub").expect("the document has it");
/// assert_eq!(claim.parent().map(|parent| parent.id().as_str()), Some("arguments"));
/// ```
#[derive(Clone, Copy)]
pub struct SacmElement<'a> {
    document: &'a Document,
    /// The element's handle in the document's arena.
    node: usize,
}

impl<'a> SacmElement<'a> {
    /// The element's id.
    #[must_use]
    pub fn id(self) -> &'a ElementId {
        &self.node().id
    }

    /// The element's kind.
    #[must_use]
    pub fn kind(self) -> ElementKind {
        self.node().kind
    }

    /// The element that contains this one, or `None` for a root.
    #[must_use]
    pub fn parent(self) -> Option<SacmElement<'a>> {
        self.node().parent.map(|parent| self.document.view(parent))
    }

    /// The elements this one contains directly, in containment order.
    ///
    /// The order is the document order, across every kind of child.
    pub fn children(self) -> impl Iterator<Item = SacmElement<'a>> {
        let document = self.document;
        self.node()
            .children
            .iter()
            .map(move |&child| document.view(child))
    }

    /// The element's node in the arena.
    fn node(self) -> &'a Node {
        // Views are only made from handles the document handed out, and
        // nodes are never removed while the document is borrowed.
        &self.document.nodes[self.node]
    }
}

impl fmt::Debug for SacmElement<'_> {
    /// Shows the element's kind and id, not the document it borrows.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SacmElement")
            .field("kind", &self.kind())
            .field("id", self.id())
            .finish_non_exhaustive()
    }
}

// `Document::insert` is not an intra-doc link here: without the feature, that
// method does not exist.
/// Why `Document::insert` refused an element.
///
/// **Not part of the stable API**: public only under the `test-support`
/// feature, with `Document::insert`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
#[cfg_attr(
    not(feature = "test-support"),
    expect(
        unreachable_pub,
        reason = "public only under the test-support feature, crate-visible otherwise"
    )
)]
pub enum InsertError {
    /// The id is the empty string.
    EmptyId,
    /// The document already has an element with this id.
    DuplicateId(ElementId),
    /// The parent is not an element of the document.
    UnknownParent(ElementId),
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("the element id is empty"),
            Self::DuplicateId(id) => {
                write!(f, "the document already has an element with id \"{id}\"")
            }
            Self::UnknownParent(id) => {
                write!(f, "the document has no element \"{id}\" to be the parent")
            }
        }
    }
}

impl std::error::Error for InsertError {}
