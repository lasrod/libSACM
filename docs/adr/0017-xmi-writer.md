# 0017. The XMI writer

- Status: T1–T5 accepted (the maintainer, 2026-11-12); W1–W6 proposed
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

**Departures from C++, proposed:**
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
- **W6.** There is no option to export under another SACM namespace. C++ has
  `SaveOptions::namespace_uri` (`options.h:26-43`; `xmi_writer.cpp:524-527`), which can
  also re-export under `LoadResult::source_namespace` (C++ decision #18). This release
  always writes the pinned URI. The option is deferred to LIB-COMPAT-001, where it belongs
  with the tolerant load modes and their accepted namespaces.
- **W5.** The namespace constants stay crate-private. C++ makes them public
  (`metadata/namespaces.h:17-34`). They can be made public later without a break.

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
- Until SACM23-XMI-012 and SACM23-XMI-017 land, only nameless documents with default
  assertion declarations can be written.
- Adding diagnostics to a successful write later would change the return type. That is
  accepted, because C++ shows no such case.

## Alternatives considered

- `xsi:type`, as C++ writes it. Rejected by the maintainer for XMI conformance.
- `write_to(impl io::Write)`. Not now: it would need an I/O error variant, and bytes could
  reach the sink before a refusal is found. It can be added later.
- Writing an empty `xmi:XMI`. Rejected: the reader cannot read it back.
