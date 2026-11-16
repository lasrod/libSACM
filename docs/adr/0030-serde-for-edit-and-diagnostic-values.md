# 0030. serde for edit and diagnostic values

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: LIB-API-006. It extends ADR 0011, ADR 0019 and ADR 0029.

## Context

Assurance Forge runs libSACM in a Rust core with a web-view UI, and also in the browser.
Every edit crosses a process boundary as data, travels between users, and is kept as an
audit log (GitHub issue #57). Without serde, an application mirrors every operation struct
and keeps the mirror in step with each release. XMI stays the only serialization of the
model, so `Document` is not serialized.

## Decision

- **Feature.** An optional `serde` feature, off by default: `serde = ["dep:serde"]`, with
  `serde` 1.x and `derive`. The trait impls tie the crate to serde 1.
- **Types.** These implement `Serialize` and `Deserialize`:
  - `Operation` and every operation struct;
  - `Mutation` and `EditError`;
  - `ElementId`, `ElementKind`, `LangString`, `MultiLangString`, `AssertionDeclaration`,
    `Flag` and `ReferenceEnd`;
  - `Diagnostic`, `Code`, `Severity` and `SourceLocation`.

  `Document`, `SacmElement`, `Difference`, `ReadError`, `WriteError` and `InsertError` do
  not.
- **The JSON shape is a stability promise,** like `operation_name()` (the maintainer,
  2026-11-15):
  - Renaming or removing a key, or changing its meaning, is a breaking change.
  - Adding a key with a default that keeps the old meaning, or adding a variant, is not.
  - JSON written by a newer release may be refused by an older one.
- **Shape rules:**
  - **R1. Tags.** An enum with data is internally tagged with `"type"`. For `Operation`, the
    tag is exactly `operation_name()`.
  - **R2. Keys.** Keys are the snake_case accessor names. `Mutation` has `operation`,
    `created_ids`, `changed_ids` and `deleted_ids`.
  - **R3. Writing and reading.** Every key is written: `null` for none, `[]` for empty.
    - On input, a key may be omitted only where a builder or a documented default sets it:
      for example an operation's `id`, a LangString's `lang` or `content`, and a
      Diagnostic's `clause`, `affected` or `location`.
    - A package create's `parent: null` means a root. Omitting `parent` is refused.
  - **R4. Unknown keys are refused.** A misspelt key must not become a different edit.
  - **R5. Names and codes are strings.**
    - Names: `ElementKind` `"Claim"`, `Flag` `"isCounter"`, `ReferenceEnd` `"metaClaim"`,
      `AssertionDeclaration` `"needsSupport"`, `Severity` `"error"`.
    - Codes: `Code` `"SACM-CMD-001"`.
    - Unknown text is refused.
  - **R6. Values.**
    - `ElementId` is a string. It is not checked here; `apply` checks caller ids.
    - `LangString` is `{"lang": …, "content": …}`. `{}` is `LangString::default()`, which
      differs from `{"content": ""}` (ADR 0012).
    - `MultiLangString` is an array of LangString.
    - `SourceLocation` is `{"line", "column"}`, and 0 is refused.
  - **R7. Errors carry their code.** `EditError` also writes `"code"` after `"type"`, so
    readers outside Rust get the catalog code. On input it must match the variant.
- **Deserializing proves nothing about origin.** A deserialized `Operation` is checked by
  `check` and `apply` exactly as a constructed one: no operation constructor checks
  anything `apply` does not. A deserialized `Mutation`, `EditError` or `Diagnostic` is a
  record: `has_errors` reads a deserialized diagnostic's severity as given, and nothing
  else in the API takes one as input.
- **The CLI's JSON stays as it is** (ADR 0023). It is a separate versioned contract that
  names the requirement `requirement`, where serde writes `requirement_id`.

Example: `{"type":"CreateClaim","parent":"arguments","id":null,"name":{"lang":"en","content":"The system is safe"}}`.

**Adopted from C++:** the tag value is the operation name (`operations.h:406`).

**Differences from C++:**
- **S1.** C++ serializes no operations, mutations or diagnostics in the library.
  `MutationResult.operation` is a name string, and `ChangeRecord.before` and `after` are
  opaque strings (`mutation.h:18-48`). Here typed values serialize in full.
- **S2.** A serialized `Diagnostic` has no `operation` (ADR 0011 X4).

## Consequences

- `Code` gets a crate-private table of every catalog code, defined by the same macro as the
  constants, so reading a code back cannot drift from the catalog.
- `EditError`'s `&'static str` fields read back through a private mirror type that maps
  each name to its static string.
- `Diagnostic`'s private `requirement_id` and `clause` fields become `Cow<'static, str>`;
  the accessors are unchanged.
- The promise covers JSON and other self-describing formats. Internal tags do not work with
  bincode or postcard.
- Once users can build their own diagnostic codes (LIB-DIAG-003), the unknown-code rule can
  widen. That change is additive.

## Alternatives considered

- **Externally tagged enums** (`{"CreateClaim": {...}}`). These are harder to read and to
  match on in TypeScript, and they gain nothing for a self-describing format.
- **Ignoring unknown keys.** A misspelt key would silently fall back to its default.
- **Serializing `Document`.** That would freeze a second serialization of the model beside
  XMI.

## Amendment 1 (2026-11-16): the JSON shape follows semver until 1.0

This replaces the stability promise.
- **What is breaking:** a change to a key, a tag or a meaning. It ships only in a release that
  bumps the minor version (0.x) and is listed under Breaking. JSON written by one such
  release need not read in another.
- **What is not breaking:** adding a variant or a defaulted key.
- **Deserialize:** it stays for every listed type, `EditError` and `Diagnostic` included.

*Amendment 1 status: accepted (the maintainer, 2026-11-16).*
