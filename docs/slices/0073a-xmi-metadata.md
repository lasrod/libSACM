---
slice: 0073a
name: xmi-metadata
state: in-progress
step: verification
rows: [SACM23-XMI-019, SACM23-XMI-030]
mode: pr
base: main (271ed77)
---

# Slice 0073a — XMI metadata and same-file proxies

## Scope

The first half of the M6 "metadata and links" slice. The design brief split it in two.

- **SACM23-XMI-019.** Other tools' XMI metadata (documentation, extensions, differences,
  `xmi:uuid`, `xmi:label`, schema locations) is kept exactly and written back by both saves.
  `SacmElement::uuid()` and `label()` read it. XMI-namespace elements that XMI does not define
  are SACM-XMI-003.
- **SACM23-XMI-030 (new).** A same-file proxy of a root package is resolved: the root moves
  into the proxy's place. Chains are followed. A cycle, a non-root, a wrong kind or a shared
  target is kept with SACM-XMI-003.

The other half, slice 0073b, holds SACM23-XMI-029 and LIB-EDIT-020.

## Basis

- The maintainer's M6 decisions Q2 and Q3 (2026-11-19), and their answer on 0073's open
  question 2 (2026-11-20): `href` or `xmi:idref` on a definition is SACM-XMI-003.
- ADR 0042 (accepted).
- The design brief, `target/slice-0073-design.md` (git-ignored), items tagged [A].

## Log

- 2026-11-20 design: XMI-019 is reworded and analyzed. XMI-030 is new, in M6, and analyzed.
  ADR 0042 is filed as accepted.
- 2026-11-20 tests: the builder wrote every [A] test (XMI-019: 13, XMI-030: 9) and the fixtures. The lead confirmed the red: 71 failing for missing behaviour or citations, and the gated build fails only on the missing API. The tests are cited, the deleted `lib_api_011_xmi_metadata_…` is dropped, and the XMI-030 cycle acceptance is cleaned. Snapshot red-0073a.tar.
  - The lead's rulings on the builder's discrepancies:
    - accept the real LIB-API-017 sample (`unread-attribute` at the `href`) and `<xmi:bar/>` for acceptance 9;
    - `read-errors.json` keeps one value per variant and one `Unsupported` per code (LIB-API-017 acceptance 6), so no duplicate samples;
    - counts from the real tree: 104 refusal, 108 diagnostic, 212 in all;
    - the stale test names are renamed in step 2;
    - the six new fixtures' `messages-0.1.0.txt` lines are accepted.
- 2026-11-20 implementation: the builder implemented the [A] items. Its 26 hand mutants are all killed.
  - The lead's rulings:
    - the third cycle case follows the stated check order (`b`, named twice, gives `proxy-shared`; the roots are `[a, b]`), so the test and its acceptance are corrected;
    - the changed expectations for the group, reference-element, location and UTF-16 tests are accepted, following ADR 0042 D4 (production 2c, rule 4e);
    - the 16 renamed tests are cited.
  - The design's [A] list of verified rows is reworded, and each affected row carries a slice 0073a note. D-PRESERVE and D-STRICT are updated, and ADRs 0033 and 0035 are marked amended by ADR 0042.
