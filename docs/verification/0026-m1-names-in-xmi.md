# Verification 0026 — Names and exact strings in XMI

- Date: 2026-11-12
- Row verdict: SACM23-XMI-011: PASS
- Row verdict: SACM23-XMI-012: PASS
- Row verdict: LIB-API-010: PASS
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: SACM23-XMI-002: PASS
- Row verdict: LIB-API-009: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: 32d0a95. Evidence:
  - CI runs 43152954871 (e291aff) and 43157339857 (32d0a95);
  - full mutants run 43157337263 (32d0a95): 899 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0026-m1-names-in-xmi.md](../slices/0026-m1-names-in-xmi.md)

## Result

Writing:
- `xmi::write` writes a ModelElement's name as its first child, `<name lang content />`.
- Absent and empty fields stay distinct, and a missing name gets no `name` child.
- Every value is escaped so that `lang` and `content` read back exactly.
- `WriteError::UnrepresentableCharacter` names the attribute that holds the character, as a
  typed `WriteAttribute`.

Reading:
- `xmi::read` reads one `name` child of a package root.
- The type is resolved in `name`'s own scope, and `xmlns=""` on `name` means no namespace.
- Everything else on or in `name` is `Unsupported`, never dropped.

LIB-API-010 now refuses only a non-default `assertionDeclaration`. XMI-001, XMI-002 and
LIB-API-009 were re-checked after their `<name/>` examples became `<note/>`. Every hand
mutation, including writer, reader and `WriteAttribute` mutants, is killed.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | SACM23-XMI-011 | `lang` exactness was untested; an unescaped, partly escaped or trimmed `lang` passed | `lang` round-trip and read cases (32d0a95) |
| Medium | SACM23-XMI-012 | The type on `name` resolved on the parent passed | A scope test on `name` (32d0a95) |
| Medium | SACM23-XMI-012 | Dropping the empty-namespace filter on `name` passed | An `xmlns=""` test (32d0a95) |
| Low | SACM23-XMI-012 | The reader's C++ differences had no ADR lines | ADR 0017 amendment 1 (32d0a95) |
| Low | SACM23-XMI-011/012 | The new public field was not marked as public API | Brief corrected; amendment 1 proposed (32d0a95) |
| Low | LIB-API-009 | The acceptance example and the test disagreed | `note` child case (32d0a95) |
