# Documentation map

**Read first:** the [README](../README.md), then [architecture.md](architecture.md) and
[loop.md](loop.md). After that, [matrix/README.md](matrix/README.md) explains the rules
every change follows.

| Where | What it holds |
|---|---|
| [architecture.md](architecture.md) | The planned shape of the library: modules, data flow, quality gates. |
| [loop.md](loop.md) | How work is done: slices, agent roles, verification, checkpoints. |
| [matrix/](matrix/) | The functionality matrix: every requirement as a row with a status. Edit the `.toml`, then run `cargo xtask matrix render`. The `.md` is generated. |
| [adr/](adr/) | Architecture decision records: decisions with lasting consequences, and why. |
| [sacm/](sacm/) | The standard as this project reads it: [decisions.md](sacm/decisions.md) (settled interpretations), [diagnostics.md](sacm/diagnostics.md) (stable diagnostic codes), [metamodel-inventory.md](sacm/metamodel-inventory.md), [compliance-points.md](sacm/compliance-points.md) and the [specification defects](sacm/reference/specification-defects.md). |
| [quality/](quality/) | [mutants-triage.md](quality/mutants-triage.md): the mutation-testing configuration and the disposition of every surviving mutant. |
| [slices/](slices/) | One brief per slice: scope, design decisions, log. |
| [verification/](verification/) | One record per verified slice: the verifier's findings and verdicts. Together with the slice briefs, this is the audit trail. |

The normative SACM 2.3 sources and XMI 2.5.1 are not in the repository.
`cargo xtask fetch-spec` downloads them into `third_party/sacm-2.3/`.
