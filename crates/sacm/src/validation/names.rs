//! Rules about names (formal/23-05-08 §8.6).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document};

/// SACM23-VAL-018: each `ModelElement` with no name is one SACM-MULT-001
/// Error, because §8.6 gives `name:LangString[1]`. A name whose `lang` or
/// `content` is absent or empty is still a name.
pub(super) fn missing_name(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if element.kind().is_a(AbstractClass::ModelElement) && element.name().is_none() {
            let id = element.id();
            let message = format!(
                "{} {:?} has no name (name:LangString[1])",
                element.kind().name(),
                id.as_str()
            );
            found.push(
                Diagnostic::create(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Error,
                    "SACM23-VAL-018",
                    message,
                )
                .set_clause("8.6")
                .set_affected([id.clone()]),
            );
        }
    }
}
