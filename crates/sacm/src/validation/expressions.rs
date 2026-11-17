//! The rules about `ExpressionLangStrings` (formal/23-05-08 §8.4;
//! SACM23-BASE-009, SACM23-VAL-009; ADR 0038 D5).

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, LangString, SacmElement, TextRole};

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
    let subject = match place {
        None => format!(
            "the name of {} {:?}",
            element.kind().name(),
            element.id().as_str()
        ),
        Some((position, role)) => format!(
            "value {position} of the {} of {} {:?}",
            role.name(),
            element.kind().name(),
            element.id().as_str()
        ),
    };
    let diagnostic = match value.expression() {
        None => Diagnostic::create(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Error,
            MISSING_ROW,
            format!(
                "{subject} is an ExpressionLangString with no expression \
                 (expression:ExpressionElement[1])"
            ),
        ),
        Some(expression) if value.content().is_some_and(|content| !content.is_empty()) => {
            Diagnostic::create(
                Code::EXPRESSION_WITH_CONTENT,
                Severity::Warning,
                CONTENT_ROW,
                format!(
                    "{subject} has the expression {:?} and a content that is not empty, and \
                     §8.4 says that if expression is not empty, then content should be empty",
                    expression.as_str()
                ),
            )
        }
        Some(_) => return,
    };
    found.push(
        diagnostic
            .set_clause("8.4")
            .set_affected([element.id().clone()]),
    );
}
