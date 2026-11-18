//! Rules about descriptions (formal/23-05-08 §8.6).

use crate::diagnostic::template::MULT_001_TOO_MANY;
use crate::diagnostic::{Argument, Diagnostic, Severity};
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
        let arguments = vec![
            Argument::text("class", element.kind().name()),
            Argument::text("id", id.as_str()),
            Argument::count_of("count", count),
            Argument::text("property", "description"),
            Argument::text("type", "Description"),
            Argument::text("multiplicity", "0..1"),
        ];
        found.push(
            Diagnostic::emit(
                &MULT_001_TOO_MANY,
                Severity::Warning,
                "SACM23-VAL-011",
                arguments,
            )
            .set_clause("8.6")
            .set_affected([id.clone()]),
        );
    }
}
