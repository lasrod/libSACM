# 0045. Public API review before 1.0

- Status: accepted (the maintainer, 2026-11-27)
- Date: 2026-11-27
- Matrix rows: LIB-API-005; LIB-API-022 to LIB-API-025.

## Context
LIB-API-005 reviews the public API against the Rust API Guidelines before 1.0, when a break
still costs only a minor version (0.5.0). Assurance Forge reported its use in
lasrod/libSACM#113.

## Decision
- **Fixed:** names take `impl Into<LangString>` (LIB-API-022); uniform traits and
  conversions (LIB-API-023); collection and result helpers (LIB-API-024); one spelling for
  the href form, `new_href` and `href()` (LIB-API-025).
- **Argument rule:** a lookup takes an id as `&impl AsRef<str>` and other text as `&str`.
  A constructor takes an id as `impl Into<ElementId>`, text as `impl Into<String>`, and a
  value as `impl Into<LangString>` or `impl Into<MultiLangString>`.
- **Return rule:** a list an element holds is a slice. A walk is `impl Iterator`, which
  promises only `Iterator` and the auto traits; more bounds may be added later. A computed
  result is a `Vec`.
- **Accepted deviations:**
  - D1. `edit::check` and `edit::preview` take `&Operation`, with no sealed trait (C-GENERIC);
    an operation struct is passed as `&op.into()`.
  - D2. One `ElementId` type for every `xmi:id`, a name's included (C-NEWTYPE): one id
    space (XMI 2.5.1 §7.6.1), and every lookup takes `AsRef<str>` anyway.
  - D3. `compare::Difference` variants are exhaustive, so callers can build expected values.
    A variant's fields stay as they are (`{id, left, right}` for most); a new comparison adds
    a variant, never a field.
  - D4. Module functions with a document receiver (`edit::apply`, `validation::validate`,
    `xmi::write`) (C-METHOD; ADR 0019 E1).
  - D5. One untyped `SacmElement` handle (ADR 0010).
  - D6. SACM primitive values are strings, kept exactly as written.
  - D7. `test-support` items are outside the stable API; semver checks run with
    `--features serde` only (LIB-QA-006).
- **Deferred, additive:** `EditError::template()` and `arguments()` (LIB-API-026, candidate).

## Consequences
After 1.0, changes to the public surface follow semver. Changing D1 or D3 would be a breaking
change.

## Alternatives considered
- A sealed `AsOperation` trait: `&x.into()` would stop compiling, and it adds a public concept
  for nothing `&op.into()` does not already give.
- An `XmiId` type: lookups take `AsRef<str>`, so it catches little, and one id space needs
  one type (`ReadError::DuplicateId`, SACM-ID-001).
- `#[non_exhaustive]` on each `Difference` variant: callers could no longer build expected
  values in tests.
