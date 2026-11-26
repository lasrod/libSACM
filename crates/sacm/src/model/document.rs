//! The document and its read-only element views.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use super::containment::{Role, role};
use super::{
    AbstractClass, AssertionDeclaration, Attr, ElementId, ElementKind, Flag, Href, LangString,
    MultiLangString, Preserved, Proxy, ReferenceEnd, Revision, Subclass,
};

/// A SACM 2.3 document: one interchange unit (what one SACM XMI file holds),
/// owning its root packages and every element under them.
///
/// A document is read through borrowing views. [`roots`](Self::roots) yields
/// the top-level packages, [`get`](Self::get) looks an element up by its
/// [`ElementId`], and [`elements`](Self::elements) visits every element. Each
/// view is a [`SacmElement`], which navigates to its
/// [`parent`](SacmElement::parent) and [`children`](SacmElement::children).
///
/// **Document order** is the order in which elements were added: for a loaded
/// file, the order they appear in it. It applies to the roots, and to each
/// element's children across every kind of child.
///
/// **Containment**: each child is held in a composite role of its parent's
/// class (SACM 2.3). Every `ModelElement` can hold descriptions,
/// implementation constraints, notes and tagged values (§8.6); an
/// `AssuranceCasePackage` holds sub-packages (§9.2); an `ArgumentPackage`
/// holds argumentation elements (§11.4); a `TerminologyPackage` holds
/// terminology elements (§10.4); an `ArtifactPackage` holds any
/// `ArtifactElement`, a `Claim` or a package included (§12.2, [ADR 0014]); and
/// an `ArtifactAsset` holds properties (§12.6). Interfaces and bindings hold
/// what their package holds, a `UtilityElement` contains nothing, and a group
/// holds only utility elements, because its members are references. How many
/// children a role holds is left to validation. So is the §11.4 rule that an
/// `ArgumentPackage` holding nested packages holds only packages.
///
/// [ADR 0014]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0014-artifact-package-admits-every-artifact-element.md
///
/// In this release [`xmi::read`](crate::xmi::read) fills a document with
/// roots of the twelve package kinds (the four packages, their interfaces
/// and their bindings) and with elements of every [`ElementKind`].
/// [`edit::apply`](crate::edit::apply) creates every kind, edits
/// the features and the order of elements of every kind, moves and copies
/// them ([`MoveElement`](crate::edit::MoveElement),
/// [`CopyElements`](crate::edit::CopyElements)), and deletes
/// elements of every kind ([`DeleteElement`](crate::edit::DeleteElement)).
/// [`Document::new`] is empty.
///
/// **References** between elements, such as
/// [`SacmElement::cited_element`] and [`SacmElement::sources`], are held as
/// the ids they name, not as views: [`get`](Self::get) resolves them. A
/// multi-valued end, such as `source`, keeps its ids in order, repeats
/// included (D-ORDER, ADR 0027). A document may hold a reference whose id
/// names no element, as a file can (ADR 0026);
/// [`validation::validate`](crate::validation::validate) reports it
/// (SACM-REF-001), no edit makes one (an edit keeps one the end already
/// holds, ADR 0029), and no id an edit generates is one a reference names.
/// Likewise a document may hold a reference to an element of a kind its end
/// does not admit, such as a `metaClaim` naming an `ArgumentReasoning`;
/// `validate` reports it (SACM-REF-002), and no edit makes one.
///
/// **Preserved content** is what [`xmi::read`](crate::xmi::read) kept, as
/// written, because it is not SACM 2.3 where it stands, such as a vendor
/// attribute or child element (ADR 0033), or, in tolerant mode, because a
/// strict read refuses it, such as a value that is no literal (ADR 0035);
/// XMI metadata, such as `xmi:uuid`, is not preserved content (ADR 0042).
/// Each element that holds some says so
/// ([`SacmElement::has_preserved_content`]), and the document holds what
/// is on or under `xmi:XMI` outside its elements, placed among the roots
/// ([`has_preserved_content`](Self::has_preserved_content) tells both). It
/// has no API of its own besides that; [`xmi::write`](crate::xmi::write)
/// writes it back in tolerant mode. The ids of its elements stay in the
/// document's id space: [`get`](Self::get) does not find them, no edit can
/// take or name one, and deleting the element that holds them frees them.
///
/// Cloning a document copies all of it, preserved content included, and the
/// counters that generated ids continue from; the copy is independent of the
/// original. Cloning, dropping and traversal do not recurse, so arbitrarily
/// deep nesting is safe, in preserved content too.
///
/// # Equality
///
/// `==` compares SACM content: two documents are equal exactly when
/// [`semantic_compare`](crate::compare::semantic_compare) finds no
/// difference between them. Elements are matched by id; kinds, parents,
/// `gid`s, the `isCitation`, `isAbstract` and `isCounter` flags, the ids
/// each reference end holds (`citedElement`, `abstractForm`, `metaClaim`,
/// `reasoning`, `source`, `target`, `structure`, `referencedArtifactElement`,
/// an `ArgumentGroup`'s `argumentElement` and an `ArtifactGroup`'s
/// `artifactElement`, in order), names, the
/// ids of names, `content`s, `key`s and `location`s with their ids,
/// `version`s, `date`s, `startTime`s, `endTime`s, `assertionDeclaration`s, the order of roots and children, each element's
/// preserved content and the document's own, outside its elements, are
/// compared. Formatting, attribute order and
/// namespace prefixes are not held, except in preserved content, which is
/// compared exactly. `==` is order-sensitive, for the ids of the
/// multi-valued ends too; see [`crate::compare`] to ignore order.
///
/// Not compared: the per-kind counters [`edit::apply`](crate::edit::apply)
/// uses to generate ids such as `claim_1`. They are not SACM content, and XMI
/// does not carry them. So two equal documents may generate different next
/// ids: an edit can free an id, such as the `xmi:id` of a `value` that
/// [`SetContent`](crate::edit::SetContent) drops, and a document whose
/// counter has passed it generates above it, while one whose counter has
/// not generates it. Nor is the [`revision`](Self::revision) compared: two
/// reads of one file are `==` and have different revisions.
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
///
/// # Serde
///
/// A document is not serialized with serde: XMI, through
/// [`xmi::write`](crate::xmi::write) and [`xmi::read`](crate::xmi::read), is
/// its only serialization (ADR 0030). The `serde` feature serializes the
/// operations that edit it and the values they carry instead.
#[cfg_attr(
    feature = "serde",
    doc = r"
With the feature, this does not compile:

```compile_fail
fn serializable<T: serde::Serialize + serde::de::DeserializeOwned>() {}
serializable::<sacm::model::Document>();
```

while the same check of an operation does:

```
fn serializable<T: serde::Serialize + serde::de::DeserializeOwned>() {}
serializable::<sacm::edit::Operation>();
```
"
)]
#[derive(Clone)]
pub struct Document {
    /// Every element, in insertion order. A node's position is its handle.
    nodes: Vec<Node>,
    /// The handles of the root elements, in document order.
    roots: Vec<usize>,
    /// The handle of each element by id. Only looked up, never iterated, so
    /// nothing observable depends on its order.
    index: HashMap<ElementId, usize>,
    /// How each id that is not in `index` is used: as an id that identifies
    /// no element, the `xmi:id` of a `name` (ADR 0024), of a `content` or
    /// `key`, or of a `value` in one (ADR 0025), which shares the file's id
    /// space though [`get`](Self::get) does not look in it; or as a target of
    /// references (ADR 0026), so that a generated id never makes a dangling
    /// reference resolve. An element id that references hold is counted
    /// here too. Derived from the nodes; only looked up, never iterated.
    /// (One map, not two, keeps `Document` the size it was.)
    other_ids: HashMap<ElementId, IdUse>,
    /// The `n` of the last id generated for each kind (`claim_<n>`, ADR
    /// 0019); a kind with none generated yet is absent. Advanced only when a
    /// generated id is used, and copied by `Clone`.
    id_counters: BTreeMap<ElementKind, u64>,
    /// The state of the document within the process (ADR 0031): fresh for
    /// a new document and after every successful edit, copied by `Clone`.
    revision: Revision,
    /// The preserved content outside every element (ADR 0035 D3): the
    /// attributes of the `xmi:XMI` element besides its XMI metadata, and its
    /// children that are neither a root nor XMI metadata, such as an
    /// `xmi:foo` XMI does not define, placed among the roots. `None` when it
    /// holds none.
    preserved: Option<Box<Preserved>>,
    /// The XMI metadata outside every element (SACM23-XMI-019, ADR 0042):
    /// the metadata attributes of the `xmi:XMI` element, such as `xmi:uuid`,
    /// and its XMI elements, such as `xmi:Documentation`, placed among the
    /// roots. `None` when it holds none.
    metadata: Option<Box<Preserved>>,
}

impl Default for Document {
    /// An empty document, with a fresh [`Revision`]: the same as
    /// [`Document::new`].
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            index: HashMap::new(),
            other_ids: HashMap::new(),
            id_counters: BTreeMap::new(),
            revision: Revision::fresh(),
            preserved: None,
            metadata: None,
        }
    }
}

/// How an id of `Document::other_ids` is used.
#[derive(Clone, Debug, Default)]
struct IdUse {
    /// The handle of the element whose `name`, `content`, `key`, `location` or `value`
    /// carries the id as its `xmi:id`, or whose preserved content does, if
    /// one does.
    owner: Option<usize>,
    /// Whether the owner holds the id in its preserved content (ADR 0033) or
    /// its XMI metadata (ADR 0042 D5), rather than on a `name`, `content`,
    /// `key`, `location` or `value`.
    preserved: bool,
    /// Whether the document's own preserved content or XMI metadata, outside
    /// every element, holds the id (ADR 0035 D3, ADR 0042 D5); `owner` is
    /// then `None`.
    document: bool,
    /// How many references, of every element and end, hold the id: the
    /// typed ones and those preserved content may hold.
    references: usize,
    /// The handle of the element that holds the id in a typed end, once per
    /// occurrence, in no particular order: the index of
    /// [`Document::referrers`] (ADR 0036 D3). Preserved content's references
    /// are not typed, so they are counted in `references` only.
    holders: Vec<usize>,
}

impl IdUse {
    /// Whether nothing uses the id any more, so its entry can go.
    fn is_unused(&self) -> bool {
        self.owner.is_none() && !self.document && self.references == 0
    }
}

/// One element of a [`Document`].
#[derive(Clone, Debug)]
struct Node {
    id: ElementId,
    kind: ElementKind,
    /// The element's `gid` (§8.2), exactly as held; `None` when absent.
    gid: Option<String>,
    /// The element's name; `None` for a missing name, and always `None` for a
    /// `UtilityElement` kind.
    name: Option<LangString>,
    /// The `xmi:id` of the element's name, if it was read with one: `None`
    /// whenever `name` is (ADR 0024).
    name_id: Option<ElementId>,
    /// The element's `content` (§8.7): `None` when absent, and always `None`
    /// unless the kind is a `UtilityElement`.
    content: Option<Text>,
    /// The element's `key` (§8.12): `None` when absent, and always `None`
    /// unless the kind is `TaggedValue`.
    key: Option<Text>,
    /// The element's `location` (§12.10): `None` when absent, and always
    /// `None` unless the kind is `Resource`.
    location: Option<Text>,
    /// The string-valued attributes the element holds, each exactly as held
    /// (D-DATE): only the ones given, each once, each one the kind holds
    /// ([`StringAttribute::is_held_by`]), in no particular order (readers
    /// look one up).
    strings: Vec<(StringAttribute, String)>,
    /// The element's `assertionDeclaration`: `Some` exactly when the kind is
    /// an Assertion.
    assertion_declaration: Option<AssertionDeclaration>,
    /// `isCitation` (§8.2): absent is `false`.
    is_citation: bool,
    /// `isAbstract` (§8.2): absent is `false`.
    is_abstract: bool,
    /// `isCounter` (§11.13): absent is `false`, and always `false` unless
    /// the kind is an `AssertedRelationship`.
    is_counter: bool,
    /// The ids each reference end of the element holds, unresolved and in
    /// document order (ADR 0026): only the ends that hold one, each once, in
    /// no particular order (readers look an end up), and never an empty list.
    references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    /// The cross-document references (`href`s) each reference end of the
    /// element holds, as read and in document order (ADR 0040 D2): only the
    /// ends that hold one, each once, in no particular order, and never an
    /// empty list. They name no element of this document, so no id use
    /// counts them.
    hrefs: Vec<(ReferenceEnd, Vec<Href>)>,
    /// The element's proxies, in document order (SACM23-XMI-029, ADR 0040
    /// amendment 1, A2): each a child in one of its composite roles that is
    /// held in another document. They are no elements of this document, so
    /// no id use and no child list counts them.
    proxies: Vec<Proxy>,
    /// The handle of the containing element; `None` for a root.
    parent: Option<usize>,
    /// The element's index among its parent's children, or among the
    /// roots: derived from `children` and `roots`, so that
    /// [`Document::referrers`] can order holders by document order without
    /// walking the document (ADR 0036 D3).
    slot: usize,
    /// The role of the parent's class that holds the element: `Some` exactly
    /// when it has a parent.
    role: Option<Role>,
    /// The handles of the contained elements, in containment order.
    children: Vec<usize>,
    /// What the element was read with and kept as written instead of read:
    /// content that is not SACM 2.3 where it stands (LIB-COMPAT-003, ADR
    /// 0033), or, in tolerant mode, a construct a strict read refuses (ADR
    /// 0035); `None` when it holds none. Boxed, so an
    /// element without it costs one word.
    preserved: Option<Box<Preserved>>,
    /// The element's XMI metadata (SACM23-XMI-019, ADR 0042): `xmi:uuid`,
    /// `xmi:label` and the XSI schema locations on its start tag, and its
    /// `xmi:extension` and `xmi:Extension` children, placed among its typed
    /// children; `None` when it holds none.
    metadata: Option<Box<Preserved>>,
    /// The XMI metadata of the element's `name`, its children all placed at
    /// index 0; `None` when it holds none, and whenever `name` is `None`.
    name_metadata: Option<Box<Preserved>>,
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

    /// The document's current state within the process, for
    /// [`edit::apply_preview`](crate::edit::apply_preview) to tell whether
    /// the document changed after a preview was taken (LIB-EDIT-007).
    ///
    /// A new, default or read document has a revision no other document of
    /// the process has had, and every successful edit gives a new one, even
    /// one that changes nothing. A clone has its original's revision until
    /// either changes. A refused edit, [`edit::check`](crate::edit::check)
    /// and [`edit::preview`](crate::edit::preview) keep it. See
    /// [`Revision`].
    ///
    /// ```
    /// use sacm::model::Document;
    ///
    /// let (first, second) = (Document::new(), Document::new());
    /// assert!(first == second); // equal content
    /// assert_ne!(first.revision(), second.revision()); // different states
    /// ```
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Gives the document a fresh revision: called once by each successful
    /// edit (ADR 0031).
    pub(crate) fn renew_revision(&mut self) {
        self.revision = Revision::fresh();
    }

