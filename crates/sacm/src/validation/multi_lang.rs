//! Rules about `MultiLangString`s (formal/23-05-08 §8.5, §8.12).

use std::collections::BTreeMap;

use crate::diagnostic::template::{
    self, MULT_001_MISSING, MULT_001_NO_VALUE, MULT_001_REPEATED_ABSENT_LANG,
    MULT_001_REPEATED_LANG,
};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{Document, ElementId, ElementKind, LangString, SacmElement, TextRole};

/// The requirement these rules enforce.
const ROW: &str = "SACM23-VAL-010";

/// SACM23-VAL-010: for each `content` and `key` a `UtilityElement` holds,
/// and each `location` a `Resource` holds (§12.10), one SACM-MULT-001 Error
/// when it has no values (§8.5
/// `value:LangString[1..*]`), and one for each language that two or more of
/// its values share (§8.5 "their +lang must be unique"), compared exactly:
/// two absent languages are equal, and absent and `""` differ (ADR 0012).
/// A `TaggedValue` with no `key` is one more (§8.12 `key:MultiLangString[1]`).
/// An absent `content` gives no finding (ADR 0025), nor does an absent
/// `location`, which the text gives no multiplicity (D-SEV).
pub(super) fn multi_lang_strings(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for role in TextRole::ALL {
            if let Some(text) = element.text(role) {
                check(element, role, text.value().values(), found);
            }
        }
        if element.kind() == ElementKind::TaggedValue && element.key().is_none() {
            let arguments = vec![
                Argument::text("class", element.kind().name()),
                Argument::text("id", element.id().as_str()),
                Argument::text("property", "key"),
                Argument::text("type", "MultiLangString"),
                Argument::text("multiplicity", "1"),
            ];
            found.push(finding(&MULT_001_MISSING, arguments, "8.12", element.id()));
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
    // The role, class and id the message names first.
    let owner = || {
        vec![
            Argument::text("role", role.name()),
            Argument::text("class", element.kind().name()),
            Argument::text("id", element.id().as_str()),
        ]
    };
    if values.is_empty() {
        found.push(finding(&MULT_001_NO_VALUE, owner(), "8.5", element.id()));
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
        let mut arguments = owner();
        arguments.push(Argument::count_of("count", count));
        let template = match lang {
            Some(lang) => {
                arguments.push(Argument::text("lang", lang));
                &MULT_001_REPEATED_LANG
            }
            None => &MULT_001_REPEATED_ABSENT_LANG,
        };
        found.push(finding(template, arguments, "8.5", element.id()));
    }
}

/// The SACM-MULT-001 Error of `template`, made from `arguments`, citing
/// `clause` and affecting the element `id`.
fn finding(
    template: &'static template::Template,
    arguments: Vec<Argument>,
    clause: &'static str,
    id: &ElementId,
) -> Diagnostic {
    Diagnostic::emit(template, Severity::Error, ROW, arguments)
        .set_clause(Specification::Sacm23, clause)
        .set_affected([id.clone()])
}
