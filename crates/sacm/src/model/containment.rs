//! Containment legality: which SACM 2.3 composite role, if any, admits an
//! element of one kind directly inside an element of another (LIB-API-008).
//!
//! Crate-private, as the C++ library's role tables are. The roles are the 12
//! composite roles of SACM 2.3 whose type is an element kind; the value-typed
//! ones (`name`, `content`, `key`, `value`) hold values, not elements. Every
//! role is `0..*` and unordered, and the document does not count children:
//! multiplicity is for validation (D-VAL-LAYER).

use super::{AbstractClass, ElementKind};

/// A SACM 2.3 composite role whose type is an element kind: a way one
/// element can contain another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContainmentRole {
    /// `ModelElement::description`: a [`ElementKind::Description`] (§8.6).
    Description,
    /// `ModelElement::implementationConstraint`: an
    /// [`ElementKind::ImplementationConstraint`] (§8.6).
    ImplementationConstraint,
    /// `ModelElement::note`: a [`ElementKind::Note`] (§8.6).
    Note,
    /// `ModelElement::taggedValue`: a [`ElementKind::TaggedValue`] (§8.6).
    TaggedValue,
    /// `AssuranceCasePackage::assuranceCasePackage`: an assurance case
    /// package, interface or binding (§9.2).
    AssuranceCasePackage,
    /// `AssuranceCasePackage::argumentPackage`: an argument package,
    /// interface or binding (§9.2).
    ArgumentPackage,
    /// `AssuranceCasePackage::artifactPackage`: an artifact package,
    /// interface or binding (§9.2).
    ArtifactPackage,
    /// `AssuranceCasePackage::terminologyPackage`: a terminology package,
    /// interface or binding (§9.2).
    TerminologyPackage,
    /// `ArgumentPackage::argumentElement`: any `ArgumentationElement`
    /// (§11.4).
    ArgumentElement,
    /// `TerminologyPackage::terminologyElement`: any `TerminologyElement`,
    /// `TerminologyGroup` included by the declared type (§10.4, spec defect
    /// D9).
    TerminologyElement,
    /// `ArtifactPackage::artifactElement`: any `ArtifactElement` (§12.2,
    /// ADR 0014).
    ArtifactElement,
    /// `ArtifactAsset::property`: a [`ElementKind::Property`] (§12.6), held by
    /// every `ArtifactAsset`, `ArtifactAssetRelationship` included (§12.14).
    Property,
}

/// The role of `parent`'s class that admits a `child` element, or `None` if
/// no role does.
///
/// Package interfaces and bindings specialize their package, so they hold its
/// roles. A `UtilityElement` contains nothing. A group (`ArgumentGroup`,
/// `ArtifactGroup`, `TerminologyGroup`) has only the `ModelElement` roles: its
/// members are references, not children. Each legal pair has exactly one
/// role.
pub(crate) const fn role_of(parent: ElementKind, child: ElementKind) -> Option<ContainmentRole> {
    // Only a ModelElement has composite roles; a UtilityElement has none.
    if !parent.is_a(AbstractClass::ModelElement) {
        return None;
    }
    // The ModelElement roles (§8.6), on every ModelElement.
    match child {
        ElementKind::Description => return Some(ContainmentRole::Description),
        ElementKind::ImplementationConstraint => {
            return Some(ContainmentRole::ImplementationConstraint);
        }
        ElementKind::Note => return Some(ContainmentRole::Note),
        ElementKind::TaggedValue => return Some(ContainmentRole::TaggedValue),
        _ => {}
    }
    if parent.is_kind_of(ElementKind::AssuranceCasePackage) {
        if child.is_kind_of(ElementKind::AssuranceCasePackage) {
            Some(ContainmentRole::AssuranceCasePackage)
        } else if child.is_kind_of(ElementKind::ArgumentPackage) {
            Some(ContainmentRole::ArgumentPackage)
        } else if child.is_kind_of(ElementKind::ArtifactPackage) {
            Some(ContainmentRole::ArtifactPackage)
        } else if child.is_kind_of(ElementKind::TerminologyPackage) {
            Some(ContainmentRole::TerminologyPackage)
        } else {
            None
        }
    } else if parent.is_kind_of(ElementKind::ArgumentPackage) {
        if child.is_a(AbstractClass::ArgumentationElement) {
            Some(ContainmentRole::ArgumentElement)
        } else {
            None
        }
    } else if parent.is_kind_of(ElementKind::TerminologyPackage) {
        if child.is_a(AbstractClass::TerminologyElement) {
            Some(ContainmentRole::TerminologyElement)
        } else {
            None
        }
    } else if parent.is_kind_of(ElementKind::ArtifactPackage) {
        if child.is_a(AbstractClass::ArtifactElement) {
            Some(ContainmentRole::ArtifactElement)
        } else {
            None
        }
    } else if parent.is_a(AbstractClass::ArtifactAsset) {
        if child.is_kind_of(ElementKind::Property) {
            Some(ContainmentRole::Property)
        } else {
            None
        }
    } else {
        None
    }
}
