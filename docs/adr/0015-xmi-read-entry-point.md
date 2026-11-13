# 0015. The XMI read entry point

- Status: accepted (the maintainer approved it on 2026-11-11)
- Date: 2026-11-11
- Matrix rows: LIB-SEC-001, LIB-SEC-002, LIB-API-009; inherited by SACM23-XMI-001, LIB-COMPAT-001

## Context

The crate's first XML input arrives with the security rows LIB-SEC-001 and
LIB-SEC-002. These rows refuse bad input, but reading SACM content belongs to the XMI
rows, starting with SACM23-XMI-001. D-PRESERVE allows only preserving or refusing, never
dropping. The C++ library offers `load_xmi_file` and `load_xmi_string` (`io/xmi.h`),
which return a `LoadResult` with an `ok` flag and an optional document
(`io/options.h:45-56`).

## Decision

- **The call:** `sacm::xmi::read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError>`.
  It takes bytes, so that invalid UTF-8 can be diagnosed with a location.
- **No file function.** The caller does the I/O: `xmi::read(&std::fs::read(path)?, &options)`.
  `read` stays pure and deterministic, and `ReadError` has no I/O variant.
- **`Err` or `Ok`:**
  - `Err(ReadError)` when no faithful `Document` exists.
  - `Ok(ReadOutput)` always holds a complete `Document` plus its diagnostics
    (SACM23-XMI-001 adds the accessors).
  - This adapts the C++ `LoadResult` to ADR 0011 X1: a failure is `Err(…)`, not
    `ok = false` plus diagnostics.
- **Refusals:**
  - `ReadError::Doctype`: SACM-SEC-001.
  - `ReadError::Malformed`: SACM-XML-001, for input that is not well-formed, truncated or
    not valid UTF-8.
  - `ReadError::Unsupported`: well-formed input this version cannot represent. It has
    no catalog code. It reports a limit of this version, not a defect in the input,
    and the set of such inputs shrinks from release to release, while codes are
    stable.
  - Each refusal carries a `SourceLocation` where one is known, and `ReadError::code()`
    gives the code.
- **Interim rule (LIB-API-009).** `read` never returns a document that is missing
  content from its input. Until a verified row reads some content, that content is
  refused as `Unsupported`, with its location. In the first release with this module,
  every well-formed input is refused that way, and SACM23-XMI-001 then reads the first
  roots.
- **Options:** `ReadOptions` is `#[non_exhaustive]` and passed by reference, so the load
  mode (LIB-COMPAT-001) and the limits (LIB-SEC-003) are added without changing call
  sites.
- **Deviations from C++:**
  - There is no file function.
  - SACM-SEC-001 carries a location; C++ gives none.
  - The DOCTYPE is detected by the parser (ADR 0005 B3).

## Consequences

- One way to read, which is easy to test with byte strings.
- Until SACM23-XMI-001, `read` refuses every well-formed file. The crate is unpublished,
  so this reaches no users.
- Callers that read files handle `io::Error` themselves.

## Alternatives considered

- **A crate-internal parser tested from inside `src/`:** the matrix gate refuses traced
  tests under `src/`.
- **Taking SACM23-XMI-001 into the same slice:** too large. Reading a root honestly needs
  names, ids, namespaces and children.
- **A file function as in C++:** it adds an I/O variant to `ReadError` and gives a second
  way to read.

## Amendment 1 (2026-11-13): what `Unsupported` carries, and how the interim rule closes

- **Two meanings, for now.** `ReadError::Unsupported` carries two kinds of refusal:
  - **limits of this version:** conforming input that no verified row reads yet, such as an
    element with no `xmi:id` (XMI 2.5.1 §7.6.1 makes it optional; SACM23-XMI-020);
  - **defects in the input**, which have no catalog code yet:
    - an empty `xmi:id`, which is not an `xsd:ID`;
    - a duplicate `xmi:id` (SACM23-VAL-001);
    - an `assertionDeclaration` value that is not a SACM literal (SACM23-VAL-006);
    - `assertionDeclaration` on an element that is not an Assertion;
    - a type attribute that does not conform to the role, names an abstract or unknown
      class, disagrees with another type attribute, or on a root names another class
      (ADR 0016 D2);
    - any other attribute, namespace-qualified attribute or text that the schema does not
      allow.

  Today only the literal refusal says, in the `ReadError` docs, that it is a defect in the
  input. A caller cannot tell the two kinds apart by type. The coded rows replace those
  refusals with diagnostics. The load and save modes ADR (LIB-COMPAT-001) gives the other
  defects their codes.
- **The interim rule has an end.** Every refusal of the first kind names the row that will
  read the content. LIB-API-011 keeps them in one list, checks that each named row is not
  yet verified, and labels each refusal's kind. Every compliance point depends on those
  rows, so no compliance point can be verified while schema-conforming input is still
  refused.

## Amendment 2 (2026-11-13): coded refusals for ids (slice 0036; SACM23-VAL-001, SACM23-VAL-020)

- **A defect gets its own variant.** When `read` refuses a defect in the input, the refusal
  gets its own `ReadError` variant, and `code()` returns its catalog code. `Unsupported`
  never gains a code; it keeps only the shrinking list of inputs this version cannot read
  yet.
- **`ReadError::DuplicateId { id, locations }`** (SACM-ID-001). Two or more elements carry the
  same `xmi:id`; XMI 2.5.1 §7.6.1 says the value "is unique across all elements within the
  file". A `Document` holds each id once, so the input is refused, never read with an
  element dropped or renamed (D-PRESERVE).
  - It concerns the first id that repeats in document order.
  - `locations` gives the start tag of every element carrying that id, in document order.
  - `location()` is the first repeat.
  - `ReadError::locations()` gives every location a refusal names: all of them for
    `DuplicateId`, and `[location]` or `[]` for the other variants.
- **`ReadError::InvalidId { id, location }`** (SACM-ID-002). The `xmi:id` is empty, which is
  not an NCName and which a `Document` cannot hold. Its location is the element's start tag.
- **Both refusals apply in both modes.**
- **Other ids that are not NCNames are read and held as written** (D-ID), for example `1c`,
  `a:b` and `a b`. `validation::validate` reports each one as SACM-ID-002 (SACM23-VAL-020),
  so a strict save refuses the document and a tolerant save writes the id back.
  - NCName follows XML 1.0 Fifth Edition and Namespaces in XML Third Edition, the editions
    XMI 2.5.1 cites, as ADR 0019 E6 does.
- **Ids with white space around them.** `xsd:ID` collapses white space, so `" c1 "` is a
  schema-valid id whose value is `c1`. Where trimming XML white space changes an id and the
  trimmed value is an NCName, the id is refused as `Unsupported` for now (the interim rule),
  until a row reads it by its collapsed value.

**Differences from C++, one line each:**
- C++ reads duplicate ids and reports SACM-ID-001 at load and again in `validate`
  (`xmi_reader.cpp:2287-2303`, `validate.cpp:312-318`); here the input is refused.
- C++ reads an empty id and `validate` reports it (`validate.cpp:303-309`); here it is a
  coded read refusal.
- C++ `validate` reports only empty ids; here every id that is not an NCName is reported.

*Amendment 2 status: accepted (the maintainer, 2026-11-13).*
