//! The rule about repeated targets in a multi-valued reference end
//! (ptc/22-03-13; XMI 2.5.1 §7.8.5; ADR 0027, ADR 0028).

use std::collections::HashMap;

use crate::diagnostic::template::MULT_001_REPEATED_REFERENCE;
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Document, ReferenceEnd, SacmElement};

/// SACM23-VAL-022: each id that one multi-valued end of one element holds
/// two or more times is one SACM-MULT-001 Warning, with the end's clause,
/// affecting the element, then the id. The ptc/22-03-13 ends set no
/// `isUnique`, so their values are unique by the UML default, while an
/// `xsd:IDREFS` may repeat an id (XMI 2.5.1 §7.8.5): the repeat is kept, and
/// a Warning (the maintainer, 2026-11-14) lets a strict save write it. It is
/// reported whatever else applies to the id.
pub(super) fn repeated_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for end in ReferenceEnd::ALL
            .iter()
            .copied()
            // The derived `expression` repeats an id when two texts link
            // one ExpressionElement, which is no repeat (SACM23-BASE-009).
            .filter(|end| end.is_many() && !end.is_derived())
        {
            repeated_hrefs(element, end, found);
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
                let arguments = vec![
                    Argument::text("class", element.kind().name()),
                    Argument::text("id", element.id().as_str()),
                    Argument::text("target", id.as_str()),
                    Argument::count_of("count", count),
                    Argument::text("end", end.name()),
                ];
                found.push(
                    Diagnostic::emit(
                        &MULT_001_REPEATED_REFERENCE,
                        Severity::Warning,
                        "SACM23-VAL-022",
                        arguments,
                    )
                    .set_clause(Specification::Sacm23, end.clause_on(element.kind()))
                    .set_affected([element.id().clone(), id.clone()]),
                );
            }
        }
    }
}

/// The SACM23-VAL-022 Warnings of the cross-document references `end` of
/// `element` holds: one per `href` held two or more times, compared as
/// strings, in the end's order, with `target` the `href` and affecting the
/// element only, since an `href` names no element of the document (ADR
/// 0040 D6).
fn repeated_hrefs(element: SacmElement<'_>, end: ReferenceEnd, found: &mut Vec<Diagnostic>) {
    let hrefs = element.hrefs(end);
    if hrefs.len() < 2 {
        return;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for href in hrefs {
        let count = counts.entry(href.as_str()).or_insert(0);
        *count = count.saturating_add(1);
    }
    for href in hrefs {
        let Some(count) = counts.remove(href.as_str()) else {
            continue;
        };
        if count < 2 {
            continue;
        }
        let arguments = vec![
            Argument::text("class", element.kind().name()),
            Argument::text("id", element.id().as_str()),
            Argument::text("target", href.as_str()),
            Argument::count_of("count", count),
            Argument::text("end", end.name()),
        ];
        found.push(
            Diagnostic::emit(
                &MULT_001_REPEATED_REFERENCE,
                Severity::Warning,
                "SACM23-VAL-022",
                arguments,
            )
            .set_clause(Specification::Sacm23, end.clause_on(element.kind()))
            .set_affected([element.id().clone()]),
        );
    }
}
