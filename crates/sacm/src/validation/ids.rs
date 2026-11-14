//! Rules about element ids (XMI 2.5.1 §7.6.1).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, ElementId, TextRole, is_ncname};

/// SACM23-VAL-020: each element whose id is not an XML `NCName` is one
/// SACM-ID-002 Error, because an `xmi:id` is an `xsd:ID`, whose lexical space
/// is the `NCName`s, and so is each `xmi:id` of a name (ADR 0024), a
/// `content`, a `key` or a `value` in one (ADR 0025) that is not one,
/// affecting the element that owns it. The reader holds such an id as
/// written (D-ID), so a tolerant save writes it back unchanged.
pub(super) fn invalid_id(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let id = element.id();
        if !is_ncname(id.as_str()) {
            let message = format!(
                "{} {:?} has an id that is not an NCName (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
                element.kind().name(),
                id.as_str()
            );
            found.push(finding(message, id));
        }
        if let Some(name_id) = element
            .name_id()
            .filter(|name_id| !is_ncname(name_id.as_str()))
        {
            let message = format!(
                "the name of {} {:?} has the xmi:id {:?}, which is not an NCName \
                 (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
                element.kind().name(),
                id.as_str(),
                name_id.as_str()
            );
            found.push(finding(message, id));
        }
        for role in [TextRole::Content, TextRole::Key] {
            let Some(text) = element.text(role) else {
                continue;
            };
            // The role element's id, then each value's, with the value's
            // 1-based position; the subject is formatted only for a bad id.
            let ids = text.id().map(|text_id| (None, text_id)).into_iter().chain(
                text.value_ids()
                    .iter()
                    .enumerate()
                    .filter_map(|(n, value)| {
                        value.as_ref().map(|value_id| (Some(n + 1), value_id))
                    }),
            );
            for (position, bad) in ids.filter(|(_, bad)| !is_ncname(bad.as_str())) {
                let owner = format!(
                    "the {} of {} {:?}",
                    role.name(),
                    element.kind().name(),
                    id.as_str()
                );
                let subject = match position {
                    Some(n) => format!("value {n} of {owner}"),
                    None => owner,
                };
                let message = format!(
                    "{subject} has the xmi:id {:?}, which is not an NCName \
                     (XMI 2.5.1 §7.6.1: xmi:id is an xsd:ID)",
                    bad.as_str()
                );
                found.push(finding(message, id));
            }
        }
    }
}

/// The SACM-ID-002 Error `message`, affecting the element `id`.
fn finding(message: String, id: &ElementId) -> Diagnostic {
    Diagnostic::create(Code::ID_INVALID, Severity::Error, "SACM23-VAL-020", message)
        .set_clause("7.6.1")
        .set_affected([id.clone()])
}
