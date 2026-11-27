//! The Boolean flags and the non-containment reference ends of the classes
//! the model holds (formal/23-05-08 §8.2, §9.2 to §9.4, §10.3 to §10.6,
//! §10.8 to §10.11, §11.2, §11.4 to §11.6, §11.9, §11.10, §11.12, §11.13,
//! §12.2 to §12.5, §12.14; SACM23-BASE-007, SACM23-BASE-008, SACM23-XMI-005,
//! SACM23-ARG-004 to SACM23-ARG-007, SACM23-ARG-013 to SACM23-ARG-015,
//! SACM23-ART-010 to SACM23-ART-013, SACM23-TERM-002 to SACM23-TERM-008,
//! SACM23-AC-002, SACM23-AC-003; ADR 0026, ADR 0027, ADR 0028, ADR 0034 and
//! its amendments, ADR 0039).
//!
//! [`Flag`] and [`ReferenceEnd`] are public, so that an edit can name the
//! flag or end it sets (ADR 0029), and so are their readers for a generic
//! property panel (ADR 0029 amendment 1): [`Flag::ALL`] and
//! [`ReferenceEnd::ALL`], `applies_to(kind)` and [`ReferenceEnd::bounds`].
//! `ALL` is the one table the XMI reader, the writer, the id pre-pass,
//! comparison, validation and the edits iterate, so that the name read, the
//! name written and the order written cannot drift apart. Each entry names
//! the class that owns it, and
//! [`OWNERS`] orders those classes superclass first, the order `xmi::write`
//! writes their features in. A later end adds one entry here, one accessor
//! and one `Difference` variant. A multi-valued end ([`ReferenceEnd::is_many`])
//! holds every id of its attribute, and each reference element appends one;
//! a single-valued one holds at most one. No kind has two ends of one name,
//! so the reader resolves an end by the element's kind (ADR 0034 amendment
//! 2). The owners, the declared types and the bounds by end alone are
//! crate-private, as the role tables of [`containment`](super::containment)
//! are.

use std::fmt;

use super::{AbstractClass, DeclaredType, ElementKind, MetamodelPackage};

/// The classes that own a flag, the `assertionDeclaration` or a reference
/// end, superclass first: the order in which `xmi::write` writes each one's
/// features (XMI 2.5.1 Annex B.5.2; ADR 0027, ADR 0028). Within a class it
/// writes the flags, then, on `Assertion`, the declaration, then the ends.
pub(crate) const OWNERS: [DeclaredType; 24] = [
    DeclaredType::Abstract(AbstractClass::SACMElement),
    DeclaredType::Abstract(AbstractClass::Assertion),
    DeclaredType::Abstract(AbstractClass::AssertedRelationship),
    DeclaredType::Kind(ElementKind::ArgumentReasoning),
    DeclaredType::Kind(ElementKind::ArtifactReference),
    DeclaredType::Kind(ElementKind::ArgumentGroup),
    DeclaredType::Kind(ElementKind::ArtifactGroup),
    DeclaredType::Kind(ElementKind::ArtifactAssetRelationship),
    DeclaredType::Abstract(AbstractClass::TerminologyAsset),
    DeclaredType::Kind(ElementKind::Expression),
    DeclaredType::Kind(ElementKind::Term),
    DeclaredType::Kind(ElementKind::TerminologyGroup),
    // The classes that declare `interface`, `implements` and
    // `participantPackage` (ptc/22-03-13; ADR 0039 D1), each package before
    // its interface and binding.
    DeclaredType::Kind(ElementKind::ArgumentPackage),
    DeclaredType::Kind(ElementKind::ArgumentPackageInterface),
    DeclaredType::Kind(ElementKind::ArgumentPackageBinding),
    DeclaredType::Kind(ElementKind::ArtifactPackage),
    DeclaredType::Kind(ElementKind::ArtifactPackageInterface),
    DeclaredType::Kind(ElementKind::ArtifactPackageBinding),
    DeclaredType::Kind(ElementKind::AssuranceCasePackage),
    DeclaredType::Kind(ElementKind::AssuranceCasePackageInterface),
    DeclaredType::Kind(ElementKind::AssuranceCasePackageBinding),
    DeclaredType::Kind(ElementKind::TerminologyPackage),
    DeclaredType::Kind(ElementKind::TerminologyPackageInterface),
    DeclaredType::Kind(ElementKind::TerminologyPackageBinding),
];

/// A Boolean property of an element, `false` by default: the flag a
/// [`SetFlag`](crate::edit::SetFlag) sets.
///
/// Every element has `isCitation` and `isAbstract` (SACM 2.3 §8.2); an
/// asserted relationship also has `isCounter` (§11.13). Later releases may
/// add flags, so a `match` on `Flag` needs a wildcard arm.
///
/// It displays as its [`name`](Self::name), and [`Ord`] is the order of
/// [`Flag::ALL`].
///
/// # Examples
///
/// ```
/// use sacm::model::Flag;
///
/// assert_eq!(Flag::IsCounter.name(), "isCounter");
/// assert_eq!(Flag::IsCounter.to_string(), "isCounter");
/// assert!(Flag::IsCitation < Flag::IsCounter);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
#[non_exhaustive]
pub enum Flag {
    /// `isCitation[0..1]=false` of every `SACMElement` (§8.2): whether the
    /// element cites another.
    IsCitation,
    /// `isAbstract[0..1]=false` of every `SACMElement` (§8.2): whether the
    /// element is abstract.
    IsAbstract,
    /// `isCounter:Boolean[1] = false` of every `AssertedRelationship`
    /// (§11.13): whether the relationship counters its declared purpose.
    IsCounter,
}

