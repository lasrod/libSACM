//! Rules about what an `ArgumentPackage` holds (formal/23-05-08 §11.4).

use crate::diagnostic::template::PKG_001_MIXED_CONTENT;
use crate::diagnostic::{Argument, Diagnostic, Severity};
use crate::model::{Document, ElementKind, SacmElement};

/// SACM23-VAL-012: for each plain `ArgumentPackage` with a plain
/// `ArgumentPackage` among its `argumentElement` children, each other
/// `argumentElement` child not of an `ArgumentPackage` kind is one
/// SACM-PKG-001 Error, because §11.4 says "If an `ArgumentPackage` has nested
/// `ArgumentPackages`, then it is only allowed to contain `ArgumentPackages`."
/// An interface or binding child neither triggers the rule nor is reported,
/// `UtilityElement` children are not content, and only a plain package is
/// an owner (D-PKG-HOMOG). It affects the package, then the child.
pub(super) fn argument_package_homogeneity(document: &Document, found: &mut Vec<Diagnostic>) {
    for package in document.elements() {
        if package.kind() != ElementKind::ArgumentPackage {
            continue;
        }
        let Some(nested) =
            content(package).find(|child| child.kind() == ElementKind::ArgumentPackage)
        else {
            continue;
        };
        for child in
            content(package).filter(|child| !child.kind().is_kind_of(ElementKind::ArgumentPackage))
        {
            let arguments = vec![
                Argument::text("class", package.kind().name()),
                Argument::text("id", package.id().as_str()),
                Argument::text("nested_class", nested.kind().name()),
                Argument::text("nested_id", nested.id().as_str()),
                Argument::text("child_class", child.kind().name()),
                Argument::text("child_id", child.id().as_str()),
            ];
            found.push(
                Diagnostic::emit(
                    &PKG_001_MIXED_CONTENT,
                    Severity::Error,
                    "SACM23-VAL-012",
                    arguments,
                )
                .set_clause("11.4")
                .set_affected([package.id().clone(), child.id().clone()]),
            );
        }
    }
}

/// The `argumentElement` children of `package`, in document order: its
/// content, without its `UtilityElement`s.
fn content(package: SacmElement<'_>) -> impl Iterator<Item = SacmElement<'_>> {
    package.children().filter(|child| {
        child
            .role()
            .is_some_and(|role| role.name == "argumentElement")
    })
}
