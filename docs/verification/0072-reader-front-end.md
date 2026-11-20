# Verification 0072 — The reader's front end

- Date: 2026-11-20
- Row verdict: LIB-SEC-003: PASS
- Row verdict: LIB-COMPAT-004: PASS
- Row verdict: LIB-COMPAT-016: PASS
- Row verdict: LIB-TOOL-022: PASS
- Verifier: `sacm-verifier` (independent, read-only), three rounds.
  - **Round 1, b68250b:** FAIL for LIB-SEC-003 and LIB-COMPAT-004.
  - **Round 2, b8ed305:** FAIL again, on new findings.
  - **Round 3, 39446da:** PASS for all four rows.

  The maintainer approved the extra fix round on 2026-11-20. CI run
  [44713484426](https://github.com/lasrod/libSACM/actions/runs/44713484426) is on round 3.
- Slice brief: [docs/slices/0072-reader-front-end.md](../slices/0072-reader-front-end.md)
- Decisions:
  - [ADR 0041](../adr/0041-read-limits-and-other-sacm-namespaces.md), with its amendment 1;
  - M6 decisions Q5, Q9 and Q10;
  - the maintainer, 2026-11-19 and -20: the two memory counts, the defaults lowered to 32 MiB
    and 1M nodes, and one budget for everything a read keeps.

## Result

- **LIB-SEC-003, read limits.**
  - `ReadOptions::with_limits(Limits)` sets four limits, which apply in both modes. The defaults
    are 32 MiB of input, depth 256, 1M nodes and 256 bindings in scope. A breach is
    `ReadError::LimitExceeded` (SACM-SEC-003), and the first breach ends the read.
  - **One budget.** Everything a read keeps besides the parsed tree counts against the size
    limit. That covers the namespace text preserved content keeps, counted as it is kept, and
    the text and affected ids of every diagnostic. So memory is bounded by the limits.
  - **Measured peaks:** 811 MiB for a 32 MiB package of Claims, and 1.25 GiB for 1M preserved
    nodes. Hostile inputs that took seconds to hours now fail in milliseconds.
- **LIB-COMPAT-004, other SACM revisions.**
  - In tolerant mode, an older revision's root reads as SACM 2.3 with SACM-XMI-008, and a
    SACM-like root with SACM-XMI-002. A strict read refuses both.
  - SACM 2.4 is refused in both modes. Any other root is `InvalidRoot` (SACM-XMI-001).
  - Only an Event's unqualified `occurence` is mapped.
  - Each namespace value is classified once per read, and the controls show it is linear.
- **LIB-COMPAT-016, ISO-8859-1.** Input declared ISO-8859-1 is read in both modes, in any
  letter case. Aliases are not read, and a UTF-8 byte order mark before the declaration
  contradicts it.
- **LIB-TOOL-022, XML Schema Part 2.** `cargo xtask fetch-spec` downloads XML Schema Part 2
  and checks it against its SHA-256 pin.

The reworded verified rows are true. Evidence beyond CI:
- the builder's 39 hand mutants, plus 7 and 3 in the two fix rounds;
- the verifier's 45 plus 11 plus 4 hand mutants;
- release timing probes.

## Findings

| Round | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | blocking | Diagnostics copied ids and namespaces without limit: a 169 KB input made 200 MB of text. | One budget (the maintainer; ADR 0041 amendment 1). |
| 1 | blocking | Kept namespace text was counted only after a preserved subtree was built. | It is counted as each element is kept. |
| 1 | blocking | No test covered the preserved-attribute count, the `occurence` mapping on any kind, or a qualified `occurence`. | Tests added. |
| 2 | blocking | The read went on after a breach (8.7 s for 2 MB). | The first breach ends the read (58 ms). |
| 2 | blocking | Namespace classification and family checks were O(N × URI length). | Memoized per value. Deadline controls and a deterministic unit test. |
| 2 | blocking | "Message and template are fixed text": false. | Reworded: messages repeat their arguments up to six times their size. |
| 2 | blocking | The acceptance said the opposite of the Golden 2 write test. | Golden 2 restored; the new cases moved to their own input. |
| 1 | non-blocking | Wording in the matrix, the CHANGELOG, both READMEs, the code docs, D-STRICT and the corpus. | Fixed with the verifier's text. |