impl Flag {
    /// Every flag, in the order `compare` reports them and `xmi::write`
    /// writes them: `isCitation`, `isAbstract`, `isCounter`.
    ///
    /// A slice, so that a later flag is an additive change. A generic
    /// property panel lists an element's flags with it and
    /// [`applies_to`](Self::applies_to); see
    /// [`SacmElement::references`](crate::model::SacmElement::references).
    ///
    /// ```
    /// use sacm::model::{ElementKind, Flag};
    ///
    /// let claim: Vec<&str> = Flag::ALL
    ///     .iter()
    ///     .filter(|flag| flag.applies_to(ElementKind::Claim))
    ///     .map(|flag| flag.name())
    ///     .collect();
    /// assert_eq!(claim, ["isCitation", "isAbstract"]);
    /// ```
    pub const ALL: &'static [Flag] = &[Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter];

    /// Whether an element of `kind` has the flag: every element has
    /// `isCitation` and `isAbstract` (§8.2), and only an
    /// `AssertedRelationship` has `isCounter` (§11.13).
    ///
    /// It is true exactly when a [`SetFlag`](crate::edit::SetFlag) of the
    /// flag on such an element is not
    /// [`FeatureNotApplicable`](crate::edit::EditError::FeatureNotApplicable),
    /// and [`SacmElement::flag`](crate::model::SacmElement::flag) of a flag
    /// the kind lacks is `false`.
    ///
    /// ```
    /// use sacm::model::{ElementKind, Flag};
    ///
    /// assert!(Flag::IsCounter.applies_to(ElementKind::AssertedInference));
    /// assert!(!Flag::IsCounter.applies_to(ElementKind::Claim));
    /// ```
    #[must_use]
    pub const fn applies_to(self, kind: ElementKind) -> bool {
        self.owner().admits(kind)
    }

    /// The flag's name, spelled as in the machine-readable model and in
    /// XMI: `isCitation`, `isAbstract` or `isCounter`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::IsCitation => "isCitation",
            Self::IsAbstract => "isAbstract",
            Self::IsCounter => "isCounter",
        }
    }

    /// The class that owns the flag: an element of a kind it admits has it.
    pub(crate) const fn owner(self) -> DeclaredType {
        match self {
            Self::IsCitation | Self::IsAbstract => {
                DeclaredType::Abstract(AbstractClass::SACMElement)
            }
            Self::IsCounter => DeclaredType::Abstract(AbstractClass::AssertedRelationship),
        }
    }
}

