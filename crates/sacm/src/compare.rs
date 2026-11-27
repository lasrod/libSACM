//! Find what differs between two documents, by their SACM content.
//!
//! # Entry points
//!
//! | Use | What it gives |
//! |---|---|
//! | [`semantic_compare`] | Every [`Difference`] between two documents. |
//! | `==` on two [`Document`]s | True exactly when `semantic_compare` finds none. |
//! | [`changes`] | Only the ids created, changed and deleted between them, as [`Changes`]. |
//!
//! # Rules
//!
//! - **Matched by id** across the whole document. An element that moved to another
//!   parent is one [`Difference::Parent`], not a removal and an addition.
//! - **Exact.** A missing name differs from an empty one, and a missing `content` from
//!   an empty one.
//! - **Order counts,** because equal documents must write equal bytes, though SACM 2.3
//!   orders no composition and no reference end. The example below compares without it.
//! - **`Changes` is a batch's net lists,** with the created ids in document order rather
//!   than creation order (LIB-RT-005). With the `serde` feature it serializes as a
//!   record.
//!
//! # What is compared
//!
//! For each element both documents hold, exactly and in order:
//!
//! | What | Detail |
//! |---|---|
//! | Identity and place | its kind, its parent, and the relative order of its children |
//! | `SACMElement` features (§8.2) | `gid`, `isCitation`, `isAbstract`, and the ids `citedElement` and `abstractForm` hold |
//! | Argumentation (§11.2, §11.9, §11.10, §11.12, §11.13) | `isCounter`, the [`assertionDeclaration`](crate::model::AssertionDeclaration), and the ids of `metaClaim`, `reasoning`, `source`, `target`, `structure`, `referencedArtifactElement` and a group's `argumentElement` |
//! | `ArtifactGroup` (§12.3) | the ids of `artifactElement` |
//! | Packages (§11.5, §11.6, §12.4, §12.5; ptc/22-03-13) | a package's `interface`, an interface's `implements` and a binding's `participantPackage` |
//! | Cross-document references (ADR 0040) | the `href`s of each end, as [`Difference::Hrefs`]; those of a multi-valued end in order, with repeats |
//! | Name and texts | its name and the `xmi:id` of its name ([`SacmElement::name_id`](crate::model::SacmElement::name_id)); its `content` and `key` with their ids ([`SacmElement::content`](crate::model::SacmElement::content) and the like) |
//! | Artifact values (§12.7, §12.9, §12.10, §12.11) | an `Artifact`'s `version`, the `date` of an `Artifact` or an `Event`, a `Resource`'s `location` with its ids, an `Activity`'s `startTime` and `endTime` |
//! | XMI metadata | as read (SACM23-XMI-019, ADR 0042) |
//! | Preserved content | as read (LIB-COMPAT-003, ADR 0033) |
//!
//! For the document: the relative order of the roots, and the metadata and preserved
//! content outside every element.
//!
//! Not compared:
//!
//! - **The id counters.** The per-kind counters [`edit::apply`](crate::edit::apply) uses
//!   to generate ids such as `claim_1` are not SACM content, and XMI does not carry them.
//! - **Formatting, attribute order and namespace prefixes.** A [`Document`] does not
//!   hold them, so two files that differ only in them read into equal documents.
//!   Preserved content is the exception: it holds them and is compared exactly.
//!
//! # Comparing without order
//!
//! Drop the two order variants, and compare the multi-valued ends sorted:
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
//! differences.retain(|d| match d {
//!     Difference::RootOrder { .. } | Difference::ChildOrder { .. } => false,
//!     // The multi-valued ends are compared in order too: compare them sorted.
//!     Difference::MetaClaims { left, right, .. }
//!     | Difference::Sources { left, right, .. }
//!     | Difference::ReferencedArtifactElements { left, right, .. }
//!     | Difference::GroupMembers { left, right, .. }
//!     | Difference::ArtifactGroupMembers { left, right, .. }
//!     | Difference::ArtifactSources { left, right, .. }
//!     | Difference::ArtifactTargets { left, right, .. }
//!     | Difference::Interfaces { left, right, .. }
//!     | Difference::ParticipantPackages { left, right, .. } => {
//!         let (mut left, mut right) = (left.clone(), right.clone());
//!         left.sort();
//!         right.sort();
//!         left != right
//!     }
//!     // And the cross-document references (ADR 0040).
//!     Difference::Hrefs { left, right, .. } => {
//!         let (mut left, mut right) = (left.clone(), right.clone());
//!         left.sort();
//!         right.sort();
//!         left != right
//!     }
//!     _ => true,
//! });
//! let same_ignoring_order = differences.is_empty();
//! assert!(same_ignoring_order);
//! # Ok::<(), sacm::edit::EditError>(())
//! ```
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
    AssertionDeclaration, Compared, Document, ElementId, ElementKind, Flag, Href, LangString,
    MultiLangString, Proxy, ReferenceEnd, SacmElement, StringAttribute, Text, TextRole,
};

