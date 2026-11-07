# Verification records

One record per verified slice, named `NNNN-<short-name>.md` after its slice brief and
written by the lead from
[`.claude/skills/sacm-slice/references/verification-record.md`](../../.claude/skills/sacm-slice/references/verification-record.md)
when the independent `sacm-verifier` returns PASS. A matrix row may be `verified` only
if it cites a record here that names it (`cargo xtask matrix check` enforces this).

Records are evidence, not commentary: they are never edited after the fact. A later
finding against a verified row gets a new record.