    /// The root elements, in document order.
    ///
    /// A root is an element with no parent. Every root is a package: an
    /// assurance case, argument, artifact or terminology package, or the
    /// interface or binding of one.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let case = CreateAssuranceCasePackage::root(LangString::new("Safety case")).with_id("safety_case");
    /// edit::apply(&mut doc, case)?;
    /// let arguments = CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments");
    /// edit::apply(&mut doc, arguments)?;
    ///
    /// let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    /// assert_eq!(roots, ["safety_case", "arguments"]);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    pub fn roots(&self) -> impl Iterator<Item = SacmElement<'_>> {
        self.roots.iter().map(|&node| self.view(node))
    }

    /// The element with the given id, or `None` if the document has none.
    /// It resolves a reference such as [`SacmElement::cited_element`]: `None`
    /// means the reference names no element of the document (SACM-REF-001).
    ///
    /// Ids are compared exactly, byte for byte. The id may be given as a
    /// `&str`, a `&String` or an `&ElementId`.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// let arguments = CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments");
    /// edit::apply(&mut doc, arguments)?;
    ///
    /// let arguments = doc.get("arguments").expect("the document has it");
    /// assert_eq!(arguments.kind(), ElementKind::ArgumentPackage);
    /// assert!(doc.get("Arguments").is_none());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn get<Q: AsRef<str> + ?Sized>(&self, id: &Q) -> Option<SacmElement<'_>> {
        self.index.get(id.as_ref()).map(|&node| self.view(node))
    }

    /// Whether the document has an element with the given id.
    ///
    /// Agrees with [`get`](Self::get): `contains(id)` is `get(id).is_some()`.
    /// A name's `xmi:id` ([`SacmElement::name_id`]) identifies no element,
    /// so `contains` is false for it, but [`edit::apply`](crate::edit::apply)
    /// still refuses it as a new element's id.
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

    /// Every element in document order, pre-order (parents before children).
    ///
    /// Every element is visited exactly once. The traversal does not recurse,
    /// so it is safe on arbitrarily deep documents.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = || LangString::new("N");
    /// edit::apply(&mut doc, CreateAssuranceCasePackage::root(name()).with_id("case"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::new("case", name()).with_id("arguments"))?;
    /// edit::apply(&mut doc, CreateClaim::new("arguments", name()).with_id("claim"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::new("case", name()).with_id("more"))?;
    ///
    /// let order: Vec<&str> = doc.elements().map(|element| element.id().as_str()).collect();
    /// assert_eq!(order, ["case", "arguments", "claim", "more"]);
    /// # Ok::<(), sacm::edit::EditError>(())
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
    /// `test-support` feature: it builds documents for tests with the kinds
    /// and states that the XMI reader and [`edit::apply`](crate::edit::apply)
    /// do not produce yet. It may change or disappear in
    /// any release. It checks identity, that `parent` exists, whether `kind`
    /// can have a name, and whether a containment role of the parent's class
    /// admits `kind` (see [`Document`]). It does not check how many children a
    /// role holds; validation will. A root must be a package kind.
    ///
    /// `name` is the element's `name` (SACM 2.3 §8.6). `None` is a missing
    /// name, held as missing ([ADR 0013]). Only `ModelElement` kinds have a
    /// name, so the four `UtilityElement` kinds take `None`.
    ///
    /// [ADR 0013]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0013-missing-name-stays-missing.md
    ///
    /// # Errors
    ///
    /// Refuses, and leaves the document unchanged, when, checked in this order:
    /// - `id` is empty ([`InsertError::EmptyId`]);
    /// - the document already has an element, or the name of one, with `id`
    ///   ([`InsertError::DuplicateId`]);
    /// - `parent` is not an element of the document
    ///   ([`InsertError::UnknownParent`]);
    /// - `name` is `Some` and `kind` is not a `ModelElement` kind
    ///   ([`InsertError::NameNotAllowed`]);
    /// - `parent`'s class has no SACM containment role that admits `kind`
    ///   ([`InsertError::ContainmentNotAllowed`]);
    /// - `parent` is `None` and `kind` is not a package kind
    ///   ([`InsertError::RootNotAllowed`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::model::{Document, ElementId, ElementKind, InsertError, LangString};
    ///
    /// let mut doc = Document::new();
    /// let case = ElementId::from("case");
    /// let name = LangString::new("Safety case").with_lang("en");
    /// doc.insert(None, ElementKind::AssuranceCasePackage, case.clone(), Some(name))?;
    /// // An argument package whose name is missing.
    /// doc.insert(Some(&case), ElementKind::ArgumentPackage, ElementId::from("arguments"), None)?;
    ///
    /// let again = doc.insert(None, ElementKind::ArtifactPackage, case.clone(), None);
    /// assert_eq!(again, Err(InsertError::DuplicateId(case.clone())));
    ///
    /// // A Note is a UtilityElement, so it has no name.
    /// let arguments = ElementId::from("arguments");
    /// let note = ElementId::from("note_1");
    /// let named = doc.insert(Some(&arguments), ElementKind::Note, note.clone(), Some(LangString::new("n")));
    /// assert_eq!(named, Err(InsertError::NameNotAllowed(note, ElementKind::Note)));
    ///
    /// // An AssuranceCasePackage holds only packages (and utility elements),
    /// // so a Claim cannot sit directly in it.
    /// let claim = ElementId::from("c1");
    /// let misplaced = doc.insert(Some(&case), ElementKind::Claim, claim.clone(), None);
    /// assert_eq!(
    ///     misplaced,
    ///     Err(InsertError::ContainmentNotAllowed {
    ///         id: claim,
    ///         kind: ElementKind::Claim,
    ///         parent: case,
    ///         parent_kind: ElementKind::AssuranceCasePackage,
    ///     })
    /// );
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
        name: Option<LangString>,
    ) -> Result<(), InsertError> {
        self.insert_node(parent, kind, id, name)?;
        self.renew_revision();
        Ok(())
    }

    /// Adds an element as the last child of `parent`, or as the last root
    /// when `parent` is `None`, after checking everything: on an error the
    /// document is unchanged.
    ///
    /// The crate-internal form of the `test-support` seam `insert`, present
    /// with or without the feature.
    pub(crate) fn insert_node(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: Option<LangString>,
    ) -> Result<(), InsertError> {
        self.insert_element(parent, kind, id, name, None, None)
    }

    /// [`insert_node`](Self::insert_node), with the element's `content` and
    /// `key`, held without ids.
    ///
    /// The caller gives a `content` only for a `UtilityElement` kind and a
    /// `key` only for a `TaggedValue`, as the create operations do (ADR
    /// 0025); the reader sets them with [`set_text`](Self::set_text), which
    /// checks the kind.
    pub(crate) fn insert_element(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: Option<LangString>,
        content: Option<MultiLangString>,
        key: Option<MultiLangString>,
    ) -> Result<(), InsertError> {
        self.insert_handle(parent, kind, id, name, content, key)
            .map(|_| ())
    }

    /// [`insert_node`](Self::insert_node), with the element's flags and
    /// reference ends set as given, unresolved and in order (ADR 0027): on
    /// an error the document is unchanged.
    ///
    /// The create operations of the argumentation kinds use it, so that a
    /// create is one change. Their checks come first: each flag and end is
    /// one the kind has, a single-valued end gets one target, and every
    /// target is the id of an element, so none is empty.
    pub(crate) fn insert_with_references(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: Option<LangString>,
        flags: &[(Flag, bool)],
        references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    ) -> Result<(), InsertError> {
        let node = self.insert_handle(parent, kind, id, name, None, None)?;
        for &(flag, value) in flags {
            debug_assert!(flag.owner().admits(kind), "the kind has the flag");
            *self.nodes[node].flag_mut(flag) = value;
        }
        for (end, targets) in references {
            debug_assert!(
                reference_problem(kind, end, &targets).is_none(),
                "the kind has the end, and the end can hold the targets"
            );
            self.hold_references(node, end, targets);
        }
        Ok(())
    }

    /// [`insert_node`](Self::insert_node), with the element's string-valued
    /// attributes and its `location` set as given, held exactly and without
    /// ids (ADR 0034 amendment 1): on an error the document is unchanged.
    ///
    /// The create operations of the `ArtifactAsset` kinds use it, so that a
    /// create is one change. Each attribute is one the kind holds, given
    /// once, and a `location` is given only for a `Resource`.
    pub(crate) fn insert_asset(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: LangString,
        strings: Vec<(StringAttribute, String)>,
        location: Option<MultiLangString>,
    ) -> Result<(), InsertError> {
        debug_assert!(
            strings
                .iter()
                .all(|(attribute, _)| attribute.is_held_by(kind)),
            "the kind holds each attribute"
        );
        debug_assert!(
            location.is_none() || TextRole::Location.is_held_by(kind),
            "only a Resource has a location"
        );
        let node = self.insert_handle(parent, kind, id, Some(name), None, None)?;
        self.nodes[node].strings = strings;
        self.nodes[node].location = location.map(Text::new);
        self.refresh_expressions(node);
        Ok(())
    }

    /// [`insert_node`](Self::insert_node), with the element's string-valued
    /// attributes and reference ends set as given, held exactly (ADR 0034
    /// amendment 3): on an error the document is unchanged.
    ///
    /// The creates of a Term and an Expression use it, so that a create is
    /// one change. Each attribute is one the kind holds, given once, and each
    /// end one the kind owns, within its bounds.
    pub(crate) fn insert_expression_element(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: LangString,
        strings: Vec<(StringAttribute, String)>,
        references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    ) -> Result<(), InsertError> {
        debug_assert!(
            strings
                .iter()
                .all(|(attribute, _)| attribute.is_held_by(kind)),
            "the kind holds each attribute"
        );
        let node = self.insert_handle(parent, kind, id, Some(name), None, None)?;
        self.nodes[node].strings = strings;
        for (end, targets) in references {
            debug_assert!(
                reference_problem(kind, end, &targets).is_none(),
                "the kind has the end, and the end can hold the targets"
            );
            self.hold_references(node, end, targets);
        }
        Ok(())
    }

    /// [`insert_element`](Self::insert_element), returning the new node's
    /// handle.
    fn insert_handle(
        &mut self,
        parent: Option<&ElementId>,
        kind: ElementKind,
        id: ElementId,
        name: Option<LangString>,
        content: Option<MultiLangString>,
        key: Option<MultiLangString>,
    ) -> Result<usize, InsertError> {
        if id.as_str().is_empty() {
            return Err(InsertError::EmptyId);
        }
        if self.is_id_in_use(id.as_str()) {
            return Err(InsertError::DuplicateId(id));
        }
        let parent = match parent {
            None => None,
            Some(parent_id) => match self.index.get(parent_id) {
                Some(&parent) => Some(parent),
                None => return Err(InsertError::UnknownParent(parent_id.clone())),
            },
        };
        if name.is_some() && !kind.is_a(AbstractClass::ModelElement) {
            return Err(InsertError::NameNotAllowed(id, kind));
        }
        // A child needs a role of its parent's class, which the node keeps;
        // a root must be a package.
        let role = if let Some(parent) = parent {
            let parent = &self.nodes[parent];
            let Some(role) = role(parent.kind, kind) else {
                return Err(InsertError::ContainmentNotAllowed {
                    id,
                    kind,
                    parent: parent.id.clone(),
                    parent_kind: parent.kind,
                });
            };
            Some(role)
        } else {
            if !kind.is_package() {
                return Err(InsertError::RootNotAllowed(id, kind));
            }
            None
        };

        let node = self.nodes.len();
        let slot = if let Some(parent) = parent {
            self.nodes[parent].children.push(node);
            self.nodes[parent].children.len() - 1
        } else {
            self.roots.push(node);
            self.roots.len() - 1
        };
        self.index.insert(id.clone(), node);
        self.nodes.push(Node {
            id,
            kind,
            gid: None,
            name,
            name_id: None,
            content: content.map(Text::new),
            key: key.map(Text::new),
            location: None,
            strings: Vec::new(),
            assertion_declaration: kind
                .is_a(AbstractClass::Assertion)
                .then(AssertionDeclaration::default),
            is_citation: false,
            is_abstract: false,
            is_counter: false,
            references: Vec::new(),
            hrefs: Vec::new(),
            proxies: Vec::new(),
            parent,
            slot,
            role,
            children: Vec::new(),
            preserved: None,
            metadata: None,
            name_metadata: None,
        });
        self.refresh_expressions(node);
        Ok(node)
    }

    /// Sets the `assertionDeclaration` (SACM 2.3 §11.10) of the Assertion
    /// `id`, replacing its previous value.
    ///
    /// **Not part of the stable API.** A seam of the `test-support` feature;
    /// use [`SetAssertionDeclaration`](crate::edit::SetAssertionDeclaration)
    /// instead. It may change or disappear in any release.
    ///
    /// # Errors
    ///
    /// Refuses, and leaves the document unchanged, when, checked in this order:
    /// - the document has no element `id`
    ///   ([`SetAssertionDeclarationError::UnknownElement`]);
    /// - the element's kind is not an Assertion
    ///   ([`SetAssertionDeclarationError::NotAnAssertion`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::model::{
    ///     AssertionDeclaration, Document, ElementId, ElementKind, SetAssertionDeclarationError,
    /// };
    ///
    /// let mut doc = Document::new();
    /// let id = ElementId::from;
    /// doc.insert(None, ElementKind::ArgumentPackage, id("arguments"), None).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("c1"), None).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::ArgumentReasoning, id("r1"), None).unwrap();
    ///
    /// doc.set_assertion_declaration(&id("c1"), AssertionDeclaration::Assumed)?;
    /// let c1 = doc.get("c1").expect("the document has it");
    /// assert_eq!(c1.assertion_declaration(), Some(AssertionDeclaration::Assumed));
    ///
    /// // An ArgumentReasoning is not an Assertion, so it has no declaration to set.
    /// let refused = doc.set_assertion_declaration(&id("r1"), AssertionDeclaration::Assumed);
    /// assert_eq!(
    ///     refused,
    ///     Err(SetAssertionDeclarationError::NotAnAssertion(id("r1"), ElementKind::ArgumentReasoning))
    /// );
    /// # Ok::<(), SetAssertionDeclarationError>(())
    /// ```
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    pub fn set_assertion_declaration(
        &mut self,
        id: &ElementId,
        declaration: AssertionDeclaration,
    ) -> Result<(), SetAssertionDeclarationError> {
        self.set_declaration(id, declaration)?;
        self.renew_revision();
        Ok(())
    }

    /// Sets the `assertionDeclaration` of the Assertion `id`; on an error the
    /// document is unchanged.
    ///
    /// The crate-internal form of the `test-support` seam
    /// `set_assertion_declaration`, present with or without the feature. The
    /// XMI reader sets a declaration it reads through it.
    pub(crate) fn set_declaration(
        &mut self,
        id: &ElementId,
        declaration: AssertionDeclaration,
    ) -> Result<(), SetAssertionDeclarationError> {
        let Some(&node) = self.index.get(id) else {
            return Err(SetAssertionDeclarationError::UnknownElement(id.clone()));
        };
        let node = &mut self.nodes[node];
        match node.assertion_declaration.as_mut() {
            Some(current) => {
                *current = declaration;
                Ok(())
            }
            None => Err(SetAssertionDeclarationError::NotAnAssertion(
                id.clone(),
                node.kind,
            )),
        }
    }

    /// Sets the `xmi:id` of the name of the element `owner` (ADR 0024); on an
    /// error the document is unchanged.
    ///
    /// The XMI reader keeps a name's id through it. It refuses, with the
    /// reason, an unknown owner, an owner with no name, an empty id and an id
    /// already in use, by an element or by a name.
    pub(crate) fn set_name_id(
        &mut self,
        owner: &ElementId,
        name_id: ElementId,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        if self.nodes[node].name.is_none() {
            return Err(format!("the element {:?} has no name", owner.as_str()));
        }
        if name_id.as_str().is_empty() {
            return Err("the name's id is empty".to_owned());
        }
        if self.is_id_in_use(name_id.as_str()) {
            return Err(format!("the id {:?} is already in use", name_id.as_str()));
        }
        self.other_ids.entry(name_id.clone()).or_default().owner = Some(node);
        self.nodes[node].name_id = Some(name_id);
        Ok(())
    }

    /// Gives the element `owner` the preserved content `preserved` (ADR
    /// 0033), which it did not hold; on an error the document is unchanged.
    ///
    /// The XMI reader keeps what does not conform through it. The ids of the
    /// content join the document's id space, owned by the element, and each
    /// id it may name counts as a reference, so that none is generated. It
    /// refuses, with the reason, an unknown owner, an owner that holds
    /// preserved content already, an empty id, an id given twice and an id
    /// already in use. Empty content changes nothing.
    pub(crate) fn set_preserved(
        &mut self,
        owner: &ElementId,
        preserved: Preserved,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        if self.nodes[node].preserved.is_some() {
            return Err(format!(
                "the element {:?} holds preserved content already",
                owner.as_str()
            ));
        }
        if preserved.is_empty() {
            return Ok(());
        }
        check_preserved_ids(self, &preserved)?;
        for id in &preserved.ids {
            let used = self.other_ids.entry(id.clone()).or_default();
            used.owner = Some(node);
            used.preserved = true;
        }
        for id in &preserved.references {
            let used = self.other_ids.entry(id.clone()).or_default();
            used.references = used.references.saturating_add(1);
        }
        self.nodes[node].preserved = Some(Box::new(preserved));
        Ok(())
    }

    /// Gives the element `owner` the XMI metadata `metadata` (SACM23-XMI-019,
    /// ADR 0042), on its start tag and among its children when `of_name` is
    /// `false`, on and in its `name` when `true`; on an error the document is
    /// unchanged.
    ///
    /// The XMI reader keeps metadata through it. Its ids join the document's
    /// id space, owned by the element, as preserved content's do (ADR 0042
    /// D5), and each id it may name counts as a reference. It refuses, with
    /// the reason, an unknown owner, an owner that holds such metadata
    /// already, a name's metadata without a name, an empty id, an id given
    /// twice and an id already in use. Empty metadata changes nothing.
    pub(crate) fn set_metadata(
        &mut self,
        owner: &ElementId,
        of_name: bool,
        metadata: Preserved,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        let held = if of_name {
            if self.nodes[node].name.is_none() {
                return Err(format!("the element {:?} has no name", owner.as_str()));
            }
            &self.nodes[node].name_metadata
        } else {
            &self.nodes[node].metadata
        };
        if held.is_some() {
            return Err(format!(
                "the element {:?} holds such XMI metadata already",
                owner.as_str()
            ));
        }
        if metadata.is_empty() {
            return Ok(());
        }
        check_preserved_ids(self, &metadata)?;
        self.hold_metadata(Some(node), &metadata);
        let metadata = Some(Box::new(metadata));
        if of_name {
            self.nodes[node].name_metadata = metadata;
        } else {
            self.nodes[node].metadata = metadata;
        }
        Ok(())
    }

    /// Records the ids of `metadata` in the id space, owned by the element
    /// `owner`, or by the document for `None`, and counts each id it may name
    /// as a reference (ADR 0042 D5). The caller has checked its ids.
    fn hold_metadata(&mut self, owner: Option<usize>, metadata: &Preserved) {
        for id in &metadata.ids {
            let used = self.other_ids.entry(id.clone()).or_default();
            match owner {
                Some(node) => {
                    used.owner = Some(node);
                    used.preserved = true;
                }
                None => used.document = true,
            }
        }
        for id in &metadata.references {
            let used = self.other_ids.entry(id.clone()).or_default();
            used.references = used.references.saturating_add(1);
        }
    }

    /// Frees the ids of `metadata`, which an element held, and the references
    /// it counted (ADR 0042 D6).
    fn release_metadata(&mut self, metadata: &Preserved) {
        for id in &metadata.ids {
            self.release(id, |used| {
                used.owner = None;
                used.preserved = false;
            });
        }
        for id in &metadata.references {
            self.release(id, |used| {
                used.references = used.references.saturating_sub(1);
            });
        }
    }

    /// Sets the `gid` of the element `id` (SACM 2.3 §8.2) to `gid`, exactly:
    /// `None` is absent. A gid is not an id, so nothing about its value is
    /// checked (ADR 0025). Refuses, with the reason, an unknown element; on
    /// an error the document is unchanged.
    pub(crate) fn set_gid(&mut self, id: &ElementId, gid: Option<String>) -> Result<(), String> {
        let Some(&node) = self.index.get(id) else {
            return Err(format!("the document has no element {:?}", id.as_str()));
        };
        self.nodes[node].gid = gid;
        Ok(())
    }

    /// Sets the string-valued `attribute` of the element `id` to `value`,
    /// exactly: `None` is absent (D-DATE). Nothing about the value is
    /// checked. Refuses, with the reason, an unknown element and one whose
    /// kind does not hold the attribute; on an error the document is
    /// unchanged.
    pub(crate) fn set_string(
        &mut self,
        id: &ElementId,
        attribute: StringAttribute,
        value: Option<String>,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(id) else {
            return Err(format!("the document has no element {:?}", id.as_str()));
        };
        let node = &mut self.nodes[node];
        if !attribute.is_held_by(node.kind) {
            return Err(format!(
                "the {} {:?} has no {}",
                node.kind,
                id.as_str(),
                attribute.name()
            ));
        }
        let held = node
            .strings
            .iter()
            .position(|(given, _)| *given == attribute);
        match (held, value) {
            (Some(position), Some(value)) => node.strings[position].1 = value,
            (Some(position), None) => {
                node.strings.remove(position);
            }
            (None, Some(value)) => node.strings.push((attribute, value)),
            (None, None) => {}
        }
        Ok(())
    }

    /// Sets the flag `flag` of the element `id` (SACM 2.3 §8.2, §11.13) to
    /// `value`. Refuses, with the reason, an unknown element and one whose
    /// kind has no such flag; on an error the document is unchanged.
    pub(crate) fn set_flag(
        &mut self,
        id: &ElementId,
        flag: Flag,
        value: bool,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(id) else {
            return Err(format!("the document has no element {:?}", id.as_str()));
        };
        let kind = self.nodes[node].kind;
        if !flag.owner().admits(kind) {
            return Err(format!(
                "the {kind} {:?} has no {}",
                id.as_str(),
                flag.name()
            ));
        }
        *self.nodes[node].flag_mut(flag) = value;
        Ok(())
    }

    /// Sets the targets of the reference end `end` of the element `owner` to
    /// `targets`, unresolved and in order, repeats included, replacing what
    /// it held; none removes the end (ADR 0026, ADR 0027). A target need not
    /// name an element. Refuses, with the reason, an unknown owner, an owner
    /// whose kind has no such end, an empty id, and more than one target on
    /// a single-valued end; on an error the document is unchanged.
    pub(crate) fn set_references(
        &mut self,
        owner: &ElementId,
        end: ReferenceEnd,
        targets: Vec<ElementId>,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        if let Some(problem) = reference_problem(self.nodes[node].kind, end, &targets) {
            return Err(format!("the element {:?} {problem}", owner.as_str()));
        }
        self.hold_references(node, end, targets);
        Ok(())
    }

    /// Sets the targets of `end` of the node `node` to `targets`, replacing
    /// what it held, and counts each use of an id: the change of
    /// [`set_references`](Self::set_references), once it is checked.
    fn hold_references(&mut self, node: usize, end: ReferenceEnd, targets: Vec<ElementId>) {
        let references = &mut self.nodes[node].references;
        let old = match references.iter().position(|(held, _)| *held == end) {
            Some(position) => references.remove(position).1,
            None => Vec::new(),
        };
        for target in &targets {
            let used = self.other_ids.entry(target.clone()).or_default();
            used.references = used.references.saturating_add(1);
            used.holders.push(node);
        }
        if !targets.is_empty() {
            // In no particular order: every reader looks an end up by
            // `ReferenceEnd`, and iterates `ReferenceEnd::ALL` for order.
            references.push((end, targets));
        }
        for target in &old {
            self.release(target, |used| {
                used.references = used.references.saturating_sub(1);
                release_holder(&mut used.holders, node);
            });
        }
    }

    /// Sets the cross-document references (`href`s) of the end `end` of the
    /// element `owner` to `hrefs`, in order, repeats included, replacing what
    /// it held; none removes them (ADR 0040 D2). The XMI reader sets them,
    /// and [`SetReferences`](crate::edit::SetReferences) removes them.
    /// Refuses, with the reason, an unknown owner, an owner whose kind has no
    /// such end or the derived `expression`, and more than one value, ids and
    /// `href`s together, on a single-valued end; on an error the document is
    /// unchanged.
    pub(crate) fn set_hrefs(
        &mut self,
        owner: &ElementId,
        end: ReferenceEnd,
        hrefs: Vec<Href>,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        let held = &self.nodes[node];
        if end.is_derived() || !end.applies_to(held.kind) {
            return Err(format!(
                "the {} {:?} holds no href in {}",
                held.kind,
                owner.as_str(),
                end.name()
            ));
        }
        let ids = targets_of(&held.references, end).len();
        if end.upper_bound() == Some(1) && ids + hrefs.len() > 1 {
            return Err(format!(
                "the element {:?} holds more than one value in its {}",
                owner.as_str(),
                end.name()
            ));
        }
        let ends = &mut self.nodes[node].hrefs;
        ends.retain(|(end_held, _)| *end_held != end);
        if !hrefs.is_empty() {
            ends.push((end, hrefs));
        }
        Ok(())
    }

    /// Sets the proxies of the element `owner` to `proxies`, in order,
    /// replacing what it held (SACM23-XMI-029, ADR 0040 amendment 1, A2).
    /// Only the XMI reader sets them. Refuses, with the reason, an unknown
    /// owner and a proxy in a role that is no composite role of the owner's
    /// class; on an error the document is unchanged.
    pub(crate) fn set_proxies(
        &mut self,
        owner: &ElementId,
        proxies: Vec<Proxy>,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        let kind = self.nodes[node].kind;
        if let Some(proxy) = proxies
            .iter()
            .find(|proxy| super::role_named(kind, proxy.role()).is_none())
        {
            return Err(format!(
                "the {kind} {:?} has no composite role {}",
                owner.as_str(),
                proxy.role()
            ));
        }
        self.nodes[node].proxies = proxies;
        Ok(())
    }

    /// Derives the node's `expression` end from its name and texts again
    /// (SACM23-BASE-009): called whenever one of them is set, so that the
    /// end, and the referrer index with it, stays in step.
    fn refresh_expressions(&mut self, node: usize) {
        let expressions = self.nodes[node].expressions();
        if expressions.as_slice()
            != targets_of(&self.nodes[node].references, ReferenceEnd::Expression)
        {
            self.hold_references(node, ReferenceEnd::Expression, expressions);
        }
    }

    /// Applies `release` to the use of `id` in `other_ids`, and drops the
    /// entry once nothing uses the id.
    fn release(&mut self, id: &ElementId, release: impl FnOnce(&mut IdUse)) {
        if let Some(used) = self.other_ids.get_mut(id) {
            release(used);
            if used.is_unused() {
                self.other_ids.remove(id);
            }
        }
    }

    /// Sets the `role` of the element `owner` to `text`, replacing what it
    /// held, and its ids with it (ADR 0025); on an error the document is
    /// unchanged.
    ///
    /// The XMI reader keeps a `content` or `key` through it. It refuses, with
    /// the reason, an unknown owner, an owner whose kind has no such role, an
    /// empty id, an id given twice and an id in use elsewhere, by an element
    /// or as another id that identifies no element. The ids the role held
    /// before are free for it.
    pub(crate) fn set_text(
        &mut self,
        owner: &ElementId,
        role: TextRole,
        text: Text,
    ) -> Result<(), String> {
        let Some(&node) = self.index.get(owner) else {
            return Err(format!("the document has no element {:?}", owner.as_str()));
        };
        let kind = self.nodes[node].kind;
        if !role.is_held_by(kind) {
            return Err(format!(
                "the {kind} {:?} has no {}",
                owner.as_str(),
                role.name()
            ));
        }
        // A set, so that checking every new id against the old ones is not
        // quadratic in the number of values.
        let old: BTreeSet<ElementId> = self.nodes[node]
            .text(role)
            .map(|held| {
                held.ids()
                    .chain(held.metadata().flat_map(|metadata| &metadata.ids))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut seen = BTreeSet::new();
        // Its own ids, then those of its XMI metadata (ADR 0042 D5).
        for id in text
            .ids()
            .chain(text.metadata().flat_map(|metadata| &metadata.ids))
        {
            if id.as_str().is_empty() {
                return Err(format!("an id of the {} is empty", role.name()));
            }
            let elsewhere = self.is_id_in_use(id.as_str()) && !old.contains(id);
            if !seen.insert(id.as_str()) || elsewhere {
                return Err(format!("the id {:?} is already in use", id.as_str()));
            }
        }
        if let Some(held) = self.nodes[node].text_mut(role).take() {
            self.release_text(&held);
        }
        for id in text.ids() {
            self.other_ids.entry(id.clone()).or_default().owner = Some(node);
        }
        for metadata in text.metadata() {
            self.hold_metadata(Some(node), metadata);
        }
        *self.nodes[node].text_mut(role) = Some(text);
        self.refresh_expressions(node);
        Ok(())
    }

    /// Removes the `role` of the element `owner`, and frees its ids (ADR
    /// 0029). Changes nothing when the document has no element `owner`.
    pub(crate) fn remove_text(&mut self, owner: &ElementId, role: TextRole) {
        let Some(&node) = self.index.get(owner) else {
            return;
        };
        if let Some(old) = self.nodes[node].text_mut(role).take() {
            self.release_text(&old);
        }
        self.refresh_expressions(node);
    }

    /// Frees the ids of `text`, which an element held, and those of its XMI
    /// metadata, with the references the metadata counted (ADR 0029, ADR 0042
    /// D6).
    fn release_text(&mut self, text: &Text) {
        for id in text.ids() {
            self.release(id, |used| used.owner = None);
        }
        for metadata in text.metadata() {
            self.release_metadata(metadata);
        }
    }

    /// Replaces the name of the element `id` with `name`, keeping the name's
    /// `xmi:id` (ADR 0029). The caller has checked that the element is a
    /// `ModelElement`; changes nothing when the document has no element
    /// `id`.
    pub(crate) fn replace_name(&mut self, id: &ElementId, name: LangString) {
        if let Some(&node) = self.index.get(id) {
            debug_assert!(
                self.nodes[node].kind.is_a(AbstractClass::ModelElement),
                "only a ModelElement has a name"
            );
            self.nodes[node].name = Some(name);
            self.refresh_expressions(node);
        }
    }

    /// Puts the children of `parent`, or the roots when `None`, that `order`
    /// names back into the positions they occupy, in the order `order`
    /// gives; every other child keeps its position (ADR 0029). Returns
    /// whether the order changed.
    ///
    /// The caller has checked that `order` names each of its ids once, and
    /// only children of `parent` (or roots). Changes nothing when the
    /// document has no element `parent`.
    pub(crate) fn reorder(&mut self, parent: Option<&ElementId>, order: &[ElementId]) -> bool {
        let handles: Vec<usize> = order
            .iter()
            .filter_map(|id| self.index.get(id).copied())
            .collect();
        let parent = match parent {
            None => None,
            Some(parent) => match self.index.get(parent) {
                Some(&parent) => Some(parent),
                None => return false,
            },
        };
        let held = match parent {
            None => &mut self.roots,
            Some(parent) => &mut self.nodes[parent].children,
        };
        let named: BTreeSet<usize> = handles.iter().copied().collect();
        let mut next = handles.into_iter();
        let mut changed = false;
        for slot in held.iter_mut() {
            if named.contains(slot) {
                if let Some(handle) = next.next() {
                    changed |= *slot != handle;
                    *slot = handle;
                }
            }
        }
        self.renumber_children(parent, 0);
        changed
    }

    /// Deletes the elements `deleted`, after removing every id of `deleted`
    /// from each end of each element of `holders` (ADR 0031).
    ///
    /// The caller has checked that `deleted` is closed under containment
    /// (each element's children are in it too) and that no holder is in it.
    /// The references the deleted elements held, and the ids of their names,
    /// `content`s, `key`s and `value`s, are released, so those ids are free;
    /// the generated-id counters are not rewound, so generation stays
    /// deterministic. The arena is compacted and every
    /// handle remapped, in time linear in the size of the document. Ids the
    /// document does not hold are ignored.
    pub(crate) fn remove_elements(&mut self, deleted: &[ElementId], holders: &[ElementId]) {
        let mut doomed = vec![false; self.nodes.len()];
        for id in deleted {
            if let Some(slot) = self.index.get(id).and_then(|&node| doomed.get_mut(node)) {
                *slot = true;
            }
        }
        for holder in holders {
            self.drop_references_to(holder, &doomed);
        }
        for (node, &gone) in doomed.iter().enumerate() {
            if gone {
                self.release_node(node);
            }
        }
        self.compact(&doomed);
    }

    /// Removes from each end of the element `holder`, unless it is doomed
    /// itself, every id that names a doomed element, and every id of the
    /// preserved content of a doomed element, which the delete frees (ADR
    /// 0033).
    fn drop_references_to(&mut self, holder: &ElementId, doomed: &[bool]) {
        let is_gone = |node: usize| doomed.get(node).copied().unwrap_or(false);
        let is_doomed = |document: &Self, id: &ElementId| {
            document.index.get(id).is_some_and(|&node| is_gone(node))
                || document
                    .other_ids
                    .get(id)
                    .filter(|used| used.preserved)
                    .and_then(|used| used.owner)
                    .is_some_and(is_gone)
        };
        let Some(&node) = self.index.get(holder) else {
            return;
        };
        if is_doomed(self, holder) {
            return;
        }
        // The derived end: each ExpressionLangString whose expression is
        // deleted is made plain (ADR 0038 D4), and the end follows.
        let gone: BTreeSet<ElementId> =
            targets_of(&self.nodes[node].references, ReferenceEnd::Expression)
                .iter()
                .filter(|target| is_doomed(self, target))
                .cloned()
                .collect();
        if !gone.is_empty() && self.nodes[node].plain_expressions(|id| gone.contains(id)) {
            self.refresh_expressions(node);
        }
        let kept: Vec<(ReferenceEnd, Vec<ElementId>)> = self.nodes[node]
            .references
            .iter()
            .filter(|(_, targets)| targets.iter().any(|target| is_doomed(self, target)))
            .map(|(end, targets)| {
                let kept = targets
                    .iter()
                    .filter(|target| !is_doomed(self, target))
                    .cloned()
                    .collect();
                (*end, kept)
            })
            .collect();
        for (end, targets) in kept {
            self.hold_references(node, end, targets);
        }
    }

    /// Releases the references the node `node` holds, in its ends and its
    /// preserved content, and its own id. The node stays in the arena until
    /// [`compact`](Self::compact), which also frees the ids it owns.
    fn release_node(&mut self, node: usize) {
        let references = std::mem::take(&mut self.nodes[node].references);
        let preserved = self.nodes[node].preserved.take();
        // The references its XMI metadata may hold go with it too; the ids of
        // the metadata it owns are freed by `compact`, as its parts' are
        // (ADR 0042 D6).
        let metadata: Vec<ElementId> = self.nodes[node]
            .metadata_parts()
            .flat_map(|metadata| metadata.references.iter().cloned())
            .collect();
        for target in references.iter().flat_map(|(_, targets)| targets) {
            self.release(target, |used| {
                used.references = used.references.saturating_sub(1);
                release_holder(&mut used.holders, node);
            });
        }
        for target in preserved
            .iter()
            .flat_map(|held| held.references.iter())
            .chain(&metadata)
        {
            self.release(target, |used| {
                used.references = used.references.saturating_sub(1);
            });
        }
        let id = self.nodes[node].id.clone();
        self.index.remove(&id);
    }

    /// Removes the doomed nodes from the arena, where the survivors keep
    /// their order, and remaps every handle. An id a doomed node owned, of
    /// its name, `content`, `key`, `value`s or preserved content, loses its
    /// owner, and is freed unless a reference still holds it.
    fn compact(&mut self, doomed: &[bool]) {
        let mut remap: Vec<Option<usize>> = Vec::with_capacity(doomed.len());
        let mut next = 0;
        for &gone in doomed {
            if gone {
                remap.push(None);
            } else {
                remap.push(Some(next));
                next += 1;
            }
        }
        let new = |node: usize| remap.get(node).copied().flatten();
        let old = std::mem::take(&mut self.nodes);
        self.nodes = old
            .into_iter()
            .zip(doomed)
            .filter(|(_, gone)| !**gone)
            .map(|(mut node, _)| {
                debug_assert!(
                    node.parent.is_none_or(|parent| new(parent).is_some()),
                    "the deleted elements are closed under containment"
                );
                node.parent = node.parent.and_then(new);
                // A preserved child, and an XMI metadata child, keeps its place
                // among the typed children that survive (ADR 0033, ADR 0042).
                if node.preserved.is_some() || node.metadata.is_some() {
                    let kept: Vec<bool> = node
                        .children
                        .iter()
                        .map(|&child| new(child).is_some())
                        .collect();
                    for placed in [node.preserved.as_mut(), node.metadata.as_mut()]
                        .into_iter()
                        .flatten()
                    {
                        placed.shift(&kept);
                    }
                }
                node.children = node
                    .children
                    .iter()
                    .filter_map(|&child| new(child))
                    .collect();
                node
            })
            .collect();
        // Content and XMI metadata outside every element keep their place
        // among the roots that survive (ADR 0035 D3, ADR 0042).
        if self.preserved.is_some() || self.metadata.is_some() {
            let kept: Vec<bool> = self.roots.iter().map(|&root| new(root).is_some()).collect();
            for placed in [self.preserved.as_mut(), self.metadata.as_mut()]
                .into_iter()
                .flatten()
            {
                placed.shift(&kept);
            }
        }
        self.roots = self.roots.iter().filter_map(|&root| new(root)).collect();
        for node in self.index.values_mut() {
            if let Some(moved) = new(*node) {
                *node = moved;
            }
        }
        self.other_ids.retain(|_, used| {
            used.owner = used.owner.and_then(new);
            used.preserved &= used.owner.is_some();
            // A doomed holder released its references first.
            used.holders = used
                .holders
                .iter()
                .filter_map(|&holder| new(holder))
                .collect();
            !used.is_unused()
        });
        for parent in 0..self.nodes.len() {
            self.renumber_children(Some(parent), 0);
        }
        self.renumber_children(None, 0);
    }

    /// Sets the `slot` of each child of `parent`, or of each root for
    /// `None`, from the index `from` on, to its index.
    fn renumber_children(&mut self, parent: Option<usize>, from: usize) {
        let held = match parent {
            Some(parent) => &self.nodes[parent].children,
            None => &self.roots,
        };
        let handles: Vec<usize> = held.iter().skip(from).copied().collect();
        for (offset, handle) in handles.into_iter().enumerate() {
            self.nodes[handle].slot = from + offset;
        }
    }

    /// Whether `id` is in use in the document's id space: by an element, or
    /// as an id that identifies no element: of a name (ADR 0024), a
    /// `content`, a `key` or a `value` (ADR 0025). A new element's id, given
    /// or generated, must not be.
    pub(crate) fn is_id_in_use(&self, id: &str) -> bool {
        self.index.contains_key(id)
            || self
                .other_ids
                .get(id)
                .is_some_and(|used| used.owner.is_some() || used.document)
    }

    /// Whether the element `owner` holds, as preserved content, the
    /// attribute `local` in no namespace, such as a reference end's
    /// `metaClaim=""` that held no id (ADR 0033), or an `isAbstract="yes"`
    /// that is no literal (ADR 0035).
    pub(crate) fn has_preserved_attribute(&self, owner: &ElementId, local: &str) -> bool {
        self.index
            .get(owner)
            .and_then(|&node| self.nodes[node].preserved.as_deref())
            .is_some_and(|preserved| preserved.has_unqualified(local))
    }

    /// Removes the attribute `local` in no namespace from the preserved
    /// content of the element `owner`, if it holds one: an edit that sets
    /// the property of that name replaces it, so the element never holds the
    /// property twice (ADR 0033, ADR 0035). The ids it named as a reference
    /// are released. Changes nothing otherwise.
    pub(crate) fn replace_preserved_attribute(&mut self, owner: &ElementId, local: &str) {
        let Some(&node) = self.index.get(owner) else {
            return;
        };
        let Some(preserved) = self.nodes[node].preserved.as_mut() else {
            return;
        };
        let released = preserved.remove_unqualified(local);
        if preserved.is_empty() {
            self.nodes[node].preserved = None;
        }
        for id in &released {
            self.release(id, |used| {
                used.references = used.references.saturating_sub(1);
            });
        }
    }

    /// Whether `id` is the `xmi:id` of an element of preserved content, of
    /// an element or of the document (ADR 0033, ADR 0035 D3): in the id
    /// space, though no element has it and [`get`](Self::get) does not find
    /// it.
    pub(crate) fn is_preserved_id(&self, id: &str) -> bool {
        self.other_ids
            .get(id)
            .is_some_and(|used| (used.preserved && used.owner.is_some()) || used.document)
    }

    /// Whether the document holds preserved content anywhere: in an element
    /// ([`SacmElement::has_preserved_content`]), or outside every element,
    /// on or under the `xmi:XMI` element, such as a vendor attribute or an
    /// XMI-namespace element XMI does not define there (ADR 0035 D3). XMI
    /// metadata, such as `xmi:uuid` or `xmi:Documentation`, is none
    /// (SACM23-XMI-019). A strict [`xmi::write`](crate::xmi::write)
    /// refuses a document that holds some
    /// ([`WriteError::PreservedContent`](crate::xmi::WriteError::PreservedContent)).
    ///
    /// ```
    /// use sacm::xmi::{self, Mode, ReadOptions};
    ///
    /// let input = br#"<xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
    ///     xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
    ///   <xmi:foo exporter="tool"/>
    ///   <sacm:ArgumentPackage xmi:id="p"><name content="P"/></sacm:ArgumentPackage>
    /// </xmi:XMI>"#;
    /// let document = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?.into_document();
    /// assert!(document.has_preserved_content());
    /// // No element holds it: the document does, outside its elements.
    /// assert!(document.elements().all(|element| !element.has_preserved_content()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn has_preserved_content(&self) -> bool {
        self.preserved.is_some() || self.elements().any(SacmElement::has_preserved_content)
    }

    /// The preserved content outside every element, if the document holds
    /// any (ADR 0035 D3).
    pub(crate) fn document_preserved(&self) -> Option<&Preserved> {
        self.preserved.as_deref()
    }

    /// Gives the document the preserved content `preserved` outside every
    /// element (ADR 0035 D3), which it did not hold; on an error the
    /// document is unchanged.
    ///
    /// The XMI reader keeps what the `xmi:XMI` element holds besides its
    /// roots through it. The ids of the content join the document's id
    /// space, and each id it may name counts as a reference, so that none is
    /// generated. It refuses, with the reason, a document that holds such
    /// content already, an empty id, an id given twice and an id already in
    /// use. Empty content changes nothing.
    pub(crate) fn set_document_preserved(&mut self, preserved: Preserved) -> Result<(), String> {
        if self.preserved.is_some() {
            return Err("the document holds preserved content already".to_owned());
        }
        if preserved.is_empty() {
            return Ok(());
        }
        check_preserved_ids(self, &preserved)?;
        for id in &preserved.ids {
            let used = self.other_ids.entry(id.clone()).or_default();
            used.document = true;
        }
        for id in &preserved.references {
            let used = self.other_ids.entry(id.clone()).or_default();
            used.references = used.references.saturating_add(1);
        }
        self.preserved = Some(Box::new(preserved));
        Ok(())
    }

    /// The XMI metadata outside every element, if the document holds any
    /// (SACM23-XMI-019, ADR 0042).
    pub(crate) fn document_metadata(&self) -> Option<&Preserved> {
        self.metadata.as_deref()
    }

    /// Gives the document the XMI metadata `metadata` outside every element
    /// (ADR 0042), which it did not hold; on an error the document is
    /// unchanged.
    ///
    /// The XMI reader keeps the metadata on and under `xmi:XMI` through it.
    /// Its ids join the document's id space, and each id it may name counts
    /// as a reference. It refuses, with the reason, a document that holds
    /// such metadata already, an empty id, an id given twice and an id
    /// already in use. Empty metadata changes nothing.
    pub(crate) fn set_document_metadata(&mut self, metadata: Preserved) -> Result<(), String> {
        if self.metadata.is_some() {
            return Err("the document holds XMI metadata already".to_owned());
        }
        if metadata.is_empty() {
            return Ok(());
        }
        check_preserved_ids(self, &metadata)?;
        self.hold_metadata(None, &metadata);
        self.metadata = Some(Box::new(metadata));
        Ok(())
    }

    /// Whether some reference, of any element and end, holds `id`.
    pub(crate) fn is_referenced(&self, id: &str) -> bool {
        self.other_ids
            .get(id)
            .is_some_and(|used| used.references > 0)
    }

    /// The id the next generated element of `kind` gets, with its `n`:
    /// `<snake_case kind>_<n>` for the smallest `n` above the kind's counter
    /// whose id is not in use (ADR 0019) and that no reference holds, so
    /// that a dangling reference never resolves to a new element by accident
    /// (ADR 0026). The counter is not advanced; see
    /// [`commit_generated_id`](Self::commit_generated_id).
    pub(crate) fn next_generated_id(&self, kind: ElementKind) -> (ElementId, u64) {
        let prefix = snake_case(kind.name());
        let mut n = self
            .id_counters
            .get(&kind)
            .copied()
            .unwrap_or(0)
            .wrapping_add(1);
        let mut id = generated_id(&prefix, n);
        // A candidate is taken only if it is an element's id or an id in
        // `other_ids`, each counted once here, and the candidates are
        // distinct, so one of the first `taken + 1` is free: the last
        // candidate needs no check, and the loop is bounded.
        let taken = self.nodes.len() + self.other_ids.len();
        for _ in 0..taken {
            if !self.is_id_in_use(id.as_str()) && !self.is_referenced(id.as_str()) {
                break;
            }
            n = n.wrapping_add(1);
            id = generated_id(&prefix, n);
        }
        (id, n)
    }

    /// Records that the id generated with `n` for `kind` is now in use, so
    /// that generation for `kind` continues above it. Called only after the
    /// element is added.
    pub(crate) fn commit_generated_id(&mut self, kind: ElementKind, n: u64) {
        self.id_counters.insert(kind, n);
    }

    /// The roots, in document order, as [`crate::compare`] compares them.
    ///
    /// The one place `compare` reads the document as a whole. `Document` is
    /// destructured with no `..`, so a field added to it fails to compile
    /// here until `compare` handles it (ADR 0020).
    pub(crate) fn compared_roots(&self) -> impl Iterator<Item = SacmElement<'_>> {
        let Self {
            // Compared element by element, through `SacmElement::compared`.
            nodes: _,
            // Compared as the root order.
            roots,
            // Derived from `nodes`: the id of each element.
            index: _,
            // Derived from `nodes`: the owner of each id that identifies no
            // element and the references to each id, which are compared
            // through `SacmElement::compared`.
            other_ids: _,
            // Not compared: the generated-id counters are not SACM content
            // and XMI does not carry them (ADR 0020).
            id_counters: _,
            // Not compared: the revision identifies a state within the
            // process, not SACM content (ADR 0031).
            revision: _,
            // Compared through `document_preserved`, as
            // `Difference::DocumentPreservedContent` (ADR 0035 D3).
            preserved: _,
            // Compared through `document_metadata`, as
            // `Difference::DocumentMetadata` (ADR 0042).
            metadata: _,
        } = self;
        roots.iter().map(|&node| self.view(node))
    }

    /// The view of the node with handle `node`.
    fn view(&self, node: usize) -> SacmElement<'_> {
        SacmElement {
            document: self,
            node,
        }
    }
}

