//! The Boolean flags and the non-containment reference ends of the classes
//! the model holds (formal/23-05-08 §8.2, §11.2, §11.9, §11.10, §11.12,
//! §11.13, §12.3; SACM23-BASE-007, SACM23-BASE-008, SACM23-XMI-005,
//! SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-006, SACM23-ARG-007,
//! SACM23-ARG-013, SACM23-ART-011; ADR 0026, ADR 0027, ADR 0028, ADR 0034).
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
//! a single-valued one holds at most one. The owners, the declared types
//! and the bounds by end alone are crate-private, as the role tables of
//! [`containment`](super::containment) are.

use super::{AbstractClass, DeclaredType, ElementKind};

/// The classes that own a flag, the `assertionDeclaration` or a reference
/// end, superclass first: the order in which `xmi::write` writes each one's
/// features (XMI 2.5.1 Annex B.5.2; ADR 0027, ADR 0028). Within a class it
/// writes the flags, then, on `Assertion`, the declaration, then the ends.
pub(crate) const OWNERS: [DeclaredType; 7] = [
    DeclaredType::Abstract(AbstractClass::SACMElement),
    DeclaredType::Abstract(AbstractClass::Assertion),
    DeclaredType::Abstract(AbstractClass::AssertedRelationship),
    DeclaredType::Kind(ElementKind::ArgumentReasoning),
    DeclaredType::Kind(ElementKind::ArtifactReference),
    DeclaredType::Kind(ElementKind::ArgumentGroup),
    DeclaredType::Kind(ElementKind::ArtifactGroup),
];

/// A Boolean property of an element, `false` by default: the flag a
/// [`SetFlag`](crate::edit::SetFlag) sets.
///
/// Every element has `isCitation` and `isAbstract` (SACM 2.3 §8.2); an
/// asserted relationship also has `isCounter` (§11.13). Later releases may
/// add flags, so a `match` on `Flag` needs a wildcard arm.
///
/// # Examples
///
/// ```
/// use sacm::model::Flag;
///
/// assert_eq!(Flag::IsCounter.name(), "isCounter");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
/// (`[0..1]`, or exactly one for `target`), or any number in order, repeats
/// included (`[0..*]`, or at least one for `source`). Later releases may add
/// ends, so a `match` on `ReferenceEnd` needs a wildcard arm.
///
/// # Examples
///
/// ```
/// use sacm::model::ReferenceEnd;
///
/// assert_eq!(ReferenceEnd::Source.name(), "source");
/// // A group's members are its `argumentElement` references, and an
/// // ArtifactGroup's its `artifactElement` references.
/// assert_eq!(ReferenceEnd::GroupMember.name(), "argumentElement");
/// assert_eq!(ReferenceEnd::ArtifactGroupMember.name(), "artifactElement");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
}

