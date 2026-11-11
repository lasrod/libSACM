# 0016. Interchange roots: deviations from the C++ reader

- Status: accepted (maintainer, 2026-11-11)
- Date: 2026-11-11
- Matrix rows: SACM23-XMI-001, LIB-SEC-005. Constrains SACM23-XMI-002, SACM23-XMI-019,
  LIB-COMPAT-002, LIB-COMPAT-004 and LIB-COMPAT-009.

## Context

SACM23-XMI-001 is the first row where `xmi::read` returns a document. The assurance-forge
C++ reader (`libs/sacm/src/io/xmi_reader.cpp`) is the design reference (CLAUDE.md rule 6).

SACM 2.3 clause 2 (§2.2, §2.4) requires an importer to accept XMI documents that conform
to the SACM XML Schema. ADR 0015 lets a release refuse content it cannot represent yet as
`ReadError::Unsupported`. That is a limit of the release, never a drop (D-PRESERVE).

## Decision

We adopt the C++ classification:

- An interchange root is the document element, or each element child of the XMI element
  (`xmi_reader.cpp:2433-2441`), except as D1 says.
- A root whose local name is not a package kind is SACM-XMI-001 (`:2227`, `:2265-2271`;
  `metadata/element_kind.cpp:87-105`).
- A wrapper with no package is SACM-XMI-001 (`:2448-2461`).

The code is `Code::XMI_INVALID_ROOT`, after the C++ `kXmiInvalidRoot`
(`validation/codes.h:14`). It belongs to two `ReadError` variants:

- `InvalidRoot`, with the element's name as written;
- `NoPackage`, for a wrapper with no package.

We deviate from C++ in seven ways:

- **D1. The wrapper is matched by namespace and local name.** C++ takes any element whose
  local name is `XMI` (`:2433`). We require `http://www.omg.org/spec/XMI/20131001`, because
  XMI §7.5.3 defines the XMI element in that namespace, and Namespaces 1.0 names an element
  by URI and local part. An `XMI` element in another namespace, or in none, is
  `Unsupported` before its children are classified, because whether they are roots is
  not decided. LIB-COMPAT-002 may read it later. A child of the XMI element that is in the
  XMI namespace, such as `xmi:documentation`, is XMI data, not a root: it is
  `Unsupported` (SACM23-XMI-019). C++ passes it to its root reader, which reports
  SACM-XMI-001. A schema-conforming document uses the XMI namespace, so this stays
  compliant.
- **D2. A type attribute does not override the element name.** In C++, a resolved
  `xsi:type` or `xmi:type` decides the root's kind (`:2194-2197`). An unresolved one falls
  back to the element name, with a diagnostic (`:2198-2224`). Here the element name decides
  (XMI §7.8.1). A type is accepted only if it names the same class. Any other type is
  `Unsupported`, including a subtype such as `xsi:type="sacm:ArgumentPackageInterface"`,
  which waits for D5. A type value is resolved as an XSD QName, against the namespaces in
  scope (SACM23-XMI-002). An undeclared prefix, or an unprefixed value with no default
  namespace, is `Unsupported`, which C++ (`xmi_reader.cpp:597-658`) resolves to the bare local
  name. This clarifies D2; it does not change it.
- **D3. No partial document.** C++ reports an invalid root, reads the roots beside it and
  returns a document along with the errors. Here the first refusal is the whole result
  (ADR 0015; ADR 0011 X1). Among the refusals found while reading the roots, SACM-XMI-001
  takes precedence over every `Unsupported`, so the roots that get the code do not depend
  on what a release can read. The input checks before them (encoding, DOCTYPE, `Malformed`)
  come first, and so does D1's `Unsupported` for an `XMI` element in another namespace.
- **D4. Unread attributes are refused.** C++ silently skips any XMI, XSI or SACM attribute
  it does not read (`:1543-1546`). Here every attribute that no verified row reads is
  `Unsupported`. Only these are accepted:
  - namespace declarations;
  - `xmi:id`;
  - `xmi:version`;
  - a redundant type.

  This follows D-PRESERVE, and XMI §7.3.3, which expects extension data to round-trip.
  SACM23-XMI-019 and the attribute rows will narrow the refused set.
- **D5. Only AssuranceCasePackage and ArgumentPackage roots are read, for now.** C++ reads
  all twelve package kinds as roots. Here all twelve are recognised, so none gets
  SACM-XMI-001. The other ten stay `Unsupported` until these rows read them:
  - SACM23-ART-001;
  - SACM23-TERM-001;
  - SACM23-AC-002 and SACM23-AC-003;
  - SACM23-ARG-014 and SACM23-ARG-015.
- **D6. A package name outside the pinned namespace is `Unsupported`.** For a root in no
  namespace or in another one, strict C++ reports SACM-XMI-002, or SACM-XMI-008 for an
  older revision, and still reads the root (`:2104-2135`). Here such a root is
  `Unsupported` until LIB-COMPAT-004 assigns those codes. This is temporary.
- **D7. The empty-wrapper refusal has a location.** C++ gives none (`:2448-2461`). Here it
  is located at the XMI element.

LIB-SEC-005 checks the XML declaration before parsing. Its other checks run after
roxmltree has parsed the input, so a DOCTYPE refusal can come before an earlier `<?XML?>`
or surrogate reference. `xmi::read` documents the order.

## Consequences

- Where C++ returns a document along with an error, `read` returns an error with a
  location. Whenever `read` returns `Ok`, the document holds all of the input's content.
- The temporary deviations (D2's subtypes, D4, D5, D6) each close in a named row. Closing
  one turns an `Unsupported` into a read or into a refusal with a code, never into
  silence.
- A tolerant mode (LIB-COMPAT-001) may take the lenient C++ behaviour for D1 and D2.
  Strict mode stays as decided here.

## Alternatives considered

- **Match C++ exactly**, with a wrapper matched by local name, the type overriding the
  name, and XMI and XSI attributes ignored. Rejected, because each of these either drops
  data or reads a kind the element name does not state.
- **Give the other ten package kinds SACM-XMI-001 until they can be read.** Rejected,
  because the code would then change meaning from release to release (ADR 0015).
