//! Rules about names (formal/23-05-08 §8.6).

use crate::diagnostic::template::MULT_001_MISSING;
use crate::diagnostic::{Argument, Diagnostic, Severity};
use crate::model::{AbstractClass, Document};

/// SACM23-VAL-018: each `ModelElement` with no name is one SACM-MULT-001
/// Error, because §8.6 gives `name:LangString[1]`. A name whose `lang` or
/// `content` is absent or empty is still a name.
pub(super) fn missing_name(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if element.kind().is_a(AbstractClass::ModelElement) && element.name().is_none() {
            let id = element.id();
            let arguments = vec![
                Argument::text("class", element.kind().name()),
                Argument::text("id", id.as_str()),
                Argument::text("property", "name"),
                Argument::text("type", "LangString"),
                Argument::text("multiplicity", "1"),
            ];
            found.push(
                Diagnostic::emit(
                    &MULT_001_MISSING,
                    Severity::Error,
                    "SACM23-VAL-018",
                    arguments,
                )
                .set_clause("8.6")
                .set_affected([id.clone()]),
            );
        }
    }
}
