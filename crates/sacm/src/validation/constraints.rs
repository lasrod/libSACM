//! Rules about implementation constraints (formal/23-05-08 §8.6).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document};

/// SACM23-VAL-021: each `ModelElement` with one or more
/// `implementationConstraint` children whose `isAbstract` is not true is one
/// SACM-ABS-001 Error, with clause 8.6, affecting the element, because §8.6
/// gives the OCL `implementationConstraint->size() > 0 implies isAbstract =
/// true` (spelled `implmentationConstraint` there, spec defect D15; D-SEV).
pub(super) fn implementation_constraints(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if !element.kind().is_a(AbstractClass::ModelElement) || element.is_abstract() {
            continue;
        }
        let count = element.implementation_constraints().count();
        if count == 0 {
            continue;
        }
        let constraints = if count == 1 {
            "an implementationConstraint".to_owned()
        } else {
            format!("{count} implementationConstraints")
        };
        let message = format!(
            "{} {:?} has {constraints} but isAbstract is not true: implementation \
             constraints are only specified on an abstract element (§8.6)",
            element.kind().name(),
            element.id().as_str()
        );
        found.push(
            Diagnostic::create(
                Code::ABSTRACT_FORM_INVALID,
                Severity::Error,
                "SACM23-VAL-021",
                message,
            )
            .set_clause("8.6")
            .set_affected([element.id().clone()]),
        );
    }
}
