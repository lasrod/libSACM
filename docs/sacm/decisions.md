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
| D-PRESERVE | Conforming SACM content is never silently dropped. Content the model cannot type yet is preserved in tolerant mode with a diagnostic; strict save refuses a document containing it rather than discard it. |
| D-STRICT | Two modes. **Strict** is SACM 2.3 conformance behaviour and the default for saving. Strict import accepts (reads without loss, ADR 0022) every document that conforms to the SACM XML Schema in a recognised SACM namespace (D-NS-PIN, LIB-COMPAT-004), including the element forms the schema allows (SACM23-XMI-014 and SACM23-XMI-015); a value given more than once for a single-valued property is refused, though the generated schema allows the repeat (ADR 0021 amendment 1). Strict save writes only schema-conforming, valid models: it refuses a document that validation reports an Error for (ADR 0022, LIB-COMPAT-007). **Tolerant** accepts and reports third-party and legacy variations and is the default for loading; tolerant save writes the document as it is held. Tolerant acceptance is never presented as conformance. |
| D-RT | Semantic round trip (load → save → load yields an equal model) is guaranteed. Textual round trip is not. Output is deterministic: the same model always saves to the same bytes. |
| D-ID | Caller-provided and library-generated ids are both supported. Ids read from a file are preserved exactly, as `xsd:ID` values: XML white space around an NCName is trimmed, with an Info (ADR 0024). Generated ids are stable once created and deterministic for a given sequence of operations. |
| D-NO-ID | An element read with no `xmi:id` gets a generated id in the edit scheme, unused in the input and deterministic for it. It is an ordinary id: unmarked, compared like any id, and written on save. It is reported as Info SACM-XMI-004 (ADR 0024; the maintainer, 2026-11-14). |
| D-NS-PIN | The strict export namespace is pinned to `http://www.omg.org/spec/SACM/20220301`. This is a project choice: the standard defines no instance namespace, so no file is reported non-conformant solely for using another one (see `metamodel-inventory.md`). |

## Editing

| ID | Decision |
|---|---|
| D-ATOMIC | Every public mutation either succeeds and leaves the document structurally valid, or fails and leaves it unchanged. |
| D-PREVIEW | Destructive operations (delete, replace) can be previewed: the preview lists every element and relationship affected before anything changes. A preview taken before the document changed is rejected when applied. |
| D-POLICY | Cascading deletion never happens implicitly. The caller chooses a policy (e.g. reject if referenced / delete referencing relationships; reject if non-empty / delete recursively). |
| D-RESULT | Mutation results report created, changed and deleted ids and enough data for undo/redo and audit logs. Undo itself is a later row. |
| D-VAL-LAYER | Two validation layers. **Structural** invariants (ids unique, references resolve, containment legal, multiplicities of required ends) hold after every successful mutation. **Content** constraints (the clause-level rules below) are checked by `validate()`, because a loaded document may legitimately violate them and must still be loadable, inspectable and fixable. Edit operations do not refuse a change for a content-level finding. |

## Diagnostics

| ID | Decision |
|---|---|
| D-DIAG | Diagnostics are machine-readable: stable code, severity, the matrix row and clause they enforce, affected element ids, source location where available, and a message. Codes are API and never change meaning. Catalog: [diagnostics.md](diagnostics.md). |
| D-SEV | **Severity follows the clause's own modal verb.** "must" or an OCL invariant → error; "should" without OCL → warning. (assurance-forge #335.) This is why clause 8.2's abstractForm rules split: the citing element's `isAbstract` is stated flatly (error); the referred element's `isAbstract` and type are "should" (warning). A multiplicity on which the text and the model agree is normative structure, and violating it is an error (the maintainer, 2026-11-12; §2.4, §6.3). Example: §8.6 `name:LangString[1]`. A requirement stated in the indicative, with no modal verb, counts as "must" (the maintainer, 2026-11-14), so §8.2 "a unique identifier that is unique within the scope of the model instance" makes a duplicate `gid` an error (SACM23-VAL-002). |

## Resolutions of defective or contradictory clauses

Background for each is in [reference/specification-defects.md](reference/specification-defects.md).

