//! Rules about what an `ArgumentPackage` holds (formal/23-05-08 §11.4).

use crate::diagnostic::{Code, Diagnostic, Severity};
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
            let message = format!(
                "{} {:?} nests the {} {:?}, so it may contain only ArgumentPackages, \
                 but it also contains {} {:?}",
                package.kind().name(),
                package.id().as_str(),
                nested.kind().name(),
                nested.id().as_str(),
                child.kind().name(),
                child.id().as_str()
            );
            found.push(
                Diagnostic::create(
                    Code::PACKAGE_CONTENT_INVALID,
                    Severity::Error,
                    "SACM23-VAL-012",
                    message,
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
