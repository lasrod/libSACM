//! Rules about package interfaces and bindings (formal/23-05-08 §9.3, §9.4,
//! §10.5, §10.6, §11.5, §11.6, §12.4, §12.5; D-IFACE-CITE, D-BIND-PART,
//! D-SEV; slice 0068).

use std::collections::BTreeSet;

use crate::diagnostic::template::{
    MULT_001_MISSING, MULT_001_TOO_FEW, PKG_001_BINDING_PARTICIPANT, PKG_001_CITED_KIND,
    PKG_001_CITED_OUTSIDE, PKG_001_NOT_A_CITATION, PKG_001_NOT_A_CITATION_OR_RELATIONSHIP,
    PKG_001_NOT_AN_INTERFACE, PKG_002_OUTSIDE_IMPLEMENTED_PACKAGE, Template,
};
use crate::diagnostic::{Argument, Diagnostic, Severity, Specification};
use crate::model::{
    AbstractClass, Document, ElementId, ElementKind, Href, ReferenceEnd, SacmElement,
};

/// What a binding may hold beside citations (D-IFACE-CITE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Besides {
    /// Nothing: only citations.
    Nothing,
    /// The `AssertedRelationship`s of its own argument (§11.1, §11.5).
    AssertedRelationships,
    /// `ArtifactAssetRelationship`s (§12.4).
    ArtifactAssetRelationships,
}

impl Besides {
    /// Whether a non-citation of `kind` is allowed.
    fn allows(self, kind: ElementKind) -> bool {
        match self {
            Self::Nothing => false,
            Self::AssertedRelationships => kind.is_a(AbstractClass::AssertedRelationship),
            Self::ArtifactAssetRelationships => kind == ElementKind::ArtifactAssetRelationship,
        }
    }

    /// The name of what it allows, for the message.
    fn name(self) -> Option<&'static str> {
        match self {
            Self::Nothing => None,
            Self::AssertedRelationships => Some("AssertedRelationship"),
            Self::ArtifactAssetRelationships => Some("ArtifactAssetRelationship"),
        }
    }
}

/// One holder of citations: its kind, the role of its content, its clause,
/// what it allows beside citations, and, for an interface, the class its
/// citations cite.
struct Holder {
    kind: ElementKind,
    role: &'static str,
    clause: &'static str,
    besides: Besides,
    cited: Option<AbstractClass>,
}

/// The six holders of SACM23-VAL-013 (D-IFACE-CITE). The
/// `AssuranceCasePackageInterface` and `AssuranceCasePackageBinding` have no
/// citation rule.
const HOLDERS: [Holder; 6] = [
    Holder {
        kind: ElementKind::ArgumentPackageInterface,
        role: "argumentElement",
        clause: "11.6",
        besides: Besides::Nothing,
        cited: Some(AbstractClass::ArgumentAsset),
    },
    Holder {
        kind: ElementKind::ArtifactPackageInterface,
        role: "artifactElement",
        clause: "12.5",
        besides: Besides::Nothing,
        cited: Some(AbstractClass::ArtifactAsset),
    },
    Holder {
        kind: ElementKind::TerminologyPackageInterface,
        role: "terminologyElement",
        clause: "10.5",
        besides: Besides::Nothing,
        cited: Some(AbstractClass::TerminologyElement),
    },
    Holder {
        kind: ElementKind::ArgumentPackageBinding,
        role: "argumentElement",
        clause: "11.5",
        besides: Besides::AssertedRelationships,
        cited: None,
    },
    Holder {
        kind: ElementKind::ArtifactPackageBinding,
        role: "artifactElement",
        clause: "12.4",
        besides: Besides::ArtifactAssetRelationships,
        cited: None,
    },
    Holder {
        kind: ElementKind::TerminologyPackageBinding,
        role: "terminologyElement",
        clause: "10.6",
        besides: Besides::Nothing,
        cited: None,
    },
];