impl fmt::Display for Flag {
    /// Writes [`Flag::name`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// The flag named `local` that an element of `kind` has, if any.
pub(crate) fn flag_named(kind: ElementKind, local: &str) -> Option<Flag> {
    Flag::ALL
        .iter()
        .copied()
        .find(|flag| flag.name() == local && flag.owner().admits(kind))
}

/// A non-containment reference end, which holds the ids of its targets
/// unresolved (ADR 0026): the end a [`SetReferences`](crate::edit::SetReferences),
/// [`AddReference`](crate::edit::AddReference) or
/// [`RemoveReference`](crate::edit::RemoveReference) edits.
///
/// Each end belongs to a class, and only the elements of that class have it:
/// every element has `citedElement` and `abstractForm`, every Assertion has
/// `metaClaim`, and so on, as each variant says. An end holds at most one id
/// (`[0..1]`, or exactly one for an `AssertedRelationship`'s `target`), or
/// any number in order, repeats included (`[0..*]`, or at least one for
/// `source` and for an `ArtifactAssetRelationship`'s `target`). No kind has
/// two ends of one name: an `ArtifactAssetRelationship`'s `source` and
/// `target` are [`ArtifactSource`](Self::ArtifactSource) and
/// [`ArtifactTarget`](Self::ArtifactTarget), apart from an
/// `AssertedRelationship`'s [`Source`](Self::Source) and
/// [`Target`](Self::Target). Later releases may add ends, so a `match` on
/// `ReferenceEnd` needs a wildcard arm.
///
/// It displays as its [`name`](Self::name), and [`Ord`] is the order of
/// [`ReferenceEnd::ALL`].
///
/// # Examples
///
/// ```
/// use sacm::model::ReferenceEnd;
///
/// assert_eq!(ReferenceEnd::Source.name(), "source");
/// assert_eq!(ReferenceEnd::ArtifactSource.to_string(), "source");
/// assert!(ReferenceEnd::Source < ReferenceEnd::ArtifactSource);
/// // A group's members are its `argumentElement` references, and an
/// // ArtifactGroup's its `artifactElement` references.
/// assert_eq!(ReferenceEnd::GroupMember.name(), "argumentElement");
/// assert_eq!(ReferenceEnd::ArtifactGroupMember.name(), "artifactElement");
/// // An ArtifactAssetRelationship's ends have the names of an
/// // AssertedRelationship's.
/// assert_eq!(ReferenceEnd::ArtifactTarget.name(), "target");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
#[non_exhaustive]
pub enum ReferenceEnd {
    /// `citedElement:SACMElement[0..1]` of every `SACMElement` (§8.2).
    CitedElement,
    /// `abstractForm:SACMElement[0..1]` of every `SACMElement` (§8.2).
    AbstractForm,
    /// `metaClaim:Claim[0..*]` of every `Assertion` (§11.10).
    MetaClaim,
    /// `reasoning:ArgumentReasoning[0..1]` of every `AssertedRelationship`
    /// (§11.13).
    Reasoning,
    /// `source:ArgumentAsset[1..*]` of every `AssertedRelationship` (§11.13).
    Source,
    /// `target:ArgumentAsset[1]` of every `AssertedRelationship` (§11.13).
    Target,
    /// `structure:ArgumentPackage[0..1]` of an `ArgumentReasoning` (§11.12).
    Structure,
    /// `referencedArtifactElement:Base::ArtifactElement[0..*]` of an
    /// `ArtifactReference` (§11.9).
    ReferencedArtifactElement,
    /// `argumentElement:ArgumentationElement[0..*]` of an `ArgumentGroup`
    /// (§11.2, where the text spells it `argumentationElement`; ptc/22-03-13
    /// gives no aggregation, so it is a reference, not a composition). Named
    /// for the group, because `argumentElement` is also the composite role
    /// of an `ArgumentPackage` (ADR 0028).
    #[doc(alias = "argumentElement")]
    #[cfg_attr(feature = "serde", serde(rename = "argumentElement"))]
    GroupMember,
    /// `artifactElement:ArtifactElement[0..*]` of an `ArtifactGroup` (§12.3;
    /// ptc/22-03-13 gives no aggregation, so it is a reference, not a
    /// composition). Named for the group, because `artifactElement` is also
    /// the composite role of an `ArtifactPackage` (ADR 0034).
    #[doc(alias = "artifactElement")]
    #[cfg_attr(feature = "serde", serde(rename = "artifactElement"))]
    ArtifactGroupMember,
    /// `source:ArtifactAsset[1..*]` of an `ArtifactAssetRelationship` (§12.14). Named for the
    /// Artifact package, because `source` is also the end of every `AssertedRelationship`
    /// (ADR 0034 amendment 2); with `serde` it is `artifactSource`.
    #[doc(alias = "source")]
    ArtifactSource,
    /// `target:ArtifactAsset[1..*]` of an `ArtifactAssetRelationship` (§12.14): one or more
    /// ids, where an `AssertedRelationship`'s `target` holds exactly one. With `serde` it is
    /// `artifactTarget`.
    #[doc(alias = "target")]
    ArtifactTarget,
    /// `category:Category[0..*]` of a `Category` (ptc/22-03-13; §10.8
    /// Semantics) and of every `ExpressionElement` (§10.9): one end, held by
    /// every `TerminologyAsset`. On a `Term` or an `Expression` it holds the
    /// Categories the element belongs to. On a `Category` it holds the ids as
    /// written: formal/23-05-08 does not say which way the end points (spec
    /// defect D21).
    Category,
    /// `element:ExpressionElement[0..*]` of an `Expression` (§10.10): the
    /// expression elements it is formed from.
    Element,
    /// `origin:Base::ModelElement[0..1]` of a `Term` (§10.11): the element
    /// the Term originates from.
    Origin,
    /// `terminologyElement:TerminologyElement[0..*]` of a `TerminologyGroup`
    /// (§10.3; ptc/22-03-13 gives no aggregation, so it is a reference, not a
    /// composition). Named for the group, because `terminologyElement` is
    /// also the composite role of a `TerminologyPackage` (ADR 0034 amendment
    /// 3).
    #[doc(alias = "terminologyElement")]
    #[cfg_attr(feature = "serde", serde(rename = "terminologyElement"))]
    TerminologyGroupMember,
    /// `interface:<package>Interface[0..*]` of every package kind, interfaces and bindings included:
    /// an `AssuranceCasePackage` (§9.2), a `TerminologyPackage`, an `ArgumentPackage` or an
    /// `ArtifactPackage` (ptc/22-03-13; the text of §10.4, §11.4 and §12.2 omits it): the
    /// interfaces the package declares. Its targets are the holder's family's interface class.
    /// Held as written, never synced with `implements` (ADR 0039).
    Interface,
    /// `implements:<package>[1]` of a package interface: an `AssuranceCasePackageInterface`
    /// (§9.3), a `TerminologyPackageInterface` (§10.5), an `ArgumentPackageInterface` (§11.6) or an
    /// `ArtifactPackageInterface` (§12.5): the package the interface declares.
    Implements,
    /// `participantPackage:<package>[2..*]` of a package binding: an `AssuranceCasePackageBinding`
    /// (§9.4), a `TerminologyPackageBinding` (ptc/22-03-13; §10.6 has no Associations block), an
    /// `ArgumentPackageBinding` (§11.5) or an `ArtifactPackageBinding` (§12.4): the packages the
    /// binding binds. Its lower bound counts distinct ids.
    #[doc(alias = "participantPackage")]
    ParticipantPackage,
    /// The `expression:Terminology::ExpressionElement[1]` of each `ExpressionLangString`
    /// (§8.4) the element holds, as its name or a value of its `content`, `key` or
    /// `location`: their ids in that order, one per `ExpressionLangString` that has one.
    /// Derived from the name and texts: the reference edits refuse it
    /// ([`EditError::DerivedEnd`](crate::edit::EditError::DerivedEnd)); set it with
    /// [`LangString::with_expression`](super::LangString::with_expression).
    Expression,
}

impl ReferenceEnd {
    /// Every end, in the order `xmi::write` writes them and `compare`
    /// reports them (ptc/22-03-13 order within each class, superclass
    /// first): `citedElement`, `abstractForm`, `metaClaim`, `reasoning`,
    /// `source`, `target`, `structure`, `referencedArtifactElement`, an
    /// `ArgumentGroup`'s `argumentElement`, an `ArtifactGroup`'s
    /// `artifactElement`, and an `ArtifactAssetRelationship`'s `source` and
    /// `target`, then the Terminology ends: the `category` of every
    /// `TerminologyAsset`, an `Expression`'s `element`, a `Term`'s `origin`
    /// and a `TerminologyGroup`'s `terminologyElement`; then a package's
    /// `interface`, an interface's `implements` and a binding's
    /// `participantPackage` (ADR 0039); and last the derived
    /// `expression` of every element (SACM23-BASE-009), which `xmi::write`
    /// writes with the name and texts that hold it, not as an end.
    ///
    /// A slice, so that a later end is an additive change. It is also the
    /// order of [`Document::referrers`](crate::model::Document::referrers)
    /// within one holder.
    ///
    /// ```
    /// use sacm::model::{ElementKind, ReferenceEnd};
    ///
    /// let ends: Vec<&str> = ReferenceEnd::ALL
    ///     .iter()
    ///     .filter(|end| end.applies_to(ElementKind::AssertedInference))
    ///     .map(|end| end.name())
    ///     .collect();
    /// assert_eq!(
    ///     ends,
    ///     ["citedElement", "abstractForm", "metaClaim", "reasoning", "source", "target", "expression"]
    /// );
    /// let interface: Vec<&str> = ReferenceEnd::ALL
    ///     .iter()
    ///     .filter(|end| end.applies_to(ElementKind::ArgumentPackageInterface))
    ///     .map(|end| end.name())
    ///     .collect();
    /// assert_eq!(
    ///     interface,
    ///     ["citedElement", "abstractForm", "interface", "implements", "expression"]
    /// );
    /// ```
    pub const ALL: &'static [ReferenceEnd] = &[
        ReferenceEnd::CitedElement,
        ReferenceEnd::AbstractForm,
        ReferenceEnd::MetaClaim,
        ReferenceEnd::Reasoning,
        ReferenceEnd::Source,
        ReferenceEnd::Target,
        ReferenceEnd::Structure,
        ReferenceEnd::ReferencedArtifactElement,
        ReferenceEnd::GroupMember,
        ReferenceEnd::ArtifactGroupMember,
        ReferenceEnd::ArtifactSource,
        ReferenceEnd::ArtifactTarget,
        ReferenceEnd::Category,
        ReferenceEnd::Element,
        ReferenceEnd::Origin,
        ReferenceEnd::TerminologyGroupMember,
        ReferenceEnd::Interface,
        ReferenceEnd::Implements,
        ReferenceEnd::ParticipantPackage,
        ReferenceEnd::Expression,
    ];