/// One way in which two documents differ in SACM content, as
/// [`semantic_compare`] reports it.
///
/// - **Left and right** are the first and the second document given to
///   [`semantic_compare`].
/// - **One feature of one element** per variant, except the three about the
///   document: [`RootOrder`](Self::RootOrder),
///   [`DocumentMetadata`](Self::DocumentMetadata) and
///   [`DocumentPreservedContent`](Self::DocumentPreservedContent).
/// - **Look it up.** [`element_id`](Self::element_id) gives the id to pass to
///   [`Document::get`](crate::model::Document::get) on either side.
/// - **Variants are added** as the model holds more SACM features, so a
///   `match` needs a wildcard arm.
/// - **Fields stay.** A variant's fields stay as they are, `{id, left,
///   right}` for most, so you can build an expected value in a test: a new
///   comparison adds a variant, never a field (ADR 0045 D3).
/// - **`Display`** writes one line for people to read. Its wording is not API.
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
    /// The element has a different `gid` (SACM 2.3 §8.2) on each side;
    /// `None` is no gid.
    ///
    /// Gids are compared exactly: no gid, `""` and `" g"` are three
    /// different values.
    Gid {
        /// The element's id.
        id: ElementId,
        /// The gid in the left document, or `None` when it has none.
        left: Option<String>,
        /// The gid in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// The element's `isCitation` (SACM 2.3 §8.2) differs on each side.
    ///
    /// Absent is `false`. See
    /// [`SacmElement::is_citation`](crate::model::SacmElement::is_citation).
    IsCitation {
        /// The element's id.
        id: ElementId,
        /// The flag in the left document.
        left: bool,
        /// The flag in the right document.
        right: bool,
    },
    /// The element's `isAbstract` (SACM 2.3 §8.2) differs on each side.
    ///
    /// Absent is `false`. See
    /// [`SacmElement::is_abstract`](crate::model::SacmElement::is_abstract).
    IsAbstract {
        /// The element's id.
        id: ElementId,
        /// The flag in the left document.
        left: bool,
        /// The flag in the right document.
        right: bool,
    },
    /// The asserted relationship's `isCounter` (SACM 2.3 §11.13) differs on
    /// each side.
    ///
    /// Absent is `false`, and so is the flag of a kind that has none. See
    /// [`SacmElement::is_counter`](crate::model::SacmElement::is_counter).
    IsCounter {
        /// The element's id.
        id: ElementId,
        /// The flag in the left document.
        left: bool,
        /// The flag in the right document.
        right: bool,
    },
    /// The element's `citedElement` (SACM 2.3 §8.2) differs on each side;
    /// `None` is no cited element.
    ///
    /// References are compared by id, resolved or not: two references to
    /// ids that name no element differ exactly when the ids do. See
    /// [`SacmElement::cited_element`](crate::model::SacmElement::cited_element).
    CitedElement {
        /// The element's id.
        id: ElementId,
        /// The cited id in the left document, or `None`.
        left: Option<ElementId>,
        /// The cited id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The element's `abstractForm` (SACM 2.3 §8.2) differs on each side;
    /// `None` is no abstract form.
    ///
    /// Compared as [`CitedElement`](Self::CitedElement) is. See
    /// [`SacmElement::abstract_form`](crate::model::SacmElement::abstract_form).
    AbstractForm {
        /// The element's id.
        id: ElementId,
        /// The abstract form's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The abstract form's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The Assertion's `metaClaim` ids (SACM 2.3 §11.10) differ on each side.
    ///
    /// The ids are compared in order and with repeats (D-ORDER, ADR 0027):
    /// `[m1, m2]` and `[m2, m1]` differ, and so do `[m1]` and `[m1, m1]`. See
    /// [`SacmElement::meta_claims`](crate::model::SacmElement::meta_claims).
    MetaClaims {
        /// The element's id.
        id: ElementId,
        /// The meta-claim ids in the left document, in order.
        left: Vec<ElementId>,
        /// The meta-claim ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The asserted relationship's `reasoning` (SACM 2.3 §11.13) differs on
    /// each side; `None` is no reasoning.
    ///
    /// Compared as [`CitedElement`](Self::CitedElement) is. See
    /// [`SacmElement::reasoning`](crate::model::SacmElement::reasoning).
    Reasoning {
        /// The element's id.
        id: ElementId,
        /// The reasoning's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The reasoning's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The asserted relationship's `source` ids (SACM 2.3 §11.13) differ on
    /// each side.
    ///
    /// Compared in order and with repeats, as [`MetaClaims`](Self::MetaClaims)
    /// are. See [`SacmElement::sources`](crate::model::SacmElement::sources).
    Sources {
        /// The element's id.
        id: ElementId,
        /// The source ids in the left document, in order.
        left: Vec<ElementId>,
        /// The source ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The asserted relationship's `target` (SACM 2.3 §11.13) differs on
    /// each side; `None` is no target.
    ///
    /// Compared as [`CitedElement`](Self::CitedElement) is. See
    /// [`SacmElement::target`](crate::model::SacmElement::target).
    Target {
        /// The element's id.
        id: ElementId,
        /// The target's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The target's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `ArgumentReasoning`'s `structure` (SACM 2.3 §11.12) differs on
    /// each side; `None` is no structure.
    ///
    /// Compared as [`CitedElement`](Self::CitedElement) is. See
    /// [`SacmElement::structure`](crate::model::SacmElement::structure).
    Structure {
        /// The element's id.
        id: ElementId,
        /// The structure's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The structure's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `ArtifactReference`'s `referencedArtifactElement` ids (SACM 2.3
    /// §11.9) differ on each side.
    ///
    /// Compared in order and with repeats, as [`MetaClaims`](Self::MetaClaims)
    /// are. See
    /// [`SacmElement::referenced_artifact_elements`](crate::model::SacmElement::referenced_artifact_elements).
    ReferencedArtifactElements {
        /// The element's id.
        id: ElementId,
        /// The referenced ids in the left document, in order.
        left: Vec<ElementId>,
        /// The referenced ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `ArgumentGroup`'s members, its `argumentElement` ids (SACM 2.3
    /// §11.2), differ on each side.
    ///
    /// Compared in order and with repeats, as [`MetaClaims`](Self::MetaClaims)
    /// are. See
    /// [`SacmElement::group_members`](crate::model::SacmElement::group_members).
    #[doc(alias = "argumentElement")]
    GroupMembers {
        /// The group's id.
        id: ElementId,
        /// The member ids in the left document, in order.
        left: Vec<ElementId>,
        /// The member ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `ArtifactGroup`'s members, its `artifactElement` ids (SACM 2.3
    /// §12.3), differ on each side.
    ///
    /// Compared in order and with repeats, as [`MetaClaims`](Self::MetaClaims)
    /// are. See
    /// [`SacmElement::group_members`](crate::model::SacmElement::group_members).
    ///
    /// ```
    /// use sacm::compare::Difference;
    /// use sacm::model::ElementId;
    ///
    /// let difference = Difference::ArtifactGroupMembers {
    ///     id: ElementId::from("analysis"),
    ///     left: vec![ElementId::from("fta")],
    ///     right: vec![],
    /// };
    /// assert_eq!(
    ///     difference.to_string(),
    ///     r#"element "analysis" differs in artifactElement: ["fta"] on the left, [] on the right"#
    /// );
    /// ```
    #[doc(alias = "artifactElement")]
    ArtifactGroupMembers {
        /// The group's id.
        id: ElementId,
        /// The member ids in the left document, in order.
        left: Vec<ElementId>,
        /// The member ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `ArtifactAssetRelationship`'s sources, its `source` ids (SACM 2.3
    /// §12.14), differ on each side.
    ///
    /// - **Compared** in order and with repeats, as
    ///   [`MetaClaims`](Self::MetaClaims) are. See
    ///   [`SacmElement::sources`](crate::model::SacmElement::sources).
    /// - **An asserted relationship's** are [`Sources`](Self::Sources).
    #[doc(alias = "source")]
    ArtifactSources {
        /// The relationship's id.
        id: ElementId,
        /// The source ids in the left document, in order.
        left: Vec<ElementId>,
        /// The source ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `ArtifactAssetRelationship`'s targets, its `target` ids (SACM 2.3
    /// §12.14), differ on each side.
    ///
    /// - **Compared** in order and with repeats, as
    ///   [`MetaClaims`](Self::MetaClaims) are. See
    ///   [`SacmElement::targets`](crate::model::SacmElement::targets).
    /// - **An asserted relationship's** one target is [`Target`](Self::Target).
    ///
    /// ```
    /// use sacm::compare::Difference;
    /// use sacm::model::ElementId;
    ///
    /// let difference = Difference::ArtifactTargets {
    ///     id: ElementId::from("rel"),
    ///     left: vec![ElementId::from("testing")],
    ///     right: vec![ElementId::from("testing"), ElementId::from("tester")],
    /// };
    /// assert_eq!(
    ///     difference.to_string(),
    ///     r#"element "rel" differs in target: ["testing"] on the left, ["testing", "tester"] on the right"#
    /// );
    /// ```
    #[doc(alias = "target")]
    ArtifactTargets {
        /// The relationship's id.
        id: ElementId,
        /// The target ids in the left document, in order.
        left: Vec<ElementId>,
        /// The target ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `category` ids of a Category, a Term or an Expression (SACM 2.3
    /// §10.8, §10.9) differ on each side.
    ///
    /// Compared in order and with repeats, as [`MetaClaims`](Self::MetaClaims)
    /// are. See
    /// [`SacmElement::categories`](crate::model::SacmElement::categories).
    ///
    /// ```
    /// use sacm::compare::Difference;
    /// use sacm::model::ElementId;
    ///
    /// let difference = Difference::Categories {
    ///     id: ElementId::from("mechanical"),
    ///     left: vec![ElementId::from("hazards")],
    ///     right: vec![],
    /// };
    /// assert_eq!(
    ///     difference.to_string(),
    ///     r#"element "mechanical" differs in category: ["hazards"] on the left, [] on the right"#
    /// );
    /// ```
    #[doc(alias = "category")]
    Categories {
        /// The element's id.
        id: ElementId,
        /// The category ids in the left document, in order.
        left: Vec<ElementId>,
        /// The category ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The `ExpressionElements` an Expression is formed from, its `element`
    /// ids (SACM 2.3 §10.10), differ on each side.
    ///
    /// Compared in order and with repeats. See
    /// [`SacmElement::expression_elements`](crate::model::SacmElement::expression_elements).
    #[doc(alias = "element")]
    ExpressionElements {
        /// The Expression's id.
        id: ElementId,
        /// The element ids in the left document, in order.
        left: Vec<ElementId>,
        /// The element ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// A Term's `origin` (SACM 2.3 §10.11) differs on each side; `None` is no
    /// origin.
    ///
    /// See [`SacmElement::origin`](crate::model::SacmElement::origin).
    Origin {
        /// The Term's id.
        id: ElementId,
        /// The origin's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The origin's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `TerminologyGroup`'s members, its `terminologyElement` ids (SACM
    /// 2.3 §10.3), differ on each side.
    ///
    /// - **Compared** in order and with repeats. See
    ///   [`SacmElement::group_members`](crate::model::SacmElement::group_members).
    /// - **An `ArgumentGroup`'s** are [`GroupMembers`](Self::GroupMembers).
    #[doc(alias = "terminologyElement")]
    TerminologyGroupMembers {
        /// The group's id.
        id: ElementId,
        /// The member ids in the left document, in order.
        left: Vec<ElementId>,
        /// The member ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// A package's interfaces, its `interface` ids, differ on each side.
    ///
    /// - **Any family.** An Assurance Case package (§9.2), or a Terminology,
    ///   Argumentation or Artifact package (ptc/22-03-13).
    /// - **Compared** in order and with repeats. See
    ///   [`SacmElement::interfaces`](crate::model::SacmElement::interfaces).
    #[doc(alias = "interface")]
    Interfaces {
        /// The package's id.
        id: ElementId,
        /// The interface ids in the left document, in order.
        left: Vec<ElementId>,
        /// The interface ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// An interface's `implements` differs on each side; `None` is no
    /// `implements`.
    ///
    /// On an interface of any package family (SACM 2.3 §9.3, §10.5, §11.6,
    /// §12.5). See
    /// [`SacmElement::implements`](crate::model::SacmElement::implements).
    Implements {
        /// The interface's id.
        id: ElementId,
        /// The package's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The package's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// A binding's participants, its `participantPackage` ids, differ on each
    /// side.
    ///
    /// - **Any family.** A binding of any package family (SACM 2.3 §9.4,
    ///   §11.5, §12.4; ptc/22-03-13 for a `TerminologyPackageBinding`).
    /// - **Compared** in order and with repeats. See
    ///   [`SacmElement::participant_packages`](crate::model::SacmElement::participant_packages).
    #[doc(alias = "participantPackage")]
    ParticipantPackages {
        /// The binding's id.
        id: ElementId,
        /// The participant ids in the left document, in order.
        left: Vec<ElementId>,
        /// The participant ids in the right document, in order.
        right: Vec<ElementId>,
    },
    /// The cross-document references (`href`s) of one end of an element
    /// differ, in order (SACM23-XMI-027, ADR 0040 D8).
    ///
    /// One per end that differs, in [`ReferenceEnd::ALL`] order. See
    /// [`SacmElement::hrefs`](crate::model::SacmElement::hrefs).
    #[doc(alias = "href")]
    Hrefs {
        /// The element's id.
        id: ElementId,
        /// The end.
        end: ReferenceEnd,
        /// The `href`s in the left document, in order.
        left: Vec<Href>,
        /// The `href`s in the right document, in order.
        right: Vec<Href>,
    },
    /// The proxies of an element differ, in order: its children held in
    /// other documents (SACM23-XMI-029, ADR 0040 amendment 1, A6).
    ///
    /// Compared exactly: the role and the `href` of each, with repeats. See
    /// [`SacmElement::proxies`](crate::model::SacmElement::proxies).
    #[doc(alias = "href")]
    Proxies {
        /// The element's id.
        id: ElementId,
        /// The proxies in the left document, in order.
        left: Vec<Proxy>,
        /// The proxies in the right document, in order.
        right: Vec<Proxy>,
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
    /// The `xmi:id` of the element's name differs on each side; `None` is a
    /// name without an id, or no name.
    ///
    /// A name's id is compared because equal documents must write equal
    /// bytes, and `write` writes it (ADR 0024). See
    /// [`SacmElement::name_id`](crate::model::SacmElement::name_id).
    NameId {
        /// The id of the element that owns the name.
        id: ElementId,
        /// The name's id in the left document, or `None` when it has none.
        left: Option<ElementId>,
        /// The name's id in the right document, or `None` when it has none.
        right: Option<ElementId>,
    },
    /// The `content` of a `UtilityElement` (SACM 2.3 §8.7) differs on each
    /// side; `None` is no content.
    ///
    /// Contents are compared exactly and in order: no content, an empty one,
    /// `[en, fr]` and `[fr, en]` are all different values. See
    /// [`SacmElement::content`](crate::model::SacmElement::content).
    Content {
        /// The element's id.
        id: ElementId,
        /// The content in the left document, or `None` when it has none.
        left: Option<MultiLangString>,
        /// The content in the right document, or `None` when it has none.
        right: Option<MultiLangString>,
    },
    /// The `xmi:id` of the element's `content` differs on each side; `None`
    /// is a content without an id, or none.
    ///
    /// See [`SacmElement::content_id`](crate::model::SacmElement::content_id).
    ContentId {
        /// The element's id.
        id: ElementId,
        /// The content's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The content's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `xmi:id`s of the values of the element's `content`, by position,
    /// differ on each side.
    ///
    /// A position one side lacks counts as a value without an id, so a
    /// different number of values without ids is only a
    /// [`Content`](Self::Content) difference. See
    /// [`SacmElement::content_value_ids`](crate::model::SacmElement::content_value_ids).
    ContentValueIds {
        /// The element's id.
        id: ElementId,
        /// The value ids in the left document: one per value.
        left: Vec<Option<ElementId>>,
        /// The value ids in the right document: one per value.
        right: Vec<Option<ElementId>>,
    },
    /// The `key` of a `TaggedValue` (SACM 2.3 §8.12) differs on each side;
    /// `None` is no key.
    ///
    /// Compared as [`Content`](Self::Content) is. See
    /// [`SacmElement::key`](crate::model::SacmElement::key).
    Key {
        /// The element's id.
        id: ElementId,
        /// The key in the left document, or `None` when it has none.
        left: Option<MultiLangString>,
        /// The key in the right document, or `None` when it has none.
        right: Option<MultiLangString>,
    },
    /// The `xmi:id` of the element's `key` differs on each side; `None` is a
    /// key without an id, or none.
    ///
    /// See [`SacmElement::key_id`](crate::model::SacmElement::key_id).
    KeyId {
        /// The element's id.
        id: ElementId,
        /// The key's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The key's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `xmi:id`s of the values of the element's `key`, by position,
    /// differ on each side.
    ///
    /// Compared as [`ContentValueIds`](Self::ContentValueIds) are. See
    /// [`SacmElement::key_value_ids`](crate::model::SacmElement::key_value_ids).
    KeyValueIds {
        /// The element's id.
        id: ElementId,
        /// The value ids in the left document: one per value.
        left: Vec<Option<ElementId>>,
        /// The value ids in the right document: one per value.
        right: Vec<Option<ElementId>>,
    },
    /// The `version` of an `Artifact` (SACM 2.3 §12.7) differs on each side;
    /// `None` is no version.
    ///
    /// Versions are compared exactly, as gids are: no version, `""` and
    /// `" 1.2"` are three different values. See
    /// [`SacmElement::version`](crate::model::SacmElement::version).
    Version {
        /// The element's id.
        id: ElementId,
        /// The version in the left document, or `None` when it has none.
        left: Option<String>,
        /// The version in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// The `date` of an `Artifact` or an `Event` (SACM 2.3 §12.7, §12.9)
    /// differs on each side; `None` is no date.
    ///
    /// Dates are held as written (D-DATE) and compared exactly: `2024-03-01`
    /// and `2024-03-01 ` differ, and so do no date and `""`. See
    /// [`SacmElement::date`](crate::model::SacmElement::date).
    Date {
        /// The element's id.
        id: ElementId,
        /// The date in the left document, or `None` when it has none.
        left: Option<String>,
        /// The date in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// The `location` of a `Resource` (SACM 2.3 §12.10) differs on each side;
    /// `None` is no location.
    ///
    /// Compared as [`Content`](Self::Content) is. See
    /// [`SacmElement::location`](crate::model::SacmElement::location).
    Location {
        /// The element's id.
        id: ElementId,
        /// The location in the left document, or `None` when it has none.
        left: Option<MultiLangString>,
        /// The location in the right document, or `None` when it has none.
        right: Option<MultiLangString>,
    },
    /// The `xmi:id` of the element's `location` differs on each side; `None`
    /// is a location without an id, or none.
    ///
    /// See [`SacmElement::location_id`](crate::model::SacmElement::location_id).
    LocationId {
        /// The element's id.
        id: ElementId,
        /// The location's id in the left document, or `None`.
        left: Option<ElementId>,
        /// The location's id in the right document, or `None`.
        right: Option<ElementId>,
    },
    /// The `xmi:id`s of the values of the element's `location`, by position,
    /// differ on each side.
    ///
    /// Compared as [`ContentValueIds`](Self::ContentValueIds) are. See
    /// [`SacmElement::location_value_ids`](crate::model::SacmElement::location_value_ids).
    LocationValueIds {
        /// The element's id.
        id: ElementId,
        /// The value ids in the left document: one per value.
        left: Vec<Option<ElementId>>,
        /// The value ids in the right document: one per value.
        right: Vec<Option<ElementId>>,
    },
    /// The `startTime` of an `Activity` (SACM 2.3 §12.11) differs on each
    /// side; `None` is none.
    ///
    /// Compared exactly, as [`Date`](Self::Date) is. See
    /// [`SacmElement::start_time`](crate::model::SacmElement::start_time).
    StartTime {
        /// The element's id.
        id: ElementId,
        /// The start time in the left document, or `None` when it has none.
        left: Option<String>,
        /// The start time in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// The `endTime` of an `Activity` (SACM 2.3 §12.11) differs on each side;
    /// `None` is none.
    ///
    /// Compared exactly, as [`Date`](Self::Date) is. See
    /// [`SacmElement::end_time`](crate::model::SacmElement::end_time).
    EndTime {
        /// The element's id.
        id: ElementId,
        /// The end time in the left document, or `None` when it has none.
        left: Option<String>,
        /// The end time in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// The `value` of a Term or an Expression (SACM 2.3 §10.9) differs on
    /// each side; `None` is no value.
    ///
    /// Compared exactly, as [`Version`](Self::Version) is. See
    /// [`SacmElement::value`](crate::model::SacmElement::value).
    ///
    /// ```
    /// use sacm::compare::Difference;
    /// use sacm::model::ElementId;
    ///
    /// let difference = Difference::Value {
    ///     id: ElementId::from("blade"),
    ///     left: Some("blade".to_owned()),
    ///     right: None,
    /// };
    /// assert_eq!(
    ///     difference.to_string(),
    ///     r#"element "blade" differs in value: "blade" on the left, no value on the right"#
    /// );
    /// ```
    Value {
        /// The element's id.
        id: ElementId,
        /// The value in the left document, or `None` when it has none.
        left: Option<String>,
        /// The value in the right document, or `None` when it has none.
        right: Option<String>,
    },
    /// A Term's `externalReference` (SACM 2.3 §10.11) differs on each side;
    /// `None` is none.
    ///
    /// Compared exactly. See
    /// [`SacmElement::external_reference`](crate::model::SacmElement::external_reference).
    ExternalReference {
        /// The Term's id.
        id: ElementId,
        /// The external reference in the left document, or `None`.
        left: Option<String>,
        /// The external reference in the right document, or `None`.
        right: Option<String>,
    },
    /// The element has a different `assertionDeclaration` (SACM 2.3 §11.10)
    /// on each side.
    ///
    /// `None` means the kind on that side is not an Assertion (an Assertion
    /// is a Claim or one of the five asserted-relationship kinds). So a `None`
    /// side always comes with a [`Kind`](Self::Kind) difference for the same id.
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
    /// The XMI metadata of the element, or of its name, texts or values,
    /// differs, or one side holds none (SACM23-XMI-019, ADR 0042).
    ///
    /// Compared exactly and in order, as
    /// [`PreservedContent`](Self::PreservedContent) is: `xmi:uuid`,
    /// `xmi:label`, the XSI schema locations and the `xmi:Extension`s, with
    /// where each stands among the element's children.
    Metadata {
        /// The element's id.
        id: ElementId,
    },
    /// The element's preserved content differs on each side, or one side
    /// holds none (LIB-COMPAT-003, ADR 0033, ADR 0035).
    ///
    /// - **What it is.** What [`xmi::read`](crate::xmi::read) kept as written
    ///   instead of read: content that is not SACM 2.3 where it stands or, in
    ///   tolerant mode, that a strict read refuses.
    /// - **Compared exactly and in order.** Namespace URIs, prefixes, local
    ///   names, attribute values and order, text, and where each child stands
    ///   among the element's children.
    /// - **A declared subclass.** Its class is compared too, its prefix
    ///   included (LIB-COMPAT-017).
    /// - **Never differ.** Comments and processing instructions, which are not
    ///   held.
    PreservedContent {
        /// The element's id.
        id: ElementId,
    },
    /// The XMI metadata outside every element differs on each side, or one
    /// side holds none (SACM23-XMI-019, ADR 0042).
    ///
    /// - **What it is.** The metadata attributes of the `xmi:XMI` element,
    ///   such as its `xmi:uuid`, and the XMI elements under it, such as an
    ///   `xmi:Documentation`, with where each stands among the roots.
    /// - **Compared** as [`Metadata`](Self::Metadata) is.
    /// - **No element holds it,** so [`element_id`](Difference::element_id)
    ///   is `None`.
    DocumentMetadata,
    /// The preserved content outside every element differs on each side, or
    /// one side holds none (ADR 0035 D3).
    ///
    /// - **What it is.** What [`xmi::read`](crate::xmi::read) kept of the
    ///   `xmi:XMI` element besides its roots and its XMI metadata, such as a
    ///   vendor attribute on it.
    /// - **Compared** as [`PreservedContent`](Self::PreservedContent) is, with
    ///   where each child stands among the roots.
    /// - **No element holds it,** so [`element_id`](Difference::element_id)
    ///   is `None`.
    DocumentPreservedContent,
}

impl Difference {
    /// The element the difference is about: the parent for
    /// [`ChildOrder`](Self::ChildOrder).
    ///
    /// `None` for the three about the document:
    /// [`RootOrder`](Self::RootOrder),
    /// [`DocumentMetadata`](Self::DocumentMetadata) and
    /// [`DocumentPreservedContent`](Self::DocumentPreservedContent).
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
            Self::RootOrder { .. } | Self::DocumentMetadata | Self::DocumentPreservedContent => {
                None
            }
            Self::LeftOnly { id, .. }
            | Self::RightOnly { id, .. }
            | Self::Kind { id, .. }
            | Self::Parent { id, .. }
            | Self::Gid { id, .. }
            | Self::IsCitation { id, .. }
            | Self::IsAbstract { id, .. }
            | Self::IsCounter { id, .. }
            | Self::CitedElement { id, .. }
            | Self::AbstractForm { id, .. }
            | Self::MetaClaims { id, .. }
            | Self::Reasoning { id, .. }
            | Self::Sources { id, .. }
            | Self::Target { id, .. }
            | Self::Structure { id, .. }
            | Self::ReferencedArtifactElements { id, .. }
            | Self::GroupMembers { id, .. }
            | Self::ArtifactGroupMembers { id, .. }
            | Self::ArtifactSources { id, .. }
            | Self::ArtifactTargets { id, .. }
            | Self::Categories { id, .. }
            | Self::ExpressionElements { id, .. }
            | Self::Origin { id, .. }
            | Self::TerminologyGroupMembers { id, .. }
            | Self::Interfaces { id, .. }
            | Self::Implements { id, .. }
            | Self::ParticipantPackages { id, .. }
            | Self::Hrefs { id, .. }
            | Self::Proxies { id, .. }
            | Self::Name { id, .. }
            | Self::NameId { id, .. }
            | Self::Content { id, .. }
            | Self::ContentId { id, .. }
            | Self::ContentValueIds { id, .. }
            | Self::Key { id, .. }
            | Self::KeyId { id, .. }
            | Self::KeyValueIds { id, .. }
            | Self::Version { id, .. }
            | Self::Date { id, .. }
            | Self::Location { id, .. }
            | Self::LocationId { id, .. }
            | Self::LocationValueIds { id, .. }
            | Self::StartTime { id, .. }
            | Self::EndTime { id, .. }
            | Self::Value { id, .. }
            | Self::ExternalReference { id, .. }
            | Self::AssertionDeclaration { id, .. }
            | Self::ChildOrder { id, .. }
            | Self::Metadata { id }
            | Self::PreservedContent { id } => Some(id),
        }
    }
}

impl Difference {
    /// The role a difference in a `content`, `key` or `location` is about:
    /// `key` for the three `Key` variants, `location` for the three
    /// `Location` variants, and `content` otherwise.
    fn text_role(&self) -> &'static str {
        match self {
            Self::Key { .. } | Self::KeyId { .. } | Self::KeyValueIds { .. } => {
                TextRole::Key.name()
            }
            Self::Location { .. } | Self::LocationId { .. } | Self::LocationValueIds { .. } => {
                TextRole::Location.name()
            }
            _ => TextRole::Content.name(),
        }
    }

    /// The string-valued attribute a difference in one is about, by its XMI
    /// name: `date`, `startTime` and `endTime` for theirs, and `version`
    /// otherwise.
    fn string(&self) -> &'static str {
        match self {
            Self::Date { .. } => StringAttribute::Date.name(),
            Self::StartTime { .. } => StringAttribute::StartTime.name(),
            Self::EndTime { .. } => StringAttribute::EndTime.name(),
            Self::Value { .. } => StringAttribute::Value.name(),
            Self::ExternalReference { .. } => StringAttribute::ExternalReference.name(),
            _ => StringAttribute::Version.name(),
        }
    }

    /// The flag a difference in one is about, by its XMI name:
    /// `isAbstract` and `isCounter` for theirs, and `isCitation` otherwise.
    fn flag(&self) -> &'static str {
        match self {
            Self::IsAbstract { .. } => Flag::IsAbstract.name(),
            Self::IsCounter { .. } => Flag::IsCounter.name(),
            _ => Flag::IsCitation.name(),
        }
    }

    /// The reference end a difference in one is about, by its XMI name:
    /// `citedElement` for its own and for any variant that is about no end.
    fn end(&self) -> &'static str {
        match self {
            Self::AbstractForm { .. } => ReferenceEnd::AbstractForm.name(),
            Self::MetaClaims { .. } => ReferenceEnd::MetaClaim.name(),
            Self::Reasoning { .. } => ReferenceEnd::Reasoning.name(),
            Self::Sources { .. } => ReferenceEnd::Source.name(),
            Self::Target { .. } => ReferenceEnd::Target.name(),
            Self::Structure { .. } => ReferenceEnd::Structure.name(),
            Self::ReferencedArtifactElements { .. } => {
                ReferenceEnd::ReferencedArtifactElement.name()
            }
            Self::GroupMembers { .. } => ReferenceEnd::GroupMember.name(),
            Self::ArtifactGroupMembers { .. } => ReferenceEnd::ArtifactGroupMember.name(),
            Self::ArtifactSources { .. } => ReferenceEnd::ArtifactSource.name(),
            Self::ArtifactTargets { .. } => ReferenceEnd::ArtifactTarget.name(),
            Self::Categories { .. } => ReferenceEnd::Category.name(),
            Self::ExpressionElements { .. } => ReferenceEnd::Element.name(),
            Self::Origin { .. } => ReferenceEnd::Origin.name(),
            Self::TerminologyGroupMembers { .. } => ReferenceEnd::TerminologyGroupMember.name(),
            Self::Interfaces { .. } => ReferenceEnd::Interface.name(),
            Self::Implements { .. } => ReferenceEnd::Implements.name(),
            Self::ParticipantPackages { .. } => ReferenceEnd::ParticipantPackage.name(),
            _ => ReferenceEnd::CitedElement.name(),
        }
    }
}

impl fmt::Display for Difference {
    /// One line: the quoted id, the SACM feature and both values. Ids and
    /// names are quoted with quotes and line breaks escaped, so the line
    /// never breaks.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per variant reads best as one match"
    )]
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
            Self::Gid { id, left, right } => write!(
                f,
                "element {:?} differs in gid: {} on the left, {} on the right",
                id.as_str(),
                Gid(left.as_deref()),
                Gid(right.as_deref())
            ),
            Self::IsCitation { id, left, right }
            | Self::IsAbstract { id, left, right }
            | Self::IsCounter { id, left, right } => write!(
                f,
                "element {:?} differs in {}: {left} on the left, {right} on the right",
                id.as_str(),
                self.flag()
            ),
            Self::CitedElement { id, left, right }
            | Self::AbstractForm { id, left, right }
            | Self::Reasoning { id, left, right }
            | Self::Target { id, left, right }
            | Self::Structure { id, left, right }
            | Self::Origin { id, left, right }
            | Self::Implements { id, left, right } => {
                let end = self.end();
                write!(
                    f,
                    "element {:?} differs in {end}: {} on the left, {} on the right",
                    id.as_str(),
                    Target(left.as_ref(), end),
                    Target(right.as_ref(), end)
                )
            }
            Self::MetaClaims { id, left, right }
            | Self::Sources { id, left, right }
            | Self::ReferencedArtifactElements { id, left, right }
            | Self::GroupMembers { id, left, right }
            | Self::ArtifactGroupMembers { id, left, right }
            | Self::ArtifactSources { id, left, right }
            | Self::ArtifactTargets { id, left, right }
            | Self::Categories { id, left, right }
            | Self::ExpressionElements { id, left, right }
            | Self::TerminologyGroupMembers { id, left, right }
            | Self::Interfaces { id, left, right }
            | Self::ParticipantPackages { id, left, right } => write!(
                f,
                "element {:?} differs in {}: {} on the left, {} on the right",
                id.as_str(),
                self.end(),
                Ids(left),
                Ids(right)
            ),
            Self::Hrefs {
                id,
                end,
                left,
                right,
            } => write!(
                f,
                "element {:?} differs in the hrefs of {}: {} on the left, {} on the right",
                id.as_str(),
                end.name(),
                Hrefs(left),
                Hrefs(right)
            ),
            Self::Proxies { id, left, right } => write!(
                f,
                "element {:?} differs in its proxies: {} on the left, {} on the right",
                id.as_str(),
                Proxies(left),
                Proxies(right)
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
            Self::Content { id, left, right }
            | Self::Key { id, left, right }
            | Self::Location { id, left, right } => {
                let role = self.text_role();
                write!(
                    f,
                    "element {:?} differs in {role}: {} on the left, {} on the right",
                    id.as_str(),
                    Values(left.as_ref(), role),
                    Values(right.as_ref(), role)
                )
            }
            Self::ContentId { id, left, right }
            | Self::KeyId { id, left, right }
            | Self::LocationId { id, left, right } => {
                let role = self.text_role();
                write!(
                    f,
                    "element {:?} differs in the xmi:id of its {role}: {} on the left, {} on the right",
                    id.as_str(),
                    NameId(left.as_ref()),
                    NameId(right.as_ref())
                )
            }
            Self::Version { id, left, right }
            | Self::Date { id, left, right }
            | Self::StartTime { id, left, right }
            | Self::EndTime { id, left, right }
            | Self::Value { id, left, right }
            | Self::ExternalReference { id, left, right } => {
                let attribute = self.string();
                write!(
                    f,
                    "element {:?} differs in {attribute}: {} on the left, {} on the right",
                    id.as_str(),
                    Quoted(left.as_deref(), attribute),
                    Quoted(right.as_deref(), attribute)
                )
            }
            Self::ContentValueIds { id, left, right }
            | Self::KeyValueIds { id, left, right }
            | Self::LocationValueIds { id, left, right } => {
                let role = self.text_role();
                write!(
                    f,
                    "element {:?} differs in the xmi:ids of its {role}'s values: {} on the left, {} on the right",
                    id.as_str(),
                    ValueIds(left),
                    ValueIds(right)
                )
            }
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
            Self::Metadata { id } => {
                write!(f, "element {:?} differs in its XMI metadata", id.as_str())
            }
            Self::PreservedContent { id } => write!(
                f,
                "element {:?} differs in its preserved content",
                id.as_str()
            ),
            Self::DocumentMetadata => {
                f.write_str("the documents differ in their XMI metadata outside every element")
            }
            Self::DocumentPreservedContent => {
                f.write_str("the documents differ in their preserved content outside every element")
            }
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

/// A list of `href`s for `Display`: each quoted, as [`Ids`] writes ids.
struct Hrefs<'a>(&'a [Href]);

impl fmt::Display for Hrefs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (position, href) in self.0.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{:?}", href.as_str())?;
        }
        f.write_str("]")
    }
}

/// A list of proxies for `Display`: each as its role and its quoted `href`,
/// such as `argumentPackage "o.xmi#a"`.
struct Proxies<'a>(&'a [Proxy]);

impl fmt::Display for Proxies<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (position, proxy) in self.0.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{} {:?}", proxy.role(), proxy.href().as_str())?;
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

/// A gid for `Display`: quoted, or `no gid`, so absent and empty read
/// differently.
struct Gid<'a>(Option<&'a str>);

impl fmt::Display for Gid<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(gid) => write!(f, "{gid:?}"),
            None => f.write_str("no gid"),
        }
    }
}

/// A string-valued attribute for `Display`: quoted, or `no <attribute>`, so
/// absent and empty read differently.
struct Quoted<'a>(Option<&'a str>, &'static str);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => write!(f, "{value:?}"),
            None => write!(f, "no {}", self.1),
        }
    }
}

/// A reference target for `Display`: its quoted id, or `no <end>`.
struct Target<'a>(Option<&'a ElementId>, &'static str);

impl fmt::Display for Target<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(id) => write!(f, "{:?}", id.as_str()),
            None => write!(f, "no {}", self.1),
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
        write!(f, "{}", Value(name))
    }
}

/// A `content` or `key` for `Display`: `no <role>` when absent, otherwise
/// its values in order, `[]` when empty.
struct Values<'a>(Option<&'a MultiLangString>, &'static str);

impl fmt::Display for Values<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(values) = self.0 else {
            return write!(f, "no {}", self.1);
        };
        f.write_str("[")?;
        for (position, value) in values.values().iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", Value(value))?;
        }
        f.write_str("]")
    }
}

/// The ids of a text's values for `Display`: `[no id, "v1"]`.
struct ValueIds<'a>(&'a [Option<ElementId>]);

impl fmt::Display for ValueIds<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (position, id) in self.0.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", NameId(id.as_ref()))?;
        }
        f.write_str("]")
    }
}

