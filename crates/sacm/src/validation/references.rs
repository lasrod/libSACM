//! Rules about references between elements and the `SACMElement` flags
//! (formal/23-05-08 §8.2; ADR 0026, ADR 0028).

use std::collections::HashSet;

use crate::diagnostic::template::{
    ABS_001_ABSTRACT_HOLDER, ABS_001_FORM_NOT_ABSTRACT, ABS_001_FORM_OF_ANOTHER_KIND,
    CITE_001_NOT_A_CITATION, CITE_002_CITED_NOT_AN_ASSERTION, CITE_002_NO_CITED_ELEMENT,
    CITE_002_NOT_A_CITATION, REF_001_DANGLING, REF_002_WRONG_TYPE, REF_003_PRESERVED_TARGET,
    Template,
};
use crate::diagnostic::{Argument, Diagnostic, Severity};
use crate::model::{
    AbstractClass, AssertionDeclaration, DeclaredType, Document, ElementId, ReferenceEnd,
    SacmElement,
};

/// The arguments that name `element`: its class and id.
fn named(element: SacmElement<'_>) -> [Argument; 2] {
    [
        Argument::text("class", element.kind().name()),
        Argument::text("id", element.id().as_str()),
    ]
}

/// The arguments of a finding about the id `target` of the end `end` of
/// `element`: its class, id, the end and the target.
fn end_of(element: SacmElement<'_>, end: ReferenceEnd, target: &ElementId) -> Vec<Argument> {
    let mut arguments = named(element).to_vec();
    arguments.push(Argument::text("end", end.name()));
    arguments.push(Argument::text("target", target.as_str()));
    arguments
}

/// SACM23-VAL-003: each distinct id that a reference end of an element holds
/// and that names no element of the document is one SACM-REF-001 Error,
/// with the end's clause, affecting the element, then the id. An id absent
/// from the document dangles, and so does the `xmi:id` of a `name`,
/// `content`, `key`, `location` or `value`, which identifies no element.
///
/// LIB-COMPAT-003: an id that an element of preserved content carries is in
/// the file, merely not read, so it is one SACM-REF-003 Warning instead, the
/// same way (ADR 0033).
pub(super) fn dangling_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for &end in ReferenceEnd::ALL {
            // The missing ids of this end reported so far: once per distinct
            // id, in token order, in time linear in the end's ids. Only
            // looked up, never iterated, so its order is not observable.
            let mut reported = HashSet::new();
            for target in element.references(end) {
                if document.contains(target) || !reported.insert(target.as_str()) {
                    continue;
                }
                if document.is_preserved_id(target.as_str()) {
                    let mut arguments = end_of(element, end, target);
                    arguments.push(Argument::text("type", declared_type(element, end).name()));
                    found.push(
                        Diagnostic::emit(
                            &REF_003_PRESERVED_TARGET,
                            Severity::Warning,
                            "LIB-COMPAT-003",
                            arguments,
                        )
                        .set_clause(end.clause_on(element.kind()))
                        .set_affected([element.id().clone(), target.clone()]),
                    );
                    continue;
                }
                found.push(dangling(element, end, target));
            }
        }
    }
}

/// LIB-COMPAT-007: each distinct id that a reference end of an element holds
/// and that is no `xmi:id` the output of [`xmi::write`](crate::xmi::write)
/// writes, of an element or of a `name`, `content`, `key`, `location` or `value`, is one
/// SACM-REF-001 Error, as [`dangling_references`] gives it: an IDREF naming
/// no ID is not schema-valid (XML Schema 1.0 Part 1 §3.3.4; XMI 2.5.1
/// §8.2.1 rule 4i; ADR 0035 D4). Unlike `dangling_references`, it does not
/// report a reference to the `xmi:id` of a `name`, which is written.
pub(super) fn unwritten_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for &end in ReferenceEnd::ALL {
            // As in `dangling_references`: once per distinct id.
            let mut reported = HashSet::new();
            for target in element.references(end) {
                if document.is_id_in_use(target.as_str()) || !reported.insert(target.as_str()) {
                    continue;
                }
                found.push(dangling(element, end, target));
            }
        }
    }
}

/// The declared type of the targets of `element`'s end `end`: the type its
/// kind's declaring class gives (ADR 0039 D1). An element holds ids only in
/// the ends its kind has, so the first declaration is only a fallback.
fn declared_type(element: SacmElement<'_>, end: ReferenceEnd) -> DeclaredType {
    end.target_on(element.kind())
        .unwrap_or(end.declarations()[0].1)
}

/// The SACM-REF-001 Error for the id `target` of the end `end` of
/// `element`, which names no element: with the end's clause, affecting the
/// element, then the id.
fn dangling(element: SacmElement<'_>, end: ReferenceEnd, target: &ElementId) -> Diagnostic {
    let mut arguments = end_of(element, end, target);
    arguments.push(Argument::text("type", declared_type(element, end).name()));
    Diagnostic::emit(
        &REF_001_DANGLING,
        Severity::Error,
        "SACM23-VAL-003",
        arguments,
    )
    .set_clause(end.clause_on(element.kind()))
    .set_affected([element.id().clone(), target.clone()])
}

