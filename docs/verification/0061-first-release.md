# Verification 0061 — First release published

- Date: 2026-11-17
- Row verdict: LIB-QA-019: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- The maintainer's go: given in the session on 2026-11-17. The verifier could not check
  this; it rests on the lead's statement.
- Release commit: 30c7511, the merge of PR #80 ("chore: release 0.1.0"). CI on it is green.

## Result

- **The order of `docs/releasing.md` was kept:** the tag `v0.1.0` (10:57:22Z), `sacm` 0.1.0
  published (10:58:34Z), then `sacm-cli` 0.1.0 (10:58:54Z), then the GitHub release
  (10:59:04Z). All are on the day the CHANGELOG section is dated, so no re-dating was needed.
- **The tag** `v0.1.0` is annotated ("sacm 0.1.0") and points at 30c7511 on `main`.
- **crates.io.** Both crates are at 0.1.0 and not yanked. The published `sacm-cli` depends on
  `sacm ^0.1.0` with no extra features.
- **The GitHub release** `v0.1.0` ("sacm 0.1.0") is not a draft. Its body is identical to the
  CHANGELOG's `## [0.1.0] - 2026-11-17` section.
- **docs.rs** built `sacm` 0.1.0 with the `serde` feature only. The `test-support` items
  (`Document::insert`, `InsertError` and the `Diagnostic` builders) are not shown.
- **The published packages match the tag.**
  - Both record sha1 30c7511, not dirty.
  - The download checksums match.
  - 54 files are byte-equal to `git show v0.1.0:<path>`.
  - The packages hold only the manifest, the lock file, README, LICENSE and `src/`.

## Findings

None.

## Logged

The `Document` page on docs.rs mentions the `test-support` feature, whose items it does not
show. Reword it for the next release (in docs/follow-ups.md).