/// A `LangString` for `Display`: both fields, each quoted or written as
/// absent, so absent and empty read differently; and for an
/// `ExpressionLangString` its `expression`, quoted, or `no expression`
/// (SACM23-BASE-009).
struct Value<'a>(&'a LangString);

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.0;
        match name.lang() {
            Some(lang) => write!(f, "[lang {lang:?}, ")?,
            None => f.write_str("[no lang, ")?,
        }
        match name.content() {
            Some(content) => write!(f, "content {content:?}")?,
            None => f.write_str("no content")?,
        }
        match (name.expression(), name.expression_href()) {
            (Some(expression), _) => write!(f, ", expression {:?}]", expression.as_str()),
            (None, Some(href)) => write!(f, ", expression href {:?}]", href.as_str()),
            (None, None) if name.is_expression_lang_string() => f.write_str(", no expression]"),
            (None, None) => f.write_str("]"),
        }
    }
}

/// The id of a name, a text or a value for `Display`: quoted, or `no id`.
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
/// - **Matched by id.** Elements are matched across the whole document.
/// - **What is compared.** The [module docs](crate::compare#what-is-compared)
///   list it. Every feature of an element both documents hold is compared,
///   exactly, even when its kind differs.
/// - **Not compared.** The per-kind counters that generate ids.
/// - **Swapping the arguments** gives the same differences with left and
///   right swapped, though possibly in another order.
/// - **Safe on deep documents.** The comparison does not recurse, and takes
///   time linear in the number of elements.
///
/// # Comparing without order
///
/// Order is compared because equal documents must write equal bytes, though
/// SACM 2.3 orders no composition and no reference end.
///
/// To compare without it, drop the two order variants, and compare the
/// multi-valued ends sorted. The
/// [module example](crate::compare#comparing-without-order) shows two
/// documents that differ only in order.
///
/// ```
/// # use sacm::compare::{Difference, semantic_compare};
/// # let (left, right) = (sacm::model::Document::new(), sacm::model::Document::new());
/// let mut differences = semantic_compare(&left, &right);
/// differences.retain(|d| match d {
///     Difference::RootOrder { .. } | Difference::ChildOrder { .. } => false,
///     Difference::MetaClaims { left, right, .. }
///     | Difference::Sources { left, right, .. }
///     | Difference::ReferencedArtifactElements { left, right, .. }
///     | Difference::GroupMembers { left, right, .. }
///     | Difference::ArtifactGroupMembers { left, right, .. }
///     | Difference::ArtifactSources { left, right, .. }
///     | Difference::ArtifactTargets { left, right, .. } => {
///         let (mut left, mut right) = (left.clone(), right.clone());
///         left.sort();
///         right.sort();
///         left != right
///     }
///     _ => true,
/// });
/// let same_ignoring_order = differences.is_empty();
/// # assert!(same_ignoring_order);
/// ```
///
/// # Order of the result
///
/// 1. **The document.** [`Difference::RootOrder`], then
///    [`Difference::DocumentMetadata`], then
///    [`Difference::DocumentPreservedContent`].
/// 2. **Each element of `left`,** in pre-order. [`Difference::LeftOnly`], or
///    else its differences in the order of the table below.
/// 3. **Each element only `right` holds,** in `right`'s pre-order.
///    [`Difference::RightOnly`].
///
/// The differences of one element, reading the rows top to bottom and each
/// row left to right:
///
/// | Group | Variants, in order |
/// |---|---|
/// | Identity and place | [`Kind`](Difference::Kind), [`Parent`](Difference::Parent), [`Gid`](Difference::Gid) |
/// | Flags | [`IsCitation`](Difference::IsCitation), [`IsAbstract`](Difference::IsAbstract), [`IsCounter`](Difference::IsCounter) |
/// | `SACMElement` references | [`CitedElement`](Difference::CitedElement), [`AbstractForm`](Difference::AbstractForm) |
/// | Argumentation references | [`MetaClaims`](Difference::MetaClaims), [`Reasoning`](Difference::Reasoning), [`Sources`](Difference::Sources), [`Target`](Difference::Target), [`Structure`](Difference::Structure), [`ReferencedArtifactElements`](Difference::ReferencedArtifactElements), [`GroupMembers`](Difference::GroupMembers) |
/// | Artifact references | [`ArtifactGroupMembers`](Difference::ArtifactGroupMembers), [`ArtifactSources`](Difference::ArtifactSources), [`ArtifactTargets`](Difference::ArtifactTargets) |
/// | Terminology references | [`Categories`](Difference::Categories), [`ExpressionElements`](Difference::ExpressionElements), [`Origin`](Difference::Origin), [`TerminologyGroupMembers`](Difference::TerminologyGroupMembers) |
/// | Package references | [`Interfaces`](Difference::Interfaces), [`Implements`](Difference::Implements), [`ParticipantPackages`](Difference::ParticipantPackages) |
/// | Cross-document | [`Hrefs`](Difference::Hrefs), one per end in [`ReferenceEnd::ALL`] order, then [`Proxies`](Difference::Proxies) |
/// | Name and texts | [`Name`](Difference::Name), [`NameId`](Difference::NameId), [`Content`](Difference::Content), [`ContentId`](Difference::ContentId), [`ContentValueIds`](Difference::ContentValueIds), [`Key`](Difference::Key), [`KeyId`](Difference::KeyId), [`KeyValueIds`](Difference::KeyValueIds) |
/// | Artifact values | [`Version`](Difference::Version), [`Date`](Difference::Date), [`Location`](Difference::Location), [`LocationId`](Difference::LocationId), [`LocationValueIds`](Difference::LocationValueIds), [`StartTime`](Difference::StartTime), [`EndTime`](Difference::EndTime) |
/// | Terminology values | [`Value`](Difference::Value), [`ExternalReference`](Difference::ExternalReference) |
/// | Declaration, order and the rest | [`AssertionDeclaration`](Difference::AssertionDeclaration), [`ChildOrder`](Difference::ChildOrder), [`Metadata`](Difference::Metadata), [`PreservedContent`](Difference::PreservedContent) |
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

