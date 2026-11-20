# ADR 0041 — Read limits and other SACM namespaces

- Status: accepted (the maintainer, 2026-11-19: M6 decisions Q5, Q9 and Q10, and the memory
  accounting in D2).
- Date: 2026-11-19
- Matrix rows: LIB-SEC-003, LIB-COMPAT-004, LIB-COMPAT-016.
- Amends: ADR 0005 B2, ADR 0016 D6 (closed here), ADR 0030 amendment 3 (labels) and ADR 0035 D1.

## Context

Until now a read had no limits. roxmltree's namespace table also grows with the number of
elements times the number of bindings. No SACM revision publishes a namespace (spec defect D7),
but real files use many different ones.

The maintainer decided M6 Q5 (which namespaces are SACM), Q9 (configurable limits) and Q10
(ISO-8859-1 without aliases) on 2026-11-19. The same day, they approved counting namespace
copies, so that the limits really bound memory.

## Decision

- **D1. Limits.** `ReadOptions::with_limits(Limits)` sets four limits, which apply in both
  modes. Their defaults are:
  - the input: 32 MiB, checked on the bytes first;
  - the nesting depth: 256;
  - the XML nodes: 1,000,000, checked by roxmltree's node limit;
  - the namespace bindings in scope at any element: 256.

  Depth and bindings are checked by a linear scan before parsing. A breach is
  `ReadError::LimitExceeded`, SACM-SEC-003, which names the limit, its value and the location
  where known. The parser's own capacities stay `Unsupported` with SACM-SEC-002.
- **D2. Accounting.** Two counts need no new setting:
  - The pre-parse scan counts, against the node limit, the bindings in scope at each element
    that declares one, because roxmltree copies them.
  - The read counts, against the size limit, the namespace text that preserved content keeps:
    each kept URI and each binding.

  Without these counts, a 32 MiB file within all four limits could still allocate many
  gigabytes.

  The maintainer first decided on 128 MiB and 8M nodes. On 2026-11-19 they lowered the
  defaults to 32 MiB and 1M nodes, after the builder measured about 10 GiB peak for 8M
  preserved nodes.
- **D3. No write limit.** `write` documents its output bound instead. The peak memory of a read
  at the defaults is measured and recorded.
- **D4. Namespace classes, per root:**
  - the pinned SACM 2.3 namespace: as today;
  - a recognised older revision (a closed table): read in tolerant mode with SACM-XMI-008,
    refused in strict mode;
  - SACM 2.4: refused in both modes (LIB-COMPAT-018);
  - a SACM-like namespace, meaning one whose URI contains `sacm` in any letter case, on one of
    the twelve package-kind names: read in tolerant mode with SACM-XMI-002, refused in strict
    mode;
  - anything else, or no namespace: `InvalidRoot`, SACM-XMI-001.

  These checks run in pass 1, after every root's name has been checked, so `InvalidRoot`
  keeps its precedence.
- **D5. Families.** A root's family is its SACM namespace for types: the base URI and the
  base/package URIs, or the exact URI. Every other namespace is foreign (SACM-XMI-003). Only
  `argumentationElement` and the 2.0–2.2 Event `occurence` are mapped. A write writes the
  pinned namespace.
- **D6. Labels.** The new refusal kind `Tolerated(row)` covers input that tolerant mode reads.
  Its label is `(strict mode never reads it; ROW reads it in tolerant mode)`. An xtask check
  requires its row to exist and not be deferred.
- **D7. ISO-8859-1.** The exact name, in any letter case and with no byte order mark, is
  decoded byte for byte. A UTF-8 byte order mark contradicts it. Aliases wait until a real file
  needs one.

## Consequences

- Real files in older namespaces open, with Warnings.
- Hostile input fails fast.
- An over-large trusted file needs explicit `Limits`.
- A root in no namespace now refuses with a different code. This is breaking, and is recorded
  in the CHANGELOG.
- The corpus outcomes change.

## Alternatives considered

- A write limit: rejected (Q9).
- Interning or sharing bindings instead of D2: this fixes memory, but not output.
- Declaring only the bindings a fragment uses: this changes verified output, which is
  LIB-COMPAT-013's ground.
- Accepting the ISO-8859-1 aliases: rejected (Q10).

## Amendment 1 (2026-11-20)

Verification 0072 found that the diagnostics a read emits copy ids and namespace text without
limit: a 169 KB input built 200 MB of Warning text. The maintainer decided on 2026-11-20 that
**everything a read keeps besides the parsed tree counts against the size limit, in one
budget.** That is D2's kept namespace text, counted as each element or attribute is kept
rather than after a subtree is built, and the text arguments and affected ids of every
diagnostic, counted where it is emitted. A breach is `LimitExceeded`, template
`SACM-SEC-003/kept-text`, or `kept-namespaces` when namespace text passes the limit. The
memory a read takes is then bounded by the limits.

Verification 0072 round 2 added two requirements, for the cost of a read as well as its memory. **The first breach of the budget ends the read**, so that no diagnostic is built after it. **Each namespace value is classified, and each family membership decided, once per read**, so that the classification stays linear in the input.
