//! Rules about the Terminology package (formal/23-05-08 clause 10).

use std::collections::BTreeSet;

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document, ElementKind};

/// SACM23-TERM-003: each Term or Expression with no `value` is one
/// SACM-MULT-001 Error, because §10.9 gives `value:String[1]` and the
/// machine-readable model agrees (D-SEV). An empty value is a value.
pub(super) fn missing_values(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if element.kind().is_a(AbstractClass::ExpressionElement) && element.value().is_none() {
            let id = element.id();
            let message = format!(
                "{} {:?} has no value (value:String[1])",
                element.kind().name(),
                id.as_str()
            );
            found.push(
                Diagnostic::create(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Error,
                    "SACM23-TERM-003",
                    message,
                )
                .set_clause("10.9")
                .set_affected([id.clone()]),
            );
        }
    }
}

/// SACM23-VAL-017: for each Expression whose `isAbstract` is not true, each
/// distinct id of its `element` end (§10.10) that names an abstract Term or
/// Expression of the document is one SACM-ABS-001 Error, with clause 10.10
/// and no location, affecting the Expression, then the element, because
/// §10.10 gives the OCL "self.isAbstract = false implies
/// self.element->forall(expr|expr.isAbstract = false)" (D-SEV). An abstract
/// Expression gives none. An id that names no element, or one of another
/// kind, is SACM-REF-001 or SACM-REF-002 alone.
pub(super) fn concrete_expressions(document: &Document, found: &mut Vec<Diagnostic>) {
    for expression in document.elements() {
        if expression.kind() != ElementKind::Expression || expression.is_abstract() {
            continue;
        }
        let mut reported = BTreeSet::new();
        for id in expression.expression_elements() {
            let Some(target) = document.get(id) else {
                continue;
            };
            let is_abstract_element =
                target.kind().is_a(AbstractClass::ExpressionElement) && target.is_abstract();
            if !is_abstract_element || !reported.insert(id.as_str()) {
                continue;
            }
            let message = format!(
                "{} {:?} is concrete but has the element {:?}, an abstract {}: a concrete \
                 Expression references only concrete ExpressionElements (§10.10)",
                expression.kind().name(),
                expression.id().as_str(),
                id.as_str(),
                target.kind().name()
            );
            found.push(
                Diagnostic::create(
                    Code::ABSTRACT_FORM_INVALID,
                    Severity::Error,
                    "SACM23-VAL-017",
                    message,
                )
                .set_clause("10.10")
                .set_affected([expression.id().clone(), id.clone()]),
            );
        }
    }
}
