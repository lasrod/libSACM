//! The document and its read-only element views.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use super::containment::{Role, role};
use super::{
    AbstractClass, AssertionDeclaration, ElementId, ElementKind, LangString, MultiLangString,
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
/// `AssuranceCasePackage` and `ArgumentPackage` roots, the
/// `AssuranceCasePackage`, `ArgumentPackage` and `Claim` elements in them, and
/// the descriptions, implementation constraints, notes and tagged values of
/// each of these. [`edit::apply`](crate::edit::apply) creates the same seven
/// kinds. Tests build documents with other kinds through the `test-support`
/// feature's `Document::insert`. [`Document::new`] is empty.
///
/// Cloning a document copies all of it, including the counters that generated
/// ids continue from; the copy is independent of the original. Cloning,
/// dropping and traversal do not recurse, so arbitrarily deep nesting is
/// safe.
///
/// # Equality
///
/// `==` compares SACM content: two documents are equal exactly when
/// [`semantic_compare`](crate::compare::semantic_compare) finds no
/// difference between them. Elements are matched by id; kinds, parents,
/// `gid`s, names, the ids of names, `content`s and `key`s with their ids,
/// `assertionDeclaration`s and the order of roots and children are compared. `==` is order-sensitive; see [`crate::compare`] to ignore order.
///
/// Not compared: the per-kind counters [`edit::apply`](crate::edit::apply)
/// uses to generate ids such as `claim_1`. They are not SACM content, and XMI
/// does not carry them. Today two equal documents always generate the same
/// next id; once elements can be deleted they may not, because one may have
/// generated and deleted an id the other never used.
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
    /// The handle of the owner of each id that identifies no element: the
    /// `xmi:id` of a `name` (ADR 0024), of a `content` or `key`, or of a
    /// `value` in one (ADR 0025). They share the file's id space, but
    /// [`get`](Self::get) does not look in it. Only looked up, never
    /// iterated.
    value_ids: HashMap<ElementId, usize>,
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
        match parent {
            Some(parent) => self.nodes[parent].children.push(node),
            None => self.roots.push(node),
        }
        self.index.insert(id.clone(), node);
        self.nodes.push(Node {
            id,
            kind,
            gid: None,
            name,
            name_id: None,
            content: content.map(Text::new),
            key: key.map(Text::new),
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
        self.value_ids.insert(name_id.clone(), node);
        self.nodes[node].name_id = Some(name_id);
        Ok(())
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
        let old: Vec<ElementId> = self.nodes[node]
            .text(role)
            .map(|held| held.ids().cloned().collect())
            .unwrap_or_default();
        let mut seen = BTreeSet::new();
        for id in text.ids() {
            if id.as_str().is_empty() {
                return Err(format!("an id of the {} is empty", role.name()));
            }
            let elsewhere = self.is_id_in_use(id.as_str()) && !old.contains(id);
            if !seen.insert(id.as_str()) || elsewhere {
                return Err(format!("the id {:?} is already in use", id.as_str()));
            }
        }
        for id in &old {
            self.value_ids.remove(id);
        }
        for id in text.ids() {
            self.value_ids.insert(id.clone(), node);
        }
        *self.nodes[node].text_mut(role) = Some(text);
        Ok(())
    }

    /// Whether `id` is in use in the document's id space: by an element, or
    /// as an id that identifies no element: of a name (ADR 0024), a
    /// `content`, a `key` or a `value` (ADR 0025). A new element's id, given
    /// or generated, must not be.
    pub(crate) fn is_id_in_use(&self, id: &str) -> bool {
        self.index.contains_key(id) || self.value_ids.contains_key(id)
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
        // Each element id and each id that identifies no element is in use
        // once, and the candidates are distinct, so one of the first
        // `in_use + 1` is free: the last candidate needs no check, and the
        // loop is bounded.
        for _ in 0..self.nodes.len() + self.value_ids.len() {
            if !self.is_id_in_use(id.as_str()) {
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
            // element, which is compared through `SacmElement::compared`.
            value_ids: _,
            // Not compared: the generated-id counters are not SACM content
            // and XMI does not carry them (ADR 0020).
            id_counters: _,
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
    /// // Built with the unstable test-support feature: edit::apply does not create a missing name.
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
    /// writes it as the first attribute of `name`. No edit sets it yet, so
    /// only [`xmi::read`](crate::xmi::read) gives one, and it is `None`
    /// whenever [`name`](Self::name) is.
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
    /// from sharing one, though §8.2 requires a gid to be unique; no rule
    /// reports that yet (SACM23-VAL-002). No edit sets it yet, so only
    /// [`xmi::read`](crate::xmi::read) gives one.
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
    /// (ADR 0024, ADR 0025). No edit sets it, so only
    /// [`xmi::read`](crate::xmi::read) gives one.
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

    /// The element's children held in the role named `role`, in document
    /// order.
    fn children_in(self, role: &'static str) -> impl Iterator<Item = SacmElement<'a>> {
        self.children()
            .filter(move |child| child.role().is_some_and(|held| held.name == role))
    }

    /// The element's `content` or `key`, with its ids.
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
            // Compared.
            assertion_declaration,
            // Compared, as the parent's id.
            parent,
            // Derived from the parent's kind and the element's kind
            // (`containment::role`), both compared.
            role: _,
            // Compared as the child order, through `children()`.
            children: _,
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
            assertion_declaration: *assertion_declaration,
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
    /// The element's `assertionDeclaration`; `None` when not an Assertion.
    pub(crate) assertion_declaration: Option<AssertionDeclaration>,
}

/// A role of a `UtilityElement` that holds a `MultiLangString` (ADR 0025):
/// `content` on every `UtilityElement` (SACM 2.3 §8.7), and `key` on a
/// `TaggedValue` (§8.12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextRole {
    /// `content`.
    Content,
    /// `key`.
    Key,
}

impl TextRole {
    /// The role's name, as written in XMI.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Key => "key",
        }
    }

    /// Whether an element of `kind` has this role.
    pub(crate) const fn is_held_by(self, kind: ElementKind) -> bool {
        match self {
            Self::Content => kind.is_a(AbstractClass::UtilityElement),
            Self::Key => kind.is_kind_of(ElementKind::TaggedValue),
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
}

impl Text {
    /// `value`, with no ids.
    pub(crate) fn new(value: MultiLangString) -> Self {
        let value_ids = vec![None; value.len()];
        Self {
            value,
            id: None,
            value_ids,
        }
    }

    /// Sets the `xmi:id` of the role element.
    pub(crate) fn set_id(&mut self, id: Option<ElementId>) {
        self.id = id;
    }

    /// Adds `value`, with the `xmi:id` of its element, at the end.
    pub(crate) fn push(&mut self, value: LangString, id: Option<ElementId>) {
        self.value.push(value);
        self.value_ids.push(id);
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
    /// The text the node holds in `role`.
    fn text(&self, role: TextRole) -> Option<&Text> {
        match role {
            TextRole::Content => self.content.as_ref(),
            TextRole::Key => self.key.as_ref(),
        }
    }

    /// The place of the text the node holds in `role`.
    fn text_mut(&mut self, role: TextRole) -> &mut Option<Text> {
        match role {
            TextRole::Content => &mut self.content,
            TextRole::Key => &mut self.key,
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
    use crate::model::{ElementId, ElementKind, LangString};

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
            held.push(LangString::new("x"), value_id.map(ElementId::new));
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
