//! Containment legality: whether SACM 2.3 lets an element of one kind be
//! contained directly in an element of another (LIB-API-008).
//!
//! Crate-private, as the C++ library's role tables are.

use super::{AbstractClass, ElementKind};

/// Whether a composite role of `parent`'s class admits a `child` element.
///
/// The roles are the 12 SACM 2.3 composite roles typed by an element kind:
/// `description`, `implementationConstraint`, `note` and `taggedValue` on
/// every `ModelElement` (§8.6); `assuranceCasePackage`, `argumentPackage`,
/// `artifactPackage` and `terminologyPackage` on `AssuranceCasePackage`
/// (§9.2); `argumentElement` on `ArgumentPackage` (§11.4);
/// `terminologyElement` on `TerminologyPackage` (§10.4, `TerminologyGroup`
/// included by the declared type, spec defect D9); `artifactElement` on
/// `ArtifactPackage`, admitting every `ArtifactElement` (§12.2, ADR 0014);
/// and `property` on every `ArtifactAsset` (§12.6, §12.14). Package
/// interfaces and bindings hold their package's roles. The value-typed
/// composites (`name`, `content`, `key`, `value`, `location`) hold values,
/// not elements. Every role is `0..*`; multiplicity is for validation. Role
/// identity and names come with SACM23-XMI-016.
pub(crate) const fn admits(parent: ElementKind, child: ElementKind) -> bool {
    // A UtilityElement has no composite roles.
    if !parent.is_a(AbstractClass::ModelElement) {
        return false;
    }
    // The ModelElement roles (§8.6), on every ModelElement.
    if child.is_a(AbstractClass::UtilityElement) {
        return true;
    }
    if parent.is_kind_of(ElementKind::AssuranceCasePackage) {
        child.is_kind_of(ElementKind::AssuranceCasePackage)
            || child.is_kind_of(ElementKind::ArgumentPackage)
            || child.is_kind_of(ElementKind::ArtifactPackage)
            || child.is_kind_of(ElementKind::TerminologyPackage)
    } else if parent.is_kind_of(ElementKind::ArgumentPackage) {
        child.is_a(AbstractClass::ArgumentationElement)
    } else if parent.is_kind_of(ElementKind::TerminologyPackage) {
        child.is_a(AbstractClass::TerminologyElement)
    } else if parent.is_kind_of(ElementKind::ArtifactPackage) {
        child.is_a(AbstractClass::ArtifactElement)
    } else if parent.is_a(AbstractClass::ArtifactAsset) {
        child.is_kind_of(ElementKind::Property)
    } else {
        // Groups and every other ModelElement: the ModelElement roles only.
        false
    }
}