/// Referrers (LIB-API-014; ADR 0036 D3).
impl Document {
    /// Every typed reference that names `id`, whether or not an element has
    /// that id: the holder, the end and the id's position in the end
    /// (LIB-API-014).
    ///
    /// They are ordered by holder in document pre-order, then by end in
    /// [`ReferenceEnd::ALL`] order, then by position; an id an end holds
    /// twice gives two entries. A dangling id, the id of a name, `content`,
    /// `key`, `location` or `value`, and the `xmi:id` of preserved content
    /// have referrers too. References that preserved content may hold are
    /// not typed, so they are not listed.
    ///
    /// The document keeps an index of the holders of each id, which every
    /// edit updates, so the cost grows with the number of referrers and the
    /// depth of their holders, not with the size of the document.
    ///
    /// # Examples
    ///
    /// What supports a claim: the relationships whose `target` it is.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// let name = LangString::new;
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("The system is safe")).with_id("top"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("H1 is mitigated")).with_id("h1"))?;
    /// edit::apply(&mut doc, CreateAssertedInference::new("p", name("I1"), ["h1"], "top").with_id("i1"))?;
    ///
    /// let support: Vec<&str> = doc
    ///     .referrers("top")
    ///     .into_iter()
    ///     .filter(|referrer| referrer.end() == ReferenceEnd::Target)
    ///     .map(|referrer| referrer.holder().id().as_str())
    ///     .collect();
    /// assert_eq!(support, ["i1"]);
    /// assert!(doc.referrers("i1").is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn referrers<Q: AsRef<str> + ?Sized>(&self, id: &Q) -> Vec<Referrer<'_>> {
        let id = id.as_ref();
        let Some(used) = self.other_ids.get(id) else {
            return Vec::new();
        };
        let mut holders = used.holders.clone();
        holders.sort_unstable();
        holders.dedup();
        let mut ordered: Vec<(Vec<usize>, usize)> = holders
            .into_iter()
            .map(|holder| (self.slot_path(holder), holder))
            .collect();
        ordered.sort_unstable();
        let mut found = Vec::new();
        for (_, holder) in ordered {
            let view = self.view(holder);
            for &end in ReferenceEnd::ALL {
                for (position, held) in view.references(end).iter().enumerate() {
                    if held.as_str() == id {
                        found.push(Referrer {
                            holder: view,
                            end,
                            position,
                        });
                    }
                }
            }
        }
        found
    }

    /// The slots of `node` and its ancestors, from its root down: ordered as
    /// the document's pre-order, an ancestor before its descendants.
    fn slot_path(&self, node: usize) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current = Some(node);
        while let Some(handle) = current {
            path.push(self.nodes[handle].slot);
            current = self.nodes[handle].parent;
        }
        path.reverse();
        path
    }
}

/// Moves (LIB-EDIT-017; ADR 0036 D1).
impl Document {
    /// What moving `element` under `parent` (to the roots for `None`) at
    /// `position` of the others changes, as `semantic_compare` reports it
    /// (LIB-EDIT-003): `None` when it is the place the element holds;
    /// otherwise the ids changed, in pre-order after the move. Under the same
    /// parent, that is the parent (the roots' order is the document's);
    /// under another, the element, and a parent whose preserved content is
    /// placed after the typed child removed or inserted.
    ///
    /// The caller has checked the move (ADR 0036 D1): every id names an
    /// element, the parent may hold the element and is not in its subtree,
    /// and `position` is at most the number of the others.
    pub(crate) fn move_changes(
        &self,
        element: &ElementId,
        parent: Option<&ElementId>,
        position: usize,
    ) -> Option<Vec<ElementId>> {
        let moved = *self.index.get(element)?;
        let new_parent = match parent {
            Some(parent) => Some(*self.index.get(parent)?),
            None => None,
        };
        let old_parent = self.nodes[moved].parent;
        let old_index = self.nodes[moved].slot;
        if old_parent == new_parent {
            if old_index == position {
                return None;
            }
            return Some(
                new_parent
                    .map(|parent| self.nodes[parent].id.clone())
                    .into_iter()
                    .collect(),
            );
        }
        let shifts = |holder: Option<usize>, index: usize| {
            holder.filter(|&holder| {
                let node = &self.nodes[holder];
                [node.preserved.as_deref(), node.metadata.as_deref()]
                    .into_iter()
                    .flatten()
                    .any(|placed| placed.places_after(index))
            })
        };
        let old = shifts(old_parent, old_index);
        let new = shifts(new_parent, position);
        let mut changed: Vec<usize> = Vec::with_capacity(3);
        changed.extend(new);
        changed.push(moved);
        if let Some(old) = old {
            // Where the old parent stands after the move: outside the new
            // parent, where it stood relative to that parent; inside it (or
            // among the roots), before the element exactly when its branch
            // there comes before the position. Neither parent is in the moved
            // subtree, so their own order does not change.
            let before_element =
                if let Some(new) = new_parent.filter(|&new| !self.is_inside(old, new)) {
                    self.slot_path(old) < self.slot_path(new)
                } else {
                    let branch = self.branch_under(old, new_parent);
                    self.nodes[branch].slot < position
                };
            let at = if before_element {
                // After the new parent, if that is listed and comes first.
                usize::from(new.is_some_and(|new| self.slot_path(new) < self.slot_path(old)))
            } else {
                changed.len()
            };
            changed.insert(at, old);
        }
        Some(
            changed
                .into_iter()
                .map(|node| self.nodes[node].id.clone())
                .collect(),
        )
    }

    /// Whether `node` is `ancestor` or inside it.
    fn is_inside(&self, node: usize, ancestor: usize) -> bool {
        let mut current = Some(node);
        while let Some(handle) = current {
            if handle == ancestor {
                return true;
            }
            current = self.nodes[handle].parent;
        }
        false
    }

    /// `node` or its ancestor that is a child of `parent`, or a root for
    /// `None`; `node` is inside `parent`.
    fn branch_under(&self, node: usize, parent: Option<usize>) -> usize {
        let mut branch = node;
        while self.nodes[branch].parent != parent {
            match self.nodes[branch].parent {
                Some(up) => branch = up,
                None => break,
            }
        }
        branch
    }

    /// Moves `element`, with its subtree and its preserved content, under
    /// `parent` (to the roots for `None`) at `position` of the others: its
    /// index there after the move. Its role is the new parent's role for its
    /// kind; ids, references and features do not change. A preserved child
    /// of either parent keeps its place after the typed child it followed.
    ///
    /// The caller has checked the move ([`move_changes`](Self::move_changes)).
    /// Changes nothing when an id names no element.
    pub(crate) fn move_element(
        &mut self,
        element: &ElementId,
        parent: Option<&ElementId>,
        position: usize,
    ) {
        let Some(&moved) = self.index.get(element) else {
            return;
        };
        let new_parent = match parent {
            Some(parent) => match self.index.get(parent) {
                Some(&parent) => Some(parent),
                None => return,
            },
            None => None,
        };
        let old_parent = self.nodes[moved].parent;
        let siblings = match old_parent {
            Some(parent) => &mut self.nodes[parent].children,
            None => &mut self.roots,
        };
        let Some(old_index) = siblings.iter().position(|&sibling| sibling == moved) else {
            return;
        };
        let kept: Vec<bool> = siblings.iter().map(|&sibling| sibling != moved).collect();
        siblings.remove(old_index);
        for placed in self.placed_mut(old_parent).into_iter().flatten() {
            placed.shift(&kept);
        }
        self.renumber_children(old_parent, old_index);

        let siblings = match new_parent {
            Some(parent) => &mut self.nodes[parent].children,
            None => &mut self.roots,
        };
        let at = position.min(siblings.len());
        siblings.insert(at, moved);
        for placed in self.placed_mut(new_parent).into_iter().flatten() {
            placed.make_room(at);
        }
        let kind = self.nodes[moved].kind;
        self.nodes[moved].role = new_parent.and_then(|parent| role(self.nodes[parent].kind, kind));
        self.nodes[moved].parent = new_parent;
        self.renumber_children(new_parent, at);
    }

    /// What `parent`, or the document for `None`, places among its typed
    /// children: its preserved content and its XMI metadata (ADR 0033, ADR
    /// 0042).
    fn placed_mut(&mut self, parent: Option<usize>) -> [Option<&mut Preserved>; 2] {
        match parent {
            Some(parent) => {
                let node = &mut self.nodes[parent];
                [node.preserved.as_deref_mut(), node.metadata.as_deref_mut()]
            }
            None => [self.preserved.as_deref_mut(), self.metadata.as_deref_mut()],
        }
    }
}