/// SACM23-VAL-004: each distinct id that a reference end of an element holds
/// and that names an element whose kind the end's declared type does not
/// admit is one SACM-REF-002 Error, with the end's clause, affecting the
/// element, then the id (ADR 0028). The declared types are ptc/22-03-13's,
/// on which the text agrees (D-SEV). An id that names no element is
/// SACM-REF-001's alone.
pub(super) fn wrongly_typed_references(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        for &end in ReferenceEnd::ALL {
            let declared = declared_type(element, end);
            // As in `dangling_references`: once per distinct id, linear in
            // the end's ids, never iterated.
            let mut reported = HashSet::new();
            for id in element.references(end) {
                let Some(target) = document.get(id) else {
                    continue;
                };
                if declared.admits(target.kind()) || !reported.insert(id.as_str()) {
                    continue;
                }
                let mut arguments = end_of(element, end, id);
                arguments.push(Argument::text("target_class", target.kind().name()));
                arguments.push(Argument::text("type", declared.name()));
                found.push(
                    Diagnostic::emit(
                        &REF_002_WRONG_TYPE,
                        Severity::Error,
                        "SACM23-VAL-004",
                        arguments,
                    )
                    .set_clause(end.clause_on(element.kind()))
                    .set_affected([element.id().clone(), id.clone()]),
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
        let mut arguments = named(element).to_vec();
        arguments.push(Argument::text("target", cited.as_str()));
        found.push(
            Diagnostic::emit(
                &CITE_001_NOT_A_CITATION,
                Severity::Error,
                "SACM23-VAL-007",
                arguments,
            )
            .set_clause("8.2")
            .set_affected([element.id().clone()]),
        );
    }
}

/// SACM23-VAL-019: an Assertion declared `asCited` should be a citation of
/// an Assertion, because §11.8 derives its declaration "from the value of
/// the `AssertionDeclaration` of the cited Assertion" (D-SEV). Each of these
/// is one SACM-CITE-002 Warning, with clause 11.8: its `isCitation` is not
/// true, affecting the element; it has no `citedElement`, affecting the
/// element; its `citedElement` names an element of the document that is
/// not of an Assertion kind, affecting the element, then that element. A
/// `citedElement` that names no element is left to SACM-REF-001.
pub(super) fn as_cited(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        if element.assertion_declaration() != Some(AssertionDeclaration::AsCited) {
            continue;
        }
        if !element.is_citation() {
            found.push(as_cited_finding(
                &CITE_002_NOT_A_CITATION,
                named(element).to_vec(),
                [element.id().clone()],
            ));
        }
        let Some(cited) = element.cited_element() else {
            found.push(as_cited_finding(
                &CITE_002_NO_CITED_ELEMENT,
                named(element).to_vec(),
                [element.id().clone()],
            ));
            continue;
        };
        let Some(target) = document.get(cited) else {
            continue;
        };
        if !target.kind().is_a(AbstractClass::Assertion) {
            let mut arguments = named(element).to_vec();
            arguments.push(Argument::text("target", cited.as_str()));
            arguments.push(Argument::text("target_class", target.kind().name()));
            found.push(as_cited_finding(
                &CITE_002_CITED_NOT_AN_ASSERTION,
                arguments,
                pair(element, cited),
            ));
        }
    }
}

/// A SACM-CITE-002 Warning of SACM23-VAL-019, of `template`, made from
/// `arguments`.
fn as_cited_finding(
    template: &'static Template,
    arguments: Vec<Argument>,
    affected: impl IntoIterator<Item = ElementId>,
) -> Diagnostic {
    Diagnostic::emit(template, Severity::Warning, "SACM23-VAL-019", arguments)
        .set_clause("11.8")
        .set_affected(affected)
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
        let [class, id] = named(element);
        let target_text = || Argument::text("target", form.as_str());
        if element.is_abstract() {
            let arguments = vec![class.clone(), id.clone(), target_text()];
            found.push(finding(
                &ABS_001_ABSTRACT_HOLDER,
                Severity::Error,
                arguments,
                [element.id().clone()],
            ));
        }
        let Some(target) = document.get(form) else {
            continue;
        };
        if !target.is_abstract() {
            let arguments = vec![target_text(), class.clone(), id.clone()];
            found.push(finding(
                &ABS_001_FORM_NOT_ABSTRACT,
                Severity::Warning,
                arguments,
                pair(element, form),
            ));
        }
        if target.kind() != element.kind() {
            let arguments = vec![
                target_text(),
                class,
                id,
                Argument::text("target_class", target.kind().name()),
            ];
            found.push(finding(
                &ABS_001_FORM_OF_ANOTHER_KIND,
                Severity::Warning,
                arguments,
                pair(element, form),
            ));
        }
    }
}

/// A SACM-ABS-001 finding of SACM23-VAL-008, of `template`, made from
/// `arguments`.
fn finding(
    template: &'static Template,
    severity: Severity,
    arguments: Vec<Argument>,
    affected: impl IntoIterator<Item = ElementId>,
) -> Diagnostic {
    Diagnostic::emit(template, severity, "SACM23-VAL-008", arguments)
        .set_clause("8.2")
        .set_affected(affected)
}

/// The ids of `element` and of its target `target`, in that order.
fn pair(element: SacmElement<'_>, target: &ElementId) -> [ElementId; 2] {
    [element.id().clone(), target.clone()]
}
