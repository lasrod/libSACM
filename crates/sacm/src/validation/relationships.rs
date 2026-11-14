//! Rules about asserted relationships (formal/23-05-08 §11.13; ADR 0027).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document, SacmElement};

/// SACM23-VAL-005: each asserted relationship with no `source` is one
/// SACM-MULT-001 Error, and each with no `target` is another, affecting the
/// relationship, because §11.13 and ptc/22-03-13 agree on
/// `source:ArgumentAsset[1..*]` and `target:ArgumentAsset[1]` (D-SEV). XMI
/// ignores multiplicities (§7.8.2), so such a relationship reads and is
/// kept as read. A second target cannot be held: the reader refuses it as
/// `RepeatedValue`.
pub(super) fn relationship_ends(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document
        .elements()
        .filter(|element| element.kind().is_a(AbstractClass::AssertedRelationship))
    {
        if element.sources().is_empty() {
            found.push(missing_end(element, "source", "source:ArgumentAsset[1..*]"));
        }
        if element.target().is_none() {
            found.push(missing_end(element, "target", "target:ArgumentAsset[1]"));
        }
    }
}

/// The SACM23-VAL-005 finding of `element`, which has no `end`, declared as
/// `declared`.
fn missing_end(element: SacmElement<'_>, end: &str, declared: &str) -> Diagnostic {
    let message = format!(
        "{} {:?} has no {end} ({declared})",
        element.kind().name(),
        element.id().as_str()
    );
    Diagnostic::create(
        Code::MULTIPLICITY_VIOLATION,
        Severity::Error,
        "SACM23-VAL-005",
        message,
    )
    .set_clause("11.13")
    .set_affected([element.id().clone()])
}