/// Copies (LIB-EDIT-018; ADR 0036 D2).
impl Document {
    /// The elements `listed` of the document, each with everything it
    /// contains, captured for a copy: the tops (each listed element not
    /// inside another listed one, each once) in list order, each with its
    /// subtree in pre-order. Refuses with the first id, in list order, that
    /// names no element.
    pub(crate) fn snapshot(&self, listed: &[ElementId]) -> Result<Snapshot, ElementId> {
        let mut handles = Vec::with_capacity(listed.len());
        for id in listed {
            match self.index.get(id) {
                Some(&handle) => handles.push(handle),
                None => return Err(id.clone()),
            }
        }
        let named: BTreeSet<usize> = handles.iter().copied().collect();
        let mut seen = BTreeSet::new();
        let mut elements: Vec<Captured> = Vec::new();
        for top in handles {
            let inside = {
                let mut up = self.nodes[top].parent;
                let mut inside = false;
                while let Some(ancestor) = up {
                    if named.contains(&ancestor) {
                        inside = true;
                        break;
                    }
                    up = self.nodes[ancestor].parent;
                }
                inside
            };
            if inside || !seen.insert(top) {
                continue;
            }
            let mut pending = vec![(top, None)];
            while let Some((node, parent)) = pending.pop() {
                let index = elements.len();
                elements.push(self.capture(node, parent));
                for &child in self.nodes[node].children.iter().rev() {
                    pending.push((child, Some(index)));
                }
            }
        }
        let ids = elements.iter().map(|element| element.id.clone()).collect();
        Ok(Snapshot { elements, ids })
    }

    /// The node `node` as captured, under the captured element `parent`.
    /// `Node` is destructured with no `..`, so a field added to the model
    /// fails to compile here until a copy handles it.
    fn capture(&self, node: usize, parent: Option<usize>) -> Captured {
        let Node {
            id,
            kind,
            gid,
            name,
            name_id,
            content,
            key,
            location,
            strings,
            assertion_declaration,
            is_citation,
            is_abstract,
            is_counter,
            references,
            hrefs,
            proxies,
            // Replaced by the place in the capture.
            parent: _,
            // Set where the copy is added.
            slot: _,
            // Recomputed from the copy's parent.
            role: _,
            // Captured as the elements that follow, in pre-order.
            children: _,
            preserved,
            metadata,
            name_metadata,
        } = &self.nodes[node];
        Captured {
            id: id.clone(),
            kind: *kind,
            gid: gid.clone(),
            name: name.clone(),
            name_id: name_id.clone(),
            content: content.clone(),
            key: key.clone(),
            location: location.clone(),
            strings: strings.clone(),
            assertion_declaration: *assertion_declaration,
            is_citation: *is_citation,
            is_abstract: *is_abstract,
            is_counter: *is_counter,
            references: references.clone(),
            hrefs: hrefs.clone(),
            proxies: proxies.clone(),
            preserved: preserved.clone(),
            metadata: metadata.clone(),
            name_metadata: name_metadata.clone(),
            parent,
        }
    }

    /// Whether `id` is free in the document's id space: no element, part or
    /// preserved content holds it, and no reference, typed or preserved,
    /// names it (ADR 0036 D2).
    pub(crate) fn is_free(&self, id: &str) -> bool {
        !self.is_id_in_use(id) && !self.is_referenced(id)
    }

    /// The next generated id of `kind` above `after` (the kind's counter
    /// for `None`), with its `n`, that is free and not in `avoid`.
    pub(crate) fn generated_id_avoiding(
        &self,
        kind: ElementKind,
        after: Option<u64>,
        avoid: &BTreeSet<ElementId>,
    ) -> (ElementId, u64) {
        let prefix = snake_case(kind.name());
        let start = after.unwrap_or_else(|| self.id_counters.get(&kind).copied().unwrap_or(0));
        let mut n = start.wrapping_add(1);
        let mut id = generated_id(&prefix, n);
        // Each candidate taken is distinct and taken by one of these, so one
        // of the first `taken + 1` is free, and the loop is bounded.
        let taken = self.nodes.len() + self.other_ids.len() + avoid.len();
        for _ in 0..taken {
            if self.is_free(id.as_str()) && !avoid.contains(&id) {
                break;
            }
            n = n.wrapping_add(1);
            id = generated_id(&prefix, n);
        }
        (id, n)
    }

    /// Adds the copies of the elements of `snapshot` under `parent` (as roots
    /// for `None`), after the last child or root, in the snapshot's order:
    /// the i-th with the id `ids[i]`, each part keeping its `xmi:id` only if
    /// it is in `kept`, each reference to a copied element naming its copy,
    /// and everything else as captured, preserved content included.
    ///
    /// The caller has checked the copy (ADR 0036 D2): the parent may hold
    /// each top, `ids` and the kept part ids and preserved ids are free and
    /// distinct, and every reference resolves. Changes nothing when the
    /// document has no element `parent`.
    #[expect(
        clippy::too_many_lines,
        reason = "one node built field by field, as `capture` takes it apart"
    )]
    pub(crate) fn insert_copy(
        &mut self,
        parent: Option<&ElementId>,
        snapshot: &Snapshot,
        ids: &[ElementId],
        kept: &BTreeSet<ElementId>,
    ) {
        let parent = match parent {
            Some(parent) => match self.index.get(parent) {
                Some(&parent) => Some(parent),
                None => return,
            },
            None => None,
        };
        let mapped: BTreeMap<&ElementId, &ElementId> = snapshot.ids.iter().zip(ids).collect();
        let keep = |id: &Option<ElementId>| id.clone().filter(|id| kept.contains(id));
        // An ExpressionLangString's expression names the copy of a copied
        // element, as every other reference does (SACM23-BASE-009).
        let remap = |value: &LangString| {
            value.map_expression(|expression| mapped.get(expression).map(|&copy| copy.clone()))
        };
        let keep_text = |text: &Option<Text>| text.as_ref().map(|text| text.copied(remap, keep));
        let mut handles: Vec<usize> = Vec::with_capacity(snapshot.elements.len());
        for (captured, id) in snapshot.elements.iter().zip(ids) {
            let holder = match captured.parent {
                Some(index) => handles.get(index).copied(),
                None => parent,
            };
            let node = self.nodes.len();
            let slot = if let Some(holder) = holder {
                self.nodes[holder].children.push(node);
                self.nodes[holder].children.len() - 1
            } else {
                self.roots.push(node);
                self.roots.len() - 1
            };
            let role = holder.and_then(|holder| role(self.nodes[holder].kind, captured.kind));
            self.index.insert(id.clone(), node);
            self.nodes.push(Node {
                id: id.clone(),
                kind: captured.kind,
                gid: captured.gid.clone(),
                name: captured.name.as_ref().map(remap),
                name_id: keep(&captured.name_id),
                content: keep_text(&captured.content),
                key: keep_text(&captured.key),
                location: keep_text(&captured.location),
                strings: captured.strings.clone(),
                assertion_declaration: captured.assertion_declaration,
                is_citation: captured.is_citation,
                is_abstract: captured.is_abstract,
                is_counter: captured.is_counter,
                references: Vec::new(),
                // An href names an element of another document, which a copy
                // never copies, so it is kept as it is (ADR 0040 D7).
                hrefs: captured.hrefs.clone(),
                // A proxy names a child held in another document, which a
                // copy never copies either: the copy holds the same proxy
                // (ADR 0040 amendment 1).
                proxies: captured.proxies.clone(),
                parent: holder,
                slot,
                role,
                children: Vec::new(),
                preserved: None,
                // A copy is a new object: its XMI metadata is kept, but not
                // the `xmi:uuid`s, which identify the original (XMI 2.5.1
                // §7.6.1: elements with one `xmi:uuid` "are all deemed to
                // reference the same MOF element"; ADR 0042 D6).
                metadata: captured
                    .metadata
                    .as_deref()
                    .and_then(Preserved::without_uuid),
                name_metadata: captured
                    .name_metadata
                    .as_deref()
                    .and_then(Preserved::without_uuid),
            });
            let parts: Vec<ElementId> = {
                let added = &self.nodes[node];
                added
                    .name_id
                    .iter()
                    .chain(
                        TextRole::ALL
                            .iter()
                            .filter_map(|&role| added.text(role))
                            .flat_map(Text::ids),
                    )
                    .cloned()
                    .collect()
            };
            for part in parts {
                self.other_ids.entry(part).or_default().owner = Some(node);
            }
            let metadata: Vec<Preserved> = self.nodes[node].metadata_parts().cloned().collect();
            for metadata in &metadata {
                self.hold_metadata(Some(node), metadata);
            }
            for (end, targets) in &captured.references {
                let targets = targets
                    .iter()
                    .map(|target| {
                        mapped
                            .get(target)
                            .map_or_else(|| target.clone(), |&copy| copy.clone())
                    })
                    .collect();
                self.hold_references(node, *end, targets);
            }
            self.refresh_expressions(node);
            if let Some(preserved) = &captured.preserved {
                for id in &preserved.ids {
                    let used = self.other_ids.entry(id.clone()).or_default();
                    used.owner = Some(node);
                    used.preserved = true;
                }
                for id in &preserved.references {
                    let used = self.other_ids.entry(id.clone()).or_default();
                    used.references = used.references.saturating_add(1);
                }
                self.nodes[node].preserved = Some(preserved.clone());
            }
            handles.push(node);
        }
    }
}

/// Elements of a document, each with everything it contains, captured for
/// a [`CopyElements`](crate::edit::CopyElements) when it is built (ADR 0036
/// D2): an owned snapshot, independent of the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    /// The captured elements: the tops in list order, each followed by its
    /// subtree in pre-order.
    elements: Vec<Captured>,
    /// Their ids, in the same order.
    ids: Vec<ElementId>,
}

/// One captured element: a [`Node`] with its place in the [`Snapshot`]
/// instead of handles.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Captured {
    id: ElementId,
    kind: ElementKind,
    gid: Option<String>,
    name: Option<LangString>,
    name_id: Option<ElementId>,
    content: Option<Text>,
    key: Option<Text>,
    location: Option<Text>,
    strings: Vec<(StringAttribute, String)>,
    assertion_declaration: Option<AssertionDeclaration>,
    is_citation: bool,
    is_abstract: bool,
    is_counter: bool,
    references: Vec<(ReferenceEnd, Vec<ElementId>)>,
    hrefs: Vec<(ReferenceEnd, Vec<Href>)>,
    proxies: Vec<Proxy>,
    preserved: Option<Box<Preserved>>,
    metadata: Option<Box<Preserved>>,
    name_metadata: Option<Box<Preserved>>,
    /// The index of the captured element that contains it; `None` for a
    /// top.
    parent: Option<usize>,
}

impl Snapshot {
    /// The ids of the captured elements, in order.
    pub(crate) fn ids(&self) -> &[ElementId] {
        &self.ids
    }

    /// The captured elements, in order, as views.
    pub(crate) fn elements(&self) -> impl Iterator<Item = CapturedElement<'_>> {
        self.elements
            .iter()
            .map(|captured| CapturedElement { captured })
    }
}

/// A view of one element of a [`Snapshot`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct CapturedElement<'s> {
    captured: &'s Captured,
}

impl<'s> CapturedElement<'s> {
    /// Its id in the source.
    pub(crate) fn id(self) -> &'s ElementId {
        &self.captured.id
    }

    /// Its kind.
    pub(crate) fn kind(self) -> ElementKind {
        self.captured.kind
    }

    /// Whether it is a top: not inside another captured element.
    pub(crate) fn is_top(self) -> bool {
        self.captured.parent.is_none()
    }

    /// The type attribute it keeps that only a root can carry, if any
    /// ([`Preserved::kept_type`], LIB-COMPAT-019).
    pub(crate) fn kept_type(self) -> Option<&'s Attr> {
        self.captured.preserved.as_deref()?.kept_type()
    }

    /// The ids its end `end` holds, in order.
    pub(crate) fn references(self, end: ReferenceEnd) -> &'s [ElementId] {
        targets_of(&self.captured.references, end)
    }

    /// The cross-document references its end `end` holds, in order.
    pub(crate) fn hrefs(self, end: ReferenceEnd) -> &'s [Href] {
        hrefs_of(&self.captured.hrefs, end)
    }

    /// Whether its name or a value of its texts is an `ExpressionLangString`
    /// with no `expression`, neither an id nor an href (§8.4
    /// `expression[1]`), which a copy refuses.
    pub(crate) fn has_missing_expression(self) -> bool {
        let captured = self.captured;
        captured
            .name
            .iter()
            .chain(
                [&captured.content, &captured.key, &captured.location]
                    .into_iter()
                    .flatten()
                    .flat_map(|text| text.value.values()),
            )
            .any(LangString::lacks_expression)
    }

    /// The ids of its name, `content`, `key`, `location` and their values.
    pub(crate) fn part_ids(self) -> impl Iterator<Item = &'s ElementId> {
        let captured = self.captured;
        captured.name_id.iter().chain(
            [&captured.content, &captured.key, &captured.location]
                .into_iter()
                .flatten()
                .flat_map(Text::ids),
        )
    }

    /// The `xmi:id`s of its preserved content.
    pub(crate) fn preserved_ids(self) -> &'s [ElementId] {
        self.captured
            .preserved
            .as_deref()
            .map_or(&[], |preserved| &preserved.ids)
    }

    /// The ids its preserved content may name.
    pub(crate) fn preserved_references(self) -> &'s [ElementId] {
        self.captured
            .preserved
            .as_deref()
            .map_or(&[], |preserved| &preserved.references)
    }

    /// Every XMI metadata it holds, which a copy keeps but for the
    /// `xmi:uuid`s (ADR 0042 D6): its own, its name's, then that of its texts
    /// and their values.
    fn metadata(self) -> impl Iterator<Item = &'s Preserved> {
        let captured = self.captured;
        captured
            .metadata
            .as_deref()
            .into_iter()
            .chain(captured.name_metadata.as_deref())
            .chain(
                [&captured.content, &captured.key, &captured.location]
                    .into_iter()
                    .flatten()
                    .flat_map(Text::metadata),
            )
    }

    /// The `xmi:id`s of its XMI metadata, which the copy keeps (ADR 0042 D5).
    pub(crate) fn metadata_ids(self) -> impl Iterator<Item = &'s ElementId> {
        self.metadata().flat_map(|metadata| metadata.ids.iter())
    }

    /// The ids its XMI metadata may name.
    pub(crate) fn metadata_references(self) -> impl Iterator<Item = &'s ElementId> {
        self.metadata()
            .flat_map(|metadata| metadata.references.iter())
    }
}

/// Checks that the ids of `preserved` can join the id space of `document`:
/// none is empty, given twice or already in use.
fn check_preserved_ids(document: &Document, preserved: &Preserved) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in &preserved.ids {
        if id.as_str().is_empty() {
            return Err("an id of the preserved content is empty".to_owned());
        }
        if !seen.insert(id.as_str()) || document.is_id_in_use(id.as_str()) {
            return Err(format!("the id {:?} is already in use", id.as_str()));
        }
    }
    Ok(())
}

/// Why an element of `kind` cannot hold `targets` in `end`, if it cannot:
/// its kind has no such end, a target is empty, or a single-valued end is
/// given more than one target.
fn reference_problem(
    kind: ElementKind,
    end: ReferenceEnd,
    targets: &[ElementId],
) -> Option<String> {
    if !end.applies_to(kind) {
        return Some(format!("is a {kind}, which has no {}", end.name()));
    }
    if targets.iter().any(|target| target.as_str().is_empty()) {
        return Some(format!("is given an empty target of the {}", end.name()));
    }
    if !end.is_many() && targets.len() > 1 {
        return Some(format!(
            "is given {} targets of the {}, which holds one",
            targets.len(),
            end.name()
        ));
    }
    None
}

/// Removes one occurrence of `holder` from `holders`, whose order does not
/// matter.
fn release_holder(holders: &mut Vec<usize>, holder: usize) {
    if let Some(position) = holders.iter().position(|&held| held == holder) {
        holders.swap_remove(position);
    }
}

/// `<prefix>_<n>`.
fn generated_id(prefix: &str, n: u64) -> ElementId {
    ElementId::new(format!("{prefix}_{n}"))
}

/// The ids the XMI reader generates for the elements it reads without an
/// `xmi:id` (ADR 0024), in the scheme edits use (ADR 0019), with a counter
/// per kind that [`commit`](Self::commit) hands to the document read.
///
/// Each kind's counter only grows, so the candidates tried for a kind over a
/// whole read are distinct, and generation is linear in the number of
/// elements generated and ids reserved.
#[derive(Debug, Default)]
pub(crate) struct IdGenerator {
    /// The `n` of the last id generated for each kind, with the kind's
    /// prefix.
    counters: BTreeMap<ElementKind, (String, u64)>,
}

impl IdGenerator {
    /// The next id of `kind`: `<snake_case kind>_<n>` for the smallest `n`
    /// above the kind's counter for which `is_reserved` is false. Ids of
    /// different kinds never collide, since the prefixes differ and the
    /// suffix is a number.
    pub(crate) fn next(
        &mut self,
        kind: ElementKind,
        is_reserved: impl Fn(&str) -> bool,
    ) -> ElementId {
        let (prefix, counter) = self
            .counters
            .entry(kind)
            .or_insert_with(|| (snake_case(kind.name()), 0));
        // The reserved ids are finitely many, so the loop ends.
        loop {
            *counter = counter.wrapping_add(1);
            let id = generated_id(prefix, *counter);
            if !is_reserved(id.as_str()) {
                return id;
            }
        }
    }

    /// Sets the counters of `document` to these, so that an edit generates
    /// above every id generated in the read.
    pub(crate) fn commit(self, document: &mut Document) {
        for (kind, (_, n)) in self.counters {
            document.commit_generated_id(kind, n);
        }
    }
}