    /// The end's name, spelled as in the machine-readable model and in XMI:
    /// `citedElement`, `abstractForm`, `metaClaim`, `reasoning`, `source`,
    /// `target`, `structure`, `referencedArtifactElement` or, for
    /// [`GroupMember`](Self::GroupMember), `argumentElement`, for
    /// [`ArtifactGroupMember`](Self::ArtifactGroupMember), `artifactElement`,
    /// for [`ArtifactSource`](Self::ArtifactSource) and
    /// [`ArtifactTarget`](Self::ArtifactTarget), `source` and `target`,
    /// `category`, `element`, `origin`, for
    /// [`TerminologyGroupMember`](Self::TerminologyGroupMember),
    /// `terminologyElement`, `interface`, `implements`, `participantPackage`
    /// and `expression`. It is the `end` an
    /// [`EditError`](crate::edit::EditError) names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CitedElement => "citedElement",
            Self::AbstractForm => "abstractForm",
            Self::MetaClaim => "metaClaim",
            Self::Reasoning => "reasoning",
            // The two relationship classes name their ends alike (ADR 0034
            // amendment 2).
            Self::Source | Self::ArtifactSource => "source",
            Self::Target | Self::ArtifactTarget => "target",
            Self::Structure => "structure",
            Self::ReferencedArtifactElement => "referencedArtifactElement",
            Self::GroupMember => "argumentElement",
            Self::ArtifactGroupMember => "artifactElement",
            Self::Category => "category",
            Self::Element => "element",
            Self::Origin => "origin",
            Self::TerminologyGroupMember => "terminologyElement",
            Self::Interface => "interface",
            Self::Implements => "implements",
            Self::ParticipantPackage => "participantPackage",
            Self::Expression => "expression",
        }
    }

    /// The clause that defines the end on an element of `kind`, the clause
    /// of the class that declares it there: the [`clause`](Self::clause), but
    /// §10.8 for the `category` of a `Category`, whose class declares it
    /// apart from an `ExpressionElement`'s (§10.9), and the clauses of the
    /// Assurance Case, Terminology and Artifact families for their kinds'
    /// `interface` (§9.2, §10.4, §12.2), `implements` (§9.3, §10.5, §12.5)
    /// and `participantPackage` (§9.4, §10.6, §12.4).
    pub(crate) const fn clause_on(self, kind: ElementKind) -> &'static str {
        use MetamodelPackage as P;
        match (self, kind.package()) {
            (Self::Category, _) if matches!(kind, ElementKind::Category) => "10.8",
            (Self::Interface, P::AssuranceCase) => "9.2",
            (Self::Implements, P::AssuranceCase) => "9.3",
            (Self::ParticipantPackage, P::AssuranceCase) => "9.4",
            (Self::Interface, P::Terminology) => "10.4",
            (Self::Implements, P::Terminology) => "10.5",
            (Self::ParticipantPackage, P::Terminology) => "10.6",
            (Self::Interface, P::Artifact) => "12.2",
            (Self::Implements, P::Artifact) => "12.5",
            (Self::ParticipantPackage, P::Artifact) => "12.4",
            _ => self.clause(),
        }
    }

    /// The clause that defines the end.
    pub(crate) const fn clause(self) -> &'static str {
        match self {
            Self::CitedElement | Self::AbstractForm => "8.2",
            Self::GroupMember => "11.2",
            Self::ReferencedArtifactElement => "11.9",
            Self::MetaClaim => "11.10",
            Self::Structure => "11.12",
            Self::Reasoning | Self::Source | Self::Target => "11.13",
            Self::ArtifactGroupMember => "12.3",
            Self::ArtifactSource | Self::ArtifactTarget => "12.14",
            Self::TerminologyGroupMember => "10.3",
            Self::Category => "10.9",
            Self::Element => "10.10",
            Self::Origin => "10.11",
            // The Argumentation clauses; `clause_on` gives the Artifact ones.
            Self::Interface => "11.4",
            Self::Implements => "11.6",
            Self::ParticipantPackage => "11.5",
            Self::Expression => "8.4",
        }
    }

    /// Whether the end holds any number of targets (`[0..*]` or `[1..*]`),
    /// rather than at most one.
    pub(crate) const fn is_many(self) -> bool {
        matches!(
            self,
            Self::MetaClaim
                | Self::Source
                | Self::ReferencedArtifactElement
                | Self::GroupMember
                | Self::ArtifactGroupMember
                | Self::ArtifactSource
                | Self::ArtifactTarget
                | Self::Category
                | Self::Element
                | Self::TerminologyGroupMember
                | Self::Interface
                | Self::ParticipantPackage
                | Self::Expression
        )
    }

    /// Whether the end is derived from the element's name and texts rather
    /// than held as written: only [`Expression`](Self::Expression). The
    /// writer, the element reader, the comparison of ends, the repeat rule
    /// and the reference edits leave it out (SACM23-BASE-009, ADR 0038 D3).
    pub(crate) const fn is_derived(self) -> bool {
        matches!(self, Self::Expression)
    }

    /// Whether an element of `kind` has the end: every element has
    /// `citedElement` and `abstractForm` (§8.2) and the derived `expression`
    /// (§8.4), every Assertion has
    /// `metaClaim` (§11.10), every `AssertedRelationship` has `reasoning`,
    /// `source` and `target` (§11.13), an `ArtifactAssetRelationship` has
    /// its own `source` and `target` (§12.14), every `TerminologyAsset` (a
    /// Category, a Term or an Expression) has `category` (§10.8, §10.9), and
    /// each other end belongs to one kind, as its variant says.
    ///
    /// It is true exactly when a
    /// [`SetReferences::clear`](crate::edit::SetReferences::clear) of the
    /// end on such an element is not
    /// [`FeatureNotApplicable`](crate::edit::EditError::FeatureNotApplicable),
    /// and [`SacmElement::references`](crate::model::SacmElement::references)
    /// of an end the kind lacks holds no id.
    ///
    /// ```
    /// use sacm::model::{ElementKind, ReferenceEnd};
    ///
    /// assert!(ReferenceEnd::MetaClaim.applies_to(ElementKind::Claim));
    /// assert!(!ReferenceEnd::Source.applies_to(ElementKind::Claim));
    /// // An ArtifactGroup's members are its artifactElement, not its argumentElement.
    /// assert!(!ReferenceEnd::GroupMember.applies_to(ElementKind::ArtifactGroup));
    /// // Every package family declares interface, typed to the family.
    /// assert!(ReferenceEnd::Interface.applies_to(ElementKind::ArtifactPackageBinding));
    /// assert!(ReferenceEnd::Implements.applies_to(ElementKind::TerminologyPackageInterface));
    /// ```
    #[must_use]
    pub const fn applies_to(self, kind: ElementKind) -> bool {
        self.owner_on(kind).is_some()
    }

    /// The fewest and the most ids the end holds on an element of `kind`, as
    /// `(lower, upper)` with `None` for no upper bound; `None` when the kind
    /// has no such end ([`applies_to`](Self::applies_to)).
    ///
    /// The bounds depend on the owner: §11.13 (p. 40) gives an
    /// `AssertedRelationship` `source:ArgumentAsset[1..*]` and
    /// `target:ArgumentAsset[1]`, §12.14 (p. 50) gives an
    /// `ArtifactAssetRelationship` `source:ArtifactAsset[1..*]` and
    /// `target:ArtifactAsset[1..*]`, and the other ends are `[0..1]` or
    /// `[0..*]`. An interface's `implements` is `[1]` (§9.3, §10.5, §11.6,
    /// §12.5) and a binding's `participantPackage` `[2..*]` (§9.4, §11.5,
    /// §12.4; ptc/22-03-13 for a `TerminologyPackageBinding`); a lower bound
    /// counts distinct ids (ADR 0039 D3).
    /// They are the bounds [`SetReferences`](crate::edit::SetReferences),
    /// [`AddReference`](crate::edit::AddReference) and
    /// [`RemoveReference`](crate::edit::RemoveReference) keep an end within
    /// ([`TooFewReferences`](crate::edit::EditError::TooFewReferences),
    /// [`TooManyReferences`](crate::edit::EditError::TooManyReferences)),
    /// except the derived [`Expression`](Self::Expression), which they refuse
    /// as [`DerivedEnd`](crate::edit::EditError::DerivedEnd).
    ///
    /// ```
    /// use sacm::model::{ElementKind, ReferenceEnd};
    ///
    /// assert_eq!(ReferenceEnd::Source.bounds(ElementKind::AssertedInference), Some((1, None)));
    /// assert_eq!(ReferenceEnd::Target.bounds(ElementKind::AssertedInference), Some((1, Some(1))));
    /// assert_eq!(ReferenceEnd::MetaClaim.bounds(ElementKind::Claim), Some((0, None)));
    /// assert_eq!(ReferenceEnd::Source.bounds(ElementKind::Claim), None);
    /// let link = ElementKind::ArtifactAssetRelationship;
    /// assert_eq!(ReferenceEnd::ArtifactTarget.bounds(link), Some((1, None)));
    /// assert_eq!(ReferenceEnd::Target.bounds(link), None);
    /// assert_eq!(ReferenceEnd::ParticipantPackage.bounds(ElementKind::ArtifactPackageBinding), Some((2, None)));
    /// assert_eq!(ReferenceEnd::Implements.bounds(ElementKind::ArgumentPackage), None);
    /// assert_eq!(ReferenceEnd::ParticipantPackage.bounds(ElementKind::AssuranceCasePackageBinding), Some((2, None)));
    /// ```
    #[must_use]
    pub const fn bounds(self, kind: ElementKind) -> Option<(usize, Option<usize>)> {
        if self.applies_to(kind) {
            Some((self.lower_bound(), self.upper_bound()))
        } else {
            None
        }
    }

    /// The fewest distinct ids the end holds: 1 for an
    /// `AssertedRelationship`'s `source` (`[1..*]`) and `target` (`[1]`), for
    /// an `ArtifactAssetRelationship`'s `source` and `target` (`[1..*]`) and
    /// for an interface's `implements` (`[1]`), 2 for a binding's
    /// `participantPackage` (`[2..*]`), and 0 for every other end.
    pub(crate) const fn lower_bound(self) -> usize {
        match self {
            Self::Source
            | Self::Target
            | Self::ArtifactSource
            | Self::ArtifactTarget
            | Self::Implements => 1,
            Self::ParticipantPackage => 2,
            _ => 0,
        }
    }

    /// The most ids the end holds: `Some(1)` for a single-valued end, `None`
    /// for a multi-valued one ([`is_many`](Self::is_many)).
    pub(crate) const fn upper_bound(self) -> Option<usize> {
        if self.is_many() { None } else { Some(1) }
    }

    /// The classes that declare the end, each with the declared type of its
    /// targets there, in the order [`owner_on`](Self::owner_on) tries them.
    /// Most ends have one declaring class; `interface`, `implements` and
    /// `participantPackage` are declared by one class of each package family,
    /// each typed to its own family (ADR 0039 D1).
    pub(crate) const fn declarations(self) -> &'static [(DeclaredType, DeclaredType)] {
        use AbstractClass as A;
        use DeclaredType::{Abstract, Kind};
        use ElementKind as K;
        match self {
            Self::CitedElement | Self::AbstractForm => {
                &[(Abstract(A::SACMElement), Abstract(A::SACMElement))]
            }
            Self::Expression => &[(Abstract(A::SACMElement), Abstract(A::ExpressionElement))],
            Self::MetaClaim => &[(Abstract(A::Assertion), Kind(K::Claim))],
            Self::Reasoning => &[(
                Abstract(A::AssertedRelationship),
                Kind(K::ArgumentReasoning),
            )],
            Self::Source | Self::Target => &[(
                Abstract(A::AssertedRelationship),
                Abstract(A::ArgumentAsset),
            )],
            Self::Structure => &[(Kind(K::ArgumentReasoning), Kind(K::ArgumentPackage))],
            Self::ReferencedArtifactElement => {
                &[(Kind(K::ArtifactReference), Abstract(A::ArtifactElement))]
            }
            Self::GroupMember => &[(Kind(K::ArgumentGroup), Abstract(A::ArgumentationElement))],
            Self::ArtifactGroupMember => &[(Kind(K::ArtifactGroup), Abstract(A::ArtifactElement))],
            Self::ArtifactSource | Self::ArtifactTarget => &[(
                Kind(K::ArtifactAssetRelationship),
                Abstract(A::ArtifactAsset),
            )],
            Self::Category => &[(Abstract(A::TerminologyAsset), Kind(K::Category))],
            Self::Element => &[(Kind(K::Expression), Abstract(A::ExpressionElement))],
            Self::Origin => &[(Kind(K::Term), Abstract(A::ModelElement))],
            Self::TerminologyGroupMember => {
                &[(Kind(K::TerminologyGroup), Abstract(A::TerminologyElement))]
            }
            Self::Interface => &[
                (Kind(K::ArgumentPackage), Kind(K::ArgumentPackageInterface)),
                (Kind(K::ArtifactPackage), Kind(K::ArtifactPackageInterface)),
                (
                    Kind(K::AssuranceCasePackage),
                    Kind(K::AssuranceCasePackageInterface),
                ),
                (
                    Kind(K::TerminologyPackage),
                    Kind(K::TerminologyPackageInterface),
                ),
            ],
            Self::Implements => &[
                (Kind(K::ArgumentPackageInterface), Kind(K::ArgumentPackage)),
                (Kind(K::ArtifactPackageInterface), Kind(K::ArtifactPackage)),
                (
                    Kind(K::AssuranceCasePackageInterface),
                    Kind(K::AssuranceCasePackage),
                ),
                (
                    Kind(K::TerminologyPackageInterface),
                    Kind(K::TerminologyPackage),
                ),
            ],
            Self::ParticipantPackage => &[
                (Kind(K::ArgumentPackageBinding), Kind(K::ArgumentPackage)),
                (Kind(K::ArtifactPackageBinding), Kind(K::ArtifactPackage)),
                (
                    Kind(K::AssuranceCasePackageBinding),
                    Kind(K::AssuranceCasePackage),
                ),
                (
                    Kind(K::TerminologyPackageBinding),
                    Kind(K::TerminologyPackage),
                ),
            ],
        }
    }

    /// The first declaration whose class admits `kind`, if any.
    const fn declaration_on(self, kind: ElementKind) -> Option<(DeclaredType, DeclaredType)> {
        let declarations = self.declarations();
        let mut index = 0;
        while index < declarations.len() {
            if declarations[index].0.admits(kind) {
                return Some(declarations[index]);
            }
            index += 1;
        }
        None
    }

    /// The class that declares the end on an element of `kind`, or `None`
    /// when the kind has no such end: the class with whose features
    /// `xmi::write` writes it.
    pub(crate) const fn owner_on(self, kind: ElementKind) -> Option<DeclaredType> {
        match self.declaration_on(kind) {
            Some((owner, _)) => Some(owner),
            None => None,
        }
    }

    /// The declared type of the end's targets on an element of `kind`, or
    /// `None` exactly when the kind has no such end
    /// ([`applies_to`](Self::applies_to)).
    pub(crate) const fn target_on(self, kind: ElementKind) -> Option<DeclaredType> {
        match self.declaration_on(kind) {
            Some((_, target)) => Some(target),
            None => None,
        }
    }

    /// The first class that declares the end: the class a message names for
    /// an end another kind has (ADR 0034 amendment 2).
    pub(crate) const fn first_owner(self) -> DeclaredType {
        self.declarations()[0].0
    }
}

