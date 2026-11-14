# 0018. The validation entry point

- Status: accepted (the maintainer, 2026-11-12)
- Date: 2026-11-12
- Matrix rows: SACM23-VAL-018. Every later SACM23-VAL row adds a rule under it.

## Context

SACM23-VAL-018 is the first validation row, and many follow in M2–M5. A document read from
a file can break SACM's rules and must still load, so that it can be inspected and fixed
(D-VAL-LAYER). Validation reports what is wrong; it never changes or refuses a document.
The assurance-forge C++ library has `sacm::validation::validate(const Document&)`,
`validate_structure` (`validation/validate.h`) and `has_errors` (`validation/diagnostic.h`).

## Decision

- **`sacm::validation::validate(document: &Document) -> Vec<Diagnostic>`.**
  - It takes no options and returns no wrapper. The result is sorted by `Diagnostic`'s total
    order (ADR 0011), so it merges with `ReadOutput::diagnostics()` by `extend` and `sort`.
  - It is deterministic, linear, iterative and panic-free.
  - Later releases add rules. A document with no findings today may have findings later, but
    a code's meaning never changes.
- **Rules are private.** Each is a `fn(&Document, &mut Vec<Diagnostic>)`, listed in one
  table and grouped by family (`validation/names.rs`, …). A rule sees only the public views
  (`elements()`, `get()`, `kind()`, `name()`). Adding a VAL row adds one rule and one table
  line.
- **Diagnostics from `validate` have no location** in this release, because a `Document`
  records no source positions.
- **`diagnostic::has_errors(&[Diagnostic]) -> bool` is added,** as ADR 0011 X8 scheduled for
  the first row that produces diagnostics.
- **`Code::MULTIPLICITY_VIOLATION` is SACM-MULT-001.** Its catalog text widens to any
  multiplicity violation. SACM23-VAL-018 is its first use: a ModelElement with no name
  (§8.6 `name:LangString[1]`).
- **Strict save (LIB-COMPAT-007) will call `validate`** and refuse a document with errors.
  That is not designed here.

**Differences from C++, one line each:**
- **V1.** No public `validate_structure`. The Rust `Document` keeps its structural
  invariants by construction (`validate.h:14-16`, C++ decision #16).
- **V2.** Results are sorted, not in emission order (ADR 0011 X6).
- **V3.** `has_errors` lives in `diagnostic`, not in `validation` (ADR 0011 X7).
- **V4.** A missing name is a finding at all, because names are `Option` (ADR 0013). C++
  cannot represent a missing name (`element.h:177-179`).

## Consequences

- Rules can be added without any API change.
- Tools and a later GSN layer call `validate` and filter or merge its result. They cannot
  yet build their own `Diagnostic`s (ADR 0011 X9); revisit that when the GSN layer starts.
- Once strict save is the default (LIB-COMPAT-007), writing a nameless document will be
  refused rather than written. That row must call out the change.

## Alternatives considered

- `Document::validate`: two ways to do one thing, and it couples `model` to `validation`.
- `ValidateOptions` now: there is nothing to put in it, and callers can filter afterwards.
  If an option is needed later, it arrives additively as `validate_with`.
- A result wrapper: a `Vec` already merges and filters.
- A public `Rule` trait: a semver commitment with no user yet.
- A module named `validate`: `validate::validate` stutters, and C++ names the module
  `validation`.

## Amendment 1 (2026-11-14): rules for gid, descriptions and package homogeneity (slice 0041)

The rules follow this ADR's shape (no location, sorted findings). Differences from C++, one line each:
- **V7.** A `gid` held by several elements is one SACM-ID-003 finding per value, affecting every holder in document order. C++ emits one per extra holder, affecting {that holder, the first} (`validate.cpp`, gid uniqueness).
- **V8.** These rules name their own rows (SACM23-VAL-002, -011, -012). C++ names SACM23-BASE-001/002 and SACM23-ARG-002.

*Amendment 1 status: recorded by the lead under the decisions the maintainer made on 2026-11-14 (duplicate gid as an Error); it changes no policy.*
