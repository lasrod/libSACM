# Verification 0073a — XMI metadata and same-file proxies

- Date: 2026-11-21
- Row verdict: SACM23-XMI-019: PASS
- Row verdict: SACM23-XMI-030: PASS
- Verifier: `sacm-verifier` (independent, read-only), three rounds.
  - **Round 1, 61cba0d:** FAIL for both rows.
  - **Round 2, 5628f15:** FAIL again, on test gaps in the fix round's code only.
  - **Round 3, 8daea1d:** PASS for both rows.

  The maintainer approved the extra fix round on 2026-11-21. CI run
  [44883292561](https://github.com/lasrod/libSACM/actions/runs/44883292561) is on round 3.
- Slice brief: [docs/slices/0073a-xmi-metadata.md](../slices/0073a-xmi-metadata.md)
- Decisions:
  - [ADR 0042](../adr/0042-xmi-metadata-and-same-file-proxies.md);
  - M6 decisions Q2 and Q3, and the maintainer's answer of 2026-11-20: `href` or `xmi:idref`
    on a definition is SACM-XMI-003;
  - the lead, 2026-11-21: depth is counted as resolved, and a root that keeps a link of its
    own is not moved (`SACM-XMI-003/proxy-of-linked-root`).

## Result

- **SACM23-XMI-019, XMI metadata.**
  - Other tools' XMI metadata is kept exactly and written back by both saves, with no read
    diagnostic: documentation, extensions, differences, `xmi:uuid`, `xmi:label` and the
    schema locations. It is never preserved content.
  - `SacmElement::uuid()` and `label()` read it. No edit sets it.
  - An XMI-namespace element that XMI 2.5.1 does not define where it stands is SACM-XMI-003.
  - Metadata ids share the document's id space. A strict save checks them as NCNames, and
    checks the XMI.xsd IDREF attributes of metadata.
  - A copy drops each `xmi:uuid` (XMI 2.5.1 §7.6.1), a delete removes the metadata and
    releases its ids and references, and a move keeps it.
  - `semantic_compare` reports `Difference::Metadata` and `Difference::DocumentMetadata`.
- **SACM23-XMI-030, same-file proxies.**
  - A proxy of a root package is resolved in both modes, with no diagnostic: the root moves
    into the proxy's place, and chains are followed. A write writes the nested form, which
    reads back equal.
  - A proxy is kept with SACM-XMI-003 when its id names no root, the kind does not fit, the
    root keeps an `href` or `xmi:idref` of its own, two proxies name one root, or the proxies
    form a cycle, checked in that order.
  - Depth is counted as resolved. A chain that would nest deeper than `max_depth` refuses the
    read (SACM-SEC-003), so the output always reads back and the LIB-SEC-003 write bound
    holds. A chain of 200,000 roots is refused in 7 s with no memory growth.

The reworded verified rows are true; each carries a slice 0073a note. Evidence beyond CI:
- the builder's 26 hand mutants, plus 33 and 7 in the two fix rounds;
- the verifier's 41 plus 43 plus 7 hand mutants, and about 105 behaviour probes;
- the partial CI mutation report of round 1 (the PR-diff shards timed out in each round).

## Findings

| Round | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | blocking | A proxy chain nested deeper than `max_depth`: the strict output did not read back, and the write bound was exceeded. | Depth is counted as resolved; deeper is `LimitExceeded`. |
| 1 | blocking | A moved root with a kept `href` or `xmi:idref` did not round-trip. | Such a root is not moved; new template `proxy-of-linked-root`. |
| 1 | blocking | 19 mutants of the proxy and metadata logic survived. | Tests added; one half of `release_metadata` followed in round 2. |
| 1 | blocking | Three rustdoc statements about preserved content and metadata were false. | Replaced with the verifier's text. |
| 1 | non-blocking | Acceptance wording, a wrong clause (§7.13 for §7.6.1), the `proxy-wrong-kind` message, the `WriteError::Invalid` Display for a document-level id, and an unlogged test change. | Fixed in the round. |
| 2 | blocking | Six mutants of the fix round's code survived: the depth walk, the order of the depth check, the link-attribute test, and the released references. | Four test additions; no source change. |
| 3 | non-blocking | The acceptance of the copy, delete and move test did not name the released-references case. | Added with the verifier's text. |

## Follow-ups

Logged in [docs/follow-ups.md](../follow-ups.md): the affected id of a spelled proxy of no
root (for slice 0073b), an `xmi:id` inside a Difference's `<target>`, prefixes re-declared on
each kept fragment, and the write bound under a raised depth.