/// A SACM class name in snake case: `AssuranceCasePackage` becomes
/// `assurance_case_package`.
fn snake_case(class: &str) -> String {
    let mut out = String::with_capacity(class.len() + 4);
    for (position, c) in class.char_indices() {
        if c.is_ascii_uppercase() {
            if position > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
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
/// Named after SACM's abstract class `SACMElement`
/// ([`AbstractClass::SACMElement`](super::AbstractClass::SACMElement)), which
/// every [`ElementKind`] specializes.
///
/// A view is a cheap, copyable handle that borrows the document. Everything
/// it returns, including the views it navigates to, borrows the document,
/// not the view, so it can outlive the view it came from.
///
/// # Examples
///
/// ```
/// use sacm::model::{Document, ElementId, ElementKind, SacmElement};
///
/// /// The claims directly under `package`, in document order.
/// fn claims<'d>(package: SacmElement<'d>) -> impl Iterator<Item = SacmElement<'d>> {
///     package.children().filter(|child| child.kind() == ElementKind::Claim)
/// }
///
/// // Built with the unstable test-support feature (not shown on docs.rs; see the crate's Features), which inserts an element without a name.
/// let mut doc = Document::new();
/// let id = ElementId::from;
/// doc.insert(None, ElementKind::ArgumentPackage, id("arguments"), None).unwrap();
/// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_top"), None).unwrap();
/// doc.insert(Some(&id("arguments")), ElementKind::ArgumentReasoning, id("reasoning"), None).unwrap();
/// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_sub"), None).unwrap();
///
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

    /// The element's `name` (SACM 2.3 §8.6), or `None` when the name is
    /// missing or the element's kind has no name.
    ///
    /// Every `ModelElement` has a name in SACM, but a document can hold one
    /// whose name is missing, as a file can leave it out. That is `None`:
    /// nothing invents a name ([ADR 0013]). A name whose `lang` or `content`
    /// is absent or empty is still a name.
    ///
    /// [ADR 0013]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0013-missing-name-stays-missing.md
    ///
    /// The four `UtilityElement` kinds (`Description`,
    /// `ImplementationConstraint`, `Note` and `TaggedValue`) have no name, so
    /// this is always `None` for them.
    /// `kind().is_a(AbstractClass::ModelElement)` tells the two cases apart.
    ///
    /// The name borrows the document, not the view.
    ///
    /// ```
    /// use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString};
    ///
    /// // Built with the unstable test-support feature (not shown on docs.rs; see the crate's Features): edit::apply does not create a missing name.
    /// let mut doc = Document::new();
    /// let id = ElementId::from;
    /// let name = LangString::new("Top claim").with_lang("en");
    /// doc.insert(None, ElementKind::ArgumentPackage, id("arguments"), None).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_top"), Some(name.clone())).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("claim_unnamed"), None).unwrap();
    /// doc.insert(Some(&id("claim_top")), ElementKind::Note, id("note"), None).unwrap();
    ///
    /// let claim = doc.get("claim_top").expect("the document has it");
    /// assert_eq!(claim.name(), Some(&name));
    ///
    /// // A missing name, on a kind that has one in SACM.
    /// let unnamed = doc.get("claim_unnamed").expect("the document has it");
    /// assert_eq!(unnamed.name(), None);
    /// assert!(unnamed.kind().is_a(AbstractClass::ModelElement));
    ///
    /// // A Note has no name at all.
    /// let note = doc.get("note").expect("the document has it");
    /// assert_eq!(note.name(), None);
    /// assert!(!note.kind().is_a(AbstractClass::ModelElement));
    /// ```
    #[must_use]
    pub fn name(self) -> Option<&'a LangString> {
        self.node().name.as_ref()
    }

    /// The `xmi:id` of the element's `name`, if the name was read with one,
    /// or `None`. A `LangString` is a class, so its element may carry an id
    /// (XMI 2.5.1 §7.8.3).
    ///
    /// The id is kept with its owner, not on the [`LangString`] value (ADR
    /// 0024). It shares the document's id space: no element and no other
    /// name has it, and [`edit::apply`](crate::edit::apply) neither generates
    /// it nor accepts it as a new element's id. It identifies no element, so
    /// [`Document::get`] does not find it. [`xmi::write`](crate::xmi::write)
    /// writes it as the first attribute of `name`. Only
    /// [`xmi::read`](crate::xmi::read) gives one, and it is `None` whenever
    /// [`name`](Self::name) is; a name replaced with
    /// [`SetName`](crate::edit::SetName) keeps it (ADR 0029).
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name xmi:id="arguments_name" content="Arguments"/>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let arguments = document.get("arguments").expect("the root was read");
    /// assert_eq!(arguments.name_id().map(|id| id.as_str()), Some("arguments_name"));
    /// assert!(document.get("arguments_name").is_none());
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn name_id(self) -> Option<&'a ElementId> {
        self.node().name_id.as_ref()
    }

    /// The element's `gid` (SACM 2.3 §8.2), or `None` when it has none.
    ///
    /// Every element, of every kind, may have one. It is held exactly as
    /// read: `None`, `""` and any other string are three different values,
    /// and nothing is trimmed ([ADR 0012], [ADR 0025]). A gid is not an
    /// `xmi:id`: it is in no id space, so [`Document::get`] does not find it,
    /// and it has no syntax check. The document does not stop two elements
    /// from sharing one, though §8.2 requires a gid to be unique:
    /// [`validation::validate`](crate::validation::validate) reports each
    /// non-empty value two or more elements hold as a SACM-ID-003 Error
    /// (SACM23-VAL-002). [`SetGid`](crate::edit::SetGid) sets or removes it,
    /// exactly as given, and does not refuse a duplicate.
    ///
    /// [ADR 0012]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0012-lang-string-absent-and-empty.md
    /// [ADR 0025]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0025-utility-elements-and-gid.md
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments" gid="urn:x:1">
    ///   <name content="Arguments"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" gid="">
    ///     <name content="C1"/>
    ///   </argumentElement>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
    ///     <name content="C2"/>
    ///   </argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let gid = |id: &str| document.get(id).and_then(|element| element.gid());
    /// assert_eq!(gid("arguments"), Some("urn:x:1"));
    /// assert_eq!(gid("c1"), Some(""));
    /// assert_eq!(gid("c2"), None);
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn gid(self) -> Option<&'a str> {
        self.node().gid.as_deref()
    }

    /// Whether the element cites another (SACM 2.3 §8.2
    /// `isCitation[0..1]=false`): `false` unless the document holds `true`.
    ///
    /// Every element, of every kind, has the flag. Absent and `false` are the
    /// same value, since a default is not written (ADR 0026). The flag and
    /// [`cited_element`](Self::cited_element) are independent: either may be
    /// present without the other, and nothing repairs a file on read. §8.2
    /// requires `isCitation` to be true when there is a cited element;
    /// [`validation::validate`](crate::validation::validate) reports a cited
    /// element without it as SACM-CITE-001 (SACM23-VAL-007).
    /// [`SetFlag`](crate::edit::SetFlag) sets the flag, independently of the
    /// cited element (ADR 0029).
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" isCitation="true" citedElement="c2">
    ///     <name content="The pump is safe, as the supplier argues"/>
    ///   </argumentElement>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2">
    ///     <name content="The pump is safe"/>
    ///   </argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let c1 = document.get("c1").expect("the Claim was read");
    /// assert!(c1.is_citation());
    /// // The cited element is an id, which the document resolves.
    /// let cited = c1.cited_element().and_then(|id| document.get(id));
    /// assert_eq!(cited.map(|c2| c2.id().as_str()), Some("c2"));
    /// assert!(!document.get("c2").expect("the Claim was read").is_citation());
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn is_citation(self) -> bool {
        self.node().is_citation
    }

    /// The id of the element this one cites (SACM 2.3 §8.2
    /// `citedElement:SACMElement[0..1]`), or `None`.
    ///
    /// The reference is held as the target's id, unresolved: look the target
    /// up with [`Document::get`]. The id may name no element of the document,
    /// such as an id absent from the file or the id of a `name`; the document
    /// keeps it, and [`validation::validate`](crate::validation::validate)
    /// reports it as SACM-REF-001 (SACM23-VAL-003).
    /// [`SetReferences`](crate::edit::SetReferences) sets it to an id that
    /// names an element (ADR 0029). See [`is_citation`](Self::is_citation)
    /// for an example.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn cited_element(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::CitedElement).first()
    }

    /// Whether the element is abstract (SACM 2.3 §8.2
    /// `isAbstract[0..1]=false`): `false` unless the document holds `true`.
    ///
    /// Every element, of every kind, has the flag, under the rules of
    /// [`is_citation`](Self::is_citation). An abstract element is, for
    /// example, part of a pattern or template (§8.2); a concrete element
    /// names the abstract one it conforms to through its
    /// [`abstract_form`](Self::abstract_form).
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="pattern" isAbstract="true">
    ///     <name content="{System} is safe"/>
    ///   </argumentElement>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" abstractForm="pattern">
    ///     <name content="The brakes are safe"/>
    ///   </argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let c1 = document.get("c1").expect("the Claim was read");
    /// assert!(!c1.is_abstract());
    /// let form = c1.abstract_form().and_then(|id| document.get(id)).expect("it resolves");
    /// assert!(form.is_abstract());
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn is_abstract(self) -> bool {
        self.node().is_abstract
    }

    /// The id of the abstract element this concrete one conforms to (SACM
    /// 2.3 §8.2 `abstractForm:SACMElement[0..1]`), or `None`.
    ///
    /// Held unresolved, as [`cited_element`](Self::cited_element) is. §8.2
    /// asks that the element itself be concrete, and that the target be
    /// abstract and of the same kind;
    /// [`validation::validate`](crate::validation::validate) reports each
    /// case that is not as SACM-ABS-001 (SACM23-VAL-008).
    /// [`SetReferences`](crate::edit::SetReferences) sets it, and
    /// [`SetFlag`](crate::edit::SetFlag) sets `isAbstract`, each
    /// independently (ADR 0029). See [`is_abstract`](Self::is_abstract) for
    /// an example.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn abstract_form(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::AbstractForm).first()
    }

    /// The ids of the Claims about this Assertion (SACM 2.3 §11.10
    /// `metaClaim:Claim[0..*]`), such as Claims about the confidence in it,
    /// in document order: empty when it has none or its kind is not an
    /// Assertion.
    ///
    /// Every Assertion (a `Claim` or one of the five asserted relationships;
    /// see [`AbstractClass::Assertion`]) has the end. The ids are held
    /// unresolved, as [`cited_element`](Self::cited_element) is, in the order
    /// read and with repeats (ADR 0027): `metaClaim="m1 m1"` holds `m1`
    /// twice. Each distinct id that names no element is one SACM-REF-001
    /// finding of [`validation::validate`](crate::validation::validate), each
    /// that names an element that is no Claim is one SACM-REF-002 finding
    /// (SACM23-VAL-004), and each repeated id is one SACM-MULT-001 Warning
    /// (SACM23-VAL-022). [`SetReferences`](crate::edit::SetReferences),
    /// [`AddReference`](crate::edit::AddReference) and
    /// [`RemoveReference`](crate::edit::RemoveReference) edit it.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" metaClaim="m1 m2">
    ///     <name content="The brakes are safe"/>
    ///   </argumentElement>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="m1">
    ///     <name content="The brake tests are representative"/>
    ///   </argumentElement>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="m2">
    ///     <name content="The test team is independent"/>
    ///   </argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let c1 = document.get("c1").expect("the Claim was read");
    /// let meta: Vec<&str> = c1.meta_claims().iter().map(|id| id.as_str()).collect();
    /// assert_eq!(meta, ["m1", "m2"]);
    /// assert!(document.get("m1").expect("the Claim was read").meta_claims().is_empty());
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn meta_claims(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::MetaClaim)
    }

    /// The ids of the sources of an asserted relationship (SACM 2.3 §11.13
    /// `source:ArgumentAsset[1..*]`), its starting points, or of an
    /// `ArtifactAssetRelationship` (§12.14 `source:ArtifactAsset[1..*]`), in
    /// document order: empty when it has none or its kind is neither.
    ///
    /// On an `ArtifactAssetRelationship` they are its
    /// [`ArtifactSource`](ReferenceEnd::ArtifactSource) end, on an asserted
    /// relationship its [`Source`](ReferenceEnd::Source) end (ADR 0034
    /// amendment 2).
    ///
    /// For an `AssertedInference` they are the premises; for an
    /// `AssertedContext`, the context; for an `AssertedEvidence`, the
    /// evidence, which §11.15 requires to be `ArtifactReference`s. They are
    /// held unresolved and with repeats, as [`meta_claims`](Self::meta_claims)
    /// are. A relationship needs at least one:
    /// [`validation::validate`](crate::validation::validate) reports one with
    /// none as SACM-MULT-001 (SACM23-VAL-005), and
    /// [`CreateAssertedInference`](crate::edit::CreateAssertedInference)
    /// refuses to create one. A source that is no `ArgumentAsset` is
    /// SACM-REF-002 (SACM23-VAL-004), and so is one of an `AssertedEvidence`,
    /// `AssertedArtifactSupport` or `AssertedArtifactContext` that is no
    /// `ArtifactReference` (SACM23-VAL-016).
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// for (id, text) in [("c1", "The brakes are safe"), ("c2", "Each hazard is mitigated")] {
    ///     edit::apply(&mut doc, CreateClaim::new("p", name(text)).with_id(id))?;
    /// }
    /// let inference = CreateAssertedInference::new("p", name("Mitigation supports safety"), ["c2"], "c1");
    /// edit::apply(&mut doc, inference.with_id("i1"))?;
    ///
    /// let i1 = doc.get("i1").expect("just created");
    /// assert_eq!(i1.sources().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["c2"]);
    /// assert_eq!(i1.target().map(|id| id.as_str()), Some("c1"));
    /// assert_eq!(i1.reasoning(), None);
    /// assert!(!i1.is_counter());
    /// // A Claim has no sources.
    /// assert!(doc.get("c1").expect("just created").sources().is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn sources(self) -> &'a [ElementId] {
        if self.kind() == ElementKind::ArtifactAssetRelationship {
            self.references(ReferenceEnd::ArtifactSource)
        } else {
            self.references(ReferenceEnd::Source)
        }
    }

    /// The ids of the targets of an `ArtifactAssetRelationship` (§12.14
    /// `target:ArtifactAsset[1..*]`), or the one `target` of an asserted
    /// relationship (§11.13), in order with repeats; empty when it has none
    /// or the kind is neither.
    ///
    /// On an `ArtifactAssetRelationship` they are its
    /// [`ArtifactTarget`](ReferenceEnd::ArtifactTarget) end, which holds one
    /// or more ids; on an asserted relationship its
    /// [`Target`](ReferenceEnd::Target) end, which [`target`](Self::target)
    /// also reads. A relationship needs at least one:
    /// [`validation::validate`](crate::validation::validate) reports an
    /// `ArtifactAssetRelationship` with none as SACM-MULT-001
    /// (SACM23-ART-010).
    ///
    /// ```
    /// use sacm::edit::{self, CreateActivity, CreateArtifact, CreateArtifactAssetRelationship, CreateArtifactPackage};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateArtifact::new("evidence", en("Brake test report")).with_id("report"))?;
    /// edit::apply(&mut doc, CreateActivity::new("evidence", en("Brake testing")).with_id("testing"))?;
    /// let link = CreateArtifactAssetRelationship::new("evidence", en("Produced by"), ["report"], ["testing"]);
    /// edit::apply(&mut doc, link.with_id("rel"))?;
    ///
    /// let rel = doc.get("rel").expect("just created");
    /// assert_eq!(rel.sources().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["report"]);
    /// assert_eq!(rel.targets().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["testing"]);
    /// // `target()` is an asserted relationship's.
    /// assert_eq!(rel.target(), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[doc(alias = "target")]
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn targets(self) -> &'a [ElementId] {
        if self.kind() == ElementKind::ArtifactAssetRelationship {
            self.references(ReferenceEnd::ArtifactTarget)
        } else {
            self.references(ReferenceEnd::Target)
        }
    }

    /// The id of the target of an asserted relationship (SACM 2.3 §11.13
    /// `target:ArgumentAsset[1]`), its ending point, or `None` when it has
    /// none or its kind is not an `AssertedRelationship`: `None` for an
    /// `ArtifactAssetRelationship`, whose targets are
    /// [`targets`](Self::targets).
    ///
    /// For an `AssertedInference` it is the conclusion; for an
    /// `AssertedContext`, the element the context is for, such as a Claim
    /// (§11.16); for an `AssertedEvidence`, what the evidence supports, such
    /// as a Claim (§11.15). It is held
    /// unresolved. A relationship needs one:
    /// [`validation::validate`](crate::validation::validate) reports one
    /// without it as SACM-MULT-001 (SACM23-VAL-005). A target that is no
    /// `ArgumentAsset` is SACM-REF-002 (SACM23-VAL-004), and so is one of an
    /// `AssertedArtifactSupport` or `AssertedArtifactContext` that is no
    /// `ArtifactReference` (§11.17, §11.18, SACM23-VAL-016). See
    /// [`sources`](Self::sources) for an example.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn target(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::Target).first()
    }

    /// The id of the `ArgumentReasoning` behind an asserted relationship
    /// (SACM 2.3 §11.13 `reasoning:ArgumentReasoning[0..1]`), or `None` when
    /// it has none or its kind is not an `AssertedRelationship`. It is held
    /// unresolved. See [`sources`](Self::sources) for an example.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn reasoning(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::Reasoning).first()
    }

    /// The id of the `ArgumentPackage` that gives the detailed structure of
    /// an `ArgumentReasoning` (SACM 2.3 §11.12
    /// `structure:ArgumentPackage[0..1]`), or `None` when it has none or the
    /// element is not an `ArgumentReasoning`. It is held unresolved.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArgumentReasoning};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Hazard arguments")).with_id("hazards"))?;
    /// let reasoning = CreateArgumentReasoning::new("p", name("Over each hazard")).with_structure("hazards");
    /// edit::apply(&mut doc, reasoning.with_id("r1"))?;
    ///
    /// let r1 = doc.get("r1").expect("just created");
    /// assert_eq!(r1.structure().map(|id| id.as_str()), Some("hazards"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn structure(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::Structure).first()
    }

    /// The ids of the elements an `ArtifactReference` refers to (SACM 2.3
    /// §11.9 `referencedArtifactElement:Base::ArtifactElement[0..*]`), in
    /// document order: empty when it has none or the element is not an
    /// `ArtifactReference`.
    ///
    /// An `ArtifactReference` is how an argument cites an artifact, or any
    /// other element, such as a Claim of another argument package (§11.1).
    /// Artifacts themselves (§12, the `ArtifactPackage`) are not supported
    /// yet, so for now it can name only elements of argument and
    /// assurance-case packages, or nothing. Every kind except the
    /// `UtilityElement`s (`Description`, `ImplementationConstraint`, `Note`
    /// and `TaggedValue`) conforms to `ArtifactElement`, so any of them is a
    /// well-typed target; a `UtilityElement` is not, and
    /// [`validation::validate`](crate::validation::validate) reports one as
    /// SACM-REF-002 (SACM23-VAL-004). The ids are held unresolved and with
    /// repeats, as [`meta_claims`](Self::meta_claims) are; a repeated id is a
    /// SACM-MULT-001 Warning (SACM23-VAL-022).
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference, CreateClaim};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Supplier arguments")).with_id("s"))?;
    /// edit::apply(&mut doc, CreateClaim::new("s", name("The supplier's brakes meet the spec")).with_id("c1"))?;
    /// // The argument in p cites the supplier's Claim.
    /// let reference = CreateArtifactReference::new("p", name("The supplier's brake claim"))
    ///     .with_referenced_artifact_elements(["c1"]);
    /// edit::apply(&mut doc, reference.with_id("a1"))?;
    ///
    /// let a1 = doc.get("a1").expect("just created");
    /// let ids: Vec<&str> = a1.referenced_artifact_elements().iter().map(|id| id.as_str()).collect();
    /// assert_eq!(ids, ["c1"]);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn referenced_artifact_elements(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::ReferencedArtifactElement)
    }

    /// The ids of the members of an `ArgumentGroup` (SACM 2.3 §11.2
    /// `argumentElement:ArgumentationElement[0..*]`, which the text spells
    /// `argumentationElement`), of an `ArtifactGroup` (§12.3
    /// `artifactElement:ArtifactElement[0..*]`) or of a `TerminologyGroup`
    /// (§10.3 `terminologyElement:TerminologyElement[0..*]`), in document
    /// order: empty when it has none or the element is no group.
    ///
    /// In XMI the end is named `argumentElement` on an `ArgumentGroup`, like
    /// the composite role of an `ArgumentPackage`, `artifactElement` on an
    /// `ArtifactGroup`, like the composite role of an `ArtifactPackage`, and
    /// `terminologyElement` on a `TerminologyGroup`, like the composite role
    /// of a `TerminologyPackage`. Each is a reference: a member stays where
    /// it is contained, and groups "serve no structural purpose" (§11.2,
    /// §12.3, §10.3). The ids are held
    /// unresolved and with repeats, as [`meta_claims`](Self::meta_claims)
    /// are. A member of a kind its end does not admit, such as an
    /// `AssuranceCasePackage` in an `ArgumentGroup` or a `Note` in either, is
    /// a SACM-REF-002 finding of
    /// [`validation::validate`](crate::validation::validate)
    /// (SACM23-VAL-004). The three ends are [`ReferenceEnd::GroupMember`],
    /// [`ReferenceEnd::ArtifactGroupMember`] and
    /// [`ReferenceEnd::TerminologyGroupMember`].
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentGroup, CreateArgumentPackage, CreateClaim};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("The brakes are safe")).with_id("c1"))?;
    /// let group = CreateArgumentGroup::new("p", name("Braking")).with_group_members(["c1"]);
    /// edit::apply(&mut doc, group.with_id("g1"))?;
    ///
    /// let g1 = doc.get("g1").expect("just created");
    /// assert_eq!(g1.group_members().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["c1"]);
    /// // The member is still a child of its package.
    /// let c1 = doc.get("c1").expect("just created");
    /// assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("p"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[doc(alias = "argumentElement")]
    #[doc(alias = "argumentationElement")]
    #[doc(alias = "artifactElement")]
    #[doc(alias = "terminologyElement")]
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn group_members(self) -> &'a [ElementId] {
        match self.kind() {
            ElementKind::ArtifactGroup => self.references(ReferenceEnd::ArtifactGroupMember),
            ElementKind::TerminologyGroup => self.references(ReferenceEnd::TerminologyGroupMember),
            _ => self.references(ReferenceEnd::GroupMember),
        }
    }

    /// The ids of a Category's, Term's or Expression's `category` end, in
    /// document order, repeats included (SACM 2.3 §10.8, §10.9
    /// `category:Category[0..*]`); empty for every other kind.
    ///
    /// On a Term or an Expression they are the Categories it belongs to. On a
    /// Category, formal/23-05-08 does not say which way the end points (spec
    /// defect D21), so they are held as written. It is
    /// [`references`](Self::references) of
    /// [`ReferenceEnd::Category`].
    ///
    /// ```
    /// use sacm::edit::{self, CreateCategory, CreateTerm, CreateTerminologyPackage};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
    /// edit::apply(&mut doc, CreateCategory::new("terms", en("Hazards")).with_id("hazards"))?;
    /// let blade = CreateTerm::new("terms", en("Blade"), "blade").with_categories(["hazards"]);
    /// edit::apply(&mut doc, blade.with_id("blade"))?;
    ///
    /// let blade = doc.get("blade").expect("just created");
    /// assert_eq!(blade.categories().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["hazards"]);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn categories(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::Category)
    }

    /// The `ExpressionElements` an Expression is formed from (SACM 2.3 §10.10
    /// `element:ExpressionElement[0..*]`), in document order, repeats
    /// included; empty for every other kind. It is
    /// [`references`](Self::references) of [`ReferenceEnd::Element`].
    #[doc(alias = "element")]
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn expression_elements(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::Element)
    }

    /// The origin of a Term (SACM 2.3 §10.11
    /// `origin:Base::ModelElement[0..1]`, "a reference which points to the
    /// origin of the Term"), or `None` when it has none or the element is no
    /// Term.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn origin(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::Origin).first()
    }

    /// The interfaces a package declares (`interface`, ptc/22-03-13), in
    /// document order, repeats included; empty for a kind without the end.
    /// It is [`references`](Self::references) of [`ReferenceEnd::Interface`].
    /// The end is held as written: it is never synced with the interfaces'
    /// [`implements`](Self::implements) (ADR 0039 D2).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateArgumentPackageInterface};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(en("Braking")).with_id("braking"))?;
    /// let api = CreateArgumentPackageInterface::new("braking", en("Braking interface"), "braking");
    /// edit::apply(&mut doc, api.with_id("braking_api"))?;
    /// // Creating the interface does not declare it in the package's end.
    /// assert!(doc.get("braking").expect("just created").interfaces().is_empty());
    /// edit::apply(&mut doc, AddReference::new("braking", ReferenceEnd::Interface, "braking_api"))?;
    /// assert_eq!(doc.get("braking").expect("just created").interfaces()[0].as_str(), "braking_api");
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[doc(alias = "interface")]
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn interfaces(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::Interface)
    }

    /// The package an interface declares (SACM 2.3 §9.3, §10.5, §11.6,
    /// §12.5 `implements:<package>[1]`), or `None` when it holds none or the
    /// element is no interface.
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn implements(self) -> Option<&'a ElementId> {
        self.references(ReferenceEnd::Implements).first()
    }

    /// The packages a binding binds (SACM 2.3 §9.4, §10.6, §11.5, §12.4
    /// `participantPackage:<package>[2..*]`), in order, repeats included;
    /// empty for a kind without the end.
    #[doc(alias = "participantPackage")]
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn participant_packages(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::ParticipantPackage)
    }

    /// Whether an asserted relationship counters its declared purpose (SACM
    /// 2.3 §11.13 `isCounter:Boolean[1] = false`); §11.13's example is an
    /// `AssertedEvidence` with `isCounter` true, which is counter-evidence.
    /// `false` unless the document holds `true`, and always `false` when the
    /// kind is not an `AssertedRelationship`.
    ///
    /// Absent and `false` are the same value, since a default is not
    /// written (ADR 0026, ADR 0027). See [`sources`](Self::sources) for an
    /// example.
    #[must_use]
    pub fn is_counter(self) -> bool {
        self.node().is_counter
    }

    /// The value of the flag `flag` of the element: what
    /// [`is_citation`](Self::is_citation), [`is_abstract`](Self::is_abstract)
    /// or [`is_counter`](Self::is_counter) gives. A flag the kind lacks
    /// ([`Flag::applies_to`]) is `false`.
    ///
    /// See [`references`](Self::references) for a property panel built on it.
    #[must_use]
    pub fn flag(self, flag: Flag) -> bool {
        self.node().flag(flag)
    }

    /// The ids the reference end `end` of the element holds, in order,
    /// repeats included: what the end's own accessor, such as
    /// [`meta_claims`](Self::meta_claims) or [`target`](Self::target),
    /// gives. An end the kind lacks ([`ReferenceEnd::applies_to`]) holds no
    /// id; so does an `ArgumentGroup`'s `artifactElement` and an
    /// `ArtifactGroup`'s `argumentElement`.
    ///
    /// # Examples
    ///
    /// A generic property panel: a Claim's flags and ends, with their
    /// bounds, without naming each variant.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetFlag, SetReferences};
    /// use sacm::model::{Document, Flag, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C2")).with_id("c2"))?;
    /// edit::apply(&mut doc, SetFlag::new("c2", Flag::IsAbstract, true))?;
    /// edit::apply(&mut doc, SetReferences::new("c2", ReferenceEnd::MetaClaim, ["c1"]))?;
    ///
    /// let c2 = doc.get("c2").expect("just created");
    /// let mut panel = Vec::new();
    /// for &flag in Flag::ALL.iter().filter(|flag| flag.applies_to(c2.kind())) {
    ///     panel.push(format!("{} = {}", flag.name(), c2.flag(flag)));
    /// }
    /// for &end in ReferenceEnd::ALL {
    ///     if let Some((lower, upper)) = end.bounds(c2.kind()) {
    ///         let upper = upper.map_or("*".to_owned(), |upper| upper.to_string());
    ///         let ids: Vec<&str> = c2.references(end).iter().map(|id| id.as_str()).collect();
    ///         panel.push(format!("{} [{lower}..{upper}] = {ids:?}", end.name()));
    ///     }
    /// }
    /// assert_eq!(
    ///     panel,
    ///     [
    ///         "isCitation = false",
    ///         "isAbstract = true",
    ///         "citedElement [0..1] = []",
    ///         "abstractForm [0..1] = []",
    ///         "metaClaim [0..*] = [\"c1\"]",
    ///         "expression [0..*] = []",
    ///     ]
    /// );
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    ///
    /// An `href` to another document is not here: see [`hrefs`](Self::hrefs).
    #[must_use]
    pub fn references(self, end: ReferenceEnd) -> &'a [ElementId] {
        targets_of(&self.node().references, end)
    }

    /// The cross-document references the end `end` holds: each the `href` of
    /// a reference element that names an element of another document by its
    /// id, in document order, repeats included, never resolved
    /// (SACM23-XMI-027, ADR 0040). Empty when it holds none or the kind
    /// lacks the end. [`references`](Self::references) gives the end's ids in
    /// this document; its bounds count both.
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use sacm::model::ReferenceEnd;
    /// use sacm::{validation, xmi::{self, ReadOptions}};
    ///
    /// let input = br#"<sacm:ArgumentPackageBinding xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="binding">
    ///   <name content="Braking supports the vehicle"/>
    ///   <participantPackage href="braking.sacm.xmi#braking_api"/>
    ///   <participantPackage href="vehicle.sacm.xmi#vehicle_api"/>
    /// </sacm:ArgumentPackageBinding>"#;
    /// let doc = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let binding = doc.get("binding").ok_or("not read")?;
    /// let hrefs = binding.hrefs(ReferenceEnd::ParticipantPackage);
    /// assert_eq!((hrefs[0].document(), hrefs[0].fragment()), ("braking.sacm.xmi", "braking_api"));
    /// assert!(binding.participant_packages().is_empty()); // no id of this document
    /// assert!(validation::validate(&doc).is_empty());     // two participants, never dangling
    /// let files: BTreeSet<&str> = doc.elements()
    ///     .flat_map(|e| ReferenceEnd::ALL.iter().flat_map(move |&end| e.hrefs(end)))
    ///     .map(|href| href.document()).collect();
    /// assert_eq!(files.len(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn hrefs(self, end: ReferenceEnd) -> &'a [Href] {
        hrefs_of(&self.node().hrefs, end)
    }

    /// The element's proxies, in document order: each a child in one of its
    /// composite roles that the file holds in another document and names by
    /// an `href` (XMI 2.5.1 §7.10.1; SACM23-XMI-029, ADR 0040 amendment 1).
    ///
    /// A proxy is never resolved, and it is no element of this document: it
    /// is not among [`children`](Self::children), it has no id, and no
    /// validation rule counts it. Both saves write it back, after the
    /// element's children. A copy keeps the proxies, a move takes them
    /// along, and
    /// [`PackageDeletePolicy::RejectIfNonEmpty`](crate::edit::PackageDeletePolicy::RejectIfNonEmpty)
    /// counts each as contained. Only [`xmi::read`](crate::xmi::read) gives
    /// an element proxies, and a copy copies them; no edit adds a proxy to
    /// an element or removes one from it yet.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
    ///   <name content="Case"/>
    ///   <argumentPackage href="arguments.sacm.xmi#args"/>
    ///   <argumentPackage xmi:type="sacm:ArgumentPackageInterface" href="braking.sacm.xmi#api"/>
    /// </sacm:AssuranceCasePackage>"#;
    /// let doc = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let case = doc.get("case").ok_or("not read")?;
    /// let files: Vec<&str> = case.proxies().iter().map(|p| p.href().document()).collect();
    /// assert_eq!(files, ["arguments.sacm.xmi", "braking.sacm.xmi"]);
    /// // A proxy's type attribute is held as its href's type.
    /// let typed = &case.proxies()[1];
    /// assert_eq!(typed.href().kind(), Some(ElementKind::ArgumentPackageInterface));
    /// assert_eq!(typed.href().as_str(), "sacm:ArgumentPackageInterface braking.sacm.xmi#api");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn proxies(self) -> &'a [Proxy] {
        &self.node().proxies
    }

    /// The ids the element's `ExpressionLangStrings` (§8.4) name: its name's, then each
    /// value's of its `content`, `key` and `location`, in order. An `ExpressionLangString`
    /// with no `expression` adds none. It is `references(ReferenceEnd::Expression)`.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, CreateTerm, CreateTerminologyPackage};
    /// use sacm::model::{Document, ElementId, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateTerminologyPackage::root(LangString::new("Terms")).with_id("terms"))?;
    /// edit::apply(&mut doc, CreateTerm::new("terms", LangString::new("Blade"), "blade").with_id("blade"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// let name = LangString::default().with_lang("en").with_expression("blade");
    /// edit::apply(&mut doc, CreateClaim::new("p", name).with_id("c1"))?;
    ///
    /// let c1 = doc.get("c1").expect("just created");
    /// assert_eq!(c1.expressions(), [ElementId::from("blade")]);
    /// assert_eq!(c1.references(ReferenceEnd::Expression), c1.expressions());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn expressions(self) -> &'a [ElementId] {
        self.references(ReferenceEnd::Expression)
    }

    /// The element's descriptions (SACM 2.3 §8.6, §8.9): its children in the
    /// `description` role, in document order.
    ///
    /// They are among [`children`](Self::children), which keeps every child
    /// in one document order across roles. Several are kept (D-DESC-MULT). A
    /// `UtilityElement` has none.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateDescription, CreateNote};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let text = |content: &str| LangString::new(content).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(text("Arguments")).with_id("arguments"))?;
    /// edit::apply(&mut doc, CreateDescription::new("arguments", text("Why")).with_id("d1"))?;
    /// edit::apply(&mut doc, CreateNote::new("arguments", text("Reviewed")).with_id("n1"))?;
    /// edit::apply(&mut doc, CreateDescription::new("arguments", text("How")).with_id("d2"))?;
    ///
    /// let arguments = doc.get("arguments").expect("just created");
    /// let ids: Vec<&str> = arguments.descriptions().map(|d| d.id().as_str()).collect();
    /// assert_eq!(ids, ["d1", "d2"]);
    /// assert_eq!(arguments.notes().count(), 1);
    /// assert_eq!(arguments.children().count(), 3);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    pub fn descriptions(self) -> impl Iterator<Item = SacmElement<'a>> {
        self.children_in("description")
    }

    /// The element's implementation constraints (SACM 2.3 §8.6, §8.8): its
    /// children in the `implementationConstraint` role, in document order.
    /// See [`descriptions`](Self::descriptions).
    pub fn implementation_constraints(self) -> impl Iterator<Item = SacmElement<'a>> {
        self.children_in("implementationConstraint")
    }

    /// The element's notes (SACM 2.3 §8.6, §8.11): its children in the `note`
    /// role, in document order. See [`descriptions`](Self::descriptions).
    pub fn notes(self) -> impl Iterator<Item = SacmElement<'a>> {
        self.children_in("note")
    }

    /// The element's tagged values (SACM 2.3 §8.6, §8.12): its children in
    /// the `taggedValue` role, in document order, those with equal keys
    /// included. See [`descriptions`](Self::descriptions).
    ///
    /// A tagged value is a key/value pair: its key is its
    /// [`key`](Self::key), and its value is its [`content`](Self::content)
    /// (§8.7). In XMI, a `<value>` element is one `LangString` inside a `key`
    /// or `content` (§8.5), not the tagged value's value.
    pub fn tagged_values(self) -> impl Iterator<Item = SacmElement<'a>> {
        self.children_in("taggedValue")
    }

    /// The `content` of a `UtilityElement` (SACM 2.3 §8.7), or `None` when
    /// it has none or the element is not a `UtilityElement`.
    ///
    /// Two things in SACM are called `content`. This one, the role of a
    /// `UtilityElement` (§8.7), is a whole [`MultiLangString`]: the same
    /// text in one or more languages. Each of its values is a [`LangString`],
    /// whose own [`content`](LangString::content) (§8.3
    /// `content:String[0..1]`) is the text itself.
    ///
    /// A `Description`, `ImplementationConstraint`, `Note` or `TaggedValue`
    /// holds its text as a [`MultiLangString`]. No `content` is `None`, and
    /// `<content/>` is an empty `MultiLangString`: the two are kept apart and
    /// both are written back ([ADR 0025]). The values are kept in order, with
    /// duplicates; [`validation::validate`](crate::validation::validate)
    /// reports an empty one and a repeated language (SACM-MULT-001).
    ///
    /// [ADR 0025]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0025-utility-elements-and-gid.md
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateNote};
    /// use sacm::model::{Document, LangString, MultiLangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// let content: MultiLangString = [
    ///     LangString::new("Reviewed").with_lang("en"),
    ///     LangString::new("Granskad").with_lang("sv"),
    /// ]
    /// .into_iter()
    /// .collect();
    /// edit::apply(&mut doc, CreateNote::new("p", content.clone()).with_id("n1"))?;
    ///
    /// let note = doc.get("n1").expect("just created");
    /// assert_eq!(note.content(), Some(&content));
    /// // The English text is the content of the value whose lang is "en".
    /// let english = note
    ///     .content()
    ///     .and_then(|content| content.values().iter().find(|value| value.lang() == Some("en")))
    ///     .and_then(LangString::content);
    /// assert_eq!(english, Some("Reviewed"));
    /// // A package is not a UtilityElement, so it has no content.
    /// assert_eq!(doc.get("p").and_then(|p| p.content()), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn content(self) -> Option<&'a MultiLangString> {
        self.node().content.as_ref().map(Text::value)
    }

    /// The `xmi:id` of the element's `content` element, if it was read with
    /// one, or `None`.
    ///
    /// A `MultiLangString` is a class, so its element may carry an id (XMI
    /// 2.5.1 §7.8.3). Like a name's id ([`name_id`](Self::name_id)), it is
    /// kept with the owner, shares the document's id space, identifies no
    /// element, and is written back as the first attribute of `content`
    /// (ADR 0024, ADR 0025). Only [`xmi::read`](crate::xmi::read) gives
    /// one; a `content` replaced with [`SetContent`](crate::edit::SetContent)
    /// keeps it, and each value keeps the id of the value at its position
    /// (ADR 0029).
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments"/>
    ///   <note xmi:id="n1">
    ///     <content xmi:id="n1_content">
    ///       <value xmi:id="n1_en" lang="en" content="Reviewed"/>
    ///       <value lang="sv" content="Granskad"/>
    ///     </content>
    ///   </note>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let note = document.get("n1").expect("the note was read");
    /// assert_eq!(note.content_id().map(|id| id.as_str()), Some("n1_content"));
    /// let value_ids: Vec<Option<&str>> =
    ///     note.content_value_ids().iter().map(|id| id.as_ref().map(|id| id.as_str())).collect();
    /// assert_eq!(value_ids, [Some("n1_en"), None]);
    /// assert!(document.get("n1_content").is_none());
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn content_id(self) -> Option<&'a ElementId> {
        self.node().content.as_ref().and_then(Text::id)
    }

    /// The `xmi:id` of each `value` of the element's `content`, by position:
    /// one entry per value of [`content`](Self::content), `None` for a value
    /// read without one, and empty when there is no content. See
    /// [`content_id`](Self::content_id).
    #[must_use]
    pub fn content_value_ids(self) -> &'a [Option<ElementId>] {
        self.node().content.as_ref().map_or(&[], Text::value_ids)
    }

    /// The `key` of a `TaggedValue` (SACM 2.3 §8.12), or `None` when it has
    /// none or the element is not a `TaggedValue`.
    ///
    /// It follows the rules of [`content`](Self::content): absent and empty
    /// are kept apart, and the values are kept in order, with duplicates.
    /// SACM requires a key (`key:MultiLangString[1]`), so
    /// [`validation::validate`](crate::validation::validate) reports a
    /// `TaggedValue` without one (SACM-MULT-001).
    ///
    /// The tagged value's value is its [`content`](Self::content) (§8.7). In
    /// XMI, a `<value>` element is one `LangString` inside `key` or `content`
    /// (§8.5), not the tagged value's value.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateTaggedValue};
    /// use sacm::model::{Document, LangString, MultiLangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// let tagged = CreateTaggedValue::new("p", LangString::new("ASIL"), LangString::new("D"));
    /// edit::apply(&mut doc, tagged.with_id("t1"))?;
    ///
    /// let t1 = doc.get("t1").expect("just created");
    /// assert_eq!(t1.key(), Some(&MultiLangString::from(LangString::new("ASIL"))));
    /// assert_eq!(t1.content(), Some(&MultiLangString::from(LangString::new("D"))));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn key(self) -> Option<&'a MultiLangString> {
        self.node().key.as_ref().map(Text::value)
    }

    /// The `xmi:id` of the element's `key` element, if it was read with one,
    /// or `None`. See [`content_id`](Self::content_id).
    #[must_use]
    pub fn key_id(self) -> Option<&'a ElementId> {
        self.node().key.as_ref().and_then(Text::id)
    }

    /// The `xmi:id` of each `value` of the element's `key`, by position: one
    /// entry per value of [`key`](Self::key), and empty when there is no
    /// key. See [`content_id`](Self::content_id).
    #[must_use]
    pub fn key_value_ids(self) -> &'a [Option<ElementId>] {
        self.node().key.as_ref().map_or(&[], Text::value_ids)
    }

    /// The `version` of an `Artifact` (SACM 2.3 §12.7 `version: String[0..1]`,
    /// "the version of the artifact"), exactly as written, or `None` when it
    /// has none or the element is not an `Artifact`.
    ///
    /// Nothing is trimmed or normalised: `""` and `" v2 "` are values. An
    /// `xmi:version` on the document element is the XMI version, never this
    /// one.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifact, CreateArtifactPackage, SetVersion};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// let report = CreateArtifact::new("evidence", en("Brake test report")).with_id("report");
    /// edit::apply(&mut doc, report.with_version("1.2"))?;
    /// assert_eq!(doc.get("report").and_then(|report| report.version()), Some("1.2"));
    ///
    /// edit::apply(&mut doc, SetVersion::remove("report"))?;
    /// assert_eq!(doc.get("report").and_then(|report| report.version()), None);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn version(self) -> Option<&'a str> {
        self.string(StringAttribute::Version)
    }

    /// The `date` of an `Artifact` (§12.7, "the date on which the artifact
    /// was created") or an `Event` (§12.9, "the date on which the Event
    /// occurred"), exactly as written, or `None` when it has none or the
    /// element is neither.
    ///
    /// SACM 2.3 gives `date` no lexical form, and the XML Schema its XMI
    /// rules produce types it `xsd:string`, so any string is held as written
    /// and never diagnosed: `2024-03-01`, `2024-03-01T09:30:00Z`,
    /// `spring 2024` and `""` alike (D-DATE). Parsing it is up to the
    /// application.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArtifactPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="evidence">
    ///   <name content="Evidence"/>
    ///   <artifactElement xmi:type="sacm:Event" xmi:id="released" date="2024-03-05T14:00:00+01:00">
    ///     <name content="Report released"/>
    ///   </artifactElement>
    ///   <artifactElement xmi:type="sacm:Event" xmi:id="reviewed" date="spring 2024">
    ///     <name content="Report reviewed"/>
    ///   </artifactElement>
    /// </sacm:ArtifactPackage>"#;
    /// let output = xmi::read(input, &ReadOptions::default())?;
    /// assert!(output.diagnostics().is_empty());
    /// let document = output.into_document();
    /// let date = |id: &str| document.get(id).and_then(|event| event.date());
    /// assert_eq!(date("released"), Some("2024-03-05T14:00:00+01:00"));
    /// assert_eq!(date("reviewed"), Some("spring 2024"));
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn date(self) -> Option<&'a str> {
        self.string(StringAttribute::Date)
    }

    /// The `startTime` of an `Activity` (§12.11, "time when the activity
    /// started"), held as [`date`](Self::date) is, or `None` when it has none
    /// or the element is not an `Activity`.
    #[must_use]
    pub fn start_time(self) -> Option<&'a str> {
        self.string(StringAttribute::StartTime)
    }

    /// The `endTime` of an `Activity` (§12.11, "time when the activity
    /// ended"), held as [`date`](Self::date) is, or `None` when it has none
    /// or the element is not an `Activity`. Nothing checks it against the
    /// [`start_time`](Self::start_time): SACM 2.3 states no order.
    #[must_use]
    pub fn end_time(self) -> Option<&'a str> {
        self.string(StringAttribute::EndTime)
    }

    /// The `value` of a Term or an Expression (SACM 2.3 §10.9
    /// `value:String[1]`, "the value of the expression"), exactly as written,
    /// or `None` when it has none, which
    /// [`validate`](crate::validation::validate) reports, or the element is
    /// no `ExpressionElement`.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:TerminologyPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="terms">
    ///   <name content="Terms"/>
    ///   <terminologyElement xmi:type="sacm:Term" xmi:id="blade" value="blade"
    ///       externalReference="https://example.org/terms/blade">
    ///     <name content="Blade"/>
    ///   </terminologyElement>
    /// </sacm:TerminologyPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let blade = document.get("blade").expect("the Term was read");
    /// assert_eq!(blade.value(), Some("blade"));
    /// assert_eq!(blade.external_reference(), Some("https://example.org/terms/blade"));
    /// # Ok::<(), sacm::xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn value(self) -> Option<&'a str> {
        self.string(StringAttribute::Value)
    }

    /// A Term's `externalReference` (SACM 2.3 §10.11
    /// `externalReference:String[0..1]`, "an external reference (e.g., URI) to
    /// the object referred to by the Term"), exactly as written, or `None`
    /// when it has none or the element is no Term. See
    /// [`value`](Self::value) for an example.
    #[must_use]
    pub fn external_reference(self) -> Option<&'a str> {
        self.string(StringAttribute::ExternalReference)
    }

    /// The `location` of a `Resource` (§12.10 `location:Base::MultiLangString`,
    /// "the path or URL specifying the location of the Resource, can be in
    /// multiple languages"), or `None` when it has none or the element is not
    /// a `Resource`.
    ///
    /// It is held as a [`content`](Self::content) is: no `location` is
    /// `None` and `<location/>` an empty `MultiLangString`, and the values
    /// are kept in order, with duplicates.
    /// [`validation::validate`](crate::validation::validate) checks it as
    /// every `MultiLangString` (SACM-MULT-001); the text gives `location` no
    /// multiplicity, so a Resource without one gives no finding.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateResource};
    /// use sacm::model::{Document, LangString, MultiLangString};
    ///
    /// let mut doc = Document::new();
    /// let en = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
    /// let pdf = CreateResource::new("evidence", en("Report PDF")).with_id("report_pdf");
    /// edit::apply(&mut doc, pdf.with_location(en("https://example.org/brake-1.2.pdf")))?;
    ///
    /// let location = doc.get("report_pdf").and_then(|resource| resource.location());
    /// assert_eq!(location, Some(&MultiLangString::from(en("https://example.org/brake-1.2.pdf"))));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn location(self) -> Option<&'a MultiLangString> {
        self.node().location.as_ref().map(Text::value)
    }

    /// The `xmi:id` of the element's `location` element, if it was read with
    /// one, or `None`. See [`content_id`](Self::content_id).
    #[must_use]
    pub fn location_id(self) -> Option<&'a ElementId> {
        self.node().location.as_ref().and_then(Text::id)
    }

    /// The `xmi:id` of each `value` of the element's `location`, by
    /// position: one entry per value of [`location`](Self::location), and
    /// empty when there is no location. See
    /// [`content_value_ids`](Self::content_value_ids).
    #[must_use]
    pub fn location_value_ids(self) -> &'a [Option<ElementId>] {
        self.node().location.as_ref().map_or(&[], Text::value_ids)
    }

    /// The value of the string-valued `attribute`, exactly as held, or
    /// `None` when the element has none.
    pub(crate) fn string(self, attribute: StringAttribute) -> Option<&'a str> {
        string_of(&self.node().strings, attribute)
    }

    /// The element's properties (SACM 2.3 §12.6 `property:Property[0..*]`):
    /// its children in the `property` role, in document order.
    ///
    /// Every `ArtifactAsset` has the role, such as a `Property`, a `Technique`,
    /// an `Artifact` or a `Resource`; for any other kind it is empty. A `Property` held
    /// directly in an `ArtifactPackage`'s `artifactElement` is a child of the
    /// package, not a property of it. The properties are among
    /// [`children`](Self::children).
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateProperty, CreateTechnique};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(name("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateTechnique::new("evidence", name("Fault tree analysis")).with_id("fta"))?;
    /// edit::apply(&mut doc, CreateProperty::new("fta", name("Qualified tool")).with_id("q"))?;
    /// edit::apply(&mut doc, CreateProperty::new("evidence", name("Calibrated")))?;
    ///
    /// let fta = doc.get("fta").expect("just created");
    /// assert_eq!(fta.properties().map(|p| p.id().as_str()).collect::<Vec<_>>(), ["q"]);
    /// // A Property directly in the package is no property of the package.
    /// let evidence = doc.get("evidence").expect("just created");
    /// assert_eq!(evidence.properties().count(), 0);
    /// assert_eq!(evidence.children().count(), 2);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    pub fn properties(self) -> impl Iterator<Item = SacmElement<'a>> {
        self.children_in("property")
    }

    /// The element's children held in the role named `role`, in document
    /// order.
    fn children_in(self, role: &'static str) -> impl Iterator<Item = SacmElement<'a>> {
        self.children()
            .filter(move |child| child.role().is_some_and(|held| held.name == role))
    }

    /// The element's `content`, `key` or `location`, with its ids.
    pub(crate) fn text(self, role: TextRole) -> Option<&'a Text> {
        self.node().text(role)
    }

    /// The element's `assertionDeclaration` (SACM 2.3 §11.10): whether it is
    /// asserted, assumed, needs support and so on, or `None` when its kind is
    /// not an Assertion.
    ///
    /// Every Assertion (a `Claim` or one of the five asserted relationships;
    /// see [`AbstractClass::Assertion`]) has exactly one declaration:
    /// [`AssertionDeclaration::Asserted`] unless the document holds another.
    /// There is no absent state (§11.10 `[1] = asserted`), so `None` never
    /// means "missing".
    ///
    /// ```
    /// use sacm::model::{AssertionDeclaration, Document, ElementId, ElementKind};
    ///
    /// // Built with the unstable test-support feature (not shown on docs.rs; see the crate's Features), which inserts an element without a name.
    /// let mut doc = Document::new();
    /// let id = ElementId::from;
    /// doc.insert(None, ElementKind::ArgumentPackage, id("arguments"), None).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::Claim, id("c1"), None).unwrap();
    /// doc.insert(Some(&id("arguments")), ElementKind::ArgumentReasoning, id("r1"), None).unwrap();
    ///
    /// let c1 = doc.get("c1").expect("the document has it");
    /// assert_eq!(c1.assertion_declaration(), Some(AssertionDeclaration::Asserted));
    ///
    /// doc.set_assertion_declaration(&id("c1"), AssertionDeclaration::NeedsSupport).unwrap();
    /// let c1 = doc.get("c1").expect("the document has it");
    /// assert_eq!(c1.assertion_declaration(), Some(AssertionDeclaration::NeedsSupport));
    ///
    /// // An ArgumentReasoning is not an Assertion, so it has no declaration.
    /// let r1 = doc.get("r1").expect("the document has it");
    /// assert_eq!(r1.assertion_declaration(), None);
    /// ```
    #[must_use]
    pub fn assertion_declaration(self) -> Option<AssertionDeclaration> {
        self.node().assertion_declaration
    }

    /// The element that contains this one, or `None` for a root.
    #[must_use]
    pub fn parent(self) -> Option<SacmElement<'a>> {
        self.node().parent.map(|parent| self.document.view(parent))
    }

    /// The role of the parent's class that holds this element, or `None` for
    /// a root. Stored when the element is added, so it is `Some` exactly when
    /// [`parent`](Self::parent) is.
    pub(crate) fn role(self) -> Option<Role> {
        self.node().role
    }

    /// The element's features, as [`crate::compare`] compares them.
    ///
    /// The one place `compare` reads an element. `Node` is destructured with
    /// no `..`, so a field added to the model fails to compile here until
    /// `compare` handles it (ADR 0020).
    pub(crate) fn compared(self) -> Compared<'a> {
        let Node {
            // Matched on: elements are paired by id across the document.
            id,
            // Compared.
            kind,
            // Compared.
            gid,
            // Compared.
            name,
            // Compared.
            name_id,
            // Compared: the value, its id and its values' ids.
            content,
            // Compared: the value, its id and its values' ids.
            key,
            // Compared: the value, its id and its values' ids.
            location,
            // Compared, attribute by attribute.
            strings,
            // Compared.
            assertion_declaration,
            // Compared.
            is_citation,
            // Compared.
            is_abstract,
            // Compared.
            is_counter,
            // Compared, end by end.
            references,
            // Compared, end by end (ADR 0040 D8).
            hrefs,
            // Compared, in order (ADR 0040 amendment 1, A6).
            proxies,
            // Compared, as the parent's id.
            parent,
            // Derived from the parent's children (or the roots), which are
            // compared as the child order.
            slot: _,
            // Derived from the parent's kind and the element's kind
            // (`containment::role`), both compared.
            role: _,
            // Compared as the child order, through `children()`.
            children: _,
            // Compared exactly and in order (ADR 0033).
            preserved,
            // Compared exactly and in order, with the name's and the texts'
            // (ADR 0042).
            metadata,
            // Compared, with the element's.
            name_metadata,
        } = self.node();
        Compared {
            id,
            kind: *kind,
            parent: parent.map(|parent| self.document.view(parent).id()),
            gid: gid.as_deref(),
            name: name.as_ref(),
            name_id: name_id.as_ref(),
            content: content.as_ref(),
            key: key.as_ref(),
            location: location.as_ref(),
            strings,
            assertion_declaration: *assertion_declaration,
            is_citation: *is_citation,
            is_abstract: *is_abstract,
            is_counter: *is_counter,
            references,
            hrefs,
            proxies,
            preserved: preserved.as_deref(),
            metadata: metadata.as_deref(),
            name_metadata: name_metadata.as_deref(),
        }
    }

    /// The elements this one contains directly, in document order.
    ///
    /// They are held in every role of the element's class, so a
    /// `ModelElement`'s children include its descriptions, implementation
    /// constraints, notes and tagged values, among its other children. To
    /// get one role, use [`descriptions`](Self::descriptions),
    /// [`implementation_constraints`](Self::implementation_constraints),
    /// [`notes`](Self::notes) or [`tagged_values`](Self::tagged_values); to
    /// skip them, filter by kind, for example on
    /// `!child.kind().is_a(AbstractClass::UtilityElement)`.
    pub fn children(self) -> impl Iterator<Item = SacmElement<'a>> {
        let document = self.document;
        self.node()
            .children
            .iter()
            .map(move |&child| document.view(child))
    }

    /// Whether the element holds preserved content: something it was read
    /// with from XMI and kept exactly as written instead of read, such as a
    /// vendor attribute or child element that is not SACM 2.3 where it stands
    /// (LIB-COMPAT-003, ADR 0033), or, in tolerant mode, a construct a strict
    /// read refuses, such as a value that is no literal (ADR 0035), or the
    /// type of a class the application declared a subclass of the element's
    /// kind ([`subclass`](Self::subclass), LIB-COMPAT-017). XMI metadata,
    /// such as `xmi:uuid`, is never preserved content (SACM23-XMI-019, ADR
    /// 0042).
    ///
    /// [`xmi::read`](crate::xmi::read) keeps it, with one diagnostic per
    /// construct (SACM-XMI-003 for content that is not SACM 2.3 where it
    /// stands; in tolerant mode, the Warning of the strict refusal), rather
    /// than drop it. A tolerant
    /// [`xmi::write`](crate::xmi::write) writes it back where it stood, and a
    /// strict one refuses the document
    /// ([`WriteError::PreservedContent`](crate::xmi::WriteError::PreservedContent)).
    /// An element built with [`edit::apply`](crate::edit::apply) holds none.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:vendor"
    ///     xmi:id="p" v:colour="red">
    ///   <name content="P"/>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let p = document.get("p").expect("the root was read");
    /// assert!(p.has_preserved_content());
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn has_preserved_content(self) -> bool {
        self.node().preserved.is_some()
    }

    /// The class outside SACM 2.3 that this element was read as an instance
    /// of, if a tolerant read read it as a declared subclass of its kind
    /// ([`ReadOptions::with_subclass`](crate::xmi::ReadOptions::with_subclass),
    /// LIB-COMPAT-017). It is preserved content: a tolerant write writes it
    /// as the element's `xmi:type`, and a strict write refuses the element.
    /// Only [`xmi::read`](crate::xmi::read) gives one.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    /// use sacm::xmi::{self, Mode, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:p="urn:example:profile"
    ///     xmi:id="arguments">
    ///   <argumentElement xmi:type="p:Proposition" xmi:id="c1"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2"/>
    /// </sacm:ArgumentPackage>"#;
    /// let options = ReadOptions::default()
    ///     .with_mode(Mode::Tolerant)
    ///     .with_subclass("urn:example:profile", "Proposition", ElementKind::Claim);
    /// let document = xmi::read(input, &options)?.into_document();
    /// let c1 = document.get("c1").expect("c1 is read");
    /// assert_eq!(c1.subclass().map(|class| class.name()), Some("Proposition"));
    /// assert!(c1.has_preserved_content());
    /// assert_eq!(document.get("c2").and_then(|c2| c2.subclass()), None);
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn subclass(self) -> Option<Subclass<'a>> {
        let class = self.preserved()?.subclass.as_ref()?;
        Some(Subclass::new(class.namespace.as_deref()?, &class.local))
    }

    /// The element's preserved content, if it holds any (ADR 0033).
    pub(crate) fn preserved(self) -> Option<&'a Preserved> {
        self.node().preserved.as_deref()
    }

    /// The element's `xmi:uuid` (XMI 2.5.1 §7.6.1), exactly as read, or
    /// `None`: XMI metadata, which both saves write back and a copy drops
    /// (SACM23-XMI-019, ADR 0042).
    ///
    /// Only [`xmi::read`](crate::xmi::read) gives one; an element built with
    /// [`edit::apply`](crate::edit::apply) has none.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
    ///     xmi:id="p" xmi:uuid="urn:uuid:7f3c" xmi:label="Arguments">
    ///   <name content="P"/>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let p = document.get("p").expect("the root was read");
    /// assert_eq!(p.uuid(), Some("urn:uuid:7f3c"));
    /// assert_eq!(p.label(), Some("Arguments"));
    /// // Metadata is no preserved content: a strict save writes it back.
    /// assert!(!p.has_preserved_content());
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn uuid(self) -> Option<&'a str> {
        self.metadata()
            .and_then(|metadata| metadata.xmi_attribute("uuid"))
    }

    /// The element's `xmi:label`, exactly as read, or `None`. "The value of
    /// the label attribute is ignored on import" (XMI 2.5.1 §7.6.1): the
    /// library never uses it, and both saves write it back (SACM23-XMI-019,
    /// ADR 0042).
    #[must_use]
    pub fn label(self) -> Option<&'a str> {
        self.metadata()
            .and_then(|metadata| metadata.xmi_attribute("label"))
    }

    /// The element's own XMI metadata, if it holds any (ADR 0042).
    pub(crate) fn metadata(self) -> Option<&'a Preserved> {
        self.node().metadata.as_deref()
    }

    /// The XMI metadata of the element's name, if it holds any (ADR 0042).
    pub(crate) fn name_metadata(self) -> Option<&'a Preserved> {
        self.node().name_metadata.as_deref()
    }

    /// Every XMI metadata the element holds: its own, its name's, then that
    /// of its texts and their values (ADR 0042).
    pub(crate) fn metadata_parts(self) -> impl Iterator<Item = &'a Preserved> {
        self.node().metadata_parts()
    }

    /// The element's node in the arena.
    fn node(self) -> &'a Node {
        // Views are only made from handles the document handed out, and
        // nodes are never removed while the document is borrowed.
        &self.document.nodes[self.node]
    }
}

