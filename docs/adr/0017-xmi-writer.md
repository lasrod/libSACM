# 0017. The XMI writer

- Status: accepted (the maintainer: T1–T5 on 2026-11-12; W1–W6 by approving PR #35 for merge, 2026-11-12)
- Date: 2026-11-12
- Matrix rows: SACM23-XMI-003, SACM23-XMI-004, SACM23-XMI-006, SACM23-XMI-007,
  SACM23-XMI-013, LIB-API-010

## Context

Until now the crate could only read. Export is the first step of M1's round trip. The
assurance-forge C++ writer (`libs/sacm/src/io/xmi_writer.cpp`, `io/xmi.h`, `io/options.h`)
is the design reference (CLAUDE.md rule 6). This ADR names every place where this crate
does something different.

## Decision

`sacm::xmi::write(document: &Document, options: &WriteOptions) -> Result<String,
WriteError>`. `WriteOptions` is empty for now; the save mode comes with LIB-COMPAT-001.
`WriteError` (ADR 0011) has three variants: `NoRoot`, `UnrepresentableCharacter { element,
character }` and `Unsupported { element, reason }`, and none of them has a catalog code.
`NoRoot` is a plain unit variant, so callers can match it exactly. If more data is ever
needed, that will be a new variant; the other two variants are `#[non_exhaustive]`.

**The output, byte for byte:**
- UTF-8 with no BOM, `<?xml version="1.0" encoding="UTF-8"?>`, LF line endings, two spaces
  per depth, `<x … />` (with a space before `/>`, as pugixml writes) for an element with no
  children, and a final LF.
- `xmlns:sacm`, `xmlns:xmi` and `xmi:version="2.0"` appear on the document element only.
  Then come `xmi:type` (when the class is not known from the role) and `xmi:id`.
- Roots are written as `sacm:<Kind>`. Children are written under their role name, with no
  prefix. The role is taken from one crate-private table in `model::containment`, from
  which `admits` is derived.
- In attribute values, `&`, `<`, `>`, `"`, TAB, LF and CR are escaped. Everything else is
  written as UTF-8.

**Checks, in order.** The first failure is the result.
1. A document with no root fails with `NoRoot`.
2. Then each element is checked in document order, which is `Document::elements()`
   pre-order. All of an element's checks run before the next element's:
   - its id first, where the first character that is not an XML 1.0 `Char` fails with
     `UnrepresentableCharacter`;
   - then its name;
   - then a non-default `assertionDeclaration`.

   The name and the `assertionDeclaration` each fail with `Unsupported`.

**Departures from C++, decided:**
- **T1.** `xmi:type`, as XMI 2.5.1 specifies (§7.8.8, rule 2g; SACM sets no
  `useSchemaExtensions`), and no `xsi` namespace. C++ writes `xsi:type` and always declares
  `xmlns:xsi` (`xmi_writer.cpp:435-439, 459, 540`). Import keeps accepting both forms.
  The maintainer confirmed this after a fuller analysis (2026-11-12). The reasons:
  - **Where `xsi:type` comes from.** Every existing SACM file in the C++ corpus uses
    `xsi:type`, but that is EMF's default. The third-party files are EMF output
    (`xmlns:xmi="http://www.omg.org/XMI"`, `xmi:version="2.0"`), and the rest were written by
    C++, which adopted those conventions.
  - **OMG's own XMI uses `xmi:type`.** ptc/22-03-13 (MagicDraw) has 350 `xmi:type` and no
    `xsi:type`.
  - **Schema validation.** `xsi:type` means XSD type substitution. A schema generated from
    SACM by the default XMI rules has no derived types, so `xsi:type` output would most
    likely not validate against the schema that SACM clause 2 refers to.
  - **Interoperability does not decide it.** EMF-based SACM tools use their own namespaces,
    so they need a namespace mapping for our files whichever type attribute is used.
  - **It can be changed later.** If a reader that needs `xsi:type` appears, a
    `WriteOptions` switch can be added without breaking the API.

  *Where* the attribute is written follows C++: only where the class is not known from the
  role (XMI 2.5.1 §7.6.3). Rule 2g and production 2d, read literally, require it on every
  element. XMI 2.5.1 contradicts itself here (specification defect D11), and this crate
  takes the §7.6.3 reading, as C++ and EMF do. T1 departs from C++ only in using `xmi:`
  rather than `xsi:`.
- **T2.** A document with no root is an error. C++ writes an empty `xmi:XMI`, which its own
  reader refuses (`:529-549`).
- **T3.** Roots are written in document order (ADR 0010 D1). C++ writes AssuranceCasePackage
  roots first (`:543-548`).
- **T4.** Children are written in model order (ADR 0010 D2), not grouped by role
  (`:254-337`).
- **T5.** Content no row writes yet (a name, a non-default `assertionDeclaration`) is
  refused, never dropped (LIB-API-010, the write-side twin of ADR 0015's interim rule).
  C++ writes names.
  *Superseded by amendment 2: nothing a `Document` holds is refused any more.*

**Departures from C++, accepted with the merge of PR #35:**
- **W1.** `Result<String, WriteError>`, not `SaveResult { xml, diagnostics, ok }`
  (`options.h:58-62`). This is ADR 0011 X1. C++ never fills diagnostics on a successful
  save (`xmi_writer.cpp:482-556`).
- **W2.** There is no file function. C++ has `save_xmi_file` (`xmi.h:23-24`), and it reports
  a failed file write as SACM-XML-001. This follows ADR 0015 and ADR 0011 X2.
- **W3.** A character outside XML 1.0 `Char` in an id is refused. pugixml writes C0
  controls as `&#NN;`, which is not well-formed, stops at U+0000, which silently cuts the
  id, and writes U+FFFE and U+FFFF raw (`pugixml.cpp:4021, 4048-4054`).
- **W4.** A refusal names the first element in document order. C++ lists every affected
  element (`xmi_writer.cpp:485-500`). This is the same rule as ADR 0016 D3.
- **W5.** The namespace constants stay crate-private. C++ makes them public
  (`metadata/namespaces.h:17-34`). They can be made public later without a break.
- **W6.** There is no option to export under another SACM namespace. C++ has
  `SaveOptions::namespace_uri` (`options.h:26-43`; `xmi_writer.cpp:524-527`), which can
  also re-export under `LoadResult::source_namespace` (C++ decision #18). This release
  always writes the pinned URI. The option is deferred to LIB-COMPAT-001, where it belongs
  with the tolerant load modes and their accepted namespaces.

**Kept from C++:**
- `xmi:version="2.0"` (`namespaces.h:32`; the maintainer, 2026-11-12). XMI.xsd 2.5.1 does
  not declare it, so strict output may not validate against that schema. XMI 2.5.1 is
  itself inconsistent here: §7.5.3 says the namespace identifies the version, and rule 1a
  says the XMI element specifies it.
- The `sacm` and `xmi` prefixes, and every package kind written as a root.
- `xmi:id` from the element id. An id that is not an NCName is written as it is.

## Consequences

- Written output follows the XMI rules except for `xmi:version`, and except that the type
  attribute is written under the §7.6.3 reading (D11). It is not byte-identical to C++
  output.
- Names are written since amendment 1, and every assertion declaration since amendment 2.
- Adding diagnostics to a successful write later would change the return type. That is
  accepted, because C++ shows no such case.

## Alternatives considered

- `xsi:type`, as C++ writes it. Rejected by the maintainer for XMI conformance.
- `write_to(impl io::Write)`. Not now: it would need an I/O error variant, and bytes could
  reach the sink before a refusal is found. It can be added later.
- Writing an empty `xmi:XMI`. Rejected: the reader cannot read it back.

## Amendment 1 — names (slice 0026; SACM23-XMI-011, SACM23-XMI-012, LIB-API-010)

T5 said names stay refused until SACM23-XMI-012. That row now writes them, so:

- **Names.** An element's name is written as its first child: one `<name … />`, in no
  namespace, with no `xmi:id` or type attribute, carrying `lang` and then `content`. An
  absent field is written as no attribute, and an empty one as `=""` (ADR 0012). An element
  whose name is missing gets no `name` child (ADR 0013). An element whose only content is
  its name gets a start tag, the `name` line and an end tag.
- **Check order.** For each element: its id; then a character outside `Char` in its name's
  `lang`, then in its `content`, which gives `UnrepresentableCharacter`; then a non-default
  `assertionDeclaration`, which gives `Unsupported`. T5 now refuses only the
  `assertionDeclaration` (until SACM23-XMI-017). *Superseded by amendment 2: the
  declaration is written, and the check order ends with the name.*
- **`UnrepresentableCharacter` gains a field, `attribute: WriteAttribute`.** It is a new
  `#[non_exhaustive]` `Copy` enum with the values `Id`, `NameLang` and `NameContent`, and
  later rows add more as they write more attributes. A plain string was rejected: once a
  description's `content` is written, `"content"` would be ambiguous. The message says
  where: "its xmi:id holds U+0001" (unchanged), "its name's lang holds U+0001" or "its
  name's content holds U+0001". The variant is `#[non_exhaustive]`, so this is additive.
- **The reader** reads one `name` child of a package root. A type attribute on `name`
  naming a class other than `LangString` is refused at the `name` start tag, as for roots
  (ADR 0016 D2).
- **Differences from C++, one line each:**
  - C++ writes absent and empty fields alike (`xmi_writer.cpp:260-265`); this crate keeps
    them distinct (ADR 0012).
  - C++ always writes `<name>`, even when the name is missing (`:255-256`); this crate
    writes none (ADR 0013).
  - pugixml writes `>` raw and TAB as `&#09;`; this crate writes `&gt;` and `&#9;`. That is
    a difference in bytes only.
- **Reader differences from C++, one line each.** Each is an interim refusal under ADR 0015
  unless noted.
  - Of two names, C++ keeps the last (`xmi_reader.cpp:1182-1218`), a silent drop. This
    crate refuses the second name until SACM23-XMI-021.
  - C++ reads a name's text as its content (`:805-808`). This crate refuses it
    (LIB-COMPAT-008).
  - C++ matches `name` by local name in any namespace. This crate reads only a `name` in no
    namespace.
  - C++ drops a LangString `xmi:id` with an Info diagnostic (`:793-803`). This crate refuses
    it until SACM23-XMI-022, which will preserve it.
  - C++ reads an ExpressionLangString type on `name`. This crate refuses it (SACM23-BASE-009).

*Amendment 1 status: accepted (the maintainer merged PR #37, 2026-11-12), including the
public `WriteAttribute` type.*

## Amendment 2 — assertionDeclaration (slice 0030; SACM23-XMI-017, LIB-API-010)

- **Writing.** An Assertion's `assertionDeclaration` is written as an unqualified attribute
  directly after `xmi:id`, spelled as the literal's name, and omitted when the value is
  `asserted` (§11.10 `= asserted`; XMI 2.5.1 §9.4.1 "Properties whose values are the
  default values are not serialized"), as C++ does (`xmi_writer.cpp:95-98`). The check
  order of amendment 1 loses its last step.
- **T5 ends.** Nothing a `Document` can hold is refused any more, so
  `WriteError::Unsupported` is removed before any release. The writer's internal "held in
  no role of its parent's class" check, which could not be reached, is replaced by the role
  each element stores when it is inserted. The variant's name and shape
  (`{ element, reason }`, `#[non_exhaustive]`, no catalog code) are reserved: the first row
  that lets a `Document` hold content no row writes adds it back in the same slice. That is
  additive, because `WriteError` is `#[non_exhaustive]`.
- **Reading.** The reader reads the attribute on a Claim and matches its value exactly, after
  XML attribute-value normalisation (XMI 2.5.1 §8.2.1 rule 8: "a simple type derived from
  string"; XSD 1.0 Part 2: whiteSpace `preserve`). An explicit `asserted` reads equal to an
  absent one. Any other value, including white space and other letter cases, is
  `ReadError::Unsupported` at the attribute until SACM23-VAL-006 (ADR 0015's interim rule).
- **Differences from C++, one line each:**
  - C++ reports an unknown literal as SACM-ENUM-001 and keeps the default, losing the raw
    value (`xmi_reader.cpp:1786-1794`); this crate refuses it until SACM23-VAL-006.
  - C++ maps the legacy `justification` to `axiomatic` with a TaggedValue (`:1765-1785`);
    this crate refuses it as an unknown literal (a LIB-COMPAT matter).
  - C++ maps `undeveloped="true"` to `needsSupport` (`:1796-1815`); this crate refuses it
    as an unread attribute.
  - C++ matches the attribute in any namespace; this crate refuses `sacm:assertionDeclaration`.

*Amendment 2 status: accepted (the maintainer, 2026-11-13).*
