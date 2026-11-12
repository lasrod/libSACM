//! The document and its read-only element views.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use super::containment::{Role, role};
use super::{AbstractClass, AssertionDeclaration, ElementId, ElementKind, LangString};

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
/// `AssuranceCasePackage` and `ArgumentPackage` roots and the
/// `AssuranceCasePackage`, `ArgumentPackage` and `Claim` elements in them,
/// and [`edit::apply`](crate::edit::apply) creates the same three kinds. Tests build
/// documents with other kinds through the `test-support` feature's
/// `Document::insert`. [`Document::new`] is empty.
///
/// Cloning a document copies all of it, including the counters that generated
/// ids continue from; the copy is independent of the original. Cloning,
/// dropping and traversal do not recurse, so arbitrarily deep nesting is
/// safe.
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
    /// The `n` of the last id generated for each kind (`claim_<n>`, ADR
    /// 0019); a kind with none generated yet is absent. Advanced only when a
    /// generated id is used, and copied by `Clone`.
    id_counters: BTreeMap<ElementKind, u64>,
}

/// One element of a [`Document`].
#[derive(Clone, Debug)]
struct Node {
    id: ElementId,
    kind: ElementKind,
    /// The element's name; `None` for a missing name, and always `None` for a
    /// `UtilityElement` kind.
    name: Option<LangString>,
    /// The element's `assertionDeclaration`: `Some` exactly when the kind is
    /// an Assertion.
    assertion_declaration: Option<AssertionDeclaration>,
    /// The handle of the containing element; `None` for a root.
    parent: Option<usize>,
    /// The role of the parent's class that holds the element: `Some` exactly
    /// when it has a parent.
    role: Option<Role>,
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
    /// - the document already has an element with `id`
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
        self.insert_node(parent, kind, id, name)
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
        match parent {
            Some(parent) => self.nodes[parent].children.push(node),
            None => self.roots.push(node),
        }
        self.index.insert(id.clone(), node);
        self.nodes.push(Node {
            id,
            kind,
            name,
            assertion_declaration: kind
                .is_a(AbstractClass::Assertion)
                .then(AssertionDeclaration::default),
            parent,
            role,
            children: Vec::new(),
        });
        Ok(())
    }

    /// Sets the `assertionDeclaration` (SACM 2.3 §11.10) of the Assertion
    /// `id`, replacing its previous value.
    ///
    /// **Not part of the stable API.** A seam of the `test-support` feature,
    /// until the edit operations exist. It may change or disappear in any
    /// release.
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
        self.set_declaration(id, declaration)
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

    /// The id the next generated element of `kind` gets, with its `n`:
    /// `<snake_case kind>_<n>` for the smallest `n` above the kind's counter
    /// whose id is not in use (ADR 0019). The counter is not advanced; see
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
        // At most `element_count` ids are in use and the candidates are
        // distinct, so one of the first `element_count + 1` is free: the last
        // candidate needs no check, and the loop is bounded.
        for _ in 0..self.nodes.len() {
            if !self.index.contains_key(&id) {
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

    /// The view of the node with handle `node`.
    fn view(&self, node: usize) -> SacmElement<'_> {
        SacmElement {
            document: self,
            node,
        }
    }
}

/// `<prefix>_<n>`.
fn generated_id(prefix: &str, n: u64) -> ElementId {
    ElementId::new(format!("{prefix}_{n}"))
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
/// // Built with the unstable test-support feature: edit::apply does not create an ArgumentReasoning yet.
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
    /// // Built with the unstable test-support feature: edit::apply does not create a Note or a missing name.
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
    /// // Built with the unstable test-support feature: edit::apply does not create an ArgumentReasoning yet.
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

    /// The elements this one contains directly, in document order.
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
    use super::snake_case;
    use crate::model::ElementKind;

    #[test]
    fn snake_case_splits_a_class_name_at_each_capital() {
        assert_eq!(
            snake_case(ElementKind::AssuranceCasePackage.name()),
            "assurance_case_package"
        );
        assert_eq!(snake_case(ElementKind::Claim.name()), "claim");
        assert_eq!(snake_case("lower"), "lower");
    }
}
