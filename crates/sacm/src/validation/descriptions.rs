//! Rules about descriptions (formal/23-05-08 §8.6).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document};

/// SACM23-VAL-011: each `ModelElement` with two or more descriptions is one
/// SACM-MULT-001 Warning, because the §8.6 text gives
/// `description:Description[0..1]` while the model says `0..*`
/// (D-DESC-MULT). Every description is kept.
pub(super) fn repeated_description(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if !element.kind().is_a(AbstractClass::ModelElement) {
            continue;
        }
        let count = element.descriptions().count();
        if count < 2 {
            continue;
        }
        let id = element.id();
        let message = format!(
            "{} {:?} has {count} descriptions (description:Description[0..1])",
            element.kind().name(),
            id.as_str()
        );
        found.push(
            Diagnostic::create(
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                "SACM23-VAL-011",
                message,
            )
            .set_clause("8.6")
            .set_affected([id.clone()]),
        );
    }
}