/// The ids that differ between two documents, as [`changes`] finds them.
///
/// It says, for any two documents, such as before and after restoring a
/// file, what a [`BatchMutation`](crate::edit::BatchMutation)'s net lists
/// say about a batch.
///
/// - [`created_ids`](Self::created_ids): only `after` holds the id.
/// - [`changed_ids`](Self::changed_ids): both hold it, and
///   [`semantic_compare`] reports a difference of the element itself.
/// - [`deleted_ids`](Self::deleted_ids): only `before` holds it.
/// - [`document_changed`](Self::document_changed): the order of the roots,
///   or the XMI metadata or preserved content outside every element,
///   differs.
///
/// With the `serde` feature it is
/// `{"created_ids", "changed_ids", "deleted_ids", "document_changed"}`,
/// every key required: a record (ADR 0030 amendment 8).
///
/// # Examples
///
/// ```
/// use sacm::compare::changes;
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetName};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut before = Document::new();
/// edit::apply(&mut before, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// edit::apply(&mut before, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
///
/// let mut after = before.clone();
/// edit::apply(&mut after, SetName::new("c1", LangString::new("C1 changed")))?;
/// edit::apply(&mut after, CreateClaim::new("p", LangString::new("C2")).with_id("c2"))?;
///
/// let found = changes(&before, &after);
/// assert_eq!(found.created_ids(), [ElementId::from("c2")]);
/// assert_eq!(found.changed_ids(), [ElementId::from("c1")]);
/// assert!(found.deleted_ids().is_empty() && !found.document_changed());
/// assert!(changes(&before, &before.clone()).is_empty());
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct Changes {
    created_ids: Vec<ElementId>,
    changed_ids: Vec<ElementId>,
    deleted_ids: Vec<ElementId>,
    document_changed: bool,
}

