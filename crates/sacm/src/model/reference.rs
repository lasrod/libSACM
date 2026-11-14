//! The `SACMElement` flags and reference ends (formal/23-05-08 §8.2;
//! SACM23-BASE-007, SACM23-BASE-008, SACM23-XMI-005; ADR 0026).
//!
//! Two crate-private tables, [`Flag::ALL`] and [`ReferenceEnd::ALL`], which
//! the XMI reader, the writer, the id pre-pass, comparison and validation
//! iterate, so that the name read, the name written and the order written
//! cannot drift apart. A later end adds one entry here, one accessor and one
//! `Difference` variant. Crate-private, as the role tables of
//! [`containment`](super::containment) are.

use super::{AbstractClass, DeclaredType, ElementKind};

/// A Boolean property of every `SACMElement` (§8.2), `false` by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flag {
    /// `isCitation[0..1]=false`: whether the element cites another.
    IsCitation,
    /// `isAbstract[0..1]=false`: whether the element is abstract.
    IsAbstract,
}

impl Flag {
    /// Every flag, in the order `xmi::write` writes them.
    pub(crate) const ALL: [Flag; 2] = [Flag::IsCitation, Flag::IsAbstract];

    /// The flag's name, spelled as in the machine-readable model.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::IsCitation => "isCitation",
            Self::IsAbstract => "isAbstract",
        }
    }
}

/// A non-containment reference end, which holds the ids of its targets
/// unresolved (ADR 0026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferenceEnd {
    /// `citedElement:SACMElement[0..1]` of every `SACMElement` (§8.2).
    CitedElement,
    /// `abstractForm:SACMElement[0..1]` of every `SACMElement` (§8.2).
    AbstractForm,
}

impl ReferenceEnd {
    /// Every end, in the order `xmi::write` writes them, after the flags.
    pub(crate) const ALL: [ReferenceEnd; 2] =
        [ReferenceEnd::CitedElement, ReferenceEnd::AbstractForm];

    /// The end's name, spelled as in the machine-readable model.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::CitedElement => "citedElement",
            Self::AbstractForm => "abstractForm",
        }
    }

    /// Whether the end holds more than one target: its upper bound is `*`.
    pub(crate) const fn is_many(self) -> bool {
        match self {
            Self::CitedElement | Self::AbstractForm => false,
        }
    }

    /// The clause that defines the end.
    pub(crate) const fn clause(self) -> &'static str {
        match self {
            Self::CitedElement | Self::AbstractForm => "8.2",
        }
    }

    /// The class that owns the end: an element of a kind it admits has it.
    pub(crate) const fn owner(self) -> DeclaredType {
        match self {
            Self::CitedElement | Self::AbstractForm => {
                DeclaredType::Abstract(AbstractClass::SACMElement)
            }
        }
    }

    /// The declared type of the end's targets.
    pub(crate) const fn target(self) -> DeclaredType {
        match self {
            Self::CitedElement | Self::AbstractForm => {
                DeclaredType::Abstract(AbstractClass::SACMElement)
            }
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
    use super::{Flag, ReferenceEnd, reference_end_named};
    use crate::model::ElementKind;

    #[test]
    fn every_kind_has_both_ends_by_name_and_no_other() {
        for &kind in ElementKind::ALL {
            for end in ReferenceEnd::ALL {
                assert_eq!(reference_end_named(kind, end.name()), Some(end));
                assert!(end.target().admits(kind), "{kind}");
            }
            assert_eq!(reference_end_named(kind, "isCitation"), None);
            assert_eq!(reference_end_named(kind, "citedelement"), None);
        }
    }

    #[test]
    fn names_are_the_models_spelling_in_write_order() {
        assert_eq!(Flag::ALL.map(Flag::name), ["isCitation", "isAbstract"]);
        assert_eq!(
            ReferenceEnd::ALL.map(ReferenceEnd::name),
            ["citedElement", "abstractForm"]
        );
        assert!(ReferenceEnd::ALL.iter().all(|end| !end.is_many()));
        assert!(ReferenceEnd::ALL.iter().all(|end| end.clause() == "8.2"));
    }
}
