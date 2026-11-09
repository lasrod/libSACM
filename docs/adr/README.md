# Architecture decision records

Lasting decisions about how libSACM is built, one per file, numbered in order. A
decision about how to *interpret the SACM standard* belongs in
[../sacm/decisions.md](../sacm/decisions.md) instead.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | accepted |
| [0002](0002-cargo-workspace-layout.md) | Cargo workspace: library, CLI, xtask | accepted |
| [0003](0003-sacm-2-3-sources-and-priority.md) | SACM 2.3 target, normative sources and their priority | accepted |
| [0004](0004-functionality-matrix-gate.md) | The functionality matrix is a gated TOML artifact | accepted |
| [0005](0005-xml-backend.md) | XML backend for XMI read and write | proposed (open) |
| [0006](0006-agent-roles-and-slice-loop.md) | Agent roles with enforced write scopes, and the slice loop | accepted |
| [0007](0007-msrv-and-edition.md) | Rust edition 2024, MSRV 1.85 | accepted |
| [0008](0008-markdown-parser-for-verdict-lines.md) | A CommonMark parser decides which verdict lines count | accepted |
| [0009](0009-nextest-for-mutation-testing.md) | cargo-mutants runs tests under nextest, stopping at the first failure | accepted |
| [0010](0010-document-model.md) | Document model: arena storage, ElementId, borrowing views | accepted |

Statuses: `proposed` → `accepted` (maintainer approval) → optionally `superseded by NNNN`.
Copy [template.md](template.md) for a new record.
