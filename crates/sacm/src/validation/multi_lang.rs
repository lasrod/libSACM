//! Rules about `MultiLangString`s (formal/23-05-08 §8.5, §8.12).

use std::collections::BTreeMap;

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{Document, ElementId, ElementKind, LangString, SacmElement, TextRole};

/// The requirement these rules enforce.
const ROW: &str = "SACM23-VAL-010";

/// SACM23-VAL-010: for each `content` and `key` a `UtilityElement` holds,
/// one SACM-MULT-001 Error when it has no values (§8.5
/// `value:LangString[1..*]`), and one for each language that two or more of
/// its values share (§8.5 "their +lang must be unique"), compared exactly:
/// two absent languages are equal, and absent and `""` differ (ADR 0012).
/// A `TaggedValue` with no `key` is one more (§8.12 `key:MultiLangString[1]`).
/// An absent `content` gives no finding (ADR 0025).
pub(super) fn multi_lang_strings(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for role in [TextRole::Content, TextRole::Key] {
            if let Some(text) = element.text(role) {
                check(element, role, text.value().values(), found);
            }
        }
        if element.kind() == ElementKind::TaggedValue && element.key().is_none() {
            let message = format!(
                "{} {:?} has no key (key:MultiLangString[1])",
                element.kind().name(),
                element.id().as_str()
            );
            found.push(finding(message, "8.12", element.id()));
        }
    }
}

/// The findings of the `role` of `element`, whose values are `values`.
fn check(
    element: SacmElement<'_>,
    role: TextRole,
    values: &[LangString],
    found: &mut Vec<Diagnostic>,
) {
    let owner = format!(
        "the {} of {} {:?}",
        role.name(),
        element.kind().name(),
        element.id().as_str()
    );
    if values.is_empty() {
        let message = format!("{owner} has no value (value:LangString[1..*])");
        found.push(finding(message, "8.5", element.id()));
        return;
    }
    // Ordered by language, so the findings come out the same every time.
    let mut counts: BTreeMap<Option<&str>, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(value.lang()).or_default() += 1;
    }
    for (lang, count) in counts {
        if count < 2 {
            continue;
        }
        let language = match lang {
            Some(lang) => format!("the language {lang:?}"),
            None => "no language (lang absent)".to_owned(),
        };
        let message = format!(
            "{owner} has {count} values in {language}, and each value's lang must be unique"
        );
        found.push(finding(message, "8.5", element.id()));
    }
}

/// The SACM-MULT-001 Error `message`, citing `clause` and affecting the
/// element `id`.
fn finding(message: String, clause: &'static str, id: &ElementId) -> Diagnostic {
    Diagnostic::create(Code::MULTIPLICITY_VIOLATION, Severity::Error, ROW, message)
        .set_clause(clause)
        .set_affected([id.clone()])
}
