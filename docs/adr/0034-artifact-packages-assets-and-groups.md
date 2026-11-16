# 0034. Artifact packages, assets and groups

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: SACM23-ART-001, SACM23-ART-003, SACM23-ART-008, SACM23-ART-009,
  SACM23-ART-011. It amends ADR 0016 D5 and ADR 0021 ("Readable kinds"), and supersedes
  ADR 0029's line that artifact ends reuse an existing `ReferenceEnd` variant.

## Context

M3 makes the Artifact package (SACM 2.3 clause 12) readable and editable. This first step
covers the classes without attributes of their own:
- ArtifactPackage;
- Property;
- Technique;
- Participant;
- ArtifactGroup.

Artifact, Resource, Activity, Event and ArtifactAssetRelationship have dates, versions,
locations or relationship ends. They follow in the next two slices.

The machine-readable model (ptc/22-03-13, `docs/sacm/metamodel-inventory.md`) gives:
- **ArtifactPackage:** the composite role `artifactElement : ArtifactElement [0..*]` and
  the reference end `interface : ArtifactPackageInterface [0..*]`.
- **ArtifactGroup:** the reference end `artifactElement : ArtifactElement [0..*]`. ptc
  gives it no aggregation, so it is a reference, not a containment.
- **ArtifactAsset:** the composite role `property : Property [0..*]`. Property, Technique
  and Participant are ArtifactAssets.

The same name, `artifactElement`, is a role on one class and a reference end on another.
The maintainer's M3 decisions of 2026-11-15 apply:
- **Decision 5:** artifact ends get their own `ReferenceEnd` entries.
- **Decision 6:** one create operation per class.

## Decision

- **Readable kinds.** ArtifactPackage is read in three places:
  - as a root, bare or inside `xmi:XMI`;
  - in an AssuranceCasePackage's `artifactPackage` role;
  - nested in an ArtifactPackage's `artifactElement` role, with `xmi:type`.

  Technique, Participant, ArtifactGroup and Property are read typed in `artifactElement`.
  Property is also read in the `property` role of Property, Technique and Participant,
  typed or untyped. The kinds that are already readable are also read inside an
  ArtifactPackage (ADR 0014). An untyped `artifactElement` child is `MissingType`
  (SACM-XMI-005).
- **The parent's kind decides what `artifactElement` is.**
  - Under an ArtifactPackage it is the composite role. A child in reference form is a
    proxy and is refused as not read yet (SACM23-XMI-019 for `xmi:idref`, SACM23-XMI-027
    for `href`; maintainer decision of 2026-11-16).
  - Under an ArtifactGroup it is the reference end. It can be an IDREFS attribute, or
    elements carrying `xmi:idref` or `href="#id"`. It is read as SACM23-ARG-013 reads
    `argumentElement`.
  - Elsewhere it is an attribute or role the class does not have, and is preserved
    (ADR 0033).
- **The group end.**
  - `ReferenceEnd::ArtifactGroupMember` is a new entry: `"artifactElement"`, §12.3,
    `[0..*]`, owned by ArtifactGroup, typed ArtifactElement.
  - `SacmElement::group_members()` answers for both group kinds.
  - Comparison reports `Difference::ArtifactGroupMembers`.
  - `GroupMember` keeps its name for ArgumentGroup.
- **Properties.** `SacmElement::properties()` lists the children in the `property` role. A
  Property directly in an ArtifactPackage is a child of the package, not a property of it.
- **Creates.** There is one create per class:
  - `CreateArtifactPackage`, as a root or a child;
  - `CreateProperty`, `CreateTechnique` and `CreateParticipant`;
  - `CreateArtifactGroup`, which takes its members.

  They use ADR 0019's checks, and the group's members also get ADR 0028's reference
  checks. The containment table decides the legal parents.
