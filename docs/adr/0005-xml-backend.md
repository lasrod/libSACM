# 0005. XML backend for XMI read and write

- Status: accepted (the maintainer approved roxmltree and parser-based DOCTYPE detection on 2026-11-11)
- Date: 2026-11-07, decided 2026-11-11 (slice 0019)
- Matrix rows: LIB-SEC-001, LIB-SEC-002, LIB-API-009; inherited by SACM23-XMI-001, SACM23-XMI-007, SACM23-XMI-018

## Context

XMI import needs namespace-aware parsing, source positions for diagnostics, and
refusal of DTDs/entities. Export needs exact control over output for determinism. No
XML library type may appear in the public API.

## Options to evaluate

| Option | Read | Write | Notes |
|---|---|---|---|
| `roxmltree` + hand-written writer | DOM, namespaces, text positions; DTD rejected unless enabled | Small, fully deterministic writer of our own | Read-only crate; minimal dependencies |
| `quick-xml` for both | Streaming reader with namespace resolution | Writer with escaping | One dependency; positions are byte offsets |
| `xml-rs` | Streaming | Writer | Slower, less active |

## Decision

- **Reading:** `roxmltree`, pinned at `>= 0.21.2`, the first release that parses deeply
  nested elements without recursion. DTDs stay disabled, which is its default. Its only
  dependency is `memchr`. It is MIT OR Apache-2.0, has MSRV 1.60, and uses
  `#![forbid(unsafe_code)]`.
- **Writing:** a private, hand-written writer that uses only std (SACM23-XMI-007).
- **Private DOM:** the DOM stays inside `sacm::xmi`. No roxmltree type is public, and its
  errors become our own message plus a `SourceLocation` (ADR 0011). This follows the
  C++ library, which keeps the pugixml DOM out of public headers and splits reading and
  writing in its `io` layer.
- **DOCTYPE detection:** the parser does it. roxmltree stops at a real `<!DOCTYPE`
  before it reads the internal subset, so nothing can be declared, resolved or
  expanded. That gives SACM-SEC-001, with the location of the declaration. A
  `<!DOCTYPE` or `<!ENTITY` string inside a comment, CDATA section or attribute value is
  not a declaration and is not refused.
- **Rejected:**
  - `quick-xml`: 0.42 needs Rust 1.86, above our MSRV. It is a tokenizer rather than a
    well-formedness checker, gives byte offsets only, and breaks often between
    releases.
  - `xml-rs`: slower and less active.
- **Deviations from C++:**
  - **B1.** roxmltree enforces XML 1.0 and Namespaces well-formedness, where pugixml is
    lenient. A file with an undeclared prefix, an undeclared entity reference or a
    namespace-duplicate attribute is refused with SACM-XML-001, where C++ loads it. Such
    a file does not conform to the SACM XML Schema, so clause 2 holds.
  - **B2.** UTF-8 and ASCII are read now. UTF-16 is refused as `Unsupported`, never
    misread, until SACM23-XMI-018 decodes it with std, because XML 1.0 requires every
    processor to accept UTF-16. Other declared encodings stay refused, which XML
    allows. pugixml decodes UTF-16, UTF-32 and Latin-1.
  - **B3.** C++ scans the raw bytes for `<!DOCTYPE` and `<!ENTITY` before parsing
    (`xmi_reader.cpp:2354-2366`). That also refuses a schema-conforming file that only
    mentions those strings in a comment or CDATA, which goes against clause 2. Here the
    parser detects a real declaration. A stray `<!ENTITY` outside a DOCTYPE is not
    well-formed XML, so it gets SACM-XML-001, and nothing can expand it.

## Consequences

The backend stays private to the `xmi` module, so it can be replaced without a breaking
change. Configurable resource limits are LIB-SEC-003, and fuzzing is LIB-SEC-004.