impl fmt::Display for ReferenceEnd {
    /// Writes [`ReferenceEnd::name`]: `source` for both
    /// [`Source`](ReferenceEnd::Source) and
    /// [`ArtifactSource`](ReferenceEnd::ArtifactSource).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// The reference end named `local` that an element of `kind` has, if any:
/// never the derived `expression`, which is no attribute of the element
/// (SACM23-BASE-009).
pub(crate) fn reference_end_named(kind: ElementKind, local: &str) -> Option<ReferenceEnd> {
    ReferenceEnd::ALL
        .iter()
        .copied()
        .find(|end| !end.is_derived() && end.name() == local && end.applies_to(kind))
}

#[cfg(test)]
mod tests {
    use super::{Flag, OWNERS, ReferenceEnd, flag_named, reference_end_named};
    use crate::model::{AbstractClass, DeclaredType, ElementKind, role_named};

    /// `ALL` lists each enum in declaration order, the order `Ord` derives.
    #[test]
    fn all_is_in_declaration_order() {
        for (position, flag) in Flag::ALL.iter().enumerate() {
            assert_eq!(*flag as usize, position, "{flag} is out of place in ALL");
        }
        for (position, end) in ReferenceEnd::ALL.iter().enumerate() {
            assert_eq!(*end as usize, position, "{end:?} is out of place in ALL");
        }
    }