- **Not read yet.** Artifact, Resource, Activity, Event, ArtifactAssetRelationship, and the
  Terminology kinds an ArtifactPackage can now expose, are refused as `Interim(row)`. The
  type name `ArtifactAssertedRelationship` is refused as `Tolerance(SACM23-XMI-009)` until
  that row reads it. A package's `interface` end stays refused through the package's
  interface row (ADR 0033).
- **Delete** needs no new rule:
  - a group survives losing its members;
  - Properties are content, so deleting an asset that holds them is `NotEmpty` unless the
    delete is recursive.

**Differences from C++:**
- **A1.** C++ has one `CreateArtifactAsset { kind }`, which ignores the fields a kind lacks.
  Here there is one struct per class.
- **A2.** C++ has no ArtifactGroup create. Here `CreateArtifactGroup` takes its members.
- **A3.** C++ `CreateArtifactPackage` needs a parent. Here a package can be a root.
- **A4.** C++ allows a Property only under an ArtifactAsset. Here the containment table
  decides, so a Property may also sit directly in an ArtifactPackage (§12.2's
  `artifactElement : ArtifactElement`).
- **A5.** C++ has `ArtifactGroup::artifact_elements()`. Here it is `group_members()`,
  shared with ArgumentGroup.
- **A6.** C++ reads ArtifactPackage interfaces. Here `interface` is refused until
  SACM23-ART-012.
- **A7.** C++ reads `ArtifactAssertedRelationship` silently. Here it is refused until
  SACM23-XMI-009.

## Consequences

- A document holding only these classes, with no Artifact, Resource, Activity or Event,
  now reads, round-trips and edits.
- The interim refusals gate the compliance points whose documents can hold them: SACM23-CP-001
  and SACM23-CP-003 for every kind, plus CP-002 or CP-004 for their own package's kinds (the
  maintainer, 2026-11-16).
- `Document::referrers` (LIB-API-014) and the public end readers (LIB-API-015) must
  include the new end when they land.

## Alternatives considered

- **Reuse `GroupMember` for both groups.** This would be one variant with two XMI names
  and two target types. Decision 5 rejected it, because an application could not tell the
  ends apart from the variant.
- **One `CreateArtifactAsset { kind }`, as in C++.** It accepts fields that do not apply
  to the kind. Decision 6 rejected it.

## Amendment 1 (accepted by the maintainer, 2026-11-16; slice 0055)

Artifact (§12.7), Event (§12.9), Resource (§12.10) and Activity (§12.11) are read and
written typed in an ArtifactPackage's `artifactElement`, the same way as the kinds above.
Their Interim entries in the refusal list go.

- **Values held verbatim** (D-DATE):
  - Artifact's `version`;
  - the four date-typed values: Artifact and Event `date`, Activity `startTime` and `endTime`.

  Each is read and written as `gid` is: from its attribute or its value element, and written
  as an attribute.
- **`location`** is a MultiLangString role of a Resource, read and written as a
  UtilityElement's `content` is. It is written after `name`. It has no single-language
  shorthand, and nothing is reported when it is absent: the text gives it no multiplicity,
  and the model has no such property (D-SEV, spec defect D5).
- **`occurece`**, the model's spelling of Event's `date`, is the `ModelSpelling` tolerance
  until SACM23-XMI-009 reads it. A strict read refuses it. A tolerant read keeps it with
  SACM-XMI-014.
- **Edits:** each class has one create operation with builder methods (`CreateArtifact`,
  `CreateResource`, `CreateActivity`, `CreateEvent`), and each attribute has one Set
  operation, in the pattern of SetGid and SetContent (`SetVersion`, `SetDate`,
  `SetStartTime`, `SetEndTime`, `SetLocation`).
- **Comparison:** each attribute has one `Difference` variant.

**Alternatives considered:**
- **One generic `SetValue { attribute }`, like SetFlag.** Rejected: it reads worse, and
  each of these attributes belongs to one kind (`date` to two).
- **Parsed date types.** Rejected under D-DATE: they would lose how a value was written and
  refuse values the schema accepts.
