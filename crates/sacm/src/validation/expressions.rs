//! The rules about `ExpressionLangStrings` (formal/23-05-08 §8.4;
//! SACM23-BASE-009, SACM23-VAL-009; ADR 0038 D5).

use crate::diagnostic::template::{
    EXPR_001_NAME_WITH_CONTENT, EXPR_001_VALUE_WITH_CONTENT, MULT_001_NAME_WITHOUT_EXPRESSION,
    MULT_001_VALUE_WITHOUT_EXPRESSION,
};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Document, ElementId, Href, LangString, SacmElement, TextRole};

/// The requirement of a missing `expression` (§8.4 `expression[1]`; spec
/// defect D20).
const MISSING_ROW: &str = "SACM23-BASE-009";

/// The requirement of an `expression` beside a content that is not empty.
const CONTENT_ROW: &str = "SACM23-VAL-009";

/// For each element, its name and then each value of its `content`, `key`
/// and `location`, by position:
/// - SACM23-BASE-009: an `ExpressionLangString` with no `expression` is one
///   SACM-MULT-001 Error, because §8.4 gives `expression[1]` and the
///   machine-readable model agrees (D-SEV, D20);
/// - SACM23-VAL-009: one with an `expression` and a content that is present
///   and not empty is one SACM-EXPR-001 Warning, because §8.4 says "If
///   expression is not empty, then +content should be empty." ("should", no
///   OCL); an absent content and `""` are empty (D-EXPR-CONTENT).
///
/// Each has clause 8.4 and no location, and affects the element that holds
/// the name or the text.
pub(super) fn expression_lang_strings(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if let Some(name) = element.name() {
            check(element, name, None, found);
        }
        for role in TextRole::ALL {
            let Some(text) = element.text(role) else {
                continue;
            };
            for (index, value) in text.value().values().iter().enumerate() {
                check(element, value, Some((index + 1, role)), found);
            }
        }
    }
}

/// The finding of `value`, the name of `element` when `place` is `None`, or
/// else the value at that 1-based position of that text.
fn check(
    element: SacmElement<'_>,
    value: &LangString,
    place: Option<(usize, TextRole)>,
    found: &mut Vec<Diagnostic>,
) {
    if !value.is_expression_lang_string() {
        return;
    }
    // What the message names first: the name, or the value by its position
    // in its text; then the element.
    let mut arguments = Vec::new();
    if let Some((position, role)) = place {
        arguments.push(Argument::count_of("position", position));
        arguments.push(Argument::text("role", role.name()));
    }
    arguments.push(Argument::text("class", element.kind().name()));
    arguments.push(Argument::text("id", element.id().as_str()));
    let is_name = place.is_none();
    // An href counts as the expression being present, and is named as the
    // id is (ADR 0040 amendment 1, A4).
    let expression = value
        .expression()
        .map(ElementId::as_str)
        .or_else(|| value.expression_href().map(Href::as_str));
    let diagnostic = match expression {
        None => {
            let template = if is_name {
                &MULT_001_NAME_WITHOUT_EXPRESSION
            } else {
                &MULT_001_VALUE_WITHOUT_EXPRESSION
            };
            Diagnostic::emit(template, Severity::Error, MISSING_ROW, arguments)
        }
        Some(expression) if value.content().is_some_and(|content| !content.is_empty()) => {
            let template = if is_name {
                &EXPR_001_NAME_WITH_CONTENT
            } else {
                &EXPR_001_VALUE_WITH_CONTENT
            };
            arguments.push(Argument::text("expression", expression));
            Diagnostic::emit(template, Severity::Warning, CONTENT_ROW, arguments)
        }
        Some(_) => return,
    };
    found.push(
        diagnostic
            .set_clause(Specification::Sacm23, "8.4")
            .set_affected([element.id().clone()]),
    );
}