/// The four interface kinds, whose `implements` is `[1]`.
const INTERFACES: [ElementKind; 4] = [
    ElementKind::AssuranceCasePackageInterface,
    ElementKind::TerminologyPackageInterface,
    ElementKind::ArgumentPackageInterface,
    ElementKind::ArtifactPackageInterface,
];

/// The four binding kinds, whose `participantPackage` is `[2..*]`.
const BINDINGS: [ElementKind; 4] = [
    ElementKind::AssuranceCasePackageBinding,
    ElementKind::TerminologyPackageBinding,
    ElementKind::ArgumentPackageBinding,
    ElementKind::ArtifactPackageBinding,
];

/// The roles of an `AssuranceCasePackageInterface`'s sub-packages, each with
/// the one kind the §9.3 OCL admits there (`oclIsTypeOf`).
const INTERFACE_ROLES: [(&str, ElementKind); 4] = [
    (
        "assuranceCasePackage",
        ElementKind::AssuranceCasePackageInterface,
    ),
    ("argumentPackage", ElementKind::ArgumentPackageInterface),
    ("artifactPackage", ElementKind::ArtifactPackageInterface),
    (
        "terminologyPackage",
        ElementKind::TerminologyPackageInterface,
    ),
];

/// Whether `element` is a citation: `isCitation` true and a `citedElement`,
/// an id or a cross-document `href` (the §11.5 OCL `e.isCitation = true and
/// e.citedElement <> null`; ADR 0040 D6).
fn is_citation(element: SacmElement<'_>) -> bool {
    element.is_citation()
        && (element.cited_element().is_some()
            || !element.hrefs(ReferenceEnd::CitedElement).is_empty())
}

/// Whether `element` is `ancestor` or inside it, at any depth.
fn is_within(element: SacmElement<'_>, ancestor: &ElementId) -> bool {
    let mut at = Some(element);
    while let Some(current) = at {
        if current.id() == ancestor {
            return true;
        }
        at = current.parent();
    }
    false
}

/// The children of `holder` in `role`, in document order.
fn in_role<'a>(holder: SacmElement<'a>, role: &str) -> impl Iterator<Item = SacmElement<'a>> {
    holder
        .children()
        .filter(move |child| child.role().is_some_and(|held| held.name == role))
}

/// The class and id arguments of `element`.
fn named(element: SacmElement<'_>, class: &'static str, id: &'static str) -> [Argument; 2] {
    [
        Argument::text(class, element.kind().name()),
        Argument::text(id, element.id().as_str()),
    ]
}

