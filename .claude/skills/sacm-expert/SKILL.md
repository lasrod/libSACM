---
name: sacm-expert
description: OMG SACM 2.3 (Structured Assurance Case Metamodel) domain expertise for libSACM — what packages, claims, asserted relationships, artifacts, terminology, citations, abstract forms, interfaces and bindings mean; how SACM is serialized as XMI; where the specification and its machine-readable model disagree; and the project's settled interpretation decisions. Load before analyzing, designing, testing, implementing, verifying or documenting anything that touches the SACM model, XMI import/export or validation, or when answering any question about what a SACM construct means or whether a document is conformant.
---

# SACM expert

SACM is the OMG standard for representing assurance cases — structured arguments,
backed by evidence, that a system is acceptably safe, secure or otherwise fit for
purpose — so that tools can exchange them. libSACM implements **SACM 2.3**
(formal/23-05-08, machine-readable model ptc/22-03-13).

Getting SACM wrong in this library does not crash anything. It silently changes
somebody's safety argument when their file passes through a tool built on us. Every
rule below exists to make that failure loud.

## Operating rules

1. **Quote, don't recall.** The normative sources, and XMI 2.5.1, are fetched
   (hash-pinned) by `cargo xtask fetch-spec` into `third_party/sacm-2.3/`. Any claim about what the
   standard requires cites a clause and quotes it. This skill is orientation; the PDF
   is authority. If you cannot fetch the sources, say so rather than guess.
2. **Settled is settled.** `docs/sacm/decisions.md` records interpretations already
   made (ambiguous clauses, contradictory OCL, severity policy). Apply them; do not
   reopen them unless the maintainer asks.
3. **The text and the model disagree in known places.** Read
   `docs/sacm/metamodel-inventory.md` (structure, with every resolved divergence) and
   `docs/sacm/reference/specification-defects.md` before treating a mismatch as new.
4. **Preserve, don't reinterpret.** When a document is ambiguous or uses something the
   model cannot type yet, keep it and emit a diagnostic. A reader that picks an
   interpretation is a reader that rewrites the argument.
5. **Strict is strict.** Strict mode means SACM 2.3 as specified. Anything accepted
   only for interoperability (other namespaces, legacy revisions, vendor extensions,
   model-spelling variants) is tolerant mode, and is reported as such.
6. **SACM is not GSN.** GSN (Goal Structuring Notation) is a notation often rendered
   over SACM. GSN terms — goal, strategy, solution, away goal — never name SACM
   concepts in this library's API or docs. A GSN mapping is a client concern.
7. **Severity follows the clause's modal verb.** "must"/OCL → error; "should" without
   OCL → warning (decision D-SEV).

## The metamodel in one page

Five packages. Every concrete class below inherits **ModelElement** (a required `name`
LangString; any number of `description`, `note`, `implementationConstraint`,
`taggedValue`) and **SACMElement** (`gid`, `isCitation` + `citedElement`, `isAbstract`
+ `abstractForm`).

| Package | Concrete classes |
|---|---|
| Base | LangString, MultiLangString, ExpressionLangString, Description, Note, ImplementationConstraint, TaggedValue |
| AssuranceCase | AssuranceCasePackage (+Interface, +Binding) |
| Terminology | TerminologyPackage (+Interface, +Binding), TerminologyGroup, Category, Term, Expression |
| Argumentation | ArgumentPackage (+Interface, +Binding), ArgumentGroup, Claim, ArgumentReasoning, ArtifactReference, AssertedInference, AssertedEvidence, AssertedContext, AssertedArtifactSupport, AssertedArtifactContext |
| Artifact | ArtifactPackage (+Interface, +Binding), ArtifactGroup, Artifact, Property, Event, Resource, Activity, Technique, Participant, ArtifactAssetRelationship |

Plus one enumeration, **AssertionDeclaration**: `asserted` (default), `needsSupport`,
`assumed`, `axiomatic`, `defeated`, `asCited`.

Full attribute/containment/reference tables: `docs/sacm/metamodel-inventory.md`.

## References

Load the one that matches the task.

| File | Load when |
|---|---|
| `references/semantics.md` | You need to know what a construct *means*: arguments, assertions, relationship directions, citations, abstract forms, modularity, artifacts, terminology. |
| `references/xmi.md` | Reading or writing XMI: roots, namespaces, ids, references, `xsi:type`, dialects, safety. |
| `docs/sacm/metamodel-inventory.md` | Exact structure: classes, attributes, multiplicities, containment vs references. |
| `docs/sacm/decisions.md` | Before deciding anything the specification leaves open. |
| `docs/sacm/diagnostics.md` | Emitting or testing a diagnostic: stable codes and severities. |
| `docs/sacm/reference/specification-defects.md` | A clause looks contradictory, unsatisfiable or malformed. |
| `docs/sacm/compliance-points.md` | Anything about what the library claims to conform to. |

## Compliance points (clause 2)

| Point | Unit of interchange | Status in this project |
|---|---|---|
| Assurance Case Model (mandatory) | `AssuranceCasePackage` | target |
| Argumentation Model | `ArgumentPackage` | target |
| Artifact Model | `ArtifactPackage` | target |
| Terminology Model | `TerminologyPackage` | target |
| SACM UML Profile | UML models | out of scope |

A compliance point is claimed only when every matrix row it depends on is verified.