/// The features of one element that [`crate::compare`] compares, from
/// [`SacmElement::compared`]. The child order is read through
/// [`SacmElement::children`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Compared<'a> {
    /// The element's id.
    pub(crate) id: &'a ElementId,
    /// The element's kind.
    pub(crate) kind: ElementKind,
    /// The id of the containing element; `None` for a root.
    pub(crate) parent: Option<&'a ElementId>,
    /// The element's `gid`; `None` when absent.
    pub(crate) gid: Option<&'a str>,
    /// The element's name; `None` when missing.
    pub(crate) name: Option<&'a LangString>,
    /// The `xmi:id` of the element's name; `None` when it has none.
    pub(crate) name_id: Option<&'a ElementId>,
    /// The element's `content`, with its ids; `None` when absent.
    pub(crate) content: Option<&'a Text>,
    /// The element's `key`, with its ids; `None` when absent.
    pub(crate) key: Option<&'a Text>,
    /// The element's `location`, with its ids; `None` when absent.
    pub(crate) location: Option<&'a Text>,
    /// The string-valued attributes the element holds: only the ones given.
    strings: &'a [(StringAttribute, String)],
    /// The element's `assertionDeclaration`; `None` when not an Assertion.
    pub(crate) assertion_declaration: Option<AssertionDeclaration>,
    /// The element's `isCitation`.
    is_citation: bool,
    /// The element's `isAbstract`.
    is_abstract: bool,
    /// The element's `isCounter`.
    is_counter: bool,
    /// The ids each reference end holds: only the ends that hold one.
    references: &'a [(ReferenceEnd, Vec<ElementId>)],
    /// The `href`s each reference end holds: only the ends that hold one.
    hrefs: &'a [(ReferenceEnd, Vec<Href>)],
    /// The element's proxies, in order.
    pub(crate) proxies: &'a [Proxy],
    /// The element's preserved content; `None` when it holds none.
    pub(crate) preserved: Option<&'a Preserved>,
    /// The element's XMI metadata; `None` when it holds none.
    metadata: Option<&'a Preserved>,
    /// The XMI metadata of the element's name; `None` when it holds none.
    name_metadata: Option<&'a Preserved>,
}

