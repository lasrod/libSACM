# Architecture decision records

Lasting decisions about how libSACM is built, one per file, numbered in order. A
decision about how to *interpret the SACM standard* belongs in
[../sacm/decisions.md](../sacm/decisions.md). Where an ADR makes such an interpretation,
decisions.md has a one-line entry pointing to it.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | accepted |
| [0002](0002-cargo-workspace-layout.md) | Cargo workspace: library, CLI, xtask | accepted |
| [0003](0003-sacm-2-3-sources-and-priority.md) | SACM 2.3 target, normative sources and their priority | accepted |
| [0004](0004-functionality-matrix-gate.md) | The functionality matrix is a gated TOML artifact | accepted |
| [0005](0005-xml-backend.md) | XML backend for XMI read and write | accepted |
| [0006](0006-agent-roles-and-slice-loop.md) | Agent roles with enforced write scopes, and the slice loop | accepted |
| [0007](0007-msrv-and-edition.md) | Rust edition 2024, MSRV 1.85 | accepted |
| [0008](0008-markdown-parser-for-verdict-lines.md) | A CommonMark parser decides which verdict lines count | accepted |
| [0009](0009-nextest-for-mutation-testing.md) | cargo-mutants runs tests under nextest, stopping at the first failure | accepted |
| [0010](0010-document-model.md) | Document model: arena storage, ElementId, borrowing views | accepted |
| [0011](0011-errors-and-diagnostics.md) | Errors and diagnostics | accepted |
| [0012](0012-lang-string-absent-and-empty.md) | LangString keeps absent and empty apart | accepted |
| [0013](0013-missing-name-stays-missing.md) | A missing name stays missing | accepted |
| [0014](0014-artifact-package-admits-every-artifact-element.md) | An ArtifactPackage admits every ArtifactElement | accepted |
| [0015](0015-xmi-read-entry-point.md) | The XMI read entry point | accepted |
| [0016](0016-interchange-roots.md) | Interchange roots: deviations from the C++ reader | accepted |
| [0017](0017-xmi-writer.md) | The XMI writer | accepted |
| [0018](0018-validation-entry-point.md) | The validation entry point | accepted |
| [0019](0019-edit-operations.md) | The edit API: operations as values | accepted |
| [0020](0020-semantic-comparison.md) | Semantic comparison | accepted |
| [0021](0021-xmi-reader-content.md) | The XMI reader: package content | accepted |
| [0022](0022-load-and-save-modes.md) | Load and save modes | accepted |
| [0023](0023-command-line.md) | The `sacm` command line | accepted |
| [0024](0024-ids-read-from-xmi.md) | Ids read from XMI | accepted |
| [0025](0025-utility-elements-and-gid.md) | UtilityElements, MultiLangString roles and gid | accepted |
| [0026](0026-references-between-elements.md) | References between elements | accepted |

Statuses: `proposed` → `accepted` (maintainer approval) → optionally `superseded by NNNN`.
Copy [template.md](template.md) for a new record.
