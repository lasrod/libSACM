# ADR 0043 — The SACM XML Schema this project generates

- Status: accepted (the maintainer, 2026-11-19: M6 decision Q1, claim all four compliance
  points; 2026-11-22: Q-A and Q-B of the slice 0074 design, both as recommended).
- Date: 2026-11-22
- Matrix rows: LIB-TOOL-023; SACM23-CP-001 to SACM23-CP-004.
- Amends: D-STRICT.

## Context

SACM 2.3 clause 2 defines each model compliance point as the ability to "import and export
XMI documents that conform with the SACM XML Schema produced by applying XMI rules to the
normative MOF metamodel" (§2.2 to §2.5). OMG publishes no such schema: the SACM 2.3 page lists
only the model, ptc/22-03-13. XMI 2.5.1 makes it the standards body's job to produce one
(§7.2), and requires any XMI schema to be equivalent to the one its production rules give
(§2.2.1).

Taken literally, the rules give a schema that rejects ordinary SACM files. Rule 4f gives a
composition whose type has subclasses a content model of child elements named after the
classes (`<name><sacm:LangString/></name>`), while document production 2a names a nested object
after its property (`<name lang="en" …/>`). A schema built that way rejected 21 of the
project's 23 strict golden files. That is spec defect D30; D29 records that clause 2 cites a
schema OMG does not publish.

## Decision

- **S1. Generated, never distributed.** `cargo xtask schema` generates `SACM-2.3.xsd` from
  the pinned ptc/22-03-13 by the XMI 2.5.1 clause 8 rules, with the Table 7.1 default tag
  values (the model sets no `org.omg.xmi` tag). The output is deterministic and its SHA-256 is
  pinned. It is never committed, published or uploaded: OMG material is fetched, not
  redistributed. A user runs the command in a checkout.
- **S2. Namespace.** The target namespace is the pinned SACM 2.3 namespace,
  `http://www.omg.org/spec/SACM/20220301` (D-NS-PIN), because the model gives no URI. Role
  elements and attributes are unqualified (D-ROLE-UNQUALIFIED).
- **S3. Model corrections.** Five recorded corrections are applied before generation (D8,
  D19, D17, D5 and the abstract ArgumentationElement of §11.3), so the schema spells what the
  text specifies.
- **S4. Candidate type at rule 4f.** Where a composition's type has subclasses, the element
  takes a generated *candidate type*: one type holding the attributes and children of every
  concrete class that rule 4f's candidate list names, in that list's order. It checks names,
  enumeration values, ids and structure, and accepts the form production 2a writes. There are
  eight candidate types for SACM.
- **S5. Rule 7 is not applied.** Every Association in ptc/22-03-13 has an unnamed end, and
  35 of 46 have no name, so no AssociationDef element can be named (D32). All ends are
  class-owned properties.
- **S6. Both extension elements.** Every class and candidate type admits `xmi:Extension`
  (§7.8.3) and `xmi:extension` (rule 4c), as XMI.xsd declares both and ADR 0042 keeps both.
- **S7. XMI metadata as read.** XMI.xsd is imported unmodified. A strict save writes XMI
  metadata as read (ADR 0042), so a file whose header is not valid against XMI.xsd keeps that
  header; OMG's own ptc/22-03-13 is one, with `<xmi:exporter>` inside `xmi:documentation`
  (D31). This is stated as a limit of the claim. CI tolerates exactly the two named fixtures
  that show it, and only when every error is on an XMI-namespace element.
- **S8. Necessary, not sufficient.** Validity against this schema does not make a document
  conformant. The schema ignores multiplicities and checks no IDREF target (XMI defaults and
  libxml2), and it accepts more than a strict read does. `validate()` (LIB-COMPAT-007) and the
  strict reader remain the model checks.
- **CI.** The `SACM XML Schema` job (LIB-TOOL-023) generates the schema and runs
  `xmllint --schema` on every strict golden file and on the strict output of every fixture and
  corpus file. It is a required check. It runs on Linux only and is not part of the local gate.

## Alternatives

- **The literal rules:** reject 21 of 23 goldens, so they measure nothing a tool writes.
- **`xmi:Any` (rule 4f with `allowMetamodelExtension`):** validates everything below a root
  and checks nothing.
- **`useSchemaExtensions`:** needs `xsi:type` and ordered content, which SACM 2.3 does not
  set.
- **Strict save refuses invalid metadata:** files from other tools with such headers could
  then be saved only tolerantly, for a header error that says nothing about SACM content.
- **No schema-based claim:** leaves clause 2 unanswered.

## Consequences

- The conformance statement (`docs/sacm/compliance-points.md`) names "the SACM XML Schema"
  as this generated schema, and lists where it departs from the literal rules (S3 to S7).
- D-STRICT: the generated schema spells `ArtifactAssetRelationship` and `date` as the text
  does. XMI metadata is written as read, and is not checked against XMI.xsd beyond its ids and
  IDREFs.
- Spec defects D29 to D32 record the clause 2 and XMI problems found on the way.