impl<'a> Compared<'a> {
    /// Whether its XMI metadata differs from that of `other`: the element's,
    /// its name's, or that of a `content`, `key` or `location` or a value of
    /// one, exactly and in order; a part one side lacks holds none there
    /// (ADR 0042).
    pub(crate) fn metadata_differs(&self, other: &Compared<'_>) -> bool {
        let none: &[Option<Box<Preserved>>] = &[];
        let ours = [self.content, self.key, self.location];
        let theirs = [other.content, other.key, other.location];
        self.metadata != other.metadata
            || self.name_metadata != other.name_metadata
            || ours.into_iter().zip(theirs).any(|(ours, theirs)| {
                let ours_values = ours.map_or(none, Text::value_metadata);
                let theirs_values = theirs.map_or(none, Text::value_metadata);
                ours.and_then(Text::role_metadata) != theirs.and_then(Text::role_metadata)
                    || (0..ours_values.len().max(theirs_values.len())).any(|position| {
                        ours_values.get(position).and_then(Option::as_deref)
                            != theirs_values.get(position).and_then(Option::as_deref)
                    })
            })
    }

    /// The value of the flag `flag`.
    pub(crate) fn flag(&self, flag: Flag) -> bool {
        match flag {
            Flag::IsCitation => self.is_citation,
            Flag::IsAbstract => self.is_abstract,
            Flag::IsCounter => self.is_counter,
        }
    }

    /// The ids the reference end `end` holds: empty when it holds none.
    pub(crate) fn references(&self, end: ReferenceEnd) -> &'a [ElementId] {
        targets_of(self.references, end)
    }

    /// The `href`s the reference end `end` holds: empty when it holds none.
    pub(crate) fn hrefs(&self, end: ReferenceEnd) -> &'a [Href] {
        hrefs_of(self.hrefs, end)
    }

    /// The value of the string-valued `attribute`, or `None` when absent.
    pub(crate) fn string(&self, attribute: StringAttribute) -> Option<&'a str> {
        string_of(self.strings, attribute)
    }
}

/// The value `attribute` has in `strings`, a node's sparse list of
/// string-valued attributes: `None` when absent.
fn string_of(strings: &[(StringAttribute, String)], attribute: StringAttribute) -> Option<&str> {
    strings
        .iter()
        .find(|(held, _)| *held == attribute)
        .map(|(_, value)| value.as_str())
}

/// The `href`s `end` holds in `hrefs`, a node's sparse list of ends: empty
/// when it holds none.
fn hrefs_of(hrefs: &[(ReferenceEnd, Vec<Href>)], end: ReferenceEnd) -> &[Href] {
    hrefs
        .iter()
        .find(|(held, _)| *held == end)
        .map_or(&[], |(_, values)| values.as_slice())
}

/// The targets `end` holds in `references`, a node's sparse list of ends:
/// empty when it holds none.
fn targets_of(references: &[(ReferenceEnd, Vec<ElementId>)], end: ReferenceEnd) -> &[ElementId] {
    references
        .iter()
        .find(|(held, _)| *held == end)
        .map_or(&[], |(_, targets)| targets.as_slice())
}

/// A role that holds a `MultiLangString` (ADR 0025, ADR 0034 amendment 1):
/// `content` on every `UtilityElement` (SACM 2.3 §8.7), `key` on a
/// `TaggedValue` (§8.12), and `location` on a `Resource` (§12.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextRole {
    /// `content`.
    Content,
    /// `key`.
    Key,
    /// `location`.
    Location,
}

impl TextRole {
    /// Every role, in the order they are written: superclass first (XMI
    /// 2.5.1 Annex B.5.2).
    pub(crate) const ALL: [Self; 3] = [Self::Content, Self::Key, Self::Location];

    /// The role's name, as written in XMI.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Key => "key",
            Self::Location => "location",
        }
    }

    /// Whether an element of `kind` has this role.
    pub(crate) const fn is_held_by(self, kind: ElementKind) -> bool {
        match self {
            Self::Content => kind.is_a(AbstractClass::UtilityElement),
            Self::Key => kind.is_kind_of(ElementKind::TaggedValue),
            Self::Location => kind.is_kind_of(ElementKind::Resource),
        }
    }
}

/// An attribute that holds a string exactly as written (D-DATE; ADR 0034
/// amendments 1 and 3): an Artifact's `version` (SACM 2.3 §12.7), the
/// `date`-typed `date` of an Artifact or an Event (§12.7, §12.9) and
/// `startTime` and `endTime` of an Activity (§12.11), which the XMI rules
/// type `xsd:string`, the `value` of an `ExpressionElement` (§10.9) and a
/// Term's `externalReference` (§10.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StringAttribute {
    /// `version`.
    Version,
    /// `date`.
    Date,
    /// `startTime`.
    StartTime,
    /// `endTime`.
    EndTime,
    /// `value`.
    Value,
    /// `externalReference`.
    ExternalReference,
}

impl StringAttribute {
    /// Every attribute, in the order they are written.
    pub(crate) const ALL: [Self; 6] = [
        Self::Version,
        Self::Date,
        Self::StartTime,
        Self::EndTime,
        Self::Value,
        Self::ExternalReference,
    ];

    /// The attribute's name, as written in XMI.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Version => "version",
            Self::Date => "date",
            Self::StartTime => "startTime",
            Self::EndTime => "endTime",
            Self::Value => "value",
            Self::ExternalReference => "externalReference",
        }
    }

    /// Whether an element of `kind` has this attribute.
    pub(crate) const fn is_held_by(self, kind: ElementKind) -> bool {
        match self {
            Self::Version => kind.is_kind_of(ElementKind::Artifact),
            Self::Date => {
                kind.is_kind_of(ElementKind::Artifact) || kind.is_kind_of(ElementKind::Event)
            }
            Self::StartTime | Self::EndTime => kind.is_kind_of(ElementKind::Activity),
            Self::Value => kind.is_a(AbstractClass::ExpressionElement),
            Self::ExternalReference => kind.is_kind_of(ElementKind::Term),
        }
    }
}

/// A `MultiLangString` held in a [`TextRole`], with the `xmi:id`s XMI lets
/// its elements carry (XMI 2.5.1 §7.8.3; ADR 0025): one for the role
/// element, and one per `value`, by position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Text {
    value: MultiLangString,
    /// The `xmi:id` of the role element.
    id: Option<ElementId>,
    /// The `xmi:id` of each value, in order: as many as `value` has values.
    value_ids: Vec<Option<ElementId>>,
    /// The XMI metadata of the role element (SACM23-XMI-019, ADR 0042), its
    /// children placed by value index; `None` when it holds none.
    metadata: Option<Box<Preserved>>,
    /// The XMI metadata of each value's element, in order: as many as
    /// `value` has values.
    value_metadata: Vec<Option<Box<Preserved>>>,
}

impl Text {
    /// `value`, with no ids and no metadata.
    pub(crate) fn new(value: MultiLangString) -> Self {
        let value_ids = vec![None; value.len()];
        let value_metadata = vec![None; value.len()];
        Self {
            value,
            id: None,
            value_ids,
            metadata: None,
            value_metadata,
        }
    }

    /// `value`, with the ids and the XMI metadata of `held` kept by position
    /// (ADR 0029, ADR 0042 D6): the role element's, and those of each value
    /// of `held` at a position `value` also has. With no `held`, it has
    /// none. A metadata child of the role element keeps its place before
    /// the value at its index, or goes after the last value if that value is
    /// gone.
    pub(crate) fn keeping_ids(value: MultiLangString, held: Option<&Text>) -> Self {
        let mut text = Self::new(value);
        if let Some(held) = held {
            text.id.clone_from(&held.id);
            for (kept, old) in text.value_ids.iter_mut().zip(&held.value_ids) {
                kept.clone_from(old);
            }
            for (kept, old) in text.value_metadata.iter_mut().zip(&held.value_metadata) {
                kept.clone_from(old);
            }
            text.metadata = held.metadata.clone().map(|mut metadata| {
                let kept: Vec<bool> = (0..held.value.len())
                    .map(|position| position < text.value.len())
                    .collect();
                metadata.shift(&kept);
                metadata
            });
        }
        text
    }

    /// Sets the `xmi:id` of the role element.
    pub(crate) fn set_id(&mut self, id: Option<ElementId>) {
        self.id = id;
    }

    /// Replaces the value at `index`, if there is one, by `map` of it.
    pub(crate) fn map_value(&mut self, index: usize, map: impl FnOnce(&LangString) -> LangString) {
        self.value.map_value(index, map);
    }

    /// Sets the XMI metadata of the role element; empty is none.
    pub(crate) fn set_metadata(&mut self, metadata: Preserved) {
        self.metadata = (!metadata.is_empty()).then(|| Box::new(metadata));
    }

    /// Adds `value`, with the `xmi:id` and the XMI metadata of its element,
    /// at the end; empty metadata is none.
    pub(crate) fn push(&mut self, value: LangString, id: Option<ElementId>, metadata: Preserved) {
        self.value.push(value);
        self.value_ids.push(id);
        self.value_metadata
            .push((!metadata.is_empty()).then(|| Box::new(metadata)));
    }

    /// The XMI metadata of the role element, if it holds any.
    pub(crate) fn role_metadata(&self) -> Option<&Preserved> {
        self.metadata.as_deref()
    }

    /// The XMI metadata of each value's element, by position.
    pub(crate) fn value_metadata(&self) -> &[Option<Box<Preserved>>] {
        &self.value_metadata
    }

    /// Every XMI metadata it holds: the role element's, then the values', in
    /// order.
    pub(crate) fn metadata(&self) -> impl Iterator<Item = &Preserved> {
        self.metadata
            .as_deref()
            .into_iter()
            .chain(self.value_metadata.iter().filter_map(Option::as_deref))
    }

    /// A copy for a [`CopyElements`](crate::edit::CopyElements): its values
    /// mapped by `value`, each id kept as `keep` says, and its metadata
    /// without the `xmi:uuid`s (XMI 2.5.1 §7.6.1: elements with one
    /// `xmi:uuid` "are all deemed to reference the same MOF element"; ADR
    /// 0042 D6).
    pub(crate) fn copied(
        &self,
        value: impl Fn(&LangString) -> LangString,
        keep: impl Fn(&Option<ElementId>) -> Option<ElementId>,
    ) -> Self {
        Self {
            value: self.value.values().iter().map(value).collect(),
            id: keep(&self.id),
            value_ids: self.value_ids.iter().map(&keep).collect(),
            metadata: self.metadata.as_deref().and_then(Preserved::without_uuid),
            value_metadata: self
                .value_metadata
                .iter()
                .map(|metadata| metadata.as_deref().and_then(Preserved::without_uuid))
                .collect(),
        }
    }

    /// The `MultiLangString`.
    pub(crate) fn value(&self) -> &MultiLangString {
        &self.value
    }

    /// The `xmi:id` of the role element, if it has one.
    pub(crate) fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// The `xmi:id` of each value, by position.
    pub(crate) fn value_ids(&self) -> &[Option<ElementId>] {
        &self.value_ids
    }

    /// Every id it holds: the role element's, then the values', in order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = &ElementId> {
        self.id.iter().chain(self.value_ids.iter().flatten())
    }
}

impl Node {
    /// The value of the flag `flag`.
    fn flag(&self, flag: Flag) -> bool {
        match flag {
            Flag::IsCitation => self.is_citation,
            Flag::IsAbstract => self.is_abstract,
            Flag::IsCounter => self.is_counter,
        }
    }

    /// The place of the flag `flag`.
    fn flag_mut(&mut self, flag: Flag) -> &mut bool {
        match flag {
            Flag::IsCitation => &mut self.is_citation,
            Flag::IsAbstract => &mut self.is_abstract,
            Flag::IsCounter => &mut self.is_counter,
        }
    }

    /// Every XMI metadata the node holds (ADR 0042): its own, its name's,
    /// then that of its `content`, `key` and `location` and their values.
    fn metadata_parts(&self) -> impl Iterator<Item = &Preserved> {
        self.metadata
            .as_deref()
            .into_iter()
            .chain(self.name_metadata.as_deref())
            .chain(
                TextRole::ALL
                    .iter()
                    .filter_map(|&role| self.text(role))
                    .flat_map(Text::metadata),
            )
    }

    /// The text the node holds in `role`.
    fn text(&self, role: TextRole) -> Option<&Text> {
        match role {
            TextRole::Content => self.content.as_ref(),
            TextRole::Key => self.key.as_ref(),
            TextRole::Location => self.location.as_ref(),
        }
    }

    /// The ids the node's `ExpressionLangStrings` name (§8.4): its name's, then
    /// each value's of its `content`, `key` and `location`, in order; one
    /// with no `expression` adds none (SACM23-BASE-009).
    fn expressions(&self) -> Vec<ElementId> {
        self.lang_strings()
            .filter_map(LangString::expression)
            .cloned()
            .collect()
    }

    /// The node's `LangString`s in write order: its name, then each value of
    /// its `content`, `key` and `location`.
    fn lang_strings(&self) -> impl Iterator<Item = &LangString> {
        self.name.iter().chain(
            TextRole::ALL
                .iter()
                .filter_map(|&role| self.text(role))
                .flat_map(|text| text.value.values()),
        )
    }

    /// Makes plain each `ExpressionLangString` of the node whose `expression`
    /// `is_gone`, keeping its `lang`, `content` and id: a delete's scrub
    /// (ADR 0038 D4). Returns whether one changed.
    fn plain_expressions(&mut self, is_gone: impl Fn(&ElementId) -> bool) -> bool {
        let mut changed = false;
        let mut scrub = |value: &mut LangString| {
            if value.expression().is_some_and(&is_gone) {
                *value = value.clone().without_expression();
                changed = true;
            }
        };
        if let Some(name) = self.name.as_mut() {
            scrub(name);
        }
        for role in TextRole::ALL {
            if let Some(text) = self.text_mut(role).as_mut() {
                text.value = text
                    .value
                    .values()
                    .iter()
                    .cloned()
                    .map(|mut value| {
                        scrub(&mut value);
                        value
                    })
                    .collect();
            }
        }
        changed
    }

    /// The place of the text the node holds in `role`.
    fn text_mut(&mut self, role: TextRole) -> &mut Option<Text> {
        match role {
            TextRole::Content => &mut self.content,
            TextRole::Key => &mut self.key,
            TextRole::Location => &mut self.location,
        }
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

/// One typed reference to an id, as [`Document::referrers`] lists it: the
/// element that holds it, the end, and the id's position in the end
/// (LIB-API-014).
///
/// It borrows the document, like [`SacmElement`].
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetReferences};
/// use sacm::model::{Document, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C2")).with_id("c2"))?;
/// edit::apply(&mut doc, SetReferences::new("c2", ReferenceEnd::MetaClaim, ["c1", "c1"]))?;
///
/// let referrers = doc.referrers("c1");
/// assert_eq!(referrers.len(), 2);
/// assert_eq!(referrers[1].holder().id().as_str(), "c2");
/// assert_eq!(referrers[1].end(), ReferenceEnd::MetaClaim);
/// assert_eq!(referrers[1].position(), 1);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Referrer<'a> {
    holder: SacmElement<'a>,
    end: ReferenceEnd,
    position: usize,
}