/// SACM23-VAL-013: each child in the family role of an interface or binding
/// of [`HOLDERS`] that is no citation is one SACM-PKG-001 Error, affecting
/// the holder, then the child; a binding's own relationships excepted
/// (D-IFACE-CITE). For a citation in an interface whose `citedElement` id
/// names an element of the document, a cited element not of the class the
/// clause names is one Warning, and, when the interface's `implements` names
/// a package of a kind the end admits, a cited element not inside it at any
/// depth is another, each affecting the interface, the child and the cited
/// element. A child with the Error gets neither Warning, and an `href` is
/// not checked for kind or place.
pub(super) fn interface_and_binding_content(document: &Document, found: &mut Vec<Diagnostic>) {
    for holder in document.elements() {
        let Some(rule) = HOLDERS.iter().find(|rule| rule.kind == holder.kind()) else {
            continue;
        };
        let implemented = rule
            .cited
            .and_then(|_| holder.implements())
            .and_then(|package| document.get(package))
            .filter(|package| {
                ReferenceEnd::Implements
                    .target_on(holder.kind())
                    .is_some_and(|declared| declared.admits(package.kind()))
            });
        for child in in_role(holder, rule.role) {
            if !is_citation(child) {
                if rule.besides.allows(child.kind()) {
                    continue;
                }
                found.push(not_a_citation(holder, child, rule));
                continue;
            }
            let Some(expected) = rule.cited else {
                continue;
            };
            let Some(cited) = child.cited_element().and_then(|id| document.get(id)) else {
                continue;
            };
            if !cited.kind().is_a(expected) {
                let [class, id] = named(holder, "class", "id");
                let [child_class, child_id] = named(child, "child_class", "child_id");
                let arguments = vec![
                    class,
                    id,
                    child_class,
                    child_id,
                    Argument::text("target", cited.id().as_str()),
                    Argument::text("target_class", cited.kind().name()),
                    Argument::text("clause", rule.clause),
                    Argument::text("expected", expected.name()),
                ];
                found.push(content_warning(
                    &PKG_001_CITED_KIND,
                    rule.clause,
                    arguments,
                    [holder, child, cited],
                ));
            }
            if let Some(package) = implemented.filter(|package| !is_within(cited, package.id())) {
                let [class, id] = named(holder, "class", "id");
                let [child_class, child_id] = named(child, "child_class", "child_id");
                let arguments = vec![
                    class,
                    id,
                    child_class,
                    child_id,
                    Argument::text("target", cited.id().as_str()),
                    Argument::text("clause", rule.clause),
                    Argument::text("package", package.id().as_str()),
                ];
                found.push(content_warning(
                    &PKG_001_CITED_OUTSIDE,
                    rule.clause,
                    arguments,
                    [holder, child, cited],
                ));
            }
        }
    }
}

/// The SACM-PKG-001 Error of `child` of `holder`, which is no citation.
fn not_a_citation(holder: SacmElement<'_>, child: SacmElement<'_>, rule: &Holder) -> Diagnostic {
    let [class, id] = named(holder, "class", "id");
    let [child_class, child_id] = named(child, "child_class", "child_id");
    let mut arguments = vec![
        class,
        id,
        child_class,
        child_id,
        Argument::text("clause", rule.clause),
    ];
    let template = match rule.besides.name() {
        Some(relationship) => {
            arguments.push(Argument::text("relationship", relationship));
            &PKG_001_NOT_A_CITATION_OR_RELATIONSHIP
        }
        None => &PKG_001_NOT_A_CITATION,
    };
    Diagnostic::emit(template, Severity::Error, "SACM23-VAL-013", arguments)
        .set_clause(Specification::Sacm23, rule.clause)
        .set_affected([holder.id().clone(), child.id().clone()])
}

