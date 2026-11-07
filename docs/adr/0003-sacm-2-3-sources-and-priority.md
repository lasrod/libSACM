# 0003. SACM 2.3 target, normative sources and their priority

- Status: accepted
- Date: 2026-11-07
- Matrix rows: LIB-TOOL-002

## Context

The library implements OMG SACM 2.3. Its normative sources are OMG documents that this
repository should not redistribute. The specification text and its machine-readable
model disagree in known places, and agents left to their memory of "SACM" would mix
revisions.

## Decision

- Target **SACM 2.3**: formal/23-05-08 (PDF) and ptc/22-03-13 (machine-readable model).
- Do not commit them. `cargo xtask fetch-spec` downloads both into the git-ignored
  `third_party/sacm-2.3/` and verifies them against SHA-256 pins in
  `xtask/src/fetch.rs`. A changed upstream file is an error to investigate, not an
  update to absorb.
- Source priority for every decision: (1) formal/23-05-08, (2) ptc/22-03-13,
  (3) XMI/MOF rules it references, (4) OMG issues as clarification, (5) third-party
  tools as interoperability evidence only, (6) the assurance-forge C++ library as prior
  art.
- Text/model divergences are resolved as recorded in `docs/sacm/metamodel-inventory.md`
  and `docs/sacm/decisions.md` (D-TEXT-MODEL).

## Consequences

Every agent analyses the same bytes. Research reports quote clauses, and the verifier
checks the quotations against the pinned PDF.