impl<'a> Referrer<'a> {
    /// The element that holds the reference.
    #[must_use]
    pub fn holder(self) -> SacmElement<'a> {
        self.holder
    }

    /// The end that holds it.
    #[must_use]
    pub fn end(self) -> ReferenceEnd {
        self.end
    }

    /// The id's position in the end, from 0: for a single-valued end, 0.
    #[must_use]
    pub fn position(self) -> usize {
        self.position
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
    /// The element was given a name, but its kind is one of the four
    /// `UtilityElement` kinds, which have no name. Holds the element's id and
    /// kind.
    NameNotAllowed(ElementId, ElementKind),
    /// No SACM containment role of the parent's class admits the element's
    /// kind: a Claim directly under an AssuranceCasePackage, say, rather than
    /// under one of its argument packages.
    ContainmentNotAllowed {
        /// The refused element's id.
        id: ElementId,
        /// The refused element's kind.
        kind: ElementKind,
        /// The id of the parent it was to be contained in.
        parent: ElementId,
        /// The parent's kind.
        parent_kind: ElementKind,
    },
    /// The element has no parent and its kind is not a package kind: a root
    /// must be a package, a package interface or a package binding (ADR 0010
    /// D1). Holds the element's id and kind.
    RootNotAllowed(ElementId, ElementKind),
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("the element id is empty"),
            // `{:?}` quotes the id and escapes line breaks, so the message
            // stays on one line.
            Self::DuplicateId(id) => write!(
                f,
                "the document already has an element with id {:?}",
                id.as_str()
            ),
            Self::UnknownParent(id) => write!(
                f,
                "the document has no element {:?} to be the parent",
                id.as_str()
            ),
            Self::NameNotAllowed(id, kind) => write!(
                f,
                "element {:?} of kind {kind} cannot have a name",
                id.as_str()
            ),
            Self::ContainmentNotAllowed {
                id,
                kind,
                parent,
                parent_kind,
            } => write!(
                f,
                "element {:?} of kind {kind} cannot be contained in element {:?} of kind {parent_kind}",
                id.as_str(),
                parent.as_str()
            ),
            Self::RootNotAllowed(id, kind) => write!(
                f,
                "element {:?} of kind {kind} cannot be a root",
                id.as_str()
            ),
        }
    }
}

impl std::error::Error for InsertError {}

// `Document::set_assertion_declaration` is not an intra-doc link here:
// without the feature, that method does not exist.
/// Why `Document::set_assertion_declaration` refused to set a declaration.
///
/// **Not part of the stable API**: public only under the `test-support`
/// feature, with `Document::set_assertion_declaration`.
#[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(
    not(feature = "test-support"),
    expect(
        unreachable_pub,
        reason = "public only under the test-support feature, crate-visible otherwise"
    )
)]
pub enum SetAssertionDeclarationError {
    /// The document has no element with this id.
    UnknownElement(ElementId),
    /// The element's kind is not an Assertion, so it has no
    /// `assertionDeclaration`. Holds the element's id and kind.
    NotAnAssertion(ElementId, ElementKind),
}

impl fmt::Display for SetAssertionDeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // `{:?}` quotes the id and escapes line breaks, so the message
            // stays on one line.
            Self::UnknownElement(id) => {
                write!(f, "the document has no element {:?}", id.as_str())
            }
            Self::NotAnAssertion(id, kind) => write!(
                f,
                "element {:?} of kind {kind} is not an Assertion",
                id.as_str()
            ),
        }
    }
}

impl std::error::Error for SetAssertionDeclarationError {}

#[cfg(test)]
mod tests {
    use super::{Document, IdGenerator, SacmElement, Text, TextRole, snake_case};
    use crate::model::{ElementId, ElementKind, Href, LangString};
    use crate::model::{Flag, ReferenceEnd};

    #[test]
    fn snake_case_splits_a_class_name_at_each_capital() {
        assert_eq!(
            snake_case(ElementKind::AssuranceCasePackage.name()),
            "assurance_case_package"
        );
        assert_eq!(snake_case(ElementKind::Claim.name()), "claim");
        assert_eq!(snake_case("lower"), "lower");
    }

    /// A document with a named root `p` and an unnamed root `q`.
    fn two_roots() -> Document {
        let mut document = Document::new();
        let named = Some(LangString::new("P"));
        for (id, name) in [("p", named), ("q", None)] {
            let inserted =
                document.insert_node(None, ElementKind::ArgumentPackage, ElementId::new(id), name);
            assert_eq!(inserted, Ok(()), "{id}");
        }
        document
    }

    #[test]
    fn set_name_id_refuses_what_would_break_the_id_space_and_changes_nothing() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        for (owner, name_id) in [
            ("missing", "n1"),
            ("q", "n1"),
            ("p", ""),
            ("p", "p"),
            ("p", "q"),
        ] {
            let refused = document.set_name_id(&ElementId::new(owner), ElementId::new(name_id));
            assert!(refused.is_err(), "{owner} {name_id:?}");
        }
        assert_eq!(document.get("p").and_then(SacmElement::name_id), None);
        assert!(!document.is_id_in_use("n1"));
        assert_eq!(document.set_name_id(&p, ElementId::new("n1")), Ok(()));
        assert!(document.is_id_in_use("n1") && !document.contains("n1"));
        assert_eq!(
            document
                .get("p")
                .and_then(SacmElement::name_id)
                .map(ElementId::as_str),
            Some("n1")
        );
        // A name id is in use for a second name and for a new element.
        assert!(document.set_name_id(&p, ElementId::new("n1")).is_err());
        let element = document.insert_node(
            None,
            ElementKind::ArgumentPackage,
            ElementId::new("n1"),
            None,
        );
        assert!(element.is_err());
    }

    /// A Text of `values` with the id `id` and these value ids.
    fn text(id: Option<&str>, value_ids: &[Option<&str>]) -> Text {
        let mut held = Text::default();
        held.set_id(id.map(ElementId::new));
        for value_id in value_ids {
            held.push(
                LangString::new("x"),
                value_id.map(ElementId::new),
                super::Preserved::default(),
            );
        }
        held
    }

    #[test]
    fn set_text_refuses_what_would_break_the_id_space_or_the_kind_and_changes_nothing() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        assert_eq!(document.set_name_id(&p, ElementId::new("pn")), Ok(()));
        for (kind, id) in [(ElementKind::Note, "n"), (ElementKind::TaggedValue, "t")] {
            let inserted = document.insert_node(Some(&p), kind, ElementId::new(id), None);
            assert_eq!(inserted, Ok(()), "{id}");
        }
        let n = ElementId::new("n");
        let refusals = [
            ("missing", TextRole::Content, text(None, &[])),
            ("p", TextRole::Content, text(None, &[])),
            ("n", TextRole::Key, text(None, &[])),
            ("n", TextRole::Content, text(Some(""), &[])),
            ("n", TextRole::Content, text(None, &[Some("")])),
            ("n", TextRole::Content, text(Some("w"), &[Some("w")])),
            ("n", TextRole::Content, text(None, &[Some("v"), Some("v")])),
            ("n", TextRole::Content, text(Some("q"), &[])),
            ("n", TextRole::Content, text(None, &[Some("pn")])),
        ];
        for (owner, role, held) in refusals {
            let refused = document.set_text(&ElementId::new(owner), role, held.clone());
            assert!(refused.is_err(), "{owner} {role:?} {held:?}");
        }
        assert_eq!(document.get("n").and_then(SacmElement::content), None);
        assert!(!document.is_id_in_use("w") && !document.is_id_in_use("v"));

        // Kept: the ids are in use, but identify no element.
        assert_eq!(
            document.set_text(&n, TextRole::Content, text(Some("w"), &[None, Some("v")])),
            Ok(())
        );
        assert!(document.is_id_in_use("w") && document.is_id_in_use("v"));
        assert!(!document.contains("w") && !document.contains("v"));
        let t = ElementId::new("t");
        let taken = document.set_text(&t, TextRole::Key, text(Some("v"), &[]));
        assert!(taken.is_err());
        // Replaced: the role's own ids are free for it, and the ids it held
        // are freed.
        assert_eq!(
            document.set_text(&n, TextRole::Content, text(Some("v"), &[Some("u")])),
            Ok(())
        );
        assert!(!document.is_id_in_use("w") && document.is_id_in_use("u"));
        assert_eq!(
            document.set_text(&t, TextRole::Key, text(Some("w"), &[])),
            Ok(())
        );
        let note = document.get("n");
        assert_eq!(
            note.and_then(SacmElement::content_id)
                .map(ElementId::as_str),
            Some("v")
        );
        assert_eq!(
            note.map(SacmElement::content_value_ids),
            Some([Some(ElementId::new("u"))].as_slice())
        );
        assert_eq!(
            document
                .get("t")
                .and_then(SacmElement::key_id)
                .map(ElementId::as_str),
            Some("w")
        );
        assert_eq!(
            document.next_generated_id(ElementKind::Note),
            (ElementId::new("note_1"), 1)
        );
    }

    #[test]
    fn set_gid_holds_the_value_exactly_and_refuses_an_unknown_element() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        assert!(
            document
                .set_gid(&ElementId::new("missing"), Some("g".to_owned()))
                .is_err()
        );
        assert_eq!(document.set_gid(&p, Some(" p ".to_owned())), Ok(()));
        assert_eq!(document.get("p").and_then(SacmElement::gid), Some(" p "));
        assert!(!document.is_id_in_use(" p ") && document.get(" p ").is_none());
        assert_eq!(document.set_gid(&p, None), Ok(()));
        assert_eq!(document.get("p").and_then(SacmElement::gid), None);
    }

    #[test]
    fn set_flag_sets_one_flag_and_refuses_an_unknown_element() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        assert!(
            document
                .set_flag(&ElementId::new("missing"), Flag::IsCitation, true)
                .is_err()
        );
        assert_eq!(document.set_flag(&p, Flag::IsAbstract, true), Ok(()));
        let flags = |document: &Document| {
            document
                .get("p")
                .map(|p| (p.is_citation(), p.is_abstract()))
        };
        assert_eq!(flags(&document), Some((false, true)));
        assert_eq!(document.set_flag(&p, Flag::IsAbstract, false), Ok(()));
        assert_eq!(flags(&document), Some((false, false)));
        // A package is no AssertedRelationship, so it has no isCounter.
        assert!(document.set_flag(&p, Flag::IsCounter, true).is_err());
        assert_eq!(document.get("p").map(SacmElement::is_counter), Some(false));
    }

    #[test]
    fn set_references_refuses_what_a_single_valued_end_cannot_hold_and_changes_nothing() {
        let mut document = two_roots();
        let ids = |ids: &[&str]| ids.iter().map(|id| ElementId::new(*id)).collect::<Vec<_>>();
        for (owner, end, targets) in [
            ("missing", ReferenceEnd::CitedElement, ids(&["q"])),
            ("p", ReferenceEnd::CitedElement, ids(&[""])),
            ("p", ReferenceEnd::CitedElement, ids(&["q", "q"])),
            // A package is no Assertion.
            ("p", ReferenceEnd::MetaClaim, ids(&["q"])),
        ] {
            let refused = document.set_references(&ElementId::new(owner), end, targets.clone());
            assert!(refused.is_err(), "{owner} {targets:?}");
        }
        assert_eq!(document.get("p").and_then(SacmElement::cited_element), None);
        assert!(!document.is_referenced("q"));
    }

    #[test]
    fn set_hrefs_refuses_an_end_the_kind_lacks_the_derived_end_and_a_second_value() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        for (kind, id) in [
            (ElementKind::Claim, "c"),
            (ElementKind::ArgumentPackageInterface, "i"),
            (ElementKind::ArgumentPackageInterface, "j"),
        ] {
            let inserted = document.insert_node(Some(&p), kind, ElementId::new(id), None);
            assert_eq!(inserted, Ok(()), "{id}");
        }
        let i = ElementId::new("i");
        assert_eq!(
            document.set_references(&i, ReferenceEnd::Implements, vec![p.clone()]),
            Ok(())
        );
        let hrefs = |values: &[&str]| {
            values
                .iter()
                .map(|value| Href::from_parts(None, value))
                .collect::<Vec<_>>()
        };
        let before = document.clone();
        for (owner, end, values) in [
            // A Claim has no implements.
            ("c", ReferenceEnd::Implements, hrefs(&["o.xmi#p"])),
            // The derived expression holds no href, on any element.
            ("j", ReferenceEnd::Expression, hrefs(&["o.xmi#e"])),
            // implements is single-valued: two hrefs, or an href beside an id.
            (
                "j",
                ReferenceEnd::Implements,
                hrefs(&["o.xmi#p", "o.xmi#q"]),
            ),
            ("i", ReferenceEnd::Implements, hrefs(&["o.xmi#p"])),
        ] {
            let refused = document.set_hrefs(&ElementId::new(owner), end, values.clone());
            assert!(refused.is_err(), "{owner} {end:?} {values:?}");
            assert!(document == before, "{owner} {end:?} {values:?}");
            assert!(
                document
                    .get(owner)
                    .is_some_and(|element| element.hrefs(end).is_empty()),
                "{owner} {end:?} {values:?}"
            );
        }
        // The control: one href on an implements that holds no id.
        let j = ElementId::new("j");
        assert_eq!(
            document.set_hrefs(&j, ReferenceEnd::Implements, hrefs(&["o.xmi#p"])),
            Ok(())
        );
        assert_eq!(
            document.get("j").map(|j| j.hrefs(ReferenceEnd::Implements)),
            Some(hrefs(&["o.xmi#p"]).as_slice())
        );
    }

    #[test]
    fn set_proxies_refuses_an_unknown_owner_and_a_role_the_class_lacks() {
        use crate::model::Proxy;

        let mut document = two_roots();
        let p = ElementId::new("p");
        let proxy = |role| Proxy::new(role, Href::from_parts(None, "o.xmi#c"));
        let before = document.clone();
        // `p` is an ArgumentPackage: it has no role `artifactPackage`.
        assert_eq!(
            document.set_proxies(&p, vec![proxy("argumentElement"), proxy("artifactPackage")]),
            Err("the ArgumentPackage \"p\" has no composite role artifactPackage".to_owned())
        );
        assert_eq!(
            document.set_proxies(&ElementId::new("nope"), vec![proxy("argumentElement")]),
            Err("the document has no element \"nope\"".to_owned())
        );
        assert!(
            document == before,
            "a refused set_proxies changed the document"
        );
        let held = vec![proxy("argumentElement"), proxy("argumentElement")];
        assert_eq!(document.set_proxies(&p, held.clone()), Ok(()));
        assert_eq!(
            document.get("p").map(SacmElement::proxies),
            Some(held.as_slice())
        );
        // Setting replaces, and none removes.
        assert_eq!(document.set_proxies(&p, Vec::new()), Ok(()));
        assert!(document == before, "set_proxies with none does not remove");
    }

    #[test]
    fn a_multi_valued_end_holds_every_target_in_order_with_repeats_and_counts_each() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        let claim = ElementId::new("c");
        let inserted = document.insert_node(Some(&p), ElementKind::Claim, claim.clone(), None);
        assert_eq!(inserted, Ok(()));
        let ids = |ids: &[&str]| ids.iter().map(|id| ElementId::new(*id)).collect::<Vec<_>>();
        let end = ReferenceEnd::MetaClaim;
        assert!(
            document
                .set_references(&claim, end, ids(&["claim_1", ""]))
                .is_err()
        );
        assert_eq!(
            document.set_references(&claim, end, ids(&["claim_1", "q", "claim_1"])),
            Ok(())
        );
        assert_eq!(
            document.get("c").map(SacmElement::meta_claims),
            Some(ids(&["claim_1", "q", "claim_1"]).as_slice())
        );
        assert!(document.is_referenced("claim_1"));
        // Replacing the list releases each use, repeats included.
        assert_eq!(document.set_references(&claim, end, ids(&["q"])), Ok(()));
        assert!(!document.is_referenced("claim_1") && document.is_referenced("q"));
    }

    #[test]
    fn insert_with_references_sets_the_flags_and_ends_or_changes_nothing() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        let ids = |ids: &[&str]| ids.iter().map(|id| ElementId::new(*id)).collect::<Vec<_>>();
        let references = vec![
            (ReferenceEnd::Source, ids(&["p", "q", "p"])),
            (ReferenceEnd::Target, ids(&["q"])),
        ];
        let flags = [(Flag::IsCounter, true)];
        // A refused insert sets nothing.
        let refused = document.insert_with_references(
            Some(&ElementId::new("missing")),
            ElementKind::AssertedInference,
            ElementId::new("i"),
            None,
            &flags,
            references.clone(),
        );
        assert!(refused.is_err());
        assert!(!document.contains("i") && !document.is_referenced("q"));
        let inserted = document.insert_with_references(
            Some(&p),
            ElementKind::AssertedInference,
            ElementId::new("i"),
            None,
            &flags,
            references,
        );
        assert_eq!(inserted, Ok(()));
        let i = document.get("i");
        assert_eq!(
            i.map(|i| (i.sources(), i.target(), i.is_counter())),
            Some((
                ids(&["p", "q", "p"]).as_slice(),
                Some(&ElementId::new("q")),
                true
            ))
        );
        assert!(document.is_referenced("q"));
    }

    #[test]
    fn references_are_read_by_end_counted_and_released_when_replaced() {
        let mut document = two_roots();
        let (p, q) = (ElementId::new("p"), ElementId::new("q"));
        let one = |id: &str| vec![ElementId::new(id)];
        // Set out of table order; each is read back by its end.
        assert_eq!(
            document.set_references(&p, ReferenceEnd::AbstractForm, one("claim_1")),
            Ok(())
        );
        assert_eq!(
            document.set_references(&p, ReferenceEnd::CitedElement, one("claim_1")),
            Ok(())
        );
        assert_eq!(
            document.set_references(&q, ReferenceEnd::CitedElement, one("p")),
            Ok(())
        );
        let held = document.get("p").map(|p| {
            [ReferenceEnd::CitedElement, ReferenceEnd::AbstractForm]
                .map(|end| p.references(end).first().map(ElementId::as_str))
        });
        assert_eq!(held, Some([Some("claim_1"), Some("claim_1")]));
        // A referenced id is not in use, but no id is generated for it.
        assert!(document.is_referenced("claim_1") && !document.is_id_in_use("claim_1"));
        assert!(document.is_referenced("p") && document.is_id_in_use("p"));
        assert_eq!(
            document.next_generated_id(ElementKind::Claim),
            (ElementId::new("claim_2"), 2)
        );
        // Each reference is counted: replacing one keeps the other.
        assert_eq!(
            document.set_references(&p, ReferenceEnd::AbstractForm, Vec::new()),
            Ok(())
        );
        assert!(document.is_referenced("claim_1"));
        assert_eq!(
            document.set_references(&p, ReferenceEnd::CitedElement, one("other")),
            Ok(())
        );
        assert!(!document.is_referenced("claim_1") && document.is_referenced("other"));
        assert_eq!(
            document.next_generated_id(ElementKind::Claim),
            (ElementId::new("claim_1"), 1)
        );
        // A name id that is also referenced stays in use when the reference
        // goes, and stays referenced when only its use as a name id would.
        assert_eq!(document.set_name_id(&p, ElementId::new("other")), Ok(()));
        assert_eq!(
            document.set_references(&p, ReferenceEnd::CitedElement, Vec::new()),
            Ok(())
        );
        assert!(document.is_id_in_use("other") && !document.is_referenced("other"));
    }

    #[test]
    fn remove_elements_frees_ids_releases_references_and_remaps_handles() {
        let mut document = two_roots();
        let id = ElementId::new;
        let (p, q, c1, c2, c3) = (id("p"), id("q"), id("c1"), id("c2"), id("c3"));
        let named = || Some(LangString::new("N"));
        for (parent, kind, child, name) in [
            (&p, ElementKind::Claim, &c1, named()),
            (&c1, ElementKind::Note, &id("n1"), None),
            (&p, ElementKind::Claim, &c2, named()),
            (&q, ElementKind::Claim, &c3, named()),
        ] {
            let inserted = document.insert_node(Some(parent), kind, child.clone(), name);
            assert_eq!(inserted, Ok(()), "{child}");
        }
        assert_eq!(document.set_name_id(&c1, id("c1_name")), Ok(()));
        let content = text(Some("n1_content"), &[Some("n1_value")]);
        assert_eq!(
            document.set_text(&id("n1"), TextRole::Content, content),
            Ok(())
        );
        for (owner, end, targets) in [
            (
                &c2,
                ReferenceEnd::MetaClaim,
                vec![c1.clone(), id("ghost"), c1.clone()],
            ),
            (&c1, ReferenceEnd::CitedElement, vec![c3.clone()]),
            (&c3, ReferenceEnd::MetaClaim, vec![c2.clone()]),
        ] {
            assert_eq!(document.set_references(owner, end, targets), Ok(()));
        }

        document.remove_elements(&[c1.clone(), id("n1")], std::slice::from_ref(&c2));

        let order: Vec<&str> = document.elements().map(|e| e.id().as_str()).collect();
        assert_eq!(order, ["p", "c2", "q", "c3"]);
        assert_eq!(document.element_count(), 4);
        let parent_of = |child: &str| {
            document
                .get(child)
                .and_then(SacmElement::parent)
                .map(|parent| parent.id().as_str())
        };
        assert_eq!((parent_of("c2"), parent_of("c3")), (Some("p"), Some("q")));
        let roots: Vec<&str> = document.roots().map(|r| r.id().as_str()).collect();
        assert_eq!(roots, ["p", "q"]);
        // The ids of the deleted elements, their name and their content are
        // free, and the references they held are released.
        for freed in ["c1", "n1", "c1_name", "n1_content", "n1_value"] {
            assert!(!document.is_id_in_use(freed), "{freed}");
        }
        assert!(!document.is_referenced("c1") && !document.is_referenced("c3"));
        assert!(document.is_referenced("ghost") && document.is_referenced("c2"));
        let kept = document
            .get("c2")
            .map(|c2| c2.references(ReferenceEnd::MetaClaim).to_vec());
        assert_eq!(kept, Some(vec![id("ghost")]));
        // A freed id may be given again.
        assert_eq!(
            document.insert_node(Some(&q), ElementKind::Claim, c1.clone(), named()),
            Ok(())
        );
        assert_eq!(parent_of_owned(&document, "c1"), Some("q".to_owned()));
    }

    /// The id of the parent of `child`, owned.
    fn parent_of_owned(document: &Document, child: &str) -> Option<String> {
        document
            .get(child)
            .and_then(SacmElement::parent)
            .map(|parent| parent.id().as_str().to_owned())
    }

    #[test]
    fn a_generated_id_skips_name_ids_and_reserved_ids() {
        let mut document = two_roots();
        let p = ElementId::new("p");
        assert_eq!(document.set_name_id(&p, ElementId::new("claim_1")), Ok(()));
        assert_eq!(
            document.next_generated_id(ElementKind::Claim),
            (ElementId::new("claim_2"), 2)
        );

        let mut generator = IdGenerator::default();
        let reserved = |id: &str| matches!(id, "claim_1" | "claim_2");
        assert_eq!(
            generator.next(ElementKind::Claim, reserved).as_str(),
            "claim_3"
        );
        assert_eq!(
            generator.next(ElementKind::Claim, reserved).as_str(),
            "claim_4"
        );
        assert_eq!(
            generator
                .next(ElementKind::ArgumentPackage, reserved)
                .as_str(),
            "argument_package_1"
        );
        let mut read = Document::new();
        generator.commit(&mut read);
        assert_eq!(
            read.next_generated_id(ElementKind::Claim),
            (ElementId::new("claim_5"), 5)
        );
        assert_eq!(
            read.next_generated_id(ElementKind::ArgumentPackage),
            (ElementId::new("argument_package_2"), 2)
        );
        assert_eq!(
            read.next_generated_id(ElementKind::AssuranceCasePackage),
            (ElementId::new("assurance_case_package_1"), 1)
        );
    }
}
