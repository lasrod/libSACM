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

## Amendment 2 (accepted by the maintainer, 2026-11-17; slice 0057)

ArtifactAssetRelationship (§12.14) is read, written, created, edited, compared and
validated like the other ArtifactAssets (SACM23-ART-010). Its Interim entry in the refusal
list goes. The spellings ptc/22-03-13 and the text use instead are read tolerantly
(SACM23-XMI-009). This applies the maintainer's M3 decisions 3, 5 and 7 (2026-11-15).

- **Ends (decision 5):**
  - `ReferenceEnd::ArtifactSource` and `ReferenceEnd::ArtifactTarget`;
  - XMI names `source` and `target`; serde names `artifactSource` and `artifactTarget`,
    because `Source` and `Target` already hold the XMI names for an AssertedRelationship;
  - `[1..*]`, typed ArtifactAsset, clause 12.14. A missing end is a SACM-MULT-001 Error,
    because the text and the model agree on the bound (D-SEV).

  No kind has two ends of one name, so the reader resolves an end by the element's kind.
- **Accessors:** `SacmElement::sources()` and the new `targets()` answer for both
  relationship kinds; `target()` stays the AssertedRelationship's.
- **Delete (decision 7):** as for asserted relationships. `DeleteReferencingRelationships`
  deletes a relationship whose source or target is deleted. `ScrubReferences` removes the id
  and deletes the relationship only when an end is left empty.
- **`EditError::FeatureNotApplicable`** names the other class when the element has an end
  of the same name, e.g. `the AssertedInference "i1" has no target of an
  ArtifactAssetRelationship`. Its JSON is unchanged.
- **Spellings (decision 3):** `ArtifactAssertedRelationship`, an Event's `occurece`, and
  `argumentationElement` as an ArgumentPackage role or an ArgumentGroup end.
  - A tolerant read reads each as the resolved name, with a SACM-XMI-013 Warning.
  - A strict read keeps it as preserved content with a SACM-XMI-013 Error, so a strict save
    refuses the document.
  - The resolved name wins over a spelled repeat.
  - The C++ library's `occurence` is not read.
  - This replaces the last sentence of the "Not read yet" bullet above, and Amendment 1's
    `occurece` bullet.
- **Not here:** the §10.4 spelling `TerminologyElement` is SACM23-XMI-028 (M4), for when a
  TerminologyPackage is read.

**Alternatives considered:**
- **Reuse `Source` and `Target`, with bounds that depend on the kind.** Rejected: `target`
  would hold several ids on one kind, and code that assumes one target would quietly handle
  artifact links wrongly.
- **Keep the strict refusal of the spellings.** Rejected: it contradicts decision 3.
- **Map the spellings in strict mode too.** Rejected: a strict save would quietly rewrite
  the names.

## Amendment 3 (accepted by the maintainer, 2026-11-17; slice 0062)

The Terminology kinds of formal/23-05-08 clause 10 are read, written, created, edited, compared and validated in the pattern of the artifact kinds (SACM23-TERM-001 to -006):
- TerminologyPackage;
- Category, Term and Expression;
- TerminologyGroup.

Their Interim entries in the refusal list go. TerminologyPackageInterface and TerminologyPackageBinding stay `Interim` (SACM23-TERM-007, -008, M5).

- **Placements.** Each kind is read where the containment table admits it: a TerminologyPackage as a root, in an AssuranceCasePackage's `terminologyPackage`, nested in a TerminologyPackage's `terminologyElement`, and in an ArtifactPackage's `artifactElement`; the other kinds in a TerminologyPackage's `terminologyElement` and in an ArtifactPackage's `artifactElement`.
- **`value` is required.** ExpressionElement's `value : String [1]` is required in the create operations. `SetValue` has no removal. A file without a `value` still reads, and `validate` reports SACM-MULT-001 (the precedent of TaggedValue `key`; D-SEV).
- **The new ends:**
  - `ReferenceEnd::Category`: one entry for the `category : Category [0..*]` of Category (ptc/22-03-13) and of every ExpressionElement (§10.9).
    - On a Term or an Expression it holds the Categories the element belongs to.
    - On a Category, formal/23-05-08 does not say which way the end points (spec defect D21; SACM 2.4 Beta1 makes the Category list its members). The crate holds the ids as written and states no direction.
  - `ReferenceEnd::Element`: Expression's `element`, read by `SacmElement::expression_elements()`.
  - `ReferenceEnd::Origin`: Term's `origin`.
  - `ReferenceEnd::TerminologyGroupMember`: TerminologyGroup's `terminologyElement` (XMI and serde name `terminologyElement`). Under a TerminologyPackage the same name is the composite role; the parent's kind decides which.
  - None of the new ends has a lower bound, so a scrub keeps the holder. An Expression is not a relationship, so no delete cascades to it.
- **Not checked.** §10.10's two "should" sentences about naming elements in `value` (D-EXPR-NAMES).
- **The spelling `TerminologyElement`** (§10.4; spec defect D9) is read under the M3 decision 3 rule: a tolerant read reads it with a SACM-XMI-013 Warning, and a strict read keeps it as preserved content with a SACM-XMI-013 Error (SACM23-XMI-028).

**Alternatives considered:**
- **Two `category` ends, one for Category and one for ExpressionElement.** Rejected: they have the same name, type and bounds.
- **Documenting a direction for Category's `category`.** Rejected: the 2.3 text does not state one, and 2.4 Beta1 states the other.
