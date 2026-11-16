# SACM interpretation and policy decisions

Settled decisions for libSACM. Agents treat this page as policy: apply it, do not
reopen it. Only the maintainer changes a decision; when one changes, edit its row, say
what changed and when, and update any affected matrix rows.

Most of these were settled in the assurance-forge C++ SACM library (same author) and
carried over because they are about the standard, not about a particular
implementation. Their original discussion is linked where it exists.

## Scope and posture

| ID | Decision |
|---|---|
| D-SCOPE | Complete SACM 2.3 compliance (all four model compliance points) is the goal, reached through small verified slices. Interim releases describe the covered subset honestly; no compliance point is claimed until every row it depends on is verified. The SACM UML Profile compliance point is out of scope. |
| D-VERSION | SACM 2.3 only. Older revisions are read in tolerant mode with a warning and written as 2.3. The design must not make a future 2.4 impossible (see the 2.4 watch notes in assurance-forge). |
| D-TERM | The public API and docs use SACM terminology only. GSN terms (goal, strategy, solution, context node, away goal, module) never name SACM concepts. A GSN mapping, if ever offered, lives outside the `sacm` crate. |
| D-NOLAYOUT | Diagram layout, coordinates and visual state are not SACM and have no place in the model or in strict output. |
| D-API | The first external interface is a Rust API plus the `sacm` CLI. Other bindings (C, Python, WASM) wait until the model API is stable. |

## Data integrity

