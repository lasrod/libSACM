---
slice: 0079
name: proxy-of-typed-root
state: in-progress
step: verification
rows: [LIB-COMPAT-020]
mode: pr
base: main (4ad29a6)
---

# Slice 0079 — A same-file proxy does not move a root that keeps a root-only type

## Scope

- **LIB-COMPAT-020** (M6 close audit, finding 1). A same-file proxy can name a root that
  keeps `xmi:type` or `xsi:type`, which only a root can carry. Reading it moved the root
  into the role with its type, and the tolerant save read back as a different document
  (the package kept whole, or retyped). The proxy is now kept, with one new SACM-XMI-003
  template `proxy-of-typed-root`, and the root stays a root, as edits already do
  (`EditError::KeptRootType`).

Left out: the other M6 close findings (the M6-close slice next).

## Basis

XMI 2.5.1 §7.10.1, §9.5.2 rule 2g, §7.8.1; D-ROOT-NAME, D-RT, D-PRESERVE; LIB-COMPAT-019 (4)
(slice 0076 Q1, the maintainer, 2026-11-25).

## Design

- Design brief: `target/slice-0079-design.md` (git-ignored).
- Public API: one template, `SACM-XMI-003/proxy-of-typed-root` (`element`, `target`,
  `attribute`, `value`, `holder_class`, `holder_id`). ADR 0042 amendment 1 (accepted, lead
  ruling applying slice 0076 Q1).

## Log

- 2026-11-25 started from main 4ad29a6; LIB-COMPAT-020 filed `analyzed`; ADR 0042
  amendment 1, D-ROOT-NAME extended, re-check notes on SACM23-XMI-030, LIB-COMPAT-019,
  LIB-API-018. Builder step 1 started.
- 2026-11-25 step 1 done: four `same_file_proxies.rs` tests, the LIB-API-018 table case, the
  JSON golden, `DIAGNOSTIC_TEMPLATES` 110 -> 111 in both count tests (design-prescribed).
  Red confirmed by the lead at runtime (the proxy resolves and the root moves; no
  `proxy-of-typed-root`); the "as before" cases of test 2 and the linked-root case of
  test 3 pass as guards. Snapshot `target/red-0079-tests.tar`, row `tests-failing`.
- 2026-11-25 step 2 done (builder): built as designed; builder's gate green (3267 workspace,
  497 xtask, corpus not skipped); 21 hand mutants, 20 killed, `min_by_key` vs `find` in
  `Kept::kept_type` equivalent (root types are kept in input order). Row `implemented`.
