//! The rule about repeated targets in a multi-valued reference end
//! (ptc/22-03-13; XMI 2.5.1 §7.8.5; ADR 0027, ADR 0028).

use std::collections::HashMap;

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, ReferenceEnd};

/// SACM23-VAL-022: each id that one multi-valued end of one element holds
/// two or more times is one SACM-MULT-001 Warning, with the end's clause,
/// affecting the element, then the id. The ptc/22-03-13 ends set no
/// `isUnique`, so their values are unique by the UML default, while an
/// `xsd:IDREFS` may repeat an id (XMI 2.5.1 §7.8.5): the repeat is kept, and
/// a Warning (the maintainer, 2026-11-14) lets a strict save write it. It is
/// reported whatever else applies to the id.
pub(super) fn repeated_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for end in ReferenceEnd::ALL.into_iter().filter(|end| end.is_many()) {
            let targets = element.references(end);
            if targets.len() < 2 {
                continue;
            }
            // How often each id occurs: only looked up and removed, never
            // iterated, so the findings follow the end's order, in time
            // linear in its ids.
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for id in targets {
                let count = counts.entry(id.as_str()).or_insert(0);
                *count = count.saturating_add(1);
            }
            for id in targets {
                // Taken out at its first occurrence, so each id is reported
                // once.
                let Some(count) = counts.remove(id.as_str()) else {
                    continue;
                };
                if count < 2 {
                    continue;
                }
                let message = format!(
                    "{} {:?} holds {:?} {count} times in its {}, whose ids are unique in \
                     SACM 2.3 (the repeat is kept)",
                    element.kind().name(),
                    element.id().as_str(),
                    id.as_str(),
                    end.name()
                );
                found.push(
                    Diagnostic::create(
                        Code::MULTIPLICITY_VIOLATION,
                        Severity::Warning,
                        "SACM23-VAL-022",
                        message,
                    )
                    .set_clause(end.clause())
                    .set_affected([element.id().clone(), id.clone()]),
                );
            }
        }
    }
}
