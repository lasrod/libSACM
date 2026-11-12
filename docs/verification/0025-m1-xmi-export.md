# Verification 0025 — XMI export

- Date: 2026-11-12
- Row verdict: SACM23-XMI-003: PASS
- Row verdict: SACM23-XMI-004: PASS
- Row verdict: SACM23-XMI-006: PASS
- Row verdict: SACM23-XMI-007: PASS
- Row verdict: SACM23-XMI-013: PASS
- Row verdict: LIB-API-010: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: a2be90c. Evidence:
  - CI runs 43143464291 (de4e130) and 43147248477 (a2be90c);
  - full mutants run 43143457413 (de4e130): 884 caught, 0 missed, 0 timed out. Library
    code is unchanged since de4e130.
- Slice brief: [docs/slices/0025-m1-xmi-export.md](../slices/0025-m1-xmi-export.md)

## Result

`xmi::write` writes the form ADR 0017 sets out, byte for byte against three golden files:
- the pinned namespaces, with `xmi:version` on the document element only;
- one `xmi:id` per element, escaped;
- `xmi:type` exactly where the class is not known from the role, and no `xsi`;
- one root bare, several in `xmi:XMI`, in document and model order.

It refuses, with nothing written:
- an empty document;
- an id outside XML `Char`, naming the first bad character;
- names and non-default `assertionDeclaration`s (LIB-API-010), in the documented order.

The writer has no panic paths. Every hand mutation and every cargo-mutants mutant in the
slice code is killed. One equivalent mutant (`WriteError::code -> None`) is excluded and
triaged.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | SACM23-XMI-006 | XMI 2.5.1 both requires and limits the type attribute; the reading taken was not stated | Specification defect D11; ADR 0017 T1 and the row source state the §7.6.3 reading (a2be90c) |
| Medium | SACM23-XMI-003 | The C++ `namespace_uri` save option was not named as a departure | ADR 0017 W6, deferred to LIB-COMPAT-001 (a2be90c) |
| Low | SACM23-XMI-004 | "The first bad character" was not pinned | Two-character test, both orders (a2be90c) |
| Low | LIB-API-010 | The name-before-declaration order was not pinned | Full-message assert; the order is in ADR 0017 (a2be90c) |