| ID | Clauses | Decision |
|---|---|---|
| D-TEXT-MODEL | 8.4–8.7, 11.2/11.4, 10.5, 12.9–12.14 | Where formal/23-05-08 and ptc/22-03-13 disagree, follow the resolutions recorded in [metamodel-inventory.md](metamodel-inventory.md): mostly toward the model for structure; toward the text for `Resource.location`, §8.5 `MultiLangString.value` (a list), §8.7 `UtilityElement.content` (optional, ADR 0025), `ArtifactAssetRelationship` naming/generalization, `Event.date`, date types and `ArgumentationElement` abstractness. Machine-readable spellings are accepted on import (tolerant). |
| D-BIND-PART | 9.4, 10.5/10.6, 11.5 | **A package binding is not a legal participant of a binding — warning, not error.** The parallel clauses give three different participant rules; all agree participants are the packages being bound, and none gives a binding-of-bindings a meaning. Diagnosed uniformly across all four families at warning severity because one clause's OCL admits it. The narrower 11.5 reading (participants must be interfaces) is not enforced: it contradicts its own Associations block and the other clauses. (assurance-forge #333.) |
| D-IFACE-CITE | 9.3, 11.5, 11.6, 12.4, 12.5 | **Interface and binding contents must be citations — error.** Stated as a requirement wherever it is stated, and 11.5 backs it with OCL. The locality half — the citation points into the implemented package — is enforced for interfaces only, at warning severity; for bindings it is not enforced, because a participant may be an interface residing inside the package the citation targets, so the document has two legitimate readings. (assurance-forge #333.) |
| D-PKG-HOMOG | 11.4, 11.6 | **ArgumentPackage homogeneity is an error, but a contained interface or binding does not trigger it.** Read literally, 11.4 ("only allowed to contain ArgumentPackages") is unsatisfiable alongside 11.6 (an interface resides inside the package it describes), so 11.4 is triggered only by a nested *plain* ArgumentPackage. (assurance-forge #334.) Only a plain ArgumentPackage is checked as an owner, and only its `argumentElement` children count as its content; UtilityElement children and `name` do not (the lead, slice 0041, following C++). |
| D-NAME-MISSING | 8.3, 8.6 | A missing name stays missing, and an absent `lang` or `content` differs from an empty one; nothing invents a name ([ADR 0012](../adr/0012-lang-string-absent-and-empty.md), [ADR 0013](../adr/0013-missing-name-stays-missing.md)). |
| D-ARTPKG | 12.2 | An ArtifactPackage admits every ArtifactElement, as `artifactElement:Base::ArtifactElement[0..*]` says ([ADR 0014](../adr/0014-artifact-package-admits-every-artifact-element.md)). |
| D-TYPE-ATTR | XMI 2.5.1 §7.6.3, §7.8.8, rule 2g | The type attribute is `xmi:type`, written only where the class is not known from the role (specification defect D11); import also accepts `xsi:type` ([ADR 0017](../adr/0017-xmi-writer.md) T1). |
| D-ORDER | 8–12 | The order of roots and of each element's children is kept and compared, although SACM 2.3 marks no composition ordered, so that equal documents write equal bytes ([ADR 0010](../adr/0010-document-model.md) D1/D2, [ADR 0020](../adr/0020-semantic-comparison.md)). |
| D-XMI-VERSION | XMI 2.5.1 §7.5.3, rule 1a | The XMI version is identified by the XMI namespace, not by an `xmi:version` attribute; export stops writing it (SACM23-XMI-024; [ADR 0017](../adr/0017-xmi-writer.md) amendment 3). |
| D-ROLE-UNQUALIFIED | XMI 2.5.1 rule 2a | Role elements and `name` are matched in no namespace; a namespace-qualified one is refused, since rule 2a is ambiguous and the refusal is reversible ([ADR 0021](../adr/0021-xmi-reader-content.md)). |
| D-ROOT-NAME | XMI 2.5.1 §7.8.1 | A root's element name decides its class; a type attribute naming another class is refused ([ADR 0016](../adr/0016-interchange-roots.md) D2). |
| D-ENUM-EXACT | 11.8; XMI 2.5.1 §8.2.1 rule 8 | An enumeration value must be exactly one literal, after XML attribute-value normalisation, with no trimming or case folding ([ADR 0021](../adr/0021-xmi-reader-content.md)). |
| D-DESC-MULT | 8.6 | `ModelElement.description` is stored as a list (the model says `0..*`) so files with several descriptions round-trip, and the text's `0..1` is enforced in validation as a warning. |
| D-GID-EMPTY | 8.2 | An absent or empty (`""`) `gid` is not an identity and is not compared for uniqueness; every other value is compared exactly, letter case and white space included (the lead, slice 0041, following the catalog and C++). |

## Open questions

Recorded so they are not decided by accident in code. The lead raises them with the
maintainer when a slice needs them.

| ID | Question | Needed by |
|---|---|---|
| Q-UNDO | Should undo be built on inverse operations or snapshots? | LIB-EDIT-011 |
| Q-DATE | How should `date`-typed attributes be represented: validated `xs:date`/`xs:dateTime` lexical forms preserved as text, or parsed values? | SACM23-ART-013 |
| Q-INTEROP | Which third-party corpus files may be committed (licence review)? assurance-forge carries EPL-2.0 and MIT samples under `libs/sacm/tests/data/interop-thirdparty/`. | LIB-COMPAT-005 |
