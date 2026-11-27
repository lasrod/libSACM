# SACM 2.3 conformance statement

libSACM implements OMG SACM 2.3 (formal/23-05-08, with the machine-readable model
ptc/22-03-13) and claims four of the five compliance points of its clause 2.

| Compliance point | Clause | Unit of interchange | Mandatory | Matrix row | Claimed |
|---|---|---|---|---|---|
| Assurance Case Model | 2.4 | `SACM::AssuranceCasePackage` | yes | SACM23-CP-001 | yes |
| Argumentation Model | 2.2 | `Argumentation::ArgumentPackage` | no | SACM23-CP-002 | yes |
| Artifact Model | 2.3 | `Artifact::ArtifactPackage` | no | SACM23-CP-003 | yes |
| Terminology Model | 2.5 | `Terminology::TerminologyPackage` | no | SACM23-CP-004 | yes |
| SACM UML Profile | 2.6 | UML models with the SACM profile | no | SACM23-CP-005 | no (out of scope) |

A point is claimed only while its row and every row it depends on are `verified` in the
[functionality matrix](../matrix/functionality-matrix.md).

## What the standard requires

> "Software that conforms to the specification at the Assurance Case Model compliance
> point shall be able to import and export XMI documents that conform with the SACM XML
> Schema produced by applying XMI rules to the normative MOF metamodel defined in this
> entire specification. The top object of the Assurance Case package as a unit of
> interchange shall be the SACM::AssuranceCasePackage element." (§2.4)

The other three points say the same of their own unit:

| Clause | Says it of | Top object |
|---|---|---|
| §2.2 | the Argumentation subpackage, "including the common elements defined in the Common and Predefined diagrams" | ArgumentPackage |
| §2.3 | the Artifact subpackage, "including the common elements defined in the Common and Predefined diagrams" | ArtifactPackage |
| §2.5 | "this entire specification", by a slip (spec defect D29) | TerminologyPackage |

## What the claim means

For each claimed point, with the unit's package (or its interface or binding) as root:

- **Import.** A strict read reads without loss every document that is valid against the
  SACM XML Schema, except as the limits below say.
- **Export.** A strict write gives a document that is valid against the SACM XML Schema.
- **Round trip.** Read, write, read gives an equal document, and writing the same document
  always gives the same bytes.
- **Constraints.** `validate` reports the constraints of the governing clauses as
  diagnostics. A strict write does not validate: a draft saves.

## The SACM XML Schema

OMG publishes no XML Schema for SACM 2.3. `cargo xtask schema` generates one from
ptc/22-03-13 by the XMI 2.5.1 schema production rules (clause 8) with the default tag
values ([ADR 0043](../adr/0043-the-sacm-xml-schema.md)). It is generated, not distributed.

It differs from a literal application of the rules in these ways, each needed because the
standard leaves a gap:

- **Namespace.** The model names none. The schema uses
  `http://www.omg.org/spec/SACM/20220301`, a project choice (D-NS-PIN).
- **Five model corrections,** where the model and the specification text disagree and the
  text is followed:
  - unnamed Base properties take their association's name;
  - `ArtifactAssertedRelationship` is `ArtifactAssetRelationship`, an ArtifactAsset;
  - an Event's `occurece` is `date`;
  - a Resource has `location`;
  - `ArgumentationElement` is abstract.
- **Contained objects, by rule 4f.** It declares a contained object whose class has
  subclasses as a wrapper around an element named after the class. The document rules name
  it after its property (rule 2a), so that schema would reject every XMI document.
- **Contained objects, as generated.** The schema instead accepts, under the property's
  name, the attributes and children of any class the property admits.
- **Associations** (rule 7) get no elements: the model does not name their ends.
- **Extensions.** Every class admits both `xmi:Extension` (§7.8.3) and `xmi:extension`
  (rule 4c), as XMI.xsd declares both.
- **XMI metadata.** XMI.xsd is imported unmodified (see the limits below).

## How it is checked

- CI generates the schema and validates, with xmllint, every strict golden file and every
  strict output the `sacm` tool writes from the test fixtures and the corpus
  (LIB-TOOL-023).
- Four unit documents, one per point, hold every concrete class and every property of the
  point's packages (the Assurance Case one, of the whole model), read and validate with no
  diagnostic, and are their own strict output (SACM23-CP-001 to -004).
- A test compares the model, class by class and property by property, with the library
  (LIB-TOOL-004).

## Limits of the claim

- **Namespace, strict.** Strict mode reads and writes the pinned namespace only.
- **Namespace, tolerant.** A document whose roots are in the namespace of SACM 2.0, 2.1 or
  2.2, or in another namespace whose URI contains `sacm`, is read in tolerant mode as
  SACM 2.3, with a SACM-XMI-008 or SACM-XMI-002 Warning (LIB-COMPAT-004).
