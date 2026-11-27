//! Rules about `gid`s (formal/23-05-08 §8.2).

use std::collections::BTreeMap;

use crate::diagnostic::template::ID_003_DUPLICATE_GID;
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Document, SacmElement};

/// SACM23-VAL-002: each non-empty `gid` that two or more elements hold, of
/// any kind, is one SACM-ID-003 Error, because §8.2 gives `gid` as "a unique
/// identifier that is unique within the scope of the model instance" (D-SEV).
/// It affects every holder in document order. An absent or empty gid is not
/// compared, and values compare exactly (D-GID-EMPTY).
pub(super) fn duplicate_gid(document: &Document, found: &mut Vec<Diagnostic>) {
    // Ordered by value, so the findings come out the same every time; each
    // value's holders in document order.
    let mut holders: BTreeMap<&str, Vec<SacmElement<'_>>> = BTreeMap::new();
    for element in document.elements() {
        if let Some(gid) = element.gid().filter(|gid| !gid.is_empty()) {
            holders.entry(gid).or_default().push(element);
        }
    }
    for (gid, elements) in holders {
        if elements.len() < 2 {
            continue;
        }
        let listed: Vec<String> = elements
            .iter()
            .map(|element| format!("{} {:?}", element.kind().name(), element.id().as_str()))
            .collect();
        let arguments = vec![
            Argument::count_of("count", elements.len()),
            Argument::text("gid", gid),
            Argument::text("elements", listed.join(", ")),
        ];
        found.push(
            Diagnostic::emit(
                &ID_003_DUPLICATE_GID,
                Severity::Error,
                "SACM23-VAL-002",
                arguments,
            )
            .set_clause(Specification::Sacm23, "8.2")
            .set_affected(elements.iter().map(|element| element.id().clone())),
        );
    }
}
