---
slice: 0027
name: m1-validate-missing-name
state: done
step: verified
rows: [SACM23-VAL-018]
mode: pr
base: main (0d321d8)
---

# Slice 0027 — Validation: a missing name

## Scope

This slice builds the crate's first validation API. Given a `Document`, it returns sorted
diagnostics (ADR 0011). Its first rule: a ModelElement without a name is a SACM-MULT-001
error that names the element and cites §8.6 `name:LangString[1]`, and the element is kept.
A name with an absent or empty `lang` or `content` is not a missing name. SACM-MULT-001's
catalog text widens to cover required containments. SACM23-VAL-001 (duplicate ids) waits:
a `Document` cannot hold a duplicate id today.

## Basis

formal/23-05-08 §8.6; D-SEV; ADR 0011; ADR 0013.

## Log

- 2026-11-12 choose: SACM23-VAL-018, the first ready row. VAL-001 cannot trigger yet,
  because `insert` and `read` refuse duplicate ids. Base: `main` (0d321d8).
- 2026-11-12: ADR 0017 amendment 1 is accepted. The maintainer merged PR #37.
- 2026-11-12 design (architect): accepted, and recorded as ADR 0018 (proposed: new public
  API, which waits for the maintainer). It adds:
  - `sacm::validation::validate(&Document) -> Vec<Diagnostic>`, with no options;
  - private rules, each one table entry;
  - `diagnostic::has_errors`, in this slice per ADR 0011 X8;
  - `Code::MULTIPLICITY_VIOLATION`.
  
  The architecture doc and the craft skill say `validate`; they change to `validation`.
- 2026-11-12 research. Maintainer decisions:
  - D-SEV gains "a multiplicity on which text and model agree is an error";
  - ADR 0018 is accepted.

  VAL-018 is `analyzed`, with 12 acceptance items. The message quotes the id as write errors do. The SACM-MULT-001 catalog text is widened now.
- 2026-11-12 implementation: `validation::validate` with the private `RULES` table and `names::missing_name`; `has_errors`; `Code::MULTIPLICITY_VIOLATION`. The architecture doc, the craft skill and the READMEs now say `validation`. The local gate passes.
- 2026-11-12 verification round 1: FAIL.
  - The id quoting `{:?}` was unstated and untested (H10 survives); a new acceptance item pins it.
  - The crate README claims no diagnostic is reported.
  - The docs review (2 blocking: the crate README, and a front-page example without validate) and its should-fix items go to the implementer.
  - Logged, not required: a `ReadOutput::into_parts`, and a single cross-constant catalog test.
- 2026-11-12 verification round 2: PASS at f068309; SACM23-VAL-018 is verified.