impl Changes {
    /// The ids only the document after holds, in its
    /// [pre-order](Document::elements).
    ///
    /// Documents keep no creation order, so unlike a batch's this list is not
    /// in the order the elements were created.
    #[must_use]
    pub fn created_ids(&self) -> &[ElementId] {
        &self.created_ids
    }

    /// The ids both documents hold whose element itself differs, in the
    /// [pre-order](Document::elements) of the document after.
    ///
    /// - A moved element is changed, and its old and new parents are not,
    ///   unless the relative order of their remaining children changed.
    /// - A reorder changes the parent, not the children.
    /// - An element of another kind under the same id is changed.
    /// - A change in an element's subtree does not change the element.
    #[must_use]
    pub fn changed_ids(&self) -> &[ElementId] {
        &self.changed_ids
    }

    /// The ids only the document before holds, in its
    /// [pre-order](Document::elements).
    ///
    /// An element written under a new id is one deletion and one creation.
    #[must_use]
    pub fn deleted_ids(&self) -> &[ElementId] {
        &self.deleted_ids
    }

    /// Whether the document itself differs: the relative order of the roots
    /// both hold, or the XMI metadata or preserved content outside every element.
    ///
    /// Adding or removing a root is not a change of the root order.
    #[must_use]
    pub fn document_changed(&self) -> bool {
        self.document_changed
    }

