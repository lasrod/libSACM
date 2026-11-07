# 0005. XML backend for XMI read and write

- Status: proposed (open — decided by the first XMI slice)
- Date: 2026-11-07
- Matrix rows: SACM23-XMI-001, LIB-SEC-001, SACM23-XMI-007

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

To be made by the architect in the first XMI slice, using: DTD/entity refusal by
default, source positions, MSRV ≤ 1.85, licence, maintenance, and output determinism.
Current leaning: `roxmltree` for reading plus a private hand-written writer.

## Consequences

Whatever is chosen stays private to the `xmi` module, so it can be replaced without a
breaking change.