- **Namespace, refused.** A SACM 2.4 root, and a root in any other namespace or none, is
  refused in both modes.
- **What the schema does not check.** Some documents valid against the generated schema
  are not read as they stand, because the schema (ADR 0043 S4 and S8):
  - ignores multiplicities;
  - types Booleans as strings;
  - accepts under a property any `xmi:type` with the attributes and children of every
    class the property admits;
  - lets every object carry an `href` and an `xmi:idref`;
  - checks no IDREF target.
- **Refused by a strict read, kept by a tolerant read** as preserved content with a
  Warning:
  - a value given twice for a single-valued property, or a multi-valued property given
    both as an attribute and as elements (SACM-MULT-001);
  - a Boolean that is no `xsd:boolean` (SACM-ENUM-001);
  - a contained object with no `xmi:type` where its property's type is abstract
    (SACM-XMI-005);
  - an `xmi:XMI` element inside `xmi:XMI`, which a tolerant read keeps outside every
    element (SACM-XMI-003).
- **Other content that does not conform where it stands** is SACM-XMI-003. The
  [diagnostics catalog](diagnostics.md) lists every case and says what each mode does
  with it.
  - A strict read refuses it, or keeps it as preserved content with an Error, so that a
    strict write refuses the document (SACM-XMI-006).
  - A tolerant read keeps it with a Warning.
- **Among the SACM-XMI-003 cases:**
  - an `xmi:type` that names no concrete class the property admits, or a root's
    `xmi:type` that names another class;
  - an attribute or child that the object's class lacks but another class of the property
    has, such as `isCounter` on a Claim;
  - an `href` or `xmi:idref` on a root, on a `name`, `content`, `key`, `location` or
    `value`, or on an element that also has content, and both on one reference element;
  - a reference element with neither;
  - a same-file proxy that names no root, or a root of a class its property or its
    `xmi:type` does not admit;
  - a same-file proxy that names a root with an `href` or `xmi:idref` of its own, or a
    root that keeps an XMI 2.5.1 `xmi:type` or an `xsi:type` (which only a root can carry);
  - a same-file proxy that names a root another proxy names too, and each proxy of a cycle.
- **A spelling.** The model's spelling `ArtifactAssertedRelationship` as an `xmi:type` is
  kept by a strict read with a SACM-XMI-013 Error, and read as `ArtifactAssetRelationship`
  by a tolerant read (SACM23-XMI-009).
- **XMI metadata** (documentation, extensions, differences, `xmi:uuid`, `xmi:label`) is
  written back exactly as read. Differences are never applied.
- **XMI metadata, strict write.** It checks only its ids and IDREFs. If the metadata was
  otherwise not valid against XMI.xsd when read, it is not valid when written
  (ADR 0043 S7).
- **Links to other files** are held and written back, never resolved.
- **Comments and processing instructions** carry no model content and are not kept: a read
  drops them and a write writes none (ADR 0033).
- **Size.** A read refuses, in both modes, input over its limits;
  `ReadOptions::with_limits` raises them (LIB-SEC-003, LIB-SEC-007). The defaults:
  - 32 MiB of input;
  - depth 256;
  - 1,000,000 XML nodes;
  - 256 namespace bindings in scope at an element;
  - 256 attributes on an element, not counting one namespace declaration per prefix.
- **DOCTYPE.** A document with a DOCTYPE declaration is refused in both modes
  (LIB-SEC-001).
- **Encoding.** A read decodes UTF-8 (and so ASCII), UTF-16 with a byte order mark and
  ISO-8859-1 (LIB-COMPAT-016); a document in any other encoding, such as windows-1252, is
  refused in both modes (SACM-XML-002). XML 1.0 §4.3.3 requires only UTF-8 and UTF-16.
- **Roots.** Only SACM packages are read as roots, so SACM packages inside a non-SACM root
  element are not read. Refused in both modes (SACM-XMI-001):
  - a document element that is neither a SACM package nor `xmi:XMI`;
  - an element under `xmi:XMI`, outside the XMI namespace, that is not a SACM package:
    another SACM class such as `sacm:Claim`, or an element outside SACM and XMI.
- **The check itself.** xmllint does not check that an IDREF names an id; a strict write
  refuses a reference that names no `xmi:id` of its output (LIB-COMPAT-007).

## Not claimed

- The SACM UML Profile compliance point.
- That a document from another tool is accepted: that is tolerant mode (`LIB-COMPAT-*`),
  and is never presented as conformance.
