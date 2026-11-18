//! Rules about relationships: asserted relationships (formal/23-05-08 §11.13,
//! §11.15, §11.17, §11.18) and `ArtifactAssetRelationship`s (§12.14; ADR 0027,
//! ADR 0028, ADR 0034 amendment 2).

use std::collections::HashSet;

use crate::diagnostic::template::{MULT_001_MISSING, REF_002_NOT_AN_ARTIFACT_REFERENCE};
use crate::diagnostic::{Argument, Diagnostic, Severity};
use crate::model::{AbstractClass, Document, ElementKind, ReferenceEnd, SacmElement};

/// SACM23-VAL-005: each asserted relationship with no `source` is one
/// SACM-MULT-001 Error, and each with no `target` is another, affecting the
/// relationship, because §11.13 and ptc/22-03-13 agree on
/// `source:ArgumentAsset[1..*]` and `target:ArgumentAsset[1]` (D-SEV). XMI
/// ignores multiplicities (§7.8.2), so such a relationship reads and is
/// kept as read. A second target cannot be held: the reader refuses it as
/// `RepeatedValue`.
///
/// SACM23-ART-010: likewise each `ArtifactAssetRelationship` with no `source`,
/// and each with no `target`, is one SACM-MULT-001 Error, with clause 12.14,
/// because §12.14 and ptc/22-03-13 agree on `source:ArtifactAsset[1..*]` and
/// `target:ArtifactAsset[1..*]` (D-SEV).
pub(super) fn relationship_ends(document: &Document, found: &mut Vec<Diagnostic>) {
    for element in document.elements() {
        let ends = if element.kind().is_a(AbstractClass::AssertedRelationship) {
            ASSERTED
        } else if element.kind() == ElementKind::ArtifactAssetRelationship {
            ARTIFACT
        } else {
            continue;
        };
        if element.sources().is_empty() {
            found.push(missing_end(element, &ends, "source", ends.source));
        }
        if element.targets().is_empty() {
            found.push(missing_end(element, &ends, "target", ends.target));
        }
    }
}

/// What a missing end of one relationship class is reported with: the row,
/// the clause, and each end's type and multiplicity as the clause declares
/// them.
struct Ends {
    requirement: &'static str,
    clause: &'static str,
    source: (&'static str, &'static str),
    target: (&'static str, &'static str),
}

/// The ends of an asserted relationship (§11.13, SACM23-VAL-005).
const ASSERTED: Ends = Ends {
    requirement: "SACM23-VAL-005",
    clause: "11.13",
    source: ("ArgumentAsset", "1..*"),
    target: ("ArgumentAsset", "1"),
};

/// The ends of an `ArtifactAssetRelationship` (§12.14, SACM23-ART-010).
const ARTIFACT: Ends = Ends {
    requirement: "SACM23-ART-010",
    clause: "12.14",
    source: ("ArtifactAsset", "1..*"),
    target: ("ArtifactAsset", "1..*"),
};

/// The finding of `element`, of the relationship class `ends` describes,
/// which has no `end`, whose type and multiplicity are `declared`.
fn missing_end(
    element: SacmElement<'_>,
    ends: &Ends,
    end: &str,
    declared: (&str, &str),
) -> Diagnostic {
    let (type_name, multiplicity) = declared;
    let arguments = vec![
        Argument::text("class", element.kind().name()),
        Argument::text("id", element.id().as_str()),
        Argument::text("property", end),
        Argument::text("type", type_name),
        Argument::text("multiplicity", multiplicity),
    ];
    Diagnostic::emit(
        &MULT_001_MISSING,
        Severity::Error,
        ends.requirement,
        arguments,
    )
    .set_clause(ends.clause)
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
                let arguments = vec![
                    Argument::text("class", element.kind().name()),
                    Argument::text("id", element.id().as_str()),
                    Argument::text("end", end.name()),
                    Argument::text("target", id.as_str()),
                    Argument::text("target_class", kind.name()),
                    Argument::text("clause", clause),
                ];
                found.push(
                    Diagnostic::emit(
                        &REF_002_NOT_AN_ARTIFACT_REFERENCE,
                        Severity::Error,
                        "SACM23-VAL-016",
                        arguments,
                    )
                    .set_clause(clause)
                    .set_affected([element.id().clone(), id.clone()]),
                );
            }
        }
    }
}
