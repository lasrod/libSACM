# SACM 2.3 semantics — orientation

What the constructs mean, in plain words, so that analysis, API naming, docs and tests
start from the right mental model. This is a summary for orientation. **Clause text in
formal/23-05-08 is authority**; where this page says "typically", the exact typing rule
is a research task for the row that owns it.

## An assurance case

An `AssuranceCasePackage` is the whole case and the mandatory unit of interchange. It
contains the four kinds of content as sub-packages — argumentation, artifacts,
terminology — and may nest further assurance case packages.

- **ArgumentPackage**: the argument — what is claimed and how claims support each other.
- **ArtifactPackage**: the evidence and its provenance — documents, test results,
  the activities, techniques, people and events that produced them.
- **TerminologyPackage**: controlled vocabulary used to state claims precisely.

## Arguments: claims and asserted relationships

A **Claim** is a proposition put forward as true ("The braking system is acceptably
safe"). Its text is its `name`/`description`.

Every `Assertion` (Claim and every asserted relationship) carries an
**assertionDeclaration** saying how the assertion is put forward:

| Literal | Meaning |
|---|---|
| `asserted` | Put forward as true and (to be) supported. The default. |
| `needsSupport` | Put forward, but support is still owed — an open item. |
| `assumed` | Taken as true without support, explicitly as an assumption. |
| `axiomatic` | True by definition or self-evident; needs no support. |
| `defeated` | Shown false or refuted, typically by counter-argument. |
| `asCited` | Its declaration is that of the element it cites (see citation). |

An assertion may have **metaClaims**: claims *about* the assertion (e.g. about
confidence in it).

**Asserted relationships** are themselves assertions — the argument claims the link
holds — which is why they have declarations and can be the target of other
relationships. Each has `source` (one or more) and `target` (exactly one), an optional
`reasoning`, and `isCounter`:

| Class | Asserts that | Typical source → target |
|---|---|---|
| `AssertedInference` | the sources, taken together, support the target | Claim(s) → Claim |
| `AssertedEvidence` | the cited evidence supports the target | ArtifactReference(s) → Claim |
| `AssertedContext` | the source provides context for interpreting the target | ArtifactReference or Claim → Claim or ArgumentReasoning |
| `AssertedArtifactSupport` | one artifact supports another | ArtifactReference → ArtifactReference |
| `AssertedArtifactContext` | one artifact is context for another | ArtifactReference → ArtifactReference |

**Direction**: SACM relationships run from premise/evidence (`source`) to conclusion
(`target`). (GSN's "SupportedBy" runs the other way; irrelevant here except when
reading third-party files that confused the two.)

**isCounter = true** turns the relationship into a challenge: the sources argue
*against* the target instead of for it. This is how dialectic (counter-) argument is
represented. `isCounter` is required (`[1]`), so a writer always knows it.

**ArgumentReasoning** explains *why* an asserted relationship holds — the inference
rule or strategy — attached via the relationship's `reasoning`. It may point to an
ArgumentPackage (`structure`) that spells the reasoning out in detail. It is an
annotation on a relationship, not a node that relationships pass through.

**ArtifactReference** is how an argument points at evidence: it sits in the
ArgumentPackage and references `ArtifactElement`s (usually in an ArtifactPackage).

**ArgumentGroup** (and ArtifactGroup, TerminologyGroup) is a named grouping of
existing elements by reference. Membership does not move or own anything.

## Citations and abstract forms (SACMElement)

- **Citation** (`isCitation` + `citedElement`): an element that stands in for another
  element, typically one in a different package or case. This is SACM's mechanism for
  modular arguments: package A refers to claim X of package B by containing a citation
  of X. A citation must name what it cites.
- **Abstract form** (`isAbstract` + `abstractForm`): `isAbstract = true` marks a
  template/pattern element (with placeholders); a concrete element can name the
  abstract element it instantiates via `abstractForm`.
- **gid**: an optional model-global identifier, unique within the model when present —
  distinct from the XMI serialization id.

## Modularity: interfaces and bindings

Each package family has an **Interface** and a **Binding** subclass:

- A **package interface** declares what a package exposes to the outside. It
  `implements` exactly one package; its contents are citations of elements of that
  package.
- A **package binding** connects two or more packages (or their interfaces) as
  `participantPackage`s and contains the citations that wire them together.

The specification states these rules inconsistently across the four families; the
project's resolutions are decisions D-BIND-PART, D-IFACE-CITE and D-PKG-HOMOG in
`docs/sacm/decisions.md`.

## Artifacts

`ArtifactAsset`s live in ArtifactPackages and can carry `Property`s:

| Class | What it records |
|---|---|
| `Artifact` | A concrete item of evidence or input (document, test report, model), with `version` and `date`. |
| `Resource` | Something used to produce an artifact; `location` (text-followed, see decisions). |
| `Activity` | Something done, with `startTime` / `endTime`. |
| `Event` | Something that happened, with a `date`. |
| `Technique` | A method used in an activity. |
| `Participant` | A person, team or organisation involved. |
| `Property` | A characteristic of an asset. |
| `ArtifactAssetRelationship` | A typed link between assets (sources → targets, both 1..*), e.g. "produced by", "reviewed by". |

## Terminology

Terminology packages hold `Category`s and `ExpressionElement`s (with a required
`value`): **Term** (an atomic term, optionally with an `externalReference` and an
`origin` model element) and **Expression** (a structured expression over other
expression elements). Text anywhere in the model can point into terminology through an
**ExpressionLangString**, whose `expression` references an ExpressionElement instead of
(or — with a warning — as well as) carrying literal content.

## Language strings

- `LangString`: one piece of text with an optional language tag.
- `MultiLangString`: the same text in several languages (languages should be unique).
- Names are a single LangString; descriptions, notes, constraints and tagged-value
  content are MultiLangStrings.
