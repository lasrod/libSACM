//! Rules about asserted relationships (formal/23-05-08 §11.13, §11.15,
//! §11.17, §11.18; ADR 0027, ADR 0028).

use std::collections::HashSet;

use crate::diagnostic::{Code, Diagnostic, Severity};
use crate::model::{AbstractClass, Document, ElementKind, ReferenceEnd, SacmElement};

/// SACM23-VAL-005: each asserted relationship with no `source` is one
/// SACM-MULT-001 Error, and each with no `target` is another, affecting the
/// relationship, because §11.13 and ptc/22-03-13 agree on
/// `source:ArgumentAsset[1..*]` and `target:ArgumentAsset[1]` (D-SEV). XMI
/// ignores multiplicities (§7.8.2), so such a relationship reads and is
/// kept as read. A second target cannot be held: the reader refuses it as
/// `RepeatedValue`.
pub(super) fn relationship_ends(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document
        .elements()
        .filter(|element| element.kind().is_a(AbstractClass::AssertedRelationship))
    {
        if element.sources().is_empty() {
            found.push(missing_end(element, "source", "source:ArgumentAsset[1..*]"));
        }
        if element.target().is_none() {
            found.push(missing_end(element, "target", "target:ArgumentAsset[1]"));
        }
    }
}

/// The SACM23-VAL-005 finding of `element`, which has no `end`, declared as
/// `declared`.
fn missing_end(element: SacmElement<'_>, end: &str, declared: &str) -> Diagnostic {
    let message = format!(
        "{} {:?} has no {end} ({declared})",
        element.kind().name(),
        element.id().as_str()
    );
    Diagnostic::create(
        Code::MULTIPLICITY_VIOLATION,
        Severity::Error,
        "SACM23-VAL-005",
        message,
    )
    .set_clause("11.13")
    .set_affected([element.id().clone()])
}

/// The ends that the Constraints section of a relationship kind's clause
/// requires to be `ArtifactReference`s, with the clause: §11.15 for an
/// `AssertedEvidence`'s source (OCL
/// `self.source->forall(s|s.oclIsTypeOf(ArtifactReference))`), and §11.17 and
/// §11.18 ("must") for both ends of an `AssertedArtifactSupport` and of an
/// `AssertedArtifactContext`. Other kinds have none, and an
/// `AssertedEvidence`'s target is not narrowed.
fn artifact_reference_ends(kind: ElementKind) -> Option<(&'static [ReferenceEnd], &'static str)> {
    const SOURCE: &[ReferenceEnd] = &[ReferenceEnd::Source];
    const BOTH: &[ReferenceEnd] = &[ReferenceEnd::Source, ReferenceEnd::Target];
    match kind {
        ElementKind::AssertedEvidence => Some((SOURCE, "11.15")),
        ElementKind::AssertedArtifactSupport => Some((BOTH, "11.17")),
        ElementKind::AssertedArtifactContext => Some((BOTH, "11.18")),
        _ => None,
    }
}

/// SACM23-VAL-016: each distinct id, in an end [`artifact_reference_ends`]
/// names, that names an element of a kind the end admits (an
/// `ArgumentAsset`) other than an `ArtifactReference` is one SACM-REF-002
/// Error, with the kind's clause, affecting the relationship, then the id
/// (D-SEV; ADR 0028). An id that names no element (SACM-REF-001), or an
/// element the end does not admit (SACM23-VAL-004), is left to those rules,
/// so each wrong reference is one finding.
pub(super) fn artifact_reference_kinds(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let Some((ends, clause)) = artifact_reference_ends(element.kind()) else {
            continue;
        };
        for &end in ends {
            // Once per distinct id, linear in the end's ids, never iterated.
            let mut reported = HashSet::new();
            for id in element.references(end) {
                let Some(target) = document.get(id) else {
                    continue;
                };
                let kind = target.kind();
                if kind == ElementKind::ArtifactReference
                    || !end.target().admits(kind)
                    || !reported.insert(id.as_str())
                {
                    continue;
                }
                let message = format!(
                    "{} {:?} has the {} {:?}, of the kind {}, but §{clause} requires each {} \
                     of the {} to be an ArtifactReference",
                    element.kind().name(),
                    element.id().as_str(),
                    end.name(),
                    id.as_str(),
                    kind.name(),
                    end.name(),
                    element.kind().name()
                );
                found.push(
                    Diagnostic::create(
                        Code::REF_WRONG_TYPE,
                        Severity::Error,
                        "SACM23-VAL-016",
                        message,
                    )
                    .set_clause(clause)
                    .set_affected([element.id().clone(), id.clone()]),
                );
            }
        }
    }
}