| ID | Decision |
|---|---|
| D-PRESERVE | Conforming SACM content is never silently dropped. What a read does not read is kept as preserved content, of the element it sits in or of the document outside its elements, or it is refused. A **tolerant** read keeps everything under a SACM 2.3 root, with a Warning: SACM-XMI-003 for content that does not conform where it stands; SACM-XMI-014 (SACM-XMI-007 for a cross-document `href`) for conforming content a later row reads; and the strict refusal's code for a bad literal, a repeated value, an untyped child under an abstract role or an empty `xmi:id`. A **strict** read keeps content that does not conform with SACM-XMI-003 as an Error and refuses the rest. A strict save refuses a document holding preserved content (ADR 0033, ADR 0035; the maintainer, 2026-11-15 and 2026-11-16). |
| D-STRICT | Two modes. **Strict** is SACM 2.3 conformance behaviour and the default for saving. Strict import accepts (reads without loss, ADR 0022) every document that conforms to the SACM XML Schema in the pinned SACM 2.3 namespace (D-NS-PIN), including the element forms the schema allows (SACM23-XMI-014 and SACM23-XMI-015); a value given more than once for a single-valued property is refused by a strict read, though the generated schema allows the repeat (ADR 0021 amendment 1). Strict save writes only schema-valid output: it refuses preserved content, an `xmi:id` that is not an NCName and a reference naming no `xmi:id` of the output (XML Schema ID/IDREF), and it does not validate; an application calls `validate()` for model checks (ADR 0035; the maintainer, 2026-11-16, reversing ADR 0022's save-time validation). **Tolerant** is the default for loading: every well-formed file with SACM 2.3 roots loads. It refuses only malformed XML, an encoding it cannot decode, a DOCTYPE, no SACM 2.3 root, a duplicate id and the parser's limits, and keeps the rest per D-PRESERVE. Tolerant save writes the document as it is held. Tolerant acceptance is never presented as conformance. |
| D-RT | Semantic round trip (load → save → load yields an equal model) is guaranteed. Textual round trip is not. Output is deterministic: the same model always saves to the same bytes. |
| D-ID | Caller-provided and library-generated ids are both supported. Ids read from a file are preserved exactly, as `xsd:ID` values: XML white space around an NCName is trimmed, with an Info (ADR 0024). Generated ids are stable once created and deterministic for a given sequence of operations. A strict read refuses an empty `xmi:id`; a tolerant read gives the element a generated id, with SACM-ID-002 as a Warning (ADR 0035). |
| D-NO-ID | An element read with no `xmi:id` gets a generated id in the edit scheme, unused in the input and deterministic for it. It is an ordinary id: unmarked, compared like any id, and written on save. It is reported as Info SACM-XMI-004 (ADR 0024; the maintainer, 2026-11-14). |
| D-NS-PIN | The strict export namespace is pinned to `http://www.omg.org/spec/SACM/20220301`. This is a project choice: the standard defines no instance namespace, so no file is reported non-conformant solely for using another one (see `metamodel-inventory.md`). Strict import reads only the pinned namespace. A document in another SACM revision's namespace, or in an unrecognised SACM-like one, is read by tolerant mode only (LIB-COMPAT-004, a tolerance: the maintainer, 2026-11-16). Strict mode refuses it with its code, saying it does not read that namespace, not that the document is defective. |

## Editing

| ID | Decision |
|---|---|
| D-ATOMIC | Every public mutation either succeeds and leaves the document structurally valid, or fails and leaves it unchanged. |
| D-PREVIEW | Destructive operations (delete, replace) can be previewed: the preview lists every element and relationship affected before anything changes. A preview taken before the document changed is rejected when applied. Every operation can be previewed (`edit::preview`) and a preview applied (`edit::apply_preview`); a preview is guarded by the document's revision (ADR 0031). An operation may also be applied without a preview. |
| D-POLICY | Cascading deletion never happens implicitly. The caller chooses a policy (e.g. reject if referenced / delete referencing relationships; reject if non-empty / delete recursively). |
| D-RESULT | Mutation results report created, changed and deleted ids, enough for audit logs. What undo/redo needs (typed before-values, inverse operations or snapshots) is LIB-EDIT-011's, under Q-UNDO (ADR 0019 amendment 2, the maintainer, 2026-11-15). |
| D-VAL-LAYER | Two validation layers. **Structural** invariants (ids unique, containment legal, multiplicities of required ends, and no edit creates a dangling or wrongly typed reference) hold after every successful mutation. A dangling or wrongly typed reference read from a file is a content finding (SACM-REF-001, SACM-REF-002) that stays until fixed (ADR 0026, ADR 0028, the maintainer, 2026-11-14). **Content** constraints (the clause-level rules below) are checked by `validate()`, because a loaded document may legitimately violate them and must still be loadable, inspectable and fixable. Edit operations do not refuse a change for a content-level finding. |

## Diagnostics

| ID | Decision |
|---|---|
| D-DIAG | Diagnostics are machine-readable: stable code, severity, the matrix row and clause they enforce, affected element ids, source location where available, and a message. Codes are API and never change meaning. Catalog: [diagnostics.md](diagnostics.md). |
| D-SEV | **Severity follows the clause's own modal verb.** "must" or an OCL invariant → error; "should" without OCL → warning. (assurance-forge #335.) This is why clause 8.2's abstractForm rules split: the referring element's `isAbstract` is stated flatly (error); the referred element's `isAbstract` and type are "should" (warning). A multiplicity on which the text and the model agree is normative structure, and violating it is an error (the maintainer, 2026-11-12; §2.4, §6.3). Example: §8.6 `name:LangString[1]`. A requirement stated in the indicative, with no modal verb, counts as "must" (the maintainer, 2026-11-14), so §8.2 "a unique identifier that is unique within the scope of the model instance" makes a duplicate `gid` an error (SACM23-VAL-002). |

## Resolutions of defective or contradictory clauses

Background for each is in [reference/specification-defects.md](reference/specification-defects.md).

| ID | Clauses | Decision |
|---|---|---|
| D-TEXT-MODEL | 8.4–8.7, 11.2/11.4, 10.5, 12.9–12.14 | Where formal/23-05-08 and ptc/22-03-13 disagree, follow the resolutions recorded in [metamodel-inventory.md](metamodel-inventory.md): mostly toward the model for structure; toward the text for `Resource.location`, §8.5 `MultiLangString.value` (a list), §8.7 `UtilityElement.content` (optional, ADR 0025), the §8.2 `isCitation`/`isAbstract` `=false` defaults (ADR 0026), `ArtifactAssetRelationship` naming/generalization, `Event.date`, date types and `ArgumentationElement` abstractness. Machine-readable spellings are accepted on import (tolerant). |
| D-BIND-PART | 9.4, 10.5/10.6, 11.5 | **A package binding is not a legal participant of a binding — warning, not error.** The parallel clauses give three different participant rules; all agree participants are the packages being bound, and none gives a binding-of-bindings a meaning. Diagnosed uniformly across all four families at warning severity because one clause's OCL admits it. The narrower 11.5 reading (participants must be interfaces) is not enforced: it contradicts its own Associations block and the other clauses. (assurance-forge #333.) |
| D-IFACE-CITE | 9.3, 11.5, 11.6, 12.4, 12.5 | **Interface and binding contents must be citations — error.** Stated as a requirement wherever it is stated, and 11.5 backs it with OCL. The locality half — the citation points into the implemented package — is enforced for interfaces only, at warning severity; for bindings it is not enforced, because a participant may be an interface residing inside the package the citation targets, so the document has two legitimate readings. (assurance-forge #333.) |
| D-PKG-HOMOG | 11.4, 11.6 | **ArgumentPackage homogeneity is an error, but a contained interface or binding does not trigger it.** Read literally, 11.4 ("only allowed to contain ArgumentPackages") is unsatisfiable alongside 11.6 (an interface resides inside the package it describes), so 11.4 is triggered only by a nested *plain* ArgumentPackage. (assurance-forge #334.) Only a plain ArgumentPackage is checked as an owner, and only its `argumentElement` children count as its content; UtilityElement children and `name` do not (the lead, slice 0041, following C++). |
| D-NAME-MISSING | 8.3, 8.6 | A missing name stays missing, and an absent `lang` or `content` differs from an empty one; nothing invents a name ([ADR 0012](../adr/0012-lang-string-absent-and-empty.md), [ADR 0013](../adr/0013-missing-name-stays-missing.md)). |
| D-ARTPKG | 12.2 | An ArtifactPackage admits every ArtifactElement, as `artifactElement:Base::ArtifactElement[0..*]` says ([ADR 0014](../adr/0014-artifact-package-admits-every-artifact-element.md)). |
| D-TYPE-ATTR | XMI 2.5.1 §7.6.3, §7.8.8, rule 2g | The type attribute is `xmi:type`, written only where the class is not known from the role (specification defect D11); import also accepts `xsi:type` ([ADR 0017](../adr/0017-xmi-writer.md) T1). |
| D-ORDER | 8–12 | The order of roots, of each element's children and of the ids of each multi-valued reference end is kept and compared, although SACM 2.3 marks no composition or reference ordered, so that equal documents write equal bytes (reference ends: ADR 0027, the maintainer, 2026-11-14) ([ADR 0010](../adr/0010-document-model.md) D1/D2, [ADR 0020](../adr/0020-semantic-comparison.md)). |
| D-XMI-VERSION | XMI 2.5.1 §7.5.3, rule 1a | The XMI version is identified by the XMI namespace, not by an `xmi:version` attribute; export stops writing it (SACM23-XMI-024; [ADR 0017](../adr/0017-xmi-writer.md) amendment 3). |
| D-ROLE-UNQUALIFIED | XMI 2.5.1 rule 2a | Role elements and `name` are matched in no namespace; a namespace-qualified one is refused, since rule 2a is ambiguous and the refusal is reversible ([ADR 0021](../adr/0021-xmi-reader-content.md)). |
| D-ROOT-NAME | XMI 2.5.1 §7.8.1 | A root's element name decides its class; a type attribute naming another class is preserved and changes nothing ([ADR 0016](../adr/0016-interchange-roots.md) D2, amended by [ADR 0033](../adr/0033-preserved-content.md)). |
| D-ENUM-EXACT | 11.8; XMI 2.5.1 §8.2.1 rule 8 | An enumeration value must be exactly one literal, after XML attribute-value normalisation, with no trimming or case folding ([ADR 0021](../adr/0021-xmi-reader-content.md)). A strict read refuses any other value; a tolerant read preserves it with SACM-ENUM-001 and holds the default (ADR 0035). |
| D-DESC-MULT | 8.6 | `ModelElement.description` is stored as a list (the model says `0..*`) so files with several descriptions round-trip, and the text's `0..1` is enforced in validation as a warning. |
| D-GID-EMPTY | 8.2 | An absent or empty (`""`) `gid` is not an identity and is not compared for uniqueness; every other value is compared exactly, letter case and white space included (the lead, slice 0041, following the catalog and C++). |
| D-DATE | 4, 12.7, 12.9, 12.11; XMI 2.5.1 §8.2.1 rules 4d, 4j | The four `date`-typed attributes (Artifact and Event `date`, Activity `startTime` and `endTime`) are held as strings exactly as written. SACM 2.3 defines no lexical form, and the schema the XMI rules produce types them `xsd:string`, so no value is refused or diagnosed; an application that needs a calendar date parses it (the maintainer, 2026-11-16; slice 0055). A format check would be a project choice and can be added later as a Warning. |

## Open questions

Recorded so they are not decided by accident in code. The lead raises them with the
maintainer when a slice needs them.

| ID | Question | Needed by |
|---|---|---|
| Q-UNDO | Should undo be built on inverse operations or snapshots? | LIB-EDIT-011 |
| Q-INTEROP | Which third-party corpus files may be committed (licence review)? assurance-forge carries EPL-2.0 and MIT samples under `libs/sacm/tests/data/interop-thirdparty/`. | LIB-COMPAT-005 |
