//! Containment: which SACM 2.3 composite role of a parent's class holds a
//! child of a given kind (LIB-API-008, SACM23-XMI-006).
//!
//! One table, [`role`], gives both the role's name and its declared type, and
//! [`admits`] is derived from it, so containment legality and the role names
//! written to XMI cannot drift apart. Crate-private, as the C++ library's role
//! tables are.

use super::{AbstractClass, ElementKind};

/// A composite role of a SACM 2.3 class that holds elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Role {
    /// The role's name, spelled as in the machine-readable model.
    pub(crate) name: &'static str,
    /// The role's declared type.
    pub(crate) declared: DeclaredType,
}

/// The declared type of a [`Role`]: a concrete class or an abstract one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclaredType {
    /// A concrete class: the role holds that kind and the kinds that
    /// specialize it.
    Kind(ElementKind),
    /// An abstract class: the role holds every kind that specializes it.
    Abstract(AbstractClass),
}

impl DeclaredType {
    /// Whether an element of `kind` conforms to this type.
    const fn admits(self, kind: ElementKind) -> bool {
        match self {
            Self::Kind(declared) => kind.is_kind_of(declared),
            Self::Abstract(class) => kind.is_a(class),
        }
    }
}

/// A role named `name` with the declared type `declared`.
const fn role_of(name: &'static str, declared: DeclaredType) -> Role {
    Role { name, declared }
}

/// The roles of every `ModelElement` (§8.6).
const MODEL_ELEMENT_ROLES: &[Role] = &[
    role_of("description", DeclaredType::Kind(ElementKind::Description)),
    role_of(
        "implementationConstraint",
        DeclaredType::Kind(ElementKind::ImplementationConstraint),
    ),
    role_of("note", DeclaredType::Kind(ElementKind::Note)),
    role_of("taggedValue", DeclaredType::Kind(ElementKind::TaggedValue)),
];

/// The roles of `AssuranceCasePackage` (§9.2).
const ASSURANCE_CASE_PACKAGE_ROLES: &[Role] = &[
    role_of(
        "assuranceCasePackage",
        DeclaredType::Kind(ElementKind::AssuranceCasePackage),
    ),
    role_of(
        "argumentPackage",
        DeclaredType::Kind(ElementKind::ArgumentPackage),
    ),
    role_of(
        "artifactPackage",
        DeclaredType::Kind(ElementKind::ArtifactPackage),
    ),
    role_of(
        "terminologyPackage",
        DeclaredType::Kind(ElementKind::TerminologyPackage),
    ),
];

/// The role of `ArgumentPackage` (§11.4; the model's spelling, not the
/// text's `argumentationElement`).
const ARGUMENT_PACKAGE_ROLES: &[Role] = &[role_of(
    "argumentElement",
    DeclaredType::Abstract(AbstractClass::ArgumentationElement),
)];

/// The role of `TerminologyPackage` (§10.4; `TerminologyGroup` included by
/// the declared type, spec defect D9).
const TERMINOLOGY_PACKAGE_ROLES: &[Role] = &[role_of(
    "terminologyElement",
    DeclaredType::Abstract(AbstractClass::TerminologyElement),
)];

/// The role of `ArtifactPackage`, admitting every `ArtifactElement` (§12.2,
/// ADR 0014).
const ARTIFACT_PACKAGE_ROLES: &[Role] = &[role_of(
    "artifactElement",
    DeclaredType::Abstract(AbstractClass::ArtifactElement),
)];

/// The role of every `ArtifactAsset` (§12.6, §12.14).
const ARTIFACT_ASSET_ROLES: &[Role] = &[role_of(
    "property",
    DeclaredType::Kind(ElementKind::Property),
)];

/// The composite role of `parent`'s class that holds a `child` element, or
/// `None` if no role admits it.
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
/// interfaces and bindings hold their package's roles. A `UtilityElement`
/// has none. The value-typed composites (`name`, `content`, `key`, `value`,
/// `location`) hold values, not elements. At most one role of a class admits
/// a kind. Every role is `0..*`; multiplicity is for validation.
pub(crate) const fn role(parent: ElementKind, child: ElementKind) -> Option<Role> {
    // A UtilityElement has no composite roles.
    if !parent.is_a(AbstractClass::ModelElement) {
        return None;
    }
    if let Some(role) = find(MODEL_ELEMENT_ROLES, child) {
        return Some(role);
    }
    let own: &[Role] = if parent.is_kind_of(ElementKind::AssuranceCasePackage) {
        ASSURANCE_CASE_PACKAGE_ROLES
    } else if parent.is_kind_of(ElementKind::ArgumentPackage) {
        ARGUMENT_PACKAGE_ROLES
    } else if parent.is_kind_of(ElementKind::TerminologyPackage) {
        TERMINOLOGY_PACKAGE_ROLES
    } else if parent.is_kind_of(ElementKind::ArtifactPackage) {
        ARTIFACT_PACKAGE_ROLES
    } else if parent.is_a(AbstractClass::ArtifactAsset) {
        ARTIFACT_ASSET_ROLES
    } else {
        // Groups and every other ModelElement: the ModelElement roles only.
        &[]
    };
    find(own, child)
}

/// Whether a composite role of `parent`'s class admits a `child` element:
/// whether [`role`] finds one.
pub(crate) const fn admits(parent: ElementKind, child: ElementKind) -> bool {
    role(parent, child).is_some()
}

/// The first of `roles` whose declared type admits `child`.
const fn find(roles: &[Role], child: ElementKind) -> Option<Role> {
    let mut index = 0;
    while index < roles.len() {
        let role = roles[index];
        if role.declared.admits(child) {
            return Some(role);
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{DeclaredType, role};
    use crate::model::ElementKind;

    /// At most one role of each class admits each kind, so the role a child
    /// is written under is never a matter of table order.
    #[test]
    fn no_two_roles_of_a_class_admit_the_same_kind() {
        let tables = [
            super::ASSURANCE_CASE_PACKAGE_ROLES,
            super::ARGUMENT_PACKAGE_ROLES,
            super::TERMINOLOGY_PACKAGE_ROLES,
            super::ARTIFACT_PACKAGE_ROLES,
            super::ARTIFACT_ASSET_ROLES,
        ];
        for own in tables {
            for &kind in ElementKind::ALL {
                let count = super::MODEL_ELEMENT_ROLES
                    .iter()
                    .chain(own)
                    .filter(|role| role.declared.admits(kind))
                    .count();
                assert!(count <= 1, "{kind} is admitted by {count} roles");
            }
        }
    }

    #[test]
    fn a_role_with_an_abstract_type_is_found_for_each_conforming_kind() {
        let found = role(ElementKind::ArgumentPackage, ElementKind::Claim);
        assert_eq!(found.map(|role| role.name), Some("argumentElement"));
        assert!(matches!(
            found.map(|role| role.declared),
            Some(DeclaredType::Abstract(_))
        ));
        assert_eq!(role(ElementKind::Claim, ElementKind::Claim), None);
        assert_eq!(role(ElementKind::Note, ElementKind::Note), None);
    }
}
