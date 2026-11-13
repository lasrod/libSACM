# 0011. Errors and diagnostics

- Status: accepted (the maintainer approved it, including X1–X9 and the location-first order, on 2026-11-10)
- Date: 2026-11-10
- Matrix rows: LIB-API-004, LIB-DIAG-001

## Context

Every later row reports either a failure (it could not proceed) or a finding about the
content. The rust-library-craft skill separates the two: failures are `Result<_, Error>`,
findings are `Diagnostic` values. The assurance-forge C++ library is the design reference:
- `validation/diagnostic.h`: `Severity`, `SourceLocation` and `Diagnostic`;
- `validation/codes.h`: the codes, as string constants;
- `io/options.h` and `commands/mutation.h`: results with an `ok` flag plus diagnostics.

## Decision

- **Module.** `sacm::diagnostic` holds `Severity`, `Code`, `SourceLocation` and
  `Diagnostic`. It is not re-exported at the crate root.
- **`Severity`:** `Info < Warning < Error`, with `as_str` and `Display` giving `info`,
  `warning` and `error`.
- **`Code`:** a newtype over `&'static str`. Each row that emits a code adds its constant,
  named after the C++ one (`kRefDangling` → `REF_DANGLING`); a test checks that every
  constant is in `docs/sacm/diagnostics.md`.
- **`SourceLocation`:** a 1-based line and column, where the column counts Unicode scalar
  values. `Display` is `line L, column C`.
- **`Diagnostic`:** private fields with accessors: `code`, `severity`, `requirement_id`
  (the matrix row), `clause`, `affected` (element ids, in the emitter's order), `location`
  and `message`.
  - **Display:** one line:
    `{severity} {code} [{requirement_id}[, clause {clause}]][ at {location}]: {message}`.
  - **Construction:** only the crate builds diagnostics. The constructors are public only
    under the `test-support` feature.
- **Order.** `Diagnostic` has a total `Ord`, and every function that returns diagnostics
  returns them sorted. The key is:
  1. location, with located diagnostics first, by line and then column;
  2. code;
  3. affected ids;
  4. requirement id;
  5. clause;
  6. severity;
  7. message.

  `cmp` is `Equal` exactly when the diagnostics are equal.
- **Errors.** There is one error enum per operation, in that operation's module:
  `xmi::ReadError`, `xmi::WriteError` and `edit::EditError`. Each arrives with its row, and
  there is no crate-wide `Error`. Every error enum:
  - is `#[non_exhaustive]`, implements `std::error::Error`, and is `Send + Sync + 'static`;
  - has messages that are lowercase, one line, with no trailing punctuation;
  - returns a wrapped cause from `source()` and does not repeat the cause's text in its
    own message;
  - wraps only std types, never a dependency's: an XML parser error becomes a message plus a
    `SourceLocation`;
  - makes the `Code` reachable when it reports a refusal that C++ gives a catalog code
    (XML-001, SEC-001, XMI-006, CMD-001 to CMD-007).

  The existing `model::InsertError` already conforms.
- **Deviations from C++:**
  - **X1.** A failure is `Err(…)`, not `ok = false` plus diagnostics
    (`options.h:45-62`, `mutation.h:37-48`).
  - **X2.** An I/O failure is an error variant whose `source()` is the `io::Error`, with no
    code. C++ files it under SACM-XML-001.
  - **X3.** `SourceLocation` has no `file`, because the caller knows what it loaded.
  - **X4.** No `Diagnostic::operation`. Closed by ADR 0019: an edit refusal is a typed
    `EditError` with a code, not a `Diagnostic`. A field is added only if a row reports edit
    diagnostics.
  - **X5.** `clause` is added, because D-DIAG requires it.
  - **X6.** A canonical order replaces emission order.
  - **X7.** The module is `diagnostic`, not `validation`, because the reader and edits
    report diagnostics too.
  - **X8.** `has_errors` came with the first row that produces diagnostics (ADR 0018).
  - **X9.** `Code` is a newtype, and users cannot build a `Diagnostic` yet (LIB-DIAG-003).

## Consequences

- **What exists now:** LIB-API-004 delivers the convention, `InsertError` held to it, and
  a no-panic test of today's public surface. The real I/O, XML and edit errors come with
  their rows, and each of those rows' acceptance includes "conforms to ADR 0011".
- **The XMI reader** must convert byte offsets into scalar-value columns.
- **Later additions are additive:** user-built diagnostics, `operation` and serde.

## Alternatives considered

- **A crate-wide `sacm::Error`.** Rejected: a caller cannot tell which variants an
  operation can return.
- **`Code` as an enum.** Rejected: an enum with no variants cannot be built, and it would
  fix codes before the rows that emit them exist.
- **Emission order, as in C++.** Rejected: it is a separate contract for each producer and
  cannot be tested until a producer exists. Merged lists would also have no common order.

## Amendment 1 (2026-11-10): one line, enforced

A message may contain line breaks. `Display` escapes them so that the output stays one
line: `\n` and `\r` become their escaped forms, and U+0085, U+2028 and U+2029 become
`\u{85}`, `\u{2028}` and `\u{2029}`. `message()` returns the text unchanged. This was
raised by review on PR #21, and it makes the one-line rule above hold for every message.

## Amendment 2 (2026-11-10, accepted by the maintainer): conversion errors

- **The type:** when a model value fails to convert from text (`FromStr` or `TryFrom`),
  the error is a small struct named `Parse<Type>Error` that lives beside its type. This
  follows std's `ParseIntError`.
- **Its shape:** it is `#[non_exhaustive]` with private fields, and `input()` returns the
  rejected text unchanged.
- **The other rules:** it follows every other rule of this ADR. It is
  `Error + Send + Sync + 'static`, its message is one lowercase line with the input
  quoted and escaped, and `source()` is `None`.
- **Operations:** "one error enum per operation" still applies to operations (read,
  write, edit).
- **First instance:** `model::ParseAssertionDeclarationError` (SACM23-ARG-003).
