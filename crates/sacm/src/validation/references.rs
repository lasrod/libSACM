//! Rules about references between elements and the `SACMElement` flags
//! (formal/23-05-08 §8.2; ADR 0026).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, ElementId, ReferenceEnd, SacmElement};

/// SACM23-VAL-003: each distinct id that a reference end of an element holds
/// and that names no element of the document is one SACM-REF-001 Error,
/// with the end's clause, affecting the element, then the id. An id absent
/// from the document dangles, and so does the `xmi:id` of a `name`,
/// `content`, `key` or `value`, which identifies no element.
pub(super) fn dangling_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for end in ReferenceEnd::ALL {
            let targets = element.references(end);
            for (position, target) in targets.iter().enumerate() {
                // Once per distinct id: an earlier position reported it.
                if document.contains(target) || targets.iter().take(position).any(|t| t == target) {
                    continue;
                }
                let message = format!(
                    "{} {:?} has the {} {:?}, which names no element of the document \
                     (a {} is expected)",
                    element.kind().name(),
                    element.id().as_str(),
                    end.name(),
                    target.as_str(),
                    end.target().name()
                );
                found.push(
                    Diagnostic::create(
                        Code::REF_DANGLING,
                        Severity::Error,
                        "SACM23-VAL-003",
                        message,
                    )
                    .set_clause(end.clause())
                    .set_affected([element.id().clone(), target.clone()]),
                );
            }
        }
    }
}

/// SACM23-VAL-007: each element with a `citedElement` whose `isCitation` is
/// not true is one SACM-CITE-001 Error, affecting the element, because §8.2
/// gives the OCL `self.citedElement <> null implies self.isCitation = true`.
/// `isCitation` true alone is no finding: §8.2 states only that direction
/// (ADR 0026). The target need not resolve.
pub(super) fn citation(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let Some(cited) = element.cited_element() else {
            continue;
        };
        if element.is_citation() {
            continue;
        }
        let message = format!(
            "{} {:?} has the citedElement {:?} but isCitation is not true (§8.2: \
             citedElement <> null implies isCitation = true)",
            element.kind().name(),
            element.id().as_str(),
            cited.as_str()
        );
        found.push(
            Diagnostic::create(
                Code::CITATION_INVALID,
                Severity::Error,
                "SACM23-VAL-007",
                message,
            )
            .set_clause("8.2")
            .set_affected([element.id().clone()]),
        );
    }
}

/// SACM23-VAL-008: for each element with an `abstractForm`, §8.2 says "the
/// +isAbstract of the `SACMElement` is false, and the +isAbstract of the
/// referred `SACMElement` should be true. The referred `SACMElement` should be
/// of the same type of the `SACMElement`." (D-SEV.)
///
/// - An element whose own `isAbstract` is true is one SACM-ABS-001 Error,
///   affecting it, whatever the target.
/// - A target that is an element of the document and is not abstract is one
///   Warning, and one of another kind is another, each affecting the
///   element, then the target. A target that names no element gives neither
///   (SACM-REF-001 reports it).
pub(super) fn abstract_form(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let Some(form) = element.abstract_form() else {
            continue;
        };
        let (kind, id) = (element.kind().name(), element.id().as_str());
        if element.is_abstract() {
            let message = format!(
                "{kind} {id:?} has the abstractForm {:?} but is itself abstract (isAbstract is \
                 true); §8.2 gives abstractForm to a concrete element",
                form.as_str()
            );
            found.push(finding(Severity::Error, message, [element.id().clone()]));
        }
        let Some(target) = document.get(form) else {
            continue;
        };
        if !target.is_abstract() {
            let message = format!(
                "the abstractForm {:?} of {kind} {id:?} is not abstract: its isAbstract should \
                 be true (§8.2)",
                form.as_str()
            );
            found.push(finding(Severity::Warning, message, pair(element, form)));
        }
        if target.kind() != element.kind() {
            let message = format!(
                "the abstractForm {:?} of {kind} {id:?} is a {}, not a {kind}: it should be of \
                 the same type (§8.2)",
                form.as_str(),
                target.kind().name()
            );
            found.push(finding(Severity::Warning, message, pair(element, form)));
        }
    }
}

/// A SACM-ABS-001 finding of SACM23-VAL-008.
fn finding(
    severity: Severity,
    message: String,
    affected: impl IntoIterator<Item = ElementId>,
) -> Diagnostic {
    Diagnostic::create(
        Code::ABSTRACT_FORM_INVALID,
        severity,
        "SACM23-VAL-008",
        message,
    )
    .set_clause("8.2")
    .set_affected(affected)
}

/// The ids of `element` and of its target `target`, in that order.
fn pair(element: SacmElement<'_>, target: &ElementId) -> [ElementId; 2] {
    [element.id().clone(), target.clone()]
}
