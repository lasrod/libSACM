//! Rules about the Terminology package (formal/23-05-08 clause 10).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document};

/// SACM23-TERM-003: each Term or Expression with no `value` is one
/// SACM-MULT-001 Error, because §10.9 gives `value:String[1]` and the
/// machine-readable model agrees (D-SEV). An empty value is a value.
pub(super) fn missing_values(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if element.kind().is_a(AbstractClass::ExpressionElement) && element.value().is_none() {
            let id = element.id();
            let message = format!(
                "{} {:?} has no value (value:String[1])",
                element.kind().name(),
                id.as_str()
            );
            found.push(
                Diagnostic::create(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Error,
                    "SACM23-TERM-003",
                    message,
                )
                .set_clause("10.9")
                .set_affected([id.clone()]),
            );
        }
    }
}