    #[test]
    fn every_kind_has_its_classes_ends_and_flags_by_name_and_no_other() {
        for &kind in ElementKind::ALL {
            for &end in ReferenceEnd::ALL {
                // The end of that name whose owner admits the kind: no kind
                // has two ends of one name (ADR 0034 amendment 2).
                // The derived end is never read as an attribute.
                let expected = ReferenceEnd::ALL.iter().copied().find(|other| {
                    !other.is_derived() && other.name() == end.name() && other.applies_to(kind)
                });
                assert_eq!(reference_end_named(kind, end.name()), expected, "{kind}");
                if end.applies_to(kind) && !end.is_derived() {
                    assert_eq!(expected, Some(end), "{kind}");
                }
            }
            for &flag in Flag::ALL {
                let expected = flag.owner().admits(kind).then_some(flag);
                assert_eq!(flag_named(kind, flag.name()), expected, "{kind}");
            }
            // Every kind has the SACMElement ends and flags.
            assert!(reference_end_named(kind, "citedElement").is_some());
            assert!(flag_named(kind, "isAbstract").is_some());
            assert_eq!(reference_end_named(kind, "isCitation"), None);
            assert_eq!(reference_end_named(kind, "citedelement"), None);
            assert_eq!(reference_end_named(kind, "expression"), None);
            assert!(ReferenceEnd::Expression.applies_to(kind));
            assert_eq!(flag_named(kind, "citedElement"), None);
        }
        let relationship = ElementKind::AssertedInference;
        assert!(relationship.is_a(AbstractClass::AssertedRelationship));
        assert_eq!(flag_named(relationship, "isCounter"), Some(Flag::IsCounter));
        assert_eq!(flag_named(ElementKind::Claim, "isCounter"), None);
        assert_eq!(
            reference_end_named(ElementKind::Claim, "metaClaim"),
            Some(ReferenceEnd::MetaClaim)
        );
        assert_eq!(reference_end_named(ElementKind::Claim, "source"), None);
        assert_eq!(
            reference_end_named(ElementKind::ArgumentReasoning, "structure"),
            Some(ReferenceEnd::Structure)
        );
        assert_eq!(
            reference_end_named(ElementKind::ArgumentReasoning, "metaClaim"),
            None
        );
        // The ends of the two relationship classes share their names.
        let link = ElementKind::ArtifactAssetRelationship;
        assert_eq!(
            (
                reference_end_named(link, "source"),
                reference_end_named(link, "target"),
                reference_end_named(link, "reasoning"),
            ),
            (
                Some(ReferenceEnd::ArtifactSource),
                Some(ReferenceEnd::ArtifactTarget),
                None
            )
        );
        assert_eq!(
            reference_end_named(relationship, "target"),
            Some(ReferenceEnd::Target)
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one table per property of the ends reads best together"
    )]
    fn names_are_the_models_spelling_in_write_order() {
        assert_eq!(
            Flag::ALL
                .iter()
                .copied()
                .map(Flag::name)
                .collect::<Vec<_>>(),
            ["isCitation", "isAbstract", "isCounter"]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::name)
                .collect::<Vec<_>>(),
            [
                "citedElement",
                "abstractForm",
                "metaClaim",
                "reasoning",
                "source",
                "target",
                "structure",
                "referencedArtifactElement",
                "argumentElement",
                "artifactElement",
                "source",
                "target",
                "category",
                "element",
                "origin",
                "terminologyElement",
                "interface",
                "implements",
                "participantPackage",
                "expression"
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::clause)
                .collect::<Vec<_>>(),
            [
                "8.2", "8.2", "11.10", "11.13", "11.13", "11.13", "11.12", "11.9", "11.2", "12.3",
                "12.14", "12.14", "10.9", "10.10", "10.11", "10.3", "11.4", "11.6", "11.5", "8.4"
            ]
        );
        // A Category's category is declared by §10.8, an ExpressionElement's
        // by §10.9.
        assert_eq!(
            [
                ElementKind::Category,
                ElementKind::Term,
                ElementKind::Expression
            ]
            .map(|kind| ReferenceEnd::Category.clause_on(kind)),
            ["10.8", "10.9", "10.9"]
        );
        assert_eq!(ReferenceEnd::Origin.clause_on(ElementKind::Term), "10.11");
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::is_many)
                .collect::<Vec<_>>(),
            [
                false, false, true, false, true, false, false, true, true, true, true, true, true,
                true, false, true, true, false, true, true
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::lower_bound)
                .collect::<Vec<_>>(),
            [0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 2, 0]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::upper_bound)
                .collect::<Vec<_>>(),
            [
                Some(1),
                Some(1),
                None,
                Some(1),
                None,
                Some(1),
                Some(1),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(1),
                None,
                None,
                Some(1),
                None,
                None
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .filter(|end| end.is_derived())
                .collect::<Vec<_>>(),
            [ReferenceEnd::Expression]
        );
        assert_eq!(
            ReferenceEnd::Expression.clause_on(ElementKind::Claim),
            "8.4"
        );
        // The ends of slice 0066 take the clause of the class that declares
        // them on the holder (ADR 0039 D1).
        assert_eq!(
            [
                ReferenceEnd::Interface,
                ReferenceEnd::Implements,
                ReferenceEnd::ParticipantPackage
            ]
            .map(|end| {
                [
                    ElementKind::ArgumentPackageInterface,
                    ElementKind::ArtifactPackageInterface,
                    ElementKind::ArtifactPackageBinding,
                    ElementKind::AssuranceCasePackageBinding,
                    ElementKind::TerminologyPackage,
                ]
                .map(|kind| end.clause_on(kind))
            }),
            [
                ["11.4", "12.2", "12.2", "9.2", "10.4"],
                ["11.6", "12.5", "12.5", "9.3", "10.5"],
                ["11.5", "12.4", "12.4", "9.4", "10.6"]
            ]
        );
        // Every owner is in OWNERS, so the writer writes every feature.
        for owner in ReferenceEnd::ALL
            .iter()
            .flat_map(|end| end.declarations().iter().map(|(owner, _)| *owner))
            .chain(Flag::ALL.iter().copied().map(Flag::owner))
        {
            assert!(OWNERS.contains(&owner), "{}", owner.name());
        }
    }

    #[test]
    fn targets_are_typed_as_the_model_declares() {
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(|end| end.declarations()[0].1.name())
                .collect::<Vec<_>>(),
            [
                "SACMElement",
                "SACMElement",
                "Claim",
                "ArgumentReasoning",
                "ArgumentAsset",
                "ArgumentAsset",
                "ArgumentPackage",
                "ArtifactElement",
                "ArgumentationElement",
                "ArtifactElement",
                "ArtifactAsset",
                "ArtifactAsset",
                "Category",
                "ExpressionElement",
                "ModelElement",
                "TerminologyElement",
                "ArgumentPackageInterface",
                "ArgumentPackage",
                "ArgumentPackage",
                "ExpressionElement"
            ]
        );
    }

    /// The parent's kind decides how `argumentElement` reads: no class has a
    /// composite role and a reference end of the same name (ADR 0028).
    #[test]
    fn no_class_has_a_role_and_an_end_of_the_same_name() {
        for &kind in ElementKind::ALL {
            for &end in ReferenceEnd::ALL {
                if end.applies_to(kind) {
                    assert_eq!(role_named(kind, end.name()), None, "{kind} {}", end.name());
                }
            }
        }
        assert_eq!(
            reference_end_named(ElementKind::ArgumentGroup, "argumentElement"),
            Some(ReferenceEnd::GroupMember)
        );
        assert_eq!(
            reference_end_named(ElementKind::ArgumentPackage, "argumentElement"),
            None
        );
        assert!(role_named(ElementKind::ArgumentPackage, "argumentElement").is_some());
        // The same holds for `artifactElement` (ADR 0034).
        assert_eq!(
            reference_end_named(ElementKind::ArtifactGroup, "artifactElement"),
            Some(ReferenceEnd::ArtifactGroupMember)
        );
        assert_eq!(
            role_named(ElementKind::ArtifactGroup, "artifactElement"),
            None
        );
        assert_eq!(
            reference_end_named(ElementKind::ArtifactPackage, "artifactElement"),
            None
        );
        assert!(role_named(ElementKind::ArtifactPackage, "artifactElement").is_some());
        // Neither group has the other's end.
        assert_eq!(
            reference_end_named(ElementKind::ArtifactGroup, "argumentElement"),
            None
        );
        assert_eq!(
            reference_end_named(ElementKind::ArgumentGroup, "artifactElement"),
            None
        );
        // And for `terminologyElement` (ADR 0034 amendment 3).
        assert_eq!(
            reference_end_named(ElementKind::TerminologyGroup, "terminologyElement"),
            Some(ReferenceEnd::TerminologyGroupMember)
        );
        assert_eq!(
            role_named(ElementKind::TerminologyGroup, "terminologyElement"),
            None
        );
        assert_eq!(
            reference_end_named(ElementKind::TerminologyPackage, "terminologyElement"),
            None
        );
        assert!(role_named(ElementKind::TerminologyPackage, "terminologyElement").is_some());
    }

    #[test]
    fn owners_are_the_classes_of_the_features_superclass_first() {
        assert_eq!(
            OWNERS.map(DeclaredType::name),
            [
                "SACMElement",
                "Assertion",
                "AssertedRelationship",
                "ArgumentReasoning",
                "ArtifactReference",
                "ArgumentGroup",
                "ArtifactGroup",
                "ArtifactAssetRelationship",
                "TerminologyAsset",
                "Expression",
                "Term",
                "TerminologyGroup",
                "ArgumentPackage",
                "ArgumentPackageInterface",
                "ArgumentPackageBinding",
                "ArtifactPackage",
                "ArtifactPackageInterface",
                "ArtifactPackageBinding",
                "AssuranceCasePackage",
                "AssuranceCasePackageInterface",
                "AssuranceCasePackageBinding",
                "TerminologyPackage",
                "TerminologyPackageInterface",
                "TerminologyPackageBinding"
            ]
        );
    }

    /// `owner_on` and `target_on` take the first declaration whose class
    /// admits the kind: the family's (ADR 0039 D1).
    #[test]
    fn the_ends_of_two_families_are_typed_to_the_holder_s_family() {
        use DeclaredType::Kind;
        use ElementKind as K;
        let interface = ReferenceEnd::Interface;
        assert_eq!(
            (
                interface.owner_on(K::ArgumentPackageBinding),
                interface.target_on(K::ArgumentPackageBinding),
                interface.owner_on(K::ArtifactPackageInterface),
                interface.target_on(K::ArtifactPackageInterface),
                interface.owner_on(K::TerminologyPackageBinding),
                interface.target_on(K::AssuranceCasePackageInterface),
                interface.target_on(K::Claim),
            ),
            (
                Some(Kind(K::ArgumentPackage)),
                Some(Kind(K::ArgumentPackageInterface)),
                Some(Kind(K::ArtifactPackage)),
                Some(Kind(K::ArtifactPackageInterface)),
                Some(Kind(K::TerminologyPackage)),
                Some(Kind(K::AssuranceCasePackageInterface)),
                None
            )
        );
        assert_eq!(
            (
                ReferenceEnd::Implements.target_on(K::ArtifactPackageInterface),
                ReferenceEnd::ParticipantPackage.target_on(K::ArgumentPackageBinding),
                ReferenceEnd::ParticipantPackage.owner_on(K::ArgumentPackageInterface),
                ReferenceEnd::Implements.first_owner(),
            ),
            (
                Some(Kind(K::ArtifactPackage)),
                Some(Kind(K::ArgumentPackage)),
                None,
                Kind(K::ArgumentPackageInterface)
            )
        );
        for &end in ReferenceEnd::ALL {
            for &kind in ElementKind::ALL {
                assert_eq!(
                    end.owner_on(kind).is_some(),
                    end.target_on(kind).is_some(),
                    "{kind}"
                );
            }
        }
    }
}
