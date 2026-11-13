//! Rules about element ids (XMI 2.5.1 §7.6.1).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, is_ncname};

/// SACM23-VAL-020: each element whose id is not an XML `NCName` is one
/// SACM-ID-002 Error, because an `xmi:id` is an `xsd:ID`, whose lexical space
/// is the `NCName`s. The reader holds such an id as written (D-ID), so a
/// tolerant save writes it back unchanged.
pub(super) fn invalid_id(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let id = element.id();
        if is_ncname(id.as_str()) {
            continue;
        }
        let message = format!(
            "{} {:?} has an id that is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
            element.kind().name(),
            id.as_str()
        );
        found.push(
            Diagnostic::create(Code::ID_INVALID, Severity::Error, "SACM23-VAL-020", message)
                .set_clause("7.6.1")
                .set_affected([id.clone()]),
        );
    }
}
