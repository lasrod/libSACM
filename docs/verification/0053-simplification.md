# Verification 0053 — Simplification

- Date: 2026-11-16
- Row verdict: LIB-COMPAT-001: PASS
- Row verdict: LIB-COMPAT-003: PASS
- Row verdict: LIB-COMPAT-007: PASS
- Row verdict: LIB-API-011: PASS
- Row verdict: SACM23-VAL-006: PASS
- Row verdict: SACM23-XMI-008: PASS
- Row verdict: LIB-API-006: PASS
- Verifier: `sacm-verifier` (independent; read-only), three rounds. The maintainer allowed the third, for three named items.
- Commit: 2f96eef. Evidence: CI run
  [43846310194](https://github.com/lasrod/libSACM/actions/runs/43846310194), with all checks
  green. PR-diff mutants: 248, of which 200 were caught, 0 missed, 0 timed out and 48 unviable.
- Slice brief: [docs/slices/0053-simplification.md](../slices/0053-simplification.md)

## Result

**Reading.** A tolerant read opens any well-formed SACM file. It refuses only what it
cannot hold:
- a DOCTYPE or malformed XML;
- an invalid root, or no package;
- a duplicate id;
- an unsupported encoding or limit;
- another SACM or XMI namespace, or an XMI-namespace document element.

It keeps everything else as preserved content, with a Warning:
- SACM-ENUM-001 for a value that is no literal;
- SACM-MULT-001 for a repeat;
- SACM-XMI-005 for an untyped child of an abstract role;
- SACM-ID-002 for an empty id, which gets a generated id;
- SACM-XMI-014 for conforming content not read yet;
- SACM-XMI-007 for a cross-document `href`;
- SACM-XMI-003 for content that is not SACM 2.3 where it stands.

A strict read is unchanged.

**Document-level content.** A document holds preserved content outside its elements,
including the wrapper's `xmi:id`, which is in the id space.

**Writing.** A strict save checks schema validity only (ADR 0035 D4) and never runs
`validate`. A save with no root, and no unread root kept, is `NoRoot` in both modes.

Evidence beyond CI:
- a probe that re-read all 79 kept sites of the read-location table tolerantly, each `Ok` with a
  Warning at the strict location and a stable round trip, and the refused sets identical in both
  modes;
- about 20 hand mutations across the three rounds, all killed in the final state, including
  the arms of `holds_root` and the bindings of `Kept::rename`;
- the quotations of ADR 0035 checked against SACM 2.3, XMI 2.5.1 and XMI.xsd.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-COMPAT-003, SACM23-XMI-008 | Deleting every root beside document-level content let a tolerant write produce a file its own reader refuses. | `NoRoot` in both modes unless the document-level content holds an unread root. Tests cover `xmi:Documentation`, text and an attribute. |
| blocking | LIB-COMPAT-001, LIB-COMPAT-003 | The wrapper's `xmi:id` was neither checked nor in the id space: a repeat read without `DuplicateId`, a create could reuse it, and an empty one gave no diagnostic. | Checked by `checked_id`, held in the document's ids, ID-002 when empty. Strict is unchanged. |
| blocking | LIB-COMPAT-003, SACM23-VAL-006 | Surviving mutants in `IdUse::is_unused`, `has_preserved_content` and `Preserved::remove_unqualified`, and hand mutation H5. CI's run on e16280e missed the same ones. | A test for each. |
| blocking | LIB-COMPAT-003 | A `Kept::rename` mutant looped forever. CI counts a timeout as a failure (prescription D4). | The loop is bounded by a set of the taken numbers. |
| blocking | LIB-COMPAT-003 | Round 2: hand mutations in the fix round's own code survived: `rename` ignoring kept bindings, which would write a duplicate `xmlns:ns1`, and `holds_root`'s text arm. | `lib_compat_003_two_rebound_prefixes_on_one_holder_take_ns1_and_ns2` and the extended NoRoot test. |
| blocking | LIB-COMPAT-007 | The default-write-mode test was lost when the old tests were replaced. | Restored, with a dangling reference as the strict refusal. |
| blocking | several | Wording: XMI-008's stale Display, XMI-012's strict-save sentence, the crate root's and README's conformance-check text, `Cargo.toml`'s serde text, and the `has_preserved_content` rustdoc. | Rewritten. A conformance check reads strictly. |
| non-blocking | several | The XMI-namespace document element was missing from the both-modes refusal set, plus the XMI-006 fix text, ADR 0035's editor line, the LIB-API-009 example, VAL-011, a test citation, a `key` id case, and the triage rows. | All applied. Two `check_preserved_ids` mutants are excluded as equivalent, and the stale `set_preserved` exclusion is removed. |
| non-blocking | LIB-COMPAT-003 | Round 3: two sentences were broader than the code, in the `xmi` module doc and in a test's rustdoc. | Fixed directly. |
| non-blocking | process | The confirmed red was not committed, so the test changes from red to the first commit rest on the lead's record in the slice log. | From the next slice on, the red is committed or kept as a patch. |

## Logged for the maintainer

- `sacm validate` prints "valid" and exits 0 for a file a strict read refuses, such as
  `assertionDeclaration="Assumed"`, because its default read is tolerant. This belongs to
  LIB-CLI-002.
- A TaggedValue with `lang`, `content` and `value` gives two Warnings.