/// A SACM-PKG-001 Warning of SACM23-VAL-013, of `template`.
fn content_warning<const N: usize>(
    template: &'static Template,
    clause: &'static str,
    arguments: Vec<Argument>,
    affected: [SacmElement<'_>; N],
) -> Diagnostic {
    Diagnostic::emit(template, Severity::Warning, "SACM23-VAL-013", arguments)
        .set_clause(Specification::Sacm23, clause)
        .set_affected(affected.map(|element| element.id().clone()))
}

/// SACM23-VAL-015: each child of an `AssuranceCasePackageInterface` in a
/// sub-package role that is not exactly the interface kind of that role is
/// one SACM-PKG-001 Error, with clause 9.3, affecting the interface, then
/// the child (the §9.3 OCL `oclIsTypeOf`; its prose's `TerminologyPackages`
/// is spec defect D22). A citation is checked like any child, and an empty
/// interface gives none.
pub(super) fn assurance_case_interface_content(document: &Document, found: &mut Vec<Diagnostic>) {
    for interface in document.elements() {
        if interface.kind() != ElementKind::AssuranceCasePackageInterface {
            continue;
        }
        for (role, expected) in INTERFACE_ROLES {
            for child in in_role(interface, role).filter(|child| child.kind() != expected) {
                let [class, id] = named(interface, "class", "id");
                let [child_class, child_id] = named(child, "child_class", "child_id");
                let arguments = vec![
                    class,
                    id,
                    child_class,
                    child_id,
                    Argument::text("role", role),
                    Argument::text("expected", expected.name()),
                ];
                found.push(
                    Diagnostic::emit(
                        &PKG_001_NOT_AN_INTERFACE,
                        Severity::Error,
                        "SACM23-VAL-015",
                        arguments,
                    )
                    .set_clause(Specification::Sacm23, "9.3")
                    .set_affected([interface.id().clone(), child.id().clone()]),
                );
            }
        }
    }
}

/// SACM23-VAL-014: each interface whose `implements` holds neither an id nor
/// an `href` is one SACM-MULT-001 Error (`implements:<Package>[1]`), and each
/// binding holding fewer than two distinct participants, distinct ids and
/// distinct `href`s counted, is one (`participantPackage:<Package>[2..*]`):
/// `missing` for none and `too-few` for one, with the end's clause,
/// affecting the element (ADR 0039 D3, ADR 0040 D5). Each distinct
/// participant id that names a binding of the holder's own kind is one
/// SACM-PKG-001 Warning, affecting the binding, then the participant
/// (D-BIND-PART); an `href` is not checked.
pub(super) fn implements_and_participants(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let kind = element.kind();
        if INTERFACES.contains(&kind) {
            let end = ReferenceEnd::Implements;
            if element.references(end).is_empty() && element.hrefs(end).is_empty() {
                found.push(missing(element, end, "1"));
            }
        } else if BINDINGS.contains(&kind) {
            let end = ReferenceEnd::ParticipantPackage;
            let ids: BTreeSet<&ElementId> = element.references(end).iter().collect();
            let hrefs: BTreeSet<&str> = element.hrefs(end).iter().map(Href::as_str).collect();
            match ids.len() + hrefs.len() {
                0 => found.push(missing(element, end, "2..*")),
                1 => found.push(too_few(element, end)),
                _ => {}
            }
            let mut reported = BTreeSet::new();
            for participant in element.references(end) {
                let Some(target) = document.get(participant) else {
                    continue;
                };
                if target.kind() != kind || !reported.insert(participant) {
                    continue;
                }
                let [class, id] = named(element, "class", "id");
                let arguments = vec![
                    class,
                    id,
                    Argument::text("target", participant.as_str()),
                    Argument::text("target_class", target.kind().name()),
                ];
                found.push(
                    Diagnostic::emit(
                        &PKG_001_BINDING_PARTICIPANT,
                        Severity::Warning,
                        "SACM23-VAL-014",
                        arguments,
                    )
                    .set_clause(Specification::Sacm23, end.clause_on(kind))
                    .set_affected([element.id().clone(), participant.clone()]),
                );
            }
        }
    }
}

