> **Provenance.** Imported verbatim from the assurance-forge project
> ([lasrod/assurance-forge](https://github.com/lasrod/assurance-forge), MIT, same
> author) at commit `0f51870`, where these defects were found while implementing
> clause-level validation in its C++ SACM library. Statements about "Assurance Forge"
> or "the library" below describe that implementation. This crate's own resolutions
> are in [decisions.md](../decisions.md); links to other assurance-forge documents
> resolve in that repository.

# SACM 2.3 specification defects found by implementation

Defects in the normative SACM 2.3 text (`formal/23-05-08`) and its
machine-readable model (`ptc/22-03-13`), found while implementing clause-level
validation in `libs/sacm`. Each was found because a check had to be written
against the clause, which is a different exercise from reading it.

This page is **input to the OMG submission**, not a description of Assurance
Forge. What the library does about each one is recorded in
[decisions and questions](sacm-decisions-and-questions.md); what it validates is
in the [conformance matrix](sacm-conformance-matrix.md). Prepared for
[#200](https://github.com/lasrod/assurance-forge/issues/200).

Every quotation is from the published PDF unless marked otherwise.

## D1 — Clause 10.5 prints clause 10.6's Constraints

`TerminologyPackageInterface` (10.5) and `TerminologyPackageBinding` (10.6) carry
**identical** Constraints sections:

> The participantPackages should be either TerminologyPackage or
> TerminologyPackageInterface
> OCL: `self.participantPackage->forall(pp|pp.oclIsKindOf(Terminology::TerminologyPackage))`

`participantPackage` is a *binding* association. An interface has no
participants — it has `implements`. So 10.5 states a constraint on a feature its
own class does not have, and the constraint the interface should carry is
absent.

**Why it matters.** Its siblings both state that constraint: 11.6 says an
`ArgumentPackageInterface` is "only allowed with isCitation=true and
+citedElement refer to ArgumentAssets within the ArgumentPackage implementation
referred to by implements", and 12.5 says the same for `ArtifactPackageInterface`.
A `TerminologyPackageInterface` is left with no content rule at all, so two
conforming tools can disagree about whether one may contain non-citations.

**Suggested resolution.** Replace 10.5's Constraints with the terminology
analogue of 11.6.

## D2 — Three clauses state the participant rule four ways

The same concept — what may be a participant of a package binding — is specified
four times and agrees with itself nowhere:

OCL below is written with `<code>` and `&#124;` rather than a backslash-escaped
pipe inside backticks: a Markdown code span takes the backslash literally, so
`\|` would render the operator as `\|` and quietly misquote the specification in
a page whose whole purpose is quoting it accurately.

| Clause | Rule as written |
|---|---|
| 9.4 `AssuranceCasePackageBinding` | <code>forall(pp &#124; pp.oclIsTypeOf(AssuranceCasePackage) or pp.oclIsTypeOf(AssuranceCasePackageInterface))</code> — exact type; **excludes** a binding |
| 10.6 `TerminologyPackageBinding` | <code>forall(pp &#124; pp.oclIsKindOf(Terminology::TerminologyPackage))</code> — kind-of; **admits** a binding |
| 11.5 `ArgumentPackageBinding` | <code>forall(pp &#124; pp.oclIsTypeOf(Argument::ArgumentPackageInterface))</code> — **interfaces only** |
| 11.5 Associations block | `participantPackage : ArgumentPackage[2..*]` — the general type |

11.5 contradicts **itself**: its OCL admits only `ArgumentPackageInterface` while
its own Associations block declares `ArgumentPackage`.

**Why it matters.** A validator cannot be written to satisfy all four. A document
whose `ArgumentPackageBinding` names two plain `ArgumentPackage`s is conformant
under the Associations block and non-conformant under the OCL three lines below
it. Assurance Forge enforces only what all four agree on — that a binding is not
a participant of a binding — at warning severity, and records the reason.

**Suggested resolution.** Pick one rule and state it identically in all four
clauses. `oclIsKindOf(<Family>Package)` excluding `<Family>PackageBinding` is the
reading that satisfies every clause's prose.

## D3 — Clause 11.5's OCL is not well-formed

> `self.argumentationElement->forAll(e|e.isCitation = true and e.citedElement <> null`

The expression is unbalanced — one closing parenthesis short — and the sentence
ends without a terminator. It is the only statement of the binding-content rule
in OCL form.

**Suggested resolution.** `self.argumentationElement->forAll(e | e.isCitation = true and e.citedElement <> null)`.

## D4 — Clause 11.4 and clause 11.6 cannot both be satisfied

11.4 Constraints:

> If an ArgumentPackage has nested ArgumentPackages, then it is only allowed to
> contain ArgumentPackages.

11.6 Semantics:

> An ArgumentPackageInterface resides insided [sic] the ArgumentPackage to which
> it refers.

An `ArgumentPackageInterface` **is** an `ArgumentPackage` (11.6 Superclass), and
11.6 requires it to reside inside the package it describes. So any package that
declares an interface "has nested ArgumentPackages", and 11.4 then forbids it
from containing the claims the interface exists to expose. Every non-empty
package with an interface is non-conformant by construction.

**Suggested resolution.** Scope 11.4 to nested packages that are neither
interfaces nor bindings, which is the reading that makes both clauses hold.

## D5 — `Resource.location` is in the text and absent from the model

12.10 Attributes:

> `location:Base::MultiLangString` (composition) — the path or URL specifying the
> location of the Resource, can be in multiple languages.

`ptc/22-03-13` declares no such attribute on `Resource`. `location` is the only
payload the class has: without it a `Resource` records that a resource exists and
nothing about where.

**Why it matters.** A producer implementing the text emits `<location>`; a
consumer implementing the model has nowhere to put it. This is the most
consequential of the divergences here because it is not a spelling difference —
it is the entire content of a class.

**Suggested resolution.** Add it to the machine-readable model.

**Addendum (libSACM, slice 0055).** The text also gives `location` no multiplicity
(§12.10), so its absence is not reported (D-SEV). **This crate** reads and writes
`location` as a `Resource`'s MultiLangString role, and validates a present one as every
MultiLangString (ADR 0034 amendment 1).

## D6 — Six further prose/model divergences

Found by holding the text against `ptc/22-03-13` attribute by attribute. Listed
with the direction Assurance Forge resolved them and why, in
[the metamodel inventory](sacm-2.3-metamodel-inventory.md).

These six plus `Resource.location` (D5, reported separately because its
consequence is different in kind) are the seven the completeness audit found.

| Clause | Text | Model |
|---|---|---|
| 8.5 `MultiLangString.value` | `LangString[1..*]` composition | no bounds, i.e. `[1..1]` |
| 8.7 `UtilityElement.content` | `[0..1]` | `[1..1]` |
| 8.6 `ModelElement.description` | `Description[0..1]` | `[0..*]` |
| 8.4 `ExpressionLangString.expression` | "(composition)" marker with reference wording | plain reference |
| 11.2 / 11.4 member role | `argumentationElement` | `argumentElement` |
| 10.5 `TerminologyPackageInterface` superclass | "TerminologyElement" | `TerminologyPackage` (the clause's own diagram agrees with the model) |

8.5's is the one with a consequence beyond spelling: at `[1..1]` a
`MultiLangString` holds one language, which makes the clause's own uniqueness
constraint — "For each of the LangString in the value feature, their +lang must
be unique" — vacuous.

**Suggested resolution.** Regenerate the machine-readable model from the text, or
state which of the two is normative when they differ. Clause 2 requires importing
"XMI documents that conform with the SACM XML Schema produced by applying XMI
rules to the normative MOF metamodel", which points at the model; the rest of the
specification is the text. Implementers currently have to choose per attribute.

## D7 — No instance-document namespace is determined

The normative MOF model declares no `nsURI` on any package, and the PDF prints
none. Under XMI 2.5.1 an instance document's namespace derives from the
`org.omg.xmi.nsURI` tag, so nothing in SACM 2.3 determines one.

**Consequence, observed.** Every producer invents one and no two agree. The EMF
reference implementation ([github.com/wrwei/SACM](https://github.com/wrwei/SACM))
declares one namespace *per metamodel package* (`http://omg.sacm/2.2/base`,
`/argumentation`, …); this project pins a single document namespace. Both are
conformant, and neither can read the other without dialect-specific code. That is
the interoperability failure clause 2 exists to prevent.

**Suggested resolution.** Determine a namespace in 2.4, and say whether it is
per-document or per-package.

## D8 — The Base properties have no names

In `ptc/22-03-13`, 11 of the 60 class-owned properties carry no `name`
attribute, all of them in Base: five on ModelElement, `MultiLangString.value`,
`UtilityElement.content`, `TaggedValue.key`, `ExpressionLangString.expression` and
two on SACMElement. The role name exists only as the name of the Association:

```xml
<ownedAttribute xmi:type="uml:Property" xmi:id="_19_0_4_68a022b_1650976745149_959605_5171"
   visibility="public" aggregation="composite" type="…5022" association="…89586_5170"/>
```

**Consequence.** XMI production rules take an element's tag from the property
name, so strictly the model gives no tag for `name`, `value`, `content`, `key` and
the others. The text supplies them (§8.5 `value:…`, §8.6 `name:…`), and this
project takes the role names from the Associations, so nothing is ambiguous in
practice.

Separately, `MultiLangString.value` is `visibility="private"`, while every other Base
containment is public. No consequence for XMI is known.

**Suggested resolution.** Name the properties in 2.4, and make `value` public.

## D9 — Clause 10.4's TerminologyPackage role

*Found by libSACM (slice 0017); not part of the assurance-forge import.*

§10.4 reads:

> "TerminologyElement:TerminologyElement[0..*] (composition) - TerminologyElements
> contained in the TerminologyPackage, it can be either TerminologyPackage (and its
> sub-types) or TerminologyAssets (or its sub-types)."

- **Spelling.** ptc/22-03-13 names the end `terminologyElement`. The text capitalises it.
- **Type.** The prose leaves out TerminologyGroup, although §10.3 gives TerminologyGroup
  "Superclass TerminologyElement", so the declared type admits it.

**This crate** follows the model. It admits TerminologyGroup (LIB-API-008), as the C++
library does. Its XMI writer will write `terminologyElement`, and tolerant import will
also accept the capitalised spelling, under the rules SACM23-XMI-009 applies to the other
spellings, once a TerminologyPackage is read (SACM23-XMI-028, planned), which C++ does
not.

**Suggested resolution.** Use the model's spelling, and either list TerminologyGroup in
the prose or drop the list.

## D10 — XMI 2.5.1 states three namespace URIs for itself

*Found by libSACM (slice 0021); a defect of XMI 2.5.1 (formal/15-06-07), not of SACM.*

- **The cover and the normative XMI.xsd** use `http://www.omg.org/spec/XMI/20131001`. The
  schema is pinned as `third_party/sacm-2.3/XMI-2.5.1-XMI.xsd` (SHA-256
  `7b228718d35a15a8e2fb99f078d36eee4b23b7a2cd405e7ac81b9d27bf2fab5d`, LIB-TOOL-021), which
  `cargo xtask fetch-spec` downloads.
- **§7.5.1 (p. 8)** reads: "The version of this XMI specification is 2.4.1, and its XMI
  namespace is "http://www.omg.org/spec/XMI/20110701,"".
- **Rule 1f (p. 60)** writes `"xmlns:xmi='http://www.omg.org/spec/XMI/20100901'"`.

XMI.xsd also declares no `version` attribute, although §7.6.3 mentions "the version and
type attributes" and the §7.10.2 example writes `version="2.1"`.

**This crate** uses `http://www.omg.org/spec/XMI/20131001`, the target namespace of the
normative schema (SACM23-XMI-001). It reads `xmi:version` on the `xmi:XMI` element and on
roots and ignores it, and export does not write it (SACM23-XMI-024). One more residue of
the old attribute: the §7.8.7 (p. 20) datatype complexType still declares
`<xsd:attribute name="version" … form="qualified"/>`, for datatype values only.

**Suggested resolution.** Use the XMI.xsd namespace throughout the text, and either
declare `version` in XMI.xsd or drop it from the text.

## D11 — XMI 2.5.1 both requires and limits the type attribute

*Found by libSACM (slice 0025); a defect of XMI 2.5.1 (formal/15-06-07), not of SACM.*

- **§7.6.3 (p. 15)** limits it: "The type attribute is used to specify the type of object
  being serialized, when the type is not known from the model."
- **Rule 2g (p. 63)** opens "You must specify the class name using the "type" attribute."
- **Production 2d (p. 62)** makes `2g:TypeAttrib` mandatory: it has no `?`, while
  `IdentityAttribs` beside it is optional.

Read literally, rule 2g and production 2d put a type attribute on every element; §7.6.3
puts it only where the class cannot be known from the role.

**This crate** takes the §7.6.3 reading, as C++ (`xmi_writer.cpp:435-439`) and EMF do. It
writes `xmi:type` only where the class is not known from the role (SACM23-XMI-006, ADR
0017), and it reads either form.

**Suggested resolution.** Make 2g conditional in production 2d and rule 2g, matching
§7.6.3.

## D12 — Two editorial slips around assertionDeclaration

*Found by libSACM (slice 0030); editorial, no behaviour impact.*

- **SACM §11.11 (p. 39)** says a Claim "can be declared as being assumed (i.e.,
  assertionDeclared = assumed)". The property is `assertionDeclaration` (§11.10).
- **XMI 2.5.1 §7.8.4 (p. 18)** prints `<xsd:simpleType base="enumName" >`, which is not a
  valid schema: a simpleType takes `name`. §8.2.1 rule 8 (p. 53) has the correct
  `name='…'`, and this crate follows rule 8.

**Suggested resolution.** Correct both texts.

## D13 — The model omits the clause 8.2 Boolean defaults

*Found by libSACM (slice 0042).*

- **SACM §8.2 (p. 18)** declares "isCitation[0..1]=false" and "isAbstract[0..1]=false".
- **ptc/22-03-13** gives `SACMElement.isCitation` and `isAbstract` a `lowerValue` of 0
  and no `defaultValue`, while `AssertedRelationship.isCounter`, typed the same way, has
  one (`<defaultValue xmi:type="uml:LiteralBoolean" …/>`, false).

Without a default, a file that leaves the flag out says nothing, and XMI 2.5.1 §9.4.1
("Properties whose values are the default values are not serialized") gives a writer no
value it may omit.

**This crate** follows the text (ADR 0026): absent and `false` are the same value, and
`xmi::write` writes the flag only when true (SACM23-BASE-007, SACM23-BASE-008).

**Suggested resolution.** Add `defaultValue` false to both properties in the model.

## D14 — The model types its Booleans by a MagicDraw link

*Found by libSACM (slice 0042).*

ptc/22-03-13 types `SACMElement.isCitation`, `SACMElement.isAbstract` and
`AssertedRelationship.isCounter` as

```xml
<type href="UML_Standard_Profile.mdzip#eee_1045467100323_191782_59"/>
```

a reference into a MagicDraw tool library, not the UML primitive type
`http://www.omg.org/spec/UML/20131001/PrimitiveTypes.xmi#Boolean`, which its Strings use
(`PrimitiveTypes.xmi#String`). A tool without that library cannot resolve the type, so
only the text says these properties are Booleans.

**This crate** takes them as `Boolean` from the text, and reads them as `xsd:boolean`
(`true`, `false`, `1`, `0`), the XML Schema type XMI 2.5.1 maps a UML Boolean to.

**Suggested resolution.** Type the three properties by `PrimitiveTypes.xmi#Boolean`.

## D15 — The clause 8.6 OCL misspells implementationConstraint

formal/23-05-08 §8.6 (p. 20) gives the constraint as
`self.implmentationConstraint->size() > 0 implies self.isAbstract = true`. The property is
`implementationConstraint` (§8.6 Associations; ptc/22-03-13), so the OCL as written does
not type-check.

**This crate** enforces the intended rule (SACM23-VAL-021, SACM-ABS-001).

**Suggested resolution.** Correct the spelling.

## D16 — XMI 2.5.1 production 2b gives a nil value no element name

XMI 2.5.1 production 2b (p. 62) has the alternative `( "<xsi:nil='true'/>" )`, which names
no element and so is not an XMIValueElement; §9.4.1 (p. 56) says a null property value "is
serialized as XMIValueElement with attribute nil='true'". The production as restated on
p. 104 omits the nil alternative altogether.

**This crate** reads `xsi:nil` on a value element, as §9.4.1 says (SACM23-XMI-026).

**Suggested resolution.** Write the alternative as `"<" //xmiName// " xsi:nil='true'/>"`.

## D17 — The date attributes have no resolvable type, and Event's is misspelled

*Found by libSACM (slice 0055).*

formal/23-05-08 types four attributes `date`:

> "date: date[0..1] - the date on which the artifact was created." (§12.7, p. 48)
> "date: date[0..1] - the date on which the Event occurred." (§12.9, p. 48)
> "startTime: date[0..1] - time when the activity started." / "endTime: date[0..1] - time
> when the activity ended." (§12.11, p. 49)

The specification defines no `date` type and no lexical form; §4 (p. 6) only lists
"date" among the primitive types an attribute may have. ptc/22-03-13 types all four as

```xml
<type href="UML_Standard_Profile.mdzip#_be00301_1073305590699_364818_1"/>
```

a reference into a MagicDraw tool library, as for the Booleans (D14), and names Event's
attribute `occurece`. The model has no `org.omg.xmi.schemaType` tag, so the SACM XML
Schema the XMI rules produce types all four `xsd:string` (XMI 2.5.1 §8.2.1 rules 4d and
4j, p. 50–51; §7.11.6, p. 35, gives exactly this case for a `Date` attribute).

**This crate** holds each value as a string exactly as written and never diagnoses one
(D-DATE), reads and writes the text's `date` on an Event, and reads the model's
`occurece` as that `date` in tolerant mode, with a SACM-XMI-013 Warning; a `date` given
beside it is read instead, and the `occurece` is the repeat. A strict read keeps
`occurece` as preserved content with a SACM-XMI-013 Error (SACM23-XMI-009).

**Suggested resolution.** Add a `Date` datatype to the model, tagged
`org.omg.xmi.schemaType` `xsd:date` or `xsd:dateTime`, type the four attributes by it,
and correct the spelling `occurece` to `date`.

## D18 — Informative Semantics name classes SACM 2.3 does not define (editorial)

*Found by libSACM (slice 0055).*

§12.7 (Semantics) speaks of "counterevidence (AssertedCountedEvidence with isCounter =
true)", and §12.11 (Semantics) of "ArtifactActivityRelationships" and
"ActivityRelationships". SACM 2.3 defines none of these classes: clause 11 has
`AssertedEvidence` (§11.15), and §12.14 defines only `ArtifactAssetRelationship`.

**This crate** is not affected: these sentences name no class or property the crate reads or writes.

**Suggested resolution.** Use the defined class names: `AssertedEvidence` and
`ArtifactAssetRelationship`.

## Related

- [GSN / SACM metamodel gaps](sacm-gsn-metamodel-gaps.md) — the GSN v3 side, for the SCSC ACWG.
- [SACM 2.4 watch](sacm-2.4-watch.md) — the RTF issues that would change this.
- [Decisions and questions](sacm-decisions-and-questions.md) — what the library does about each defect.

## D19 — The model names ArtifactAssetRelationship otherwise, with no superclass

*Found by libSACM (slice 0057; recorded at M3's close).*

formal/23-05-08 §12.14 (pp. 49–50) defines

> "ArtifactAssetRelationship enables the ArtifactAssets of a structured assurance case to be
> linked together." … "Superclass ArtifactAsset"

with `source:ArtifactAsset[1..*]` and `target:ArtifactAsset[1..*]`. ptc/22-03-13 names the
class `ArtifactAssertedRelationship`, gives it no generalization, and types the same two
ends by ArtifactAsset. A schema generated from the model would therefore spell the type
`ArtifactAssertedRelationship`, and the class would inherit nothing from ArtifactAsset: no
`property`, and none of the SACMElement and ModelElement features.

**This crate** follows the text (D-TEXT-MODEL): the class is `ArtifactAssetRelationship`, an
ArtifactAsset. A tolerant read reads the model's type name with a SACM-XMI-013 Warning. A
strict read keeps it as preserved content with a SACM-XMI-013 Error, D-STRICT's one exception
(SACM23-XMI-009). `xmi::write` writes the text's name.

**Suggested resolution.** Rename the class in the model to `ArtifactAssetRelationship` and
give it the generalization ArtifactAsset, as the text states.
