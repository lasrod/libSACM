---
slice: 0072
name: reader-front-end
state: in-progress
step: verification
rows: [LIB-SEC-003, LIB-COMPAT-004, LIB-COMPAT-016, LIB-TOOL-022]
mode: pr
base: main (424248c, the 0.3.0 release)
---

# Slice 0072 — The reader's front end

## Scope

M6's first slice:
- **LIB-SEC-003:** configurable read limits (`Limits`, `ReadError::LimitExceeded`,
  SACM-SEC-003). Two internal counts make the limits bound memory too.
- **LIB-COMPAT-004:** older SACM revisions and SACM-like namespaces are read in tolerant mode,
  with SACM-XMI-008 or SACM-XMI-002. Any other root is SACM-XMI-001.
- **LIB-COMPAT-016:** ISO-8859-1 input is read in both modes.
- **LIB-TOOL-022:** XML Schema Part 2 is pinned in `cargo xtask fetch-spec`.

## Basis

- M6 decisions Q5, Q9 and Q10 (the maintainer, 2026-11-19), and the memory accounting the
  maintainer approved the same day.
- ADR 0041 (accepted).
- The design brief: `target/slice-0072-design.md` (git-ignored).

## Log

- 2026-11-19 design: the four rows are reworded and analyzed. LIB-COMPAT-016 is promoted from
  candidate. ADR 0041 is filed as accepted.
- 2026-11-19 tests: the builder wrote every designed test, and the lead confirmed the red (49 failing, for missing behaviour, the new goldens and counts, or the gate). All new and renamed tests are cited; 4 stale citations are dropped. Snapshot red-0072.tar.
  - The lead's rulings:
    - A7 keeps the builder's version (950 bytes reads, 949 is refused at the tenth child), which pins D2's formula.
    - A11 (the 8M-node default) and A13 (peak memory) are not traced tests, because LIB-TOOL-011 forbids ignored ones. A1 and A4 cover the default and the mechanism. The peak memory is measured once and recorded in the `read` docs. Both acceptance items are reworded, and the two tests are deleted in step 2.
    - `/namespaces` leaves NO_INPUT. The xtask template-count renames to 108 and 109 stand.
  - Left for the reworded verified rows: SACM23-XMI-018 (hst-lhs-007) and the LIB-SEC-006 tally (1 and 297).
- 2026-11-19 implementation: the builder implemented the design. Its 39 hand mutants are all killed, 7 of them by unit tests added after the first round.
  - The maintainer lowered the default limits to 32 MiB and 1M nodes, after the builder measured about 10 GiB peak for 8M preserved nodes. The matrix, ADR 0041 and the tests are updated.
  - The lead's rulings:
    - the LIB-TOOL-002 pin test allows W3C URLs;
    - LIB-COMPAT-016's column is (1,43), not (1,44);
    - LIB-COMPAT-003's round-trip claim is scoped: four corpus files with EMF's XMI namespace are renamed on write until LIB-COMPAT-002 reads that namespace, and nothing is lost.
  - The verified rows are reworded with the design's text and the builder's additions; each carries a slice 0072 note.
  - D-STRICT is updated, and ADR 0016 D6 is closed by ADR 0041.
- 2026-11-20 verify round 1 (b68250b): FAIL for LIB-SEC-003 and LIB-COMPAT-004; PASS for LIB-COMPAT-016 and LIB-TOOL-022.
  - B1 and B2: the diagnostics copy ids and namespace text without limit (169 KB input, 200 MB of Warning text). The maintainer decided on 2026-11-20 that everything a read keeps besides the parsed tree counts against the size limit; this is ADR 0041 amendment 1.
  - B3: the kept text was counted after a preserved subtree was built.
  - B4–B6: untested branches (preserved attributes; `occurence` only on an Event, and only unqualified).
  - The fix round:
    - the builder: the rule (new template `SACM-SEC-003/kept-text`), B3–B6 and the code-doc and README wording;
    - the lead: the ADR amendment, the matrix acceptance for the new tests, and the verifier's matrix and D-STRICT wording.
- 2026-11-20 fix round 1: the builder implemented the one budget (`Findings::push_at` counts every diagnostic, and the first breach becomes the refusal), B3–B6 and the wording. Its 7 named mutants are all killed. With the new counting, 1M preserved nodes still read, with a peak of 1.25 GiB. The lead accepted the existing-test changes the rule implies (`kept_namespace_text…` now totals 1,150; Golden 2 now holds preserved content, so a strict write refuses it), cited the 4 new tests, and updated the template counts (109 refusal, 218 in all).
- 2026-11-20 verify round 2 (b8ed305): every round-1 item is fixed, but LIB-SEC-003 and LIB-COMPAT-004 still FAIL on new findings.
  - N1: the read went on after a breach, a CPU denial of service.
  - N2: the namespace classification and family check were O(N × URI length).
  - N3 and N4: wording.

  LIB-COMPAT-016 and LIB-TOOL-022 PASS. Per the skill, the lead stopped and asked; the maintainer approved one more fix round (2026-11-20). The builder fixes N1–N3 and restores Golden 2, putting `a1` and `e2` in their own input. The lead added the acceptance for the three new controls and amended ADR 0041.
- 2026-11-20 fix round 2: the read stops at the first breach (`push_at` and every reporting helper return `Result`). Each namespace value is classified, and each family membership decided, once per read, memoized by the parser's interned `&str`. The N3 figures are corrected to "up to six" and "about seven", because Debug escaping can turn 1 byte into 6. Golden 2 is restored, with `a1` and `e2` in their own input. The type-check control uses 60,000 children (1.4 s in debug). The memo mutants are killed deterministically by `each_namespace_value_is_classified_and_compared_once`, and the stop mutant by the deadline control.
