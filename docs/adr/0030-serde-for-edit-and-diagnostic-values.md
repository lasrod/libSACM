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

## Amendment 2 (accepted by the maintainer with ADR 0036, 2026-11-17; slice 0056)

`Operation::CopyElements` is not serialized. It holds a snapshot of a source document,
which has no JSON form, since this ADR rejects serializing `Document`:
- serializing it, or a `Mutation` that holds one, is a serde error;
- the tag `"CopyElements"` does not deserialize.

A JSON client builds a draft with create operations instead.

*Amendment 2 status: accepted (the maintainer, 2026-11-17, with ADR 0036).*

## Amendment 3 (2026-11-17): message templates and arguments; ReadError and WriteError
Issue #82. This amendment adds message templates and arguments, and serde for `ReadError` and `WriteError`.
- **Templates.** Every `Diagnostic`, `ReadError` and `WriteError` has `template()` and `arguments()`.
  - `template()` is a `&'static str` id `<CODE>/<form>` that names its English sentence form.
  - `arguments()` holds the values the sentence is made from: `Argument { name, value }`, with `ArgumentValue::Text(String)` or `Count(u64)`, in the order they first appear.
  - One crate-private renderer builds each message from them. docs/sacm/diagnostics.md lists every template with its arguments and English.
- **Stability (amendment 1's rule).**
  - Renaming or removing a template id or an argument name, or changing its kind or meaning, is Breaking.
  - Adding a template, or an argument to a template, is not.
  - The English wording is not promised.
- **Codes.**
  - `ReadError::code()` and `WriteError::code()` return `Code` (Breaking, 0.2.0).
  - `Unsupported` takes the code a tolerant read gives the same input: Interim SACM-XMI-014; CrossDocumentHref SACM-XMI-007; Defect SACM-XMI-003; OtherSacmNamespace SACM-XMI-002; OtherXmiNamespace SACM-XMI-015; Encoding SACM-XML-002; Capacity and Unheld SACM-SEC-002.
  - `NoRoot` is SACM-XMI-001, `UnrepresentableCharacter` SACM-XML-003 and `Invalid` SACM-XMI-016.
- **Types.** `ReadError`, `WriteError` and `WriteAttribute` implement `Serialize` and `Deserialize`; this replaces their exclusion in the Types list. `Argument`, `ArgumentValue` and `Details` serialize only inside them.
- **R8.** `arguments` is a JSON object with keys in the template's order: Text is a string, Count a non-negative integer.
- **R9.** A `Diagnostic` writes `template` and `arguments` after `message`. On input both may be left out together. The template is then the code's text and there are no arguments; this is the form of 0.1.0 JSON and of a `test-support` diagnostic.
- **R10.** A `ReadError` or `WriteError` is tagged with `type` (the variant name), then `code`, `template`, `arguments`, then its fields in declaration order (R2, R3).
  - `kind` is an `ElementKind` name, `character` a one-character string, `attribute` a `WriteAttribute` variant name, and `diagnostics` holds Diagnostic objects.
  - Every key is required.
- **R11.** On input:
  - `template` must be one of the variant's templates, or, for a Diagnostic, one of that code's diagnostic templates or the code's text;
  - `code` must be the template's;
  - `arguments` must hold exactly its names, in order, with their kinds;
  - anything else is refused (R4, R7).
- **Records.** A deserialized `ReadError` or `WriteError` is a record, as a `Diagnostic` is. Its `Display` renders its template and arguments; its other fields are kept as given. Its `reason`, `shape` and `element_name` are not checked against what the template and arguments render (the maintainer, 2026-11-18).
- **Order.** ADR 0011's diagnostic order gains two last keys: 8, the template id; 9, the arguments.

*Amendment 3 status: accepted (the maintainer, 2026-11-18). It implements the maintainer's decisions Q8 to Q11 of 2026-11-17 and its API details: the codes of the write errors, `Details`, the default template and the table.*

## Amendment 4 (2026-11-18): ExpressionLangString
SACM23-BASE-009; the maintainer's decision Q5 of 2026-11-17. It extends R6.
- A plain `LangString` is unchanged: `{"lang": …, "content": …}`.
- An ExpressionLangString (§8.4) is `{"type": "ExpressionLangString", "lang": …, "content": …, "expression": …}`, with keys in that order. `expression` is the ExpressionElement's id, or `null` when it has none. This is R1's tag rule for LangString's one subclass.
- On input: `type`, when given, must be `"ExpressionLangString"`. `expression` is required with it and refused without it. `lang` and `content` may be left out, as for a plain one (R3). Unknown keys are refused (R4).
- A `MultiLangString` value may take either form.
- This is additive: all 0.1 LangString JSON reads as before, and a plain LangString writes as before.

*Amendment 4 status: accepted (the maintainer, 2026-11-18). It implements the maintainer's decision Q5 (2026-11-17).*

## Amendment 5 (2026-11-18): arguments in any key order
LIB-API-019; the maintainer's decision at M4's close. It relaxes R11.
- On input, a record's `arguments` may give its template's arguments in any key order; they are matched by name and held in the template's order. A missing, extra, repeated or wrongly typed argument is still refused.
- On output, arguments are written in the template's order, as before.
- Reason: JSON that passed through `serde_json::Value` without serde_json's `preserve_order` feature has sorted keys, and an application may store records that way.
- This is additive: every record that read before reads the same.

*Amendment 5 status: accepted (the maintainer, 2026-11-18). Built by LIB-API-019; until then R11's order rule holds.*

## Amendment 6 (2026-11-26): prose arguments; a non-empty requirement id
LIB-DIAG-004 and LIB-DIAG-005; the maintainer's decisions on lasrod/libSACM#113.
- **Prose.** An argument the catalog marks `(prose)` is `ArgumentValue::Prose`. It is written as a JSON string (R8) and read as Prose by its template's kind, so the JSON shape does not change.
- **R11 gains a rule:** a Diagnostic record with an empty `requirement_id` is refused, after its keys and types and before its template.
- **Withdrawn:** the consequence "Once users can build their own diagnostic codes (LIB-DIAG-003) …". LIB-DIAG-003 is retired; only the crate builds diagnostics (LIB-DIAG-005).

*Amendment 6 status: accepted (the maintainer, 2026-11-26).*

## Amendment 7 (2026-11-27): specification, position, SetTaggedValue

LIB-API-013 and LIB-EDIT-023. All of it is additive under amendment 1.
- A `Diagnostic` writes `specification` after `clause`: `"SACM 2.3"`, `"XMI 2.5.1"` or
  `null`. It may be left out on input (R3); unknown text is refused (R5). `Specification`
  joins the Types list.
- `WriteError::UnrepresentableCharacter` writes `position` after `attribute`: the value's
  index, or `null`. It may be left out on input, the one optional key under R10. An
  unrepresentable character inside a value uses the template
  `SACM-XML-003/unrepresentable-character-in-value`, which names the index.
- `SetTaggedValue` is `{"type", "element", "key", "content"}`. `key` is a string, and every
  key is required.

*Amendment 7 status: accepted (the maintainer, 2026-11-27).*

## Amendment 8 (2026-11-27): Changes

LIB-RT-005. Additive under amendment 1. `compare::Changes` is `{"created_ids",
"changed_ids", "deleted_ids", "document_changed"}`; every key is required, and it is a
record. `Changes` joins the Types list.

*Amendment 8 status: accepted (the maintainer, 2026-11-27).*