    /// Whether nothing differs: no id is listed and the document did not
    /// change. Exactly when the two documents are `==`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.created_ids.is_empty()
            && self.changed_ids.is_empty()
            && self.deleted_ids.is_empty()
            && !self.document_changed
    }
}

/// The ids created, changed and deleted between `before` and `after`, and
/// whether the document itself changed.
///
/// The rules are those of [`semantic_compare`], by id:
/// - created: only `after` holds the id, in `after`'s pre-order;
/// - changed: both hold it and `semantic_compare` reports a difference whose
///   [`element_id`](Difference::element_id) is that id, in `after`'s
///   pre-order;
/// - deleted: only `before` holds it, in `before`'s pre-order;
/// - document changed: it reports [`Difference::RootOrder`],
///   [`Difference::DocumentMetadata`] or
///   [`Difference::DocumentPreservedContent`].
///
/// So [`Changes::is_empty`] is `before == after`. No [`Difference`] is
/// built: each element stops at its first difference. It does not recurse
/// and takes time linear in the number of elements.
///
/// # Examples
///
/// ```
/// use sacm::compare::changes;
/// use sacm::edit::{self, CreateArgumentPackage, DeleteElement, ReorderElements};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut before = Document::new();
/// edit::apply(&mut before, CreateArgumentPackage::root(LangString::new("A")).with_id("a"))?;
/// edit::apply(&mut before, CreateArgumentPackage::root(LangString::new("B")).with_id("b"))?;
/// edit::apply(&mut before, CreateArgumentPackage::root(LangString::new("C")).with_id("c"))?;
///
/// let mut after = before.clone();
/// edit::apply(&mut after, DeleteElement::new("b"))?;
/// edit::apply(&mut after, ReorderElements::roots(["c", "a"]))?;
///
/// let found = changes(&before, &after);
/// assert_eq!(found.deleted_ids(), [ElementId::from("b")]);
/// // A root order is no element's change.
/// assert!(found.changed_ids().is_empty() && found.document_changed());
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[must_use]
pub fn changes(before: &Document, after: &Document) -> Changes {
    let mut created_ids = Vec::new();
    let mut changed_ids = Vec::new();
    for element in after.elements() {
        match before.get(element.id()) {
            None => created_ids.push(element.id().clone()),
            Some(old) => {
                if compare_element(before, after, old, element, &mut stop_at_first).is_break() {
                    changed_ids.push(element.id().clone());
                }
            }
        }
    }
    let deleted_ids = before
        .elements()
        .filter(|element| !after.contains(element.id()))
        .map(|element| element.id().clone())
        .collect();
    let document_changed = compare_document(before, after, &mut stop_at_first).is_break();
    Changes {
        created_ids,
        changed_ids,
        deleted_ids,
        document_changed,
    }
}

/// A sink that stops a walk at the first difference it is given.
fn stop_at_first(_: Found<'_>) -> ControlFlow<()> {
    ControlFlow::Break(())
}

