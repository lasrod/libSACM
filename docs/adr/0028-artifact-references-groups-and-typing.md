# 0028. Artifact references, argument groups and reference typing

- Status: accepted (the maintainer, 2026-11-14)
- Date: 2026-11-14
- Matrix rows: SACM23-ARG-006, SACM23-ARG-009, SACM23-ARG-011, SACM23-ARG-012,
  SACM23-ARG-013, SACM23-VAL-004, SACM23-VAL-016, SACM23-VAL-022. It extends ADR 0026 and
  ADR 0027, and amends D-VAL-LAYER.

## Context

SACM 2.3 §11 completes argumentation with:
- ArtifactReference, whose `referencedArtifactElement` is `Base::ArtifactElement[0..*]`
  (§11.9);
- AssertedEvidence (§11.15) and AssertedArtifactSupport and AssertedArtifactContext
  (§11.17, §11.18), each with a Constraints section that requires ArtifactReference ends;
- ArgumentGroup, whose `argumentElement` is `ArgumentationElement[0..*]`, a reference, not a
  composition (§11.2; ptc has no `aggregation`).

Every reference end has a declared type, and the text and the model agree on each one.

## Decision

- **New ends.**
  - `referencedArtifactElement` (ArtifactReference, many, ArtifactElement, §11.9) and
    `argumentElement` (ArgumentGroup, many, ArgumentationElement, §11.2) join the
    reference-end table.
  - Accessors: `referenced_artifact_elements()` and `group_members()`. The second is named
    for the group, because `argumentElement` is also ArgumentPackage's composite role. Its
    docs carry the SACM name as an alias.
  - Comparison: `Difference::{ReferencedArtifactElements, GroupMembers}`.
- **The name collision.** The parent's kind decides how `argumentElement` reads:
  - under an ArgumentGroup it is a reference;
  - under an ArgumentPackage it is the composite role;
  - no class has a role and an end of the same name, and a test checks this;
  - a nested element under a group's `argumentElement` is `Unsupported`, never dropped.
- **Readable kinds and creates.**
  - ArtifactReference, ArgumentGroup, AssertedEvidence, AssertedArtifactSupport and
    AssertedArtifactContext are read in an ArgumentPackage's `argumentElement`.
  - They are created by `CreateArtifactReference`, `CreateArgumentGroup`,
    `CreateAssertedEvidence`, `CreateAssertedArtifactSupport` and
    `CreateAssertedArtifactContext`.
  - Artifacts (§12) are not readable yet. An ArtifactReference can cite any element but a
    UtilityElement, because every other kind conforms to ArtifactElement (which specializes
    ModelElement in ptc/22-03-13). This wording was corrected in the docs review; the
    decision is unchanged.
- **Typing.**
  - `validate` reports a resolved reference whose target's kind does not conform to the
    end's declared type as SACM-REF-002, an Error (SACM23-VAL-004; D-SEV, text and model
    agree). There is one finding per distinct id per end, with the end's clause and
    affected `[referrer, target]`.
  - §11.15 (OCL), §11.17 and §11.18 ("must") give SACM-REF-002 Errors for a source (and,
    for the two artifact kinds, a target) that is not an ArtifactReference (SACM23-VAL-016).
    Ends that VAL-004 already reports are skipped, so each wrong reference gives one
    finding.
  - AssertedEvidence's target is not narrowed: no constraint states it.
  - SACM-REF-003 waits for LIB-COMPAT-003, because nothing is held as preserved content yet.
- **Edits refuse wrongly typed references.**
  - Every create refuses an end whose target does not conform to the declared type, as
    `EditError::WrongReferenceType` (SACM-REF-002). It runs after `ReferenceNotFound`, over
    every end in order. This applies to the existing creates too.
  - Creates do not refuse the §11.15/§11.17/§11.18 rules, which are content (D-VAL-LAYER).
- **D-VAL-LAYER amendment.** "No edit creates a dangling reference" becomes "no edit creates
  a dangling or wrongly typed reference". A wrong type read from a file is a content
  finding that stays until fixed.
- **Repeats.** A repeated id in a many-valued end is one SACM-MULT-001 Warning per id per end
  (SACM23-VAL-022, the maintainer's choice of 2026-11-14), so a strict save still writes it.

**Differences from C++, one line each:**
- C++ also reads `referencedArtifact` as an alias (`xmi_reader.cpp:1374, 1610`); here only
  the ptc name is read, and the alias is `Unsupported`.
- C++ names the group end `argument_elements()` (`argumentation.h:86`); here it is
  `group_members()`.
- C++ has no ArgumentGroup create; here `CreateArgumentGroup` creates one with its members.
- C++ checks each id for existence and then type before the next id
  (`commands.cpp:470-497`); here every id is checked for existence before any is checked
  for type.
- C++ reports both the generic and the kind-specific REF-002 for one reference
  (`validate.cpp:158-170`); here it is reported once.
- C++ reports each occurrence of a repeated id in the kind check; here it is once per
  distinct id.
- C++ refuses a repeated source in `AddRelationshipSource`; here repeats are kept, with a
  Warning.
- C++ emits REF-003 for preserved targets; here it waits for LIB-COMPAT-003.

## Consequences

- SACM argumentation is complete for packages, apart from interfaces and bindings (M5).
- Strict save refuses wrongly typed references and the §11.15/§11.17/§11.18 violations. The
  release notes list them (ADR 0022).
- The three existing creates now also refuse wrongly typed ends.
