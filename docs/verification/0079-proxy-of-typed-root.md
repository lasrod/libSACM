# Verification 0079 — A same-file proxy does not move a root that keeps a root-only type

- Date: 2026-11-25
- Row verdict: LIB-COMPAT-020: PASS
- Verifier: `sacm-verifier` (independent, read-only), one round: PASS at 60c06af; its
  wording was then applied with its text.
- Evidence: CI run 45617809204 (all green; PR-diff mutants 11 caught, 0 missed, 2
  unviable); 16 hand mutants; about 45 probe cases in both modes, each reading back `==`
  and re-writing byte-identical; the red snapshot of step 1.
- Slice brief: [docs/slices/0079-proxy-of-typed-root.md](../slices/0079-proxy-of-typed-root.md)
- Decisions: [ADR 0042](../adr/0042-xmi-metadata-and-same-file-proxies.md) amendment 1 (lead
  ruling applying the maintainer's slice 0076 Q1 to proxies); ADR 0033 amendment 2;
  D-ROOT-NAME, D-RT, D-PRESERVE.

## Result

- **LIB-COMPAT-020.** A same-file proxy that names a root keeping `type` in XMI 2.5.1's
  namespace or `xsi:type` is kept in its container with `SACM-XMI-003/proxy-of-typed-root`
  (after `proxy-of-linked-root`, before `proxy-shared` and `proxy-cycle`), and the root stays
  a root, as edits already refuse with `EditError::KeptRootType`. In tolerant mode a root
  typed with a declared subclass of its own class, and a root keeping only an EMF `type`,
  are moved as before. Every tolerant write probed reads back `==`; no root that cannot
  survive being nested is still moved, and nothing is dropped. Found by the M6 close audit
  (a moved root read back kept whole, or retyped).
- **Tests.** Four `same_file_proxies.rs` tests, the LIB-API-018 table case and a JSON
  golden. One mutant is equivalent: `min_by_key` against first-in-order in
  `Kept::kept_type`, since a root's root-only types are kept in input order.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Non-blocking | LIB-COMPAT-020 | `roots.rs` docs: an over-long line, and "rule 2g" cited without its clause. | The verifier's text. |
| Non-blocking | LIB-COMPAT-020 | `diagnostics.md`, `compliance-points.md` and the CHANGELOG said `xmi:type` where the rule is XMI 2.5.1's `type`. | The verifier's text. |
| Non-blocking | LIB-COMPAT-020 | ADR 0042 amendment 1 stated the subclass exception without its mode. | The verifier's text. |
| Non-blocking | LIB-COMPAT-020 | "At its input depth" has no direct assertion; it follows from the root not moving, and the verifier probed it. | Not added (no code is depth-specific). |