/// One difference as [`walk`] finds it: borrowed from the two documents, so
/// finding it allocates nothing. [`Found::into_difference`] makes the owned
/// [`Difference`].
enum Found<'a> {
    /// The common roots are in a different relative order.
    RootOrder,
    /// The XMI metadata outside every element differs.
    DocumentMetadata,
    /// The preserved content outside every element differs.
    DocumentPreservedContent,
    /// Only the left document holds this element.
    LeftOnly(Compared<'a>),
    /// The element's kind differs; the left and the right features.
    Kind(Compared<'a>, Compared<'a>),
    /// The element's parent differs.
    Parent(Compared<'a>, Compared<'a>),
    /// The element's `gid` differs.
    Gid(Compared<'a>, Compared<'a>),
    /// The value of one of the element's flags differs.
    Flag(Flag, Compared<'a>, Compared<'a>),
    /// The targets of one of the element's reference ends differ.
    Reference(ReferenceEnd, Compared<'a>, Compared<'a>),
    /// The `href`s of one of the element's reference ends differ.
    Hrefs(ReferenceEnd, Compared<'a>, Compared<'a>),
    /// The element's proxies differ.
    Proxies(Compared<'a>, Compared<'a>),
    /// The element's name differs.
    Name(Compared<'a>, Compared<'a>),
    /// The `xmi:id` of the element's name differs.
    NameId(Compared<'a>, Compared<'a>),
    /// The value of the element's `content` or `key` differs.
    Text(TextRole, Compared<'a>, Compared<'a>),
    /// The `xmi:id` of the element's `content` or `key` differs.
    TextId(TextRole, Compared<'a>, Compared<'a>),
    /// The ids of the values of the element's `content` or `key` differ.
    TextValueIds(TextRole, Compared<'a>, Compared<'a>),
    /// The value of one of the element's string-valued attributes differs.
    String(StringAttribute, Compared<'a>, Compared<'a>),
    /// The element's `assertionDeclaration` differs.
    AssertionDeclaration(Compared<'a>, Compared<'a>),
    /// The element's common children are in a different relative order; the
    /// element in the left and in the right document.
    ChildOrder(SacmElement<'a>, SacmElement<'a>),
    /// The element's XMI metadata, or that of a part of it, differs.
    Metadata(Compared<'a>),
    /// The element's preserved content differs.
    PreservedContent(Compared<'a>),
    /// Only the right document holds this element.
    RightOnly(SacmElement<'a>),
}

impl Found<'_> {
    /// The owned [`Difference`], for the documents the walk compared.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per kind of difference, in the order of the enum"
    )]
    fn into_difference(self, left: &Document, right: &Document) -> Difference {
        match self {
            Self::RootOrder => Difference::RootOrder {
                left: common_roots(left, right).cloned().collect(),
                right: common_roots(right, left).cloned().collect(),
            },
            Self::DocumentMetadata => Difference::DocumentMetadata,
            Self::DocumentPreservedContent => Difference::DocumentPreservedContent,
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
            Self::Gid(ours, theirs) => Difference::Gid {
                id: ours.id.clone(),
                left: ours.gid.map(str::to_owned),
                right: theirs.gid.map(str::to_owned),
            },
            Self::Flag(flag, ours, theirs) => {
                let id = ours.id.clone();
                let (left, right) = (ours.flag(flag), theirs.flag(flag));
                match flag {
                    Flag::IsCitation => Difference::IsCitation { id, left, right },
                    Flag::IsAbstract => Difference::IsAbstract { id, left, right },
                    Flag::IsCounter => Difference::IsCounter { id, left, right },
                }
            }
            Self::Reference(end, ours, theirs) => reference_difference(end, &ours, &theirs),
            Self::Hrefs(end, ours, theirs) => Difference::Hrefs {
                id: ours.id.clone(),
                end,
                left: ours.hrefs(end).to_vec(),
                right: theirs.hrefs(end).to_vec(),
            },
            Self::Proxies(ours, theirs) => Difference::Proxies {
                id: ours.id.clone(),
                left: ours.proxies.to_vec(),
                right: theirs.proxies.to_vec(),
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
            Self::Text(role, ours, theirs) => {
                let id = ours.id.clone();
                let left = text_value(&ours, role).cloned();
                let right = text_value(&theirs, role).cloned();
                match role {
                    TextRole::Content => Difference::Content { id, left, right },
                    TextRole::Key => Difference::Key { id, left, right },
                    TextRole::Location => Difference::Location { id, left, right },
                }
            }
            Self::TextId(role, ours, theirs) => {
                let id = ours.id.clone();
                let left = text_id(&ours, role).cloned();
                let right = text_id(&theirs, role).cloned();
                match role {
                    TextRole::Content => Difference::ContentId { id, left, right },
                    TextRole::Key => Difference::KeyId { id, left, right },
                    TextRole::Location => Difference::LocationId { id, left, right },
                }
            }
            Self::TextValueIds(role, ours, theirs) => {
                let id = ours.id.clone();
                let left = text_value_ids(&ours, role).to_vec();
                let right = text_value_ids(&theirs, role).to_vec();
                match role {
                    TextRole::Content => Difference::ContentValueIds { id, left, right },
                    TextRole::Key => Difference::KeyValueIds { id, left, right },
                    TextRole::Location => Difference::LocationValueIds { id, left, right },
                }
            }
            Self::String(attribute, ours, theirs) => string_difference(attribute, &ours, &theirs),
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
            Self::Metadata(ours) => Difference::Metadata {
                id: ours.id.clone(),
            },
            Self::PreservedContent(ours) => Difference::PreservedContent {
                id: ours.id.clone(),
            },
            Self::RightOnly(element) => Difference::RightOnly {
                id: element.id().clone(),
                kind: element.kind(),
            },
        }
    }
}

/// The difference in the string-valued `attribute` of one element, `ours`
/// on the left and `theirs` on the right: each value exactly, or `None`.
fn string_difference(
    attribute: StringAttribute,
    ours: &Compared<'_>,
    theirs: &Compared<'_>,
) -> Difference {
    let id = ours.id.clone();
    let left = ours.string(attribute).map(str::to_owned);
    let right = theirs.string(attribute).map(str::to_owned);
    match attribute {
        StringAttribute::Version => Difference::Version { id, left, right },
        StringAttribute::Date => Difference::Date { id, left, right },
        StringAttribute::StartTime => Difference::StartTime { id, left, right },
        StringAttribute::EndTime => Difference::EndTime { id, left, right },
        StringAttribute::Value => Difference::Value { id, left, right },
        StringAttribute::ExternalReference => Difference::ExternalReference { id, left, right },
    }
}

/// The difference in the targets of the reference end `end` of one element,
/// `ours` on the left and `theirs` on the right: the whole lists of a
/// multi-valued end, in order, and the one id or `None` of a single-valued
/// end.
fn reference_difference(
    end: ReferenceEnd,
    ours: &Compared<'_>,
    theirs: &Compared<'_>,
) -> Difference {
    let id = ours.id.clone();
    let (all_left, all_right) = (ours.references(end), theirs.references(end));
    // A single-valued end holds at most one id.
    let (left, right) = (all_left.first().cloned(), all_right.first().cloned());
    match end {
        ReferenceEnd::CitedElement => Difference::CitedElement { id, left, right },
        ReferenceEnd::AbstractForm => Difference::AbstractForm { id, left, right },
        ReferenceEnd::MetaClaim => Difference::MetaClaims {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Reasoning => Difference::Reasoning { id, left, right },
        ReferenceEnd::Source => Difference::Sources {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Target => Difference::Target { id, left, right },
        ReferenceEnd::Structure => Difference::Structure { id, left, right },
        ReferenceEnd::ReferencedArtifactElement => Difference::ReferencedArtifactElements {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::GroupMember => Difference::GroupMembers {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::ArtifactGroupMember => Difference::ArtifactGroupMembers {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::ArtifactSource => Difference::ArtifactSources {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::ArtifactTarget => Difference::ArtifactTargets {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Category => Difference::Categories {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Element => Difference::ExpressionElements {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Origin => Difference::Origin { id, left, right },
        ReferenceEnd::TerminologyGroupMember => Difference::TerminologyGroupMembers {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Interface => Difference::Interfaces {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        ReferenceEnd::Implements => Difference::Implements { id, left, right },
        ReferenceEnd::ParticipantPackage => Difference::ParticipantPackages {
            id,
            left: all_left.to_vec(),
            right: all_right.to_vec(),
        },
        // The derived end is compared through the name and the texts, which
        // hold it (SACM23-BASE-009): the walk never asks for it, and its ids
        // differ only when a name or a text does.
        ReferenceEnd::Expression => Difference::Name {
            id,
            left: ours.name.cloned(),
            right: theirs.name.cloned(),
        },
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
    compare_document(left, right, &mut sink)?;

    for element in left.elements() {
        let Some(other) = right.get(element.id()) else {
            sink(Found::LeftOnly(element.compared()))?;
            continue;
        };
        compare_element(left, right, element, other, &mut sink)?;
    }

    for element in right.elements() {
        if !left.contains(element.id()) {
            sink(Found::RightOnly(element))?;
        }
    }
    ControlFlow::Continue(())
}

/// Gives each difference of the documents themselves, not of an element, to
/// `sink`, in [`semantic_compare`]'s order, until `sink` breaks: the
/// relative order of the common roots, then the XMI metadata and the
/// preserved content outside every element.
fn compare_document<'a>(
    left: &'a Document,
    right: &'a Document,
    sink: &mut impl FnMut(Found<'a>) -> ControlFlow<()>,
) -> ControlFlow<()> {
    if common_roots(left, right).ne(common_roots(right, left)) {
        sink(Found::RootOrder)?;
    }
    if left.document_metadata() != right.document_metadata() {
        sink(Found::DocumentMetadata)?;
    }
    if left.document_preserved() != right.document_preserved() {
        sink(Found::DocumentPreservedContent)?;
    }
    ControlFlow::Continue(())
}

/// Whether the element `id`, held by both documents, differs between them:
/// whether [`semantic_compare`] reports a difference of the element itself
/// (not `LeftOnly`, `RightOnly` or `RootOrder`). `false` when either
/// document lacks it. Linear in the element's features and children.
pub(crate) fn element_differs(left: &Document, right: &Document, id: &ElementId) -> bool {
    match (left.get(id), right.get(id)) {
        (Some(element), Some(other)) => {
            compare_element(left, right, element, other, &mut stop_at_first).is_break()
        }
        _ => false,
    }
}

/// Gives each difference of `element` of `left` and `other`, the element
/// with its id in `right`, to `sink`, in [`semantic_compare`]'s order, until
/// `sink` breaks.
fn compare_element<'a>(
    left: &'a Document,
    right: &'a Document,
    element: SacmElement<'a>,
    other: SacmElement<'a>,
    sink: &mut impl FnMut(Found<'a>) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let ours = element.compared();
    let theirs = other.compared();
    if ours.kind != theirs.kind {
        sink(Found::Kind(ours, theirs))?;
    }
    if ours.parent != theirs.parent {
        sink(Found::Parent(ours, theirs))?;
    }
    if ours.gid != theirs.gid {
        sink(Found::Gid(ours, theirs))?;
    }
    for &flag in Flag::ALL {
        if ours.flag(flag) != theirs.flag(flag) {
            sink(Found::Flag(flag, ours, theirs))?;
        }
    }
    for &end in ReferenceEnd::ALL.iter().filter(|end| !end.is_derived()) {
        if ours.references(end) != theirs.references(end) {
            sink(Found::Reference(end, ours, theirs))?;
        }
    }
    // The cross-document references, after every end's ids (ADR 0040 D8).
    for &end in ReferenceEnd::ALL {
        if ours.hrefs(end) != theirs.hrefs(end) {
            sink(Found::Hrefs(end, ours, theirs))?;
        }
    }
    // The proxies, right after the hrefs (ADR 0040 amendment 1, A6).
    if ours.proxies != theirs.proxies {
        sink(Found::Proxies(ours, theirs))?;
    }
    if ours.name != theirs.name {
        sink(Found::Name(ours, theirs))?;
    }
    if ours.name_id != theirs.name_id {
        sink(Found::NameId(ours, theirs))?;
    }
    for feature in COMPARED_VALUES {
        match feature {
            Valued::Text(role) => {
                if text_value(&ours, role) != text_value(&theirs, role) {
                    sink(Found::Text(role, ours, theirs))?;
                }
                if text_id(&ours, role) != text_id(&theirs, role) {
                    sink(Found::TextId(role, ours, theirs))?;
                }
                if value_ids_differ(text_value_ids(&ours, role), text_value_ids(&theirs, role)) {
                    sink(Found::TextValueIds(role, ours, theirs))?;
                }
            }
            Valued::String(attribute) => {
                if ours.string(attribute) != theirs.string(attribute) {
                    sink(Found::String(attribute, ours, theirs))?;
                }
            }
        }
    }
    if ours.assertion_declaration != theirs.assertion_declaration {
        sink(Found::AssertionDeclaration(ours, theirs))?;
    }
    if common_children(element, right).ne(common_children(other, left)) {
        sink(Found::ChildOrder(element, other))?;
    }
    if ours.metadata_differs(&theirs) {
        sink(Found::Metadata(ours))?;
    }
    if ours.preserved != theirs.preserved {
        sink(Found::PreservedContent(ours))?;
    }
    ControlFlow::Continue(())
}

/// A feature with a value, after the name: a text role or a string-valued
/// attribute.
#[derive(Clone, Copy)]
enum Valued {
    /// A role that holds a `MultiLangString`, with its ids.
    Text(TextRole),
    /// A string-valued attribute.
    String(StringAttribute),
}

/// The features with a value, in the order [`semantic_compare`] reports
/// them: `content` and `key`, then `version` and `date`, then `location`,
/// then `startTime` and `endTime`, then `value` and `externalReference`. A
/// test of the crate keeps it total over [`TextRole::ALL`] and
/// [`StringAttribute::ALL`].
const COMPARED_VALUES: [Valued; 9] = [
    Valued::Text(TextRole::Content),
    Valued::Text(TextRole::Key),
    Valued::String(StringAttribute::Version),
    Valued::String(StringAttribute::Date),
    Valued::Text(TextRole::Location),
    Valued::String(StringAttribute::StartTime),
    Valued::String(StringAttribute::EndTime),
    Valued::String(StringAttribute::Value),
    Valued::String(StringAttribute::ExternalReference),
];

/// The text `element` holds in `role`.
fn text<'a>(element: &Compared<'a>, role: TextRole) -> Option<&'a Text> {
    match role {
        TextRole::Content => element.content,
        TextRole::Key => element.key,
        TextRole::Location => element.location,
    }
}

/// The value of the text `element` holds in `role`, or `None` without one.
fn text_value<'a>(element: &Compared<'a>, role: TextRole) -> Option<&'a MultiLangString> {
    text(element, role).map(Text::value)
}

/// The `xmi:id` of the text `element` holds in `role`, if it has one.
fn text_id<'a>(element: &Compared<'a>, role: TextRole) -> Option<&'a ElementId> {
    text(element, role).and_then(Text::id)
}

/// The ids of the values of the text `element` holds in `role`: empty
/// without one.
fn text_value_ids<'a>(element: &Compared<'a>, role: TextRole) -> &'a [Option<ElementId>] {
    text(element, role).map_or(&[], Text::value_ids)
}

/// Whether some position holds a different value id on each side. A
/// position one side lacks is a value without an id there, so a different
/// number of values is a difference in the values, not in their ids.
fn value_ids_differ(left: &[Option<ElementId>], right: &[Option<ElementId>]) -> bool {
    (0..left.len().max(right.len())).any(|position| id_at(left, position) != id_at(right, position))
}

/// The value id at `position` of `ids`, or `None` past the end.
fn id_at(ids: &[Option<ElementId>], position: usize) -> Option<&ElementId> {
    ids.get(position).and_then(Option::as_ref)
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
        walk(self, other, stop_at_first).is_continue()
    }
}

impl Eq for Document {}

#[cfg(test)]
mod tests {
    use super::{COMPARED_VALUES, Difference, Valued};
    use crate::model::{ElementId, StringAttribute, TextRole};

    /// The argumentation variants name their feature by its XMI name, and
    /// show both sides (ADR 0027).
    #[test]
    fn the_argumentation_differences_display_their_feature_and_both_sides() {
        let id = || ElementId::new("x");
        let ids = |ids: &[&str]| ids.iter().map(|id| ElementId::new(*id)).collect();
        let one = |value: &str| Some(ElementId::new(value));
        let cases = [
            (
                Difference::IsCounter {
                    id: id(),
                    left: true,
                    right: false,
                },
                "element \"x\" differs in isCounter: true on the left, false on the right",
            ),
            (
                Difference::MetaClaims {
                    id: id(),
                    left: ids(&["m1", "m2"]),
                    right: ids(&[]),
                },
                "element \"x\" differs in metaClaim: [\"m1\", \"m2\"] on the left, [] on the right",
            ),
            (
                Difference::Reasoning {
                    id: id(),
                    left: one("r1"),
                    right: None,
                },
                "element \"x\" differs in reasoning: \"r1\" on the left, no reasoning on the right",
            ),
            (
                Difference::Sources {
                    id: id(),
                    left: ids(&["c1"]),
                    right: ids(&["c1", "c1"]),
                },
                "element \"x\" differs in source: [\"c1\"] on the left, [\"c1\", \"c1\"] on the right",
            ),
            (
                Difference::Target {
                    id: id(),
                    left: None,
                    right: one("c2"),
                },
                "element \"x\" differs in target: no target on the left, \"c2\" on the right",
            ),
            (
                Difference::Structure {
                    id: id(),
                    left: one("p"),
                    right: one("q"),
                },
                "element \"x\" differs in structure: \"p\" on the left, \"q\" on the right",
            ),
        ];
        for (difference, expected) in cases {
            assert_eq!(difference.to_string(), expected);
            assert_eq!(difference.element_id(), Some(&id()));
        }
    }

    /// Every text role and every string-valued attribute is compared, each
    /// once (ADR 0034 amendment 1).
    #[test]
    fn the_compared_values_cover_every_text_role_and_string_attribute_once() {
        let roles: Vec<TextRole> = COMPARED_VALUES
            .iter()
            .filter_map(|feature| match feature {
                Valued::Text(role) => Some(*role),
                Valued::String(_) => None,
            })
            .collect();
        let attributes: Vec<StringAttribute> = COMPARED_VALUES
            .iter()
            .filter_map(|feature| match feature {
                Valued::String(attribute) => Some(*attribute),
                Valued::Text(_) => None,
            })
            .collect();
        assert_eq!(roles, TextRole::ALL);
        assert_eq!(attributes, StringAttribute::ALL);
    }

    /// The string-valued and location variants name their feature, and show
    /// both sides, absent apart from empty.
    #[test]
    fn the_asset_value_differences_display_their_feature_and_both_sides() {
        let id = || ElementId::new("x");
        let text = |value: &str| Some(value.to_owned());
        let cases = [
            (
                Difference::Version {
                    id: id(),
                    left: text(""),
                    right: None,
                },
                "element \"x\" differs in version: \"\" on the left, no version on the right",
            ),
            (
                Difference::StartTime {
                    id: id(),
                    left: None,
                    right: text("t"),
                },
                "element \"x\" differs in startTime: no startTime on the left, \"t\" on the right",
            ),
            (
                Difference::EndTime {
                    id: id(),
                    left: text("a"),
                    right: text("b"),
                },
                "element \"x\" differs in endTime: \"a\" on the left, \"b\" on the right",
            ),
            (
                Difference::LocationId {
                    id: id(),
                    left: Some(ElementId::new("l")),
                    right: None,
                },
                "element \"x\" differs in the xmi:id of its location: \"l\" on the left, no id \
                 on the right",
            ),
            (
                Difference::LocationValueIds {
                    id: id(),
                    left: vec![None],
                    right: vec![Some(ElementId::new("v"))],
                },
                "element \"x\" differs in the xmi:ids of its location's values: [no id] on the \
                 left, [\"v\"] on the right",
            ),
        ];
        for (difference, expected) in cases {
            assert_eq!(difference.to_string(), expected);
            assert_eq!(difference.element_id(), Some(&id()));
        }
    }
}
