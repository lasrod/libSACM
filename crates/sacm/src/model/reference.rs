//! The Boolean flags and the non-containment reference ends of the classes
//! the model holds (formal/23-05-08 §8.2, §11.2, §11.9, §11.10, §11.12,
//! §11.13; SACM23-BASE-007, SACM23-BASE-008, SACM23-XMI-005, SACM23-ARG-004,
//! SACM23-ARG-005, SACM23-ARG-006, SACM23-ARG-007, SACM23-ARG-013; ADR 0026,
//! ADR 0027, ADR 0028).
//!
//! Two crate-private tables, [`Flag::ALL`] and [`ReferenceEnd::ALL`], which
//! the XMI reader, the writer, the id pre-pass, comparison and validation
//! iterate, so that the name read, the name written and the order written
//! cannot drift apart. Each entry names the class that owns it, and
//! [`OWNERS`] orders those classes superclass first, the order `xmi::write`
//! writes their features in. A later end adds one entry here, one accessor
//! and one `Difference` variant. A multi-valued end ([`ReferenceEnd::is_many`])
//! holds every id of its attribute, and each reference element appends one;
//! a single-valued one holds at most one. Crate-private, as the role tables
//! of [`containment`](super::containment) are.

use super::{AbstractClass, DeclaredType, ElementKind};

/// The classes that own a flag, the `assertionDeclaration` or a reference
/// end, superclass first: the order in which `xmi::write` writes each one's
/// features (XMI 2.5.1 Annex B.5.2; ADR 0027, ADR 0028). Within a class it
/// writes the flags, then, on `Assertion`, the declaration, then the ends.
pub(crate) const OWNERS: [DeclaredType; 6] = [
    DeclaredType::Abstract(AbstractClass::SACMElement),
    DeclaredType::Abstract(AbstractClass::Assertion),
    DeclaredType::Abstract(AbstractClass::AssertedRelationship),
    DeclaredType::Kind(ElementKind::ArgumentReasoning),
    DeclaredType::Kind(ElementKind::ArtifactReference),
    DeclaredType::Kind(ElementKind::ArgumentGroup),
];

/// A Boolean property of an element, `false` by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::enum_variant_names,
    reason = "the variants are the metamodel's names: isCitation, isAbstract, isCounter"
)]
pub(crate) enum Flag {
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
    /// Every flag, in the order `compare` reports them.
    pub(crate) const ALL: [Flag; 3] = [Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter];

    /// The flag's name, spelled as in the machine-readable model.
    pub(crate) const fn name(self) -> &'static str {
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
        .into_iter()
        .find(|flag| flag.name() == local && flag.owner().admits(kind))
}

/// A non-containment reference end, which holds the ids of its targets
/// unresolved (ADR 0026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceEnd {
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
    GroupMember,
}

impl ReferenceEnd {
    /// Every end, in the order `xmi::write` writes the ends of one class and
    /// `compare` reports them (ptc/22-03-13 order within each class).
    pub(crate) const ALL: [ReferenceEnd; 9] = [
        ReferenceEnd::CitedElement,
        ReferenceEnd::AbstractForm,
        ReferenceEnd::MetaClaim,
        ReferenceEnd::Reasoning,
        ReferenceEnd::Source,
        ReferenceEnd::Target,
        ReferenceEnd::Structure,
        ReferenceEnd::ReferencedArtifactElement,
        ReferenceEnd::GroupMember,
    ];

    /// The end's name, spelled as in the machine-readable model.
    pub(crate) const fn name(self) -> &'static str {
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
        }
    }

    /// Whether the end holds any number of targets (`[0..*]` or `[1..*]`),
    /// rather than at most one.
    pub(crate) const fn is_many(self) -> bool {
        matches!(
            self,
            Self::MetaClaim | Self::Source | Self::ReferencedArtifactElement | Self::GroupMember
        )
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
            Self::ReferencedArtifactElement => {
                DeclaredType::Abstract(AbstractClass::ArtifactElement)
            }
            Self::GroupMember => DeclaredType::Abstract(AbstractClass::ArgumentationElement),
        }
    }
}

/// The reference end named `local` that an element of `kind` has, if any.
pub(crate) fn reference_end_named(kind: ElementKind, local: &str) -> Option<ReferenceEnd> {
    ReferenceEnd::ALL
        .into_iter()
        .find(|end| end.name() == local && end.owner().admits(kind))
}

#[cfg(test)]
mod tests {
    use super::{Flag, OWNERS, ReferenceEnd, flag_named, reference_end_named};
    use crate::model::{AbstractClass, ElementKind, role_named};

    #[test]
    fn every_kind_has_its_classes_ends_and_flags_by_name_and_no_other() {
        for &kind in ElementKind::ALL {
            for end in ReferenceEnd::ALL {
                let expected = end.owner().admits(kind).then_some(end);
                assert_eq!(reference_end_named(kind, end.name()), expected, "{kind}");
            }
            for flag in Flag::ALL {
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
            Flag::ALL.map(Flag::name),
            ["isCitation", "isAbstract", "isCounter"]
        );
        assert_eq!(
            ReferenceEnd::ALL.map(ReferenceEnd::name),
            [
                "citedElement",
                "abstractForm",
                "metaClaim",
                "reasoning",
                "source",
                "target",
                "structure",
                "referencedArtifactElement",
                "argumentElement"
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL.map(ReferenceEnd::clause),
            [
                "8.2", "8.2", "11.10", "11.13", "11.13", "11.13", "11.12", "11.9", "11.2"
            ]
        );
        assert_eq!(
            ReferenceEnd::ALL.map(ReferenceEnd::is_many),
            [false, false, true, false, true, false, false, true, true]
        );
        // Every owner is in OWNERS, so the writer writes every feature.
        for owner in ReferenceEnd::ALL
            .map(ReferenceEnd::owner)
            .into_iter()
            .chain(Flag::ALL.map(Flag::owner))
        {
            assert!(OWNERS.contains(&owner), "{}", owner.name());
        }
    }

    #[test]
    fn targets_are_typed_as_the_model_declares() {
        assert_eq!(
            ReferenceEnd::ALL.map(|end| end.target().name()),
            [
                "SACMElement",
                "SACMElement",
                "Claim",
                "ArgumentReasoning",
                "ArgumentAsset",
                "ArgumentAsset",
                "ArgumentPackage",
                "ArtifactElement",
                "ArgumentationElement"
            ]
        );
    }

    /// The parent's kind decides how `argumentElement` reads: no class has a
    /// composite role and a reference end of the same name (ADR 0028).
    #[test]
    fn no_class_has_a_role_and_an_end_of_the_same_name() {
        for &kind in ElementKind::ALL {
            for end in ReferenceEnd::ALL {
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
    }
}
