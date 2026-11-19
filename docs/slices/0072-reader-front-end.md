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
