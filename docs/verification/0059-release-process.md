# Verification 0059 — The release process

- Date: 2026-11-17
- Row verdict: LIB-QA-005: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- Commit: b8a05f2. Evidence: CI run
  [44039414213](https://github.com/lasrod/libSACM/actions/runs/44039414213), green on all
  jobs. The PR-diff mutants job had nothing to mutate: no library code changed.
- The follow-up round after the PASS changed only wording, with the verifier's text, and the
  one test strengthening the verifier prescribed. The lead checked both.
- Slice brief: [docs/slices/0059-release-process.md](../slices/0059-release-process.md)

## Result

- **`docs/releasing.md`** gives the steps in a working order, each with its command and a
  check:
  1. bump the version;
  2. date the CHANGELOG;
  3. run the gate and the dry run;
  4. merge;
  5. tag;
  6. publish `sacm`, then `sacm-cli`;
  7. create the GitHub release.

  It also has a failure section. The maintainer's go rule is in bold at the top. The commands
  were checked against cargo 1.99.0 and gh 2.90.0.
- **Packages.** Both list only `Cargo.toml`, `README.md`, `LICENSE`, `src/**` and the files
  cargo adds. Both `crates/*/LICENSE` files are byte-equal to the root `LICENSE`. The packaged
  `sacm-cli` depends on `sacm` `0.1.0`.
- **The packaged `sacm`**, built outside the workspace, documents with `-D warnings` under
  docs.rs's configuration, and its README doctests pass.
- **The dry run.** `cargo publish --dry-run --workspace --locked` on the clean tree:
  - `sacm`: 52 files, 1.3 MiB;
  - `sacm-cli`: 8 files, 44.8 KiB;
  - each was verified by building it from its packaged sources, `sacm-cli` against `sacm` from
    cargo's temporary registry;
  - both ended with "aborting upload due to dry run".
- **The red snapshot.** `xtask/tests/release.rs` was unchanged from the red. The post-PASS
  round then strengthened one check.

Hand mutations:
- killed:
  - removing `include`, or LICENSE from it;
  - changing or deleting a LICENSE copy;
  - desynchronising the versions;
  - a crate-level `version`;
  - a bad first CHANGELOG section;
  - another licence;
  - adding `tests/**` or `build.rs` to `include`.
- equivalent: dropping `README.md` from `include`, because cargo adds the `readme` file by
  itself.
- killed after the follow-up round: a bad date or version in the release section under an
  empty `[Unreleased]`, which survived round 1.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| non-blocking | LIB-QA-005 | The CHANGELOG check looked only at the first section, which step 2 makes `[Unreleased]` (M5b, M5c). | The test also checks the latest release section, and acceptance item 4 says so. |
| non-blocking | LIB-QA-005 | The docs said "no tests", but the unit tests in `src/` ship and run only from the repository. | Reworded in `releasing.md` and the CHANGELOG. |
| non-blocking | LIB-QA-005 | `gh release create` could create a tag on the wrong commit. Step 5 could tag a later commit. The yank rule could be misread. Step 3 pointed releasers to `CLAUDE.md`. | Reworded with the verifier's text (`--verify-tag`, check `git log -1`, the yank rule, CONTRIBUTING.md#the-gate). |

## Logged

In [docs/follow-ups.md](../follow-ups.md):
- docs.rs builds with all features, so the unstable `test-support` API would show there;
- the packaged crate's tests cannot run outside the repository;
- `is_iso_date` accepts impossible days.
