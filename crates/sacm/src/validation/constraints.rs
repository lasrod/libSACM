//! Rules about implementation constraints (formal/23-05-08 §8.6).

use crate::diagnostic::template::{
    ABS_001_IMPLEMENTATION_CONSTRAINT, ABS_001_IMPLEMENTATION_CONSTRAINTS,
};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
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
        let mut arguments = vec![
            Argument::text("class", element.kind().name()),
            Argument::text("id", element.id().as_str()),
        ];
        let template = if count == 1 {
            &ABS_001_IMPLEMENTATION_CONSTRAINT
        } else {
            arguments.push(Argument::count_of("count", count));
            &ABS_001_IMPLEMENTATION_CONSTRAINTS
        };
        found.push(
            Diagnostic::emit(template, Severity::Error, "SACM23-VAL-021", arguments)
                .set_clause(Specification::Sacm23, "8.6")
                .set_affected([element.id().clone()]),
        );
    }
}