impl ReferenceEnd {
    /// Every end, in the order `xmi::write` writes them and `compare`
    /// reports them (ptc/22-03-13 order within each class, superclass
    /// first): `citedElement`, `abstractForm`, `metaClaim`, `reasoning`,
    /// `source`, `target`, `structure`, `referencedArtifactElement`, an
    /// `ArgumentGroup`'s `argumentElement` and an `ArtifactGroup`'s
    /// `artifactElement`.
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
    /// assert_eq!(ends, ["citedElement", "abstractForm", "metaClaim", "reasoning", "source", "target"]);
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
    ];

    /// The end's name, spelled as in the machine-readable model and in XMI:
    /// `citedElement`, `abstractForm`, `metaClaim`, `reasoning`, `source`,
    /// `target`, `structure`, `referencedArtifactElement` or, for
    /// [`GroupMember`](Self::GroupMember), `argumentElement` and, for
    /// [`ArtifactGroupMember`](Self::ArtifactGroupMember), `artifactElement`.
    /// It is the `end` an [`EditError`](crate::edit::EditError) names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CitedElement => "citedElement",
            Self::AbstractForm => "abstractForm",
            Self::MetaClaim => "metaClaim",
            Self::Reasoning => "reasoning",
            Self::Source => "source",
            Self::Target => "target",
            Self::Structure => "structure",
            Self::ReferencedArtifactElement => "referencedArtifactElement",
            Self::GroupMember => "argumentElement",
            Self::ArtifactGroupMember => "artifactElement",
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
        )
    }

    /// Whether an element of `kind` has the end: every element has
    /// `citedElement` and `abstractForm` (§8.2), every Assertion has
    /// `metaClaim` (§11.10), every `AssertedRelationship` has `reasoning`,
    /// `source` and `target` (§11.13), and each other end belongs to one
    /// kind, as its variant says.
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
    /// ```
    #[must_use]
    pub const fn applies_to(self, kind: ElementKind) -> bool {
        self.owner().admits(kind)
    }

    /// The fewest and the most ids the end holds on an element of `kind`, as
    /// `(lower, upper)` with `None` for no upper bound; `None` when the kind
    /// has no such end ([`applies_to`](Self::applies_to)).
    ///
    /// The bounds depend on the owner: §11.13 (p. 40) gives an
    /// `AssertedRelationship` `source:ArgumentAsset[1..*]` and
    /// `target:ArgumentAsset[1]`, and the other ends are `[0..1]` or `[0..*]`.
    /// They are the bounds [`SetReferences`](crate::edit::SetReferences),
    /// [`AddReference`](crate::edit::AddReference) and
    /// [`RemoveReference`](crate::edit::RemoveReference) keep an end within
    /// ([`TooFewReferences`](crate::edit::EditError::TooFewReferences),
    /// [`TooManyReferences`](crate::edit::EditError::TooManyReferences)).
    ///
    /// ```
    /// use sacm::model::{ElementKind, ReferenceEnd};
    ///
    /// assert_eq!(ReferenceEnd::Source.bounds(ElementKind::AssertedInference), Some((1, None)));
    /// assert_eq!(ReferenceEnd::Target.bounds(ElementKind::AssertedInference), Some((1, Some(1))));
    /// assert_eq!(ReferenceEnd::MetaClaim.bounds(ElementKind::Claim), Some((0, None)));
    /// assert_eq!(ReferenceEnd::Source.bounds(ElementKind::Claim), None);
    /// ```
    #[must_use]
    pub const fn bounds(self, kind: ElementKind) -> Option<(usize, Option<usize>)> {
        if self.applies_to(kind) {
            Some((self.lower_bound(), self.upper_bound()))
        } else {
            None
        }
    }

    /// The fewest ids the end holds: 1 for `source` (`[1..*]`) and `target`
    /// (`[1]`), 0 for every other end.
    pub(crate) const fn lower_bound(self) -> usize {
        match self {
            Self::Source | Self::Target => 1,
            _ => 0,
        }
    }

    /// The most ids the end holds: `Some(1)` for a single-valued end, `None`
    /// for a multi-valued one ([`is_many`](Self::is_many)).
    pub(crate) const fn upper_bound(self) -> Option<usize> {
        if self.is_many() { None } else { Some(1) }
    }

    /// The class that owns the end: an element of a kind it admits has it.
    pub(crate) const fn owner(self) -> DeclaredType {
        match self {
            Self::CitedElement | Self::AbstractForm => {
                DeclaredType::Abstract(AbstractClass::SACMElement)
            }
            Self::MetaClaim => DeclaredType::Abstract(AbstractClass::Assertion),
            Self::Reasoning | Self::Source | Self::Target => {
                DeclaredType::Abstract(AbstractClass::AssertedRelationship)
            }
            Self::Structure => DeclaredType::Kind(ElementKind::ArgumentReasoning),
            Self::ReferencedArtifactElement => DeclaredType::Kind(ElementKind::ArtifactReference),
            Self::GroupMember => DeclaredType::Kind(ElementKind::ArgumentGroup),
            Self::ArtifactGroupMember => DeclaredType::Kind(ElementKind::ArtifactGroup),
        }
    }

    /// The declared type of the end's targets.
    pub(crate) const fn target(self) -> DeclaredType {
        match self {
            Self::CitedElement | Self::AbstractForm => {
                DeclaredType::Abstract(AbstractClass::SACMElement)
            }
            Self::MetaClaim => DeclaredType::Kind(ElementKind::Claim),
            Self::Reasoning => DeclaredType::Kind(ElementKind::ArgumentReasoning),
            Self::Source | Self::Target => DeclaredType::Abstract(AbstractClass::ArgumentAsset),
            Self::Structure => DeclaredType::Kind(ElementKind::ArgumentPackage),
            Self::ReferencedArtifactElement | Self::ArtifactGroupMember => {
                DeclaredType::Abstract(AbstractClass::ArtifactElement)
            }
            Self::GroupMember => DeclaredType::Abstract(AbstractClass::ArgumentationElement),
        }
    }
}

/// The reference end named `local` that an element of `kind` has, if any.
pub(crate) fn reference_end_named(kind: ElementKind, local: &str) -> Option<ReferenceEnd> {
    ReferenceEnd::ALL
        .iter()
        .copied()
        .find(|end| end.name() == local && end.owner().admits(kind))
}

#[cfg(test)]
mod tests {
    use super::{Flag, OWNERS, ReferenceEnd, flag_named, reference_end_named};
    use crate::model::{AbstractClass, DeclaredType, ElementKind, role_named};

    #[test]
    fn every_kind_has_its_classes_ends_and_flags_by_name_and_no_other() {
        for &kind in ElementKind::ALL {
            for &end in ReferenceEnd::ALL {
                let expected = end.owner().admits(kind).then_some(end);
                assert_eq!(reference_end_named(kind, end.name()), expected, "{kind}");
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
    }

    #[test]
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
                "artifactElement"
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::clause)
                .collect::<Vec<_>>(),
            [
                "8.2", "8.2", "11.10", "11.13", "11.13", "11.13", "11.12", "11.9", "11.2", "12.3"
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::is_many)
                .collect::<Vec<_>>(),
            [
                false, false, true, false, true, false, false, true, true, true
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL
                .iter()
                .copied()
                .map(ReferenceEnd::lower_bound)
                .collect::<Vec<_>>(),
            [0, 0, 0, 0, 1, 1, 0, 0, 0, 0]
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
                None
            ]
        );
        // Every owner is in OWNERS, so the writer writes every feature.
        for owner in ReferenceEnd::ALL
            .iter()
            .copied()
            .map(ReferenceEnd::owner)
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
                .map(|end| end.target().name())
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
                "ArtifactElement"
            ]
        );
    }

    /// The parent's kind decides how `argumentElement` reads: no class has a
    /// composite role and a reference end of the same name (ADR 0028).
    #[test]
    fn no_class_has_a_role_and_an_end_of_the_same_name() {
        for &kind in ElementKind::ALL {
            for &end in ReferenceEnd::ALL {
                if end.owner().admits(kind) {
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
                "ArtifactGroup"
            ]
        );
    }
}