/// The name of the type `end` declares on `element`'s kind.
fn declared(element: SacmElement<'_>, end: ReferenceEnd) -> &'static str {
    end.target_on(element.kind())
        .unwrap_or(end.declarations()[0].1)
        .name()
}

/// The SACM-MULT-001 `missing` Error of `element`, whose `end` of
/// `multiplicity` holds no value.
fn missing(element: SacmElement<'_>, end: ReferenceEnd, multiplicity: &str) -> Diagnostic {
    let [class, id] = named(element, "class", "id");
    let arguments = vec![
        class,
        id,
        Argument::text("property", end.name()),
        Argument::text("type", declared(element, end)),
        Argument::text("multiplicity", multiplicity),
    ];
    bound_error(&MULT_001_MISSING, element, end, arguments)
}

/// The SACM-MULT-001 `too-few` Error of `element`, a binding with one
/// distinct participant.
fn too_few(element: SacmElement<'_>, end: ReferenceEnd) -> Diagnostic {
    let [class, id] = named(element, "class", "id");
    let arguments = vec![
        class,
        id,
        Argument::count_of("count", 1),
        Argument::text("property", end.name()),
        Argument::text("type", declared(element, end)),
        Argument::text("multiplicity", "2..*"),
    ];
    bound_error(&MULT_001_TOO_FEW, element, end, arguments)
}

/// A SACM-MULT-001 Error of SACM23-VAL-014 on `end` of `element`.
fn bound_error(
    template: &'static Template,
    element: SacmElement<'_>,
    end: ReferenceEnd,
    arguments: Vec<Argument>,
) -> Diagnostic {
    Diagnostic::emit(template, Severity::Error, "SACM23-VAL-014", arguments)
        .set_clause(Specification::Sacm23, end.clause_on(element.kind()))
        .set_affected([element.id().clone()])
}

/// SACM23-VAL-023: each `ArgumentPackageInterface`, `ArtifactPackageInterface`
/// and `TerminologyPackageInterface` whose `isCitation` is not true and
/// whose `implements` id names an element of the document of a kind the end
/// admits, which is not one of its ancestors, is one SACM-PKG-002 Warning,
/// with the clause, affecting the interface, then the package (§10.5,
/// §11.6, §12.5 Semantics; D-SEV). An interface implementing itself is
/// reported. A citation, an unresolved or wrongly typed `implements`, and an
/// `AssuranceCasePackageInterface` are not checked.
pub(super) fn interface_placement(document: &Document, found: &mut Vec<Diagnostic>) {
    for interface in document.elements() {
        let kind = interface.kind();
        if !matches!(
            kind,
            ElementKind::ArgumentPackageInterface
                | ElementKind::ArtifactPackageInterface
                | ElementKind::TerminologyPackageInterface
        ) || interface.is_citation()
        {
            continue;
        }
        let end = ReferenceEnd::Implements;
        let Some(package) = interface.implements().and_then(|id| document.get(id)) else {
            continue;
        };
        if !end
            .target_on(kind)
            .is_some_and(|declared| declared.admits(package.kind()))
        {
            continue;
        }
        let resides = interface
            .parent()
            .is_some_and(|parent| is_within(parent, package.id()));
        if resides {
            continue;
        }
        let clause = end.clause_on(kind);
        let [class, id] = named(interface, "class", "id");
        let arguments = vec![
            class,
            id,
            Argument::text("package_class", package.kind().name()),
            Argument::text("package", package.id().as_str()),
            Argument::text("clause", clause),
        ];
        found.push(
            Diagnostic::emit(
                &PKG_002_OUTSIDE_IMPLEMENTED_PACKAGE,
                Severity::Warning,
                "SACM23-VAL-023",
                arguments,
            )
            .set_clause(Specification::Sacm23, clause)
            .set_affected([interface.id().clone(), package.id().clone()]),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::validation;
    use crate::xmi::{self, ReadOptions};

    /// A child with the no-citation Error gets neither Warning, though its
    /// `citedElement` names an element of the wrong kind outside the package
    /// (D-IFACE-CITE): the Error, and SACM-CITE-001 for the `citedElement`.
    #[test]
    fn a_non_citation_gets_neither_warning() {
        let input = br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="case"/>
  <argumentPackage xmi:id="p"><name content="p"/>
    <argumentElement xmi:type="sacm:ArgumentPackageInterface" xmi:id="i" implements="p">
      <name content="i"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="ci" citedElement="q"><name content="ci"/></argumentElement>
    </argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q"><name content="q"/></argumentPackage>
</sacm:AssuranceCasePackage>"#;
        let doc = match xmi::read(input, &ReadOptions::default()) {
            Ok(output) => output.into_document(),
            Err(error) => panic!("the input does not read: {error}"),
        };
        let templates: Vec<&str> = validation::validate(&doc)
            .iter()
            .map(crate::diagnostic::Diagnostic::template)
            .collect();
        assert_eq!(
            templates,
            [
                "SACM-CITE-001/not-a-citation",
                "SACM-PKG-001/not-a-citation"
            ]
        );
    }
}
