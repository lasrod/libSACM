# Verification 0033 — No xmi:version on export

- Date: 2026-11-13
- Row verdict: SACM23-XMI-024: PASS
- Row verdict: SACM23-XMI-003: PASS
- Row verdict: SACM23-XMI-007: PASS
- Row verdict: SACM23-XMI-013: PASS
- Verifier: `sacm-verifier` (independent; read-only), 1 round plus a re-check of the ADR fix
- Commit: 46e87c9 (docs); the code is unchanged since 86612c1. Evidence:
  - CI runs 43360770531 (86612c1; PR-diff mutants: 1 caught) and 43362180934 (688666c);
  - full mutants run 43360773989 (86612c1): 981 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0033-m1-no-xmi-version.md](../slices/0033-m1-no-xmi-version.md)

## Result

`xmi::write` writes no `xmi:version`, on a bare root or the `xmi:XMI` wrapper. The XMI
2.5.1 namespace identifies the version (§7.5.3, production 1a, §2.2.2). The reader is
unchanged: it ignores the attribute on the wrapper and on roots, and refuses it on a child.
The five golden files changed only by losing ` xmi:version="2.0"`.

XMI-003, -007 and -013 were reworded and re-checked. Seven hand mutations, each putting the
attribute back somewhere, are killed. Every quote was checked against the XMI 2.5.1 PDF.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | SACM23-XMI-024 | ADR 0017's departures from C++ did not list the `xmi:version` difference (CLAUDE.md rule 6) | Bullet added, naming `namespaces.h:32` and `xmi_writer.cpp:461, 542` (46e87c9) |
| Low | SACM23-XMI-024 | A CHANGELOG line ran past the width | Rewrapped |
| Low | SACM23-XMI-003 | Copilot: the reworded acceptance ("nothing else in the XMI namespace") excluded a bare root's `xmi:id`, which XMI-004 requires and the tests expect | Reworded to allow it; the tests (`assert_document_element_attributes`, `sacm23_xmi_003_one_root_…`) already pin exactly that |
| Logged | — | `compare.rs` labels a reader input that has `xmi:version` "the pinned form" | For the next slice that touches the file |
