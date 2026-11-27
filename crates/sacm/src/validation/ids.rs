//! Rules about element ids (XMI 2.5.1 §7.6.1).

use crate::diagnostic::template::{
    ID_002_NAME_ID_NOT_AN_NCNAME, ID_002_NOT_AN_NCNAME, ID_002_TEXT_ID_NOT_AN_NCNAME,
    ID_002_VALUE_ID_NOT_AN_NCNAME, Template,
};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Document, ElementId, TextRole, is_ncname};

/// SACM23-VAL-020: each element whose id is not an XML `NCName` is one
/// SACM-ID-002 Error, because an `xmi:id` is an `xsd:ID`, whose lexical space
/// is the `NCName`s, and so is each `xmi:id` of a name (ADR 0024), a
/// `content`, a `key`, a `location` or a `value` in one (ADR 0025) that is
/// not one,
/// affecting the element that owns it. The reader holds such an id as
/// written (D-ID), so a tolerant save writes it back unchanged.
pub(super) fn invalid_id(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let id = element.id();
        let class = || Argument::text("class", element.kind().name());
        let owner = || Argument::text("id", id.as_str());
        if !is_ncname(id.as_str()) {
            found.push(finding(&ID_002_NOT_AN_NCNAME, vec![class(), owner()], id));
        }
        if let Some(name_id) = element
            .name_id()
            .filter(|name_id| !is_ncname(name_id.as_str()))
        {
            let arguments = vec![class(), owner(), Argument::text("xmi_id", name_id.as_str())];
            found.push(finding(&ID_002_NAME_ID_NOT_AN_NCNAME, arguments, id));
        }
        for role in TextRole::ALL {
            let Some(text) = element.text(role) else {
                continue;
            };
            // The role element's id, then each value's, with the value's
            // 1-based position; the arguments are made only for a bad id.
            let ids = text.id().map(|text_id| (None, text_id)).into_iter().chain(
                text.value_ids()
                    .iter()
                    .enumerate()
                    .filter_map(|(n, value)| {
                        value.as_ref().map(|value_id| (Some(n + 1), value_id))
                    }),
            );
            for (position, bad) in ids.filter(|(_, bad)| !is_ncname(bad.as_str())) {
                let mut arguments = Vec::new();
                let template = match position {
                    Some(n) => {
                        arguments.push(Argument::count_of("position", n));
                        &ID_002_VALUE_ID_NOT_AN_NCNAME
                    }
                    None => &ID_002_TEXT_ID_NOT_AN_NCNAME,
                };
                arguments.extend([
                    Argument::text("role", role.name()),
                    class(),
                    owner(),
                    Argument::text("xmi_id", bad.as_str()),
                ]);
                found.push(finding(template, arguments, id));
            }
        }
    }
}

/// The SACM-ID-002 Error of `template`, made from `arguments`, affecting the
/// element `id`.
fn finding(template: &'static Template, arguments: Vec<Argument>, id: &ElementId) -> Diagnostic {
    Diagnostic::emit(template, Severity::Error, "SACM23-VAL-020", arguments)
        .set_clause(Specification::Xmi251, "7.6.1")
        .set_affected([id.clone()])
}
