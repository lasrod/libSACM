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
