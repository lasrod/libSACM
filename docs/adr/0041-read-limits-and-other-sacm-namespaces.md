# ADR 0041 — Read limits and other SACM namespaces

- Status: accepted (the maintainer, 2026-11-19: M6 decisions Q5, Q9 and Q10, and the memory
  accounting in D2).
- Date: 2026-11-19
- Matrix rows: LIB-SEC-003, LIB-COMPAT-004, LIB-COMPAT-016; LIB-SEC-007 (amendment 2).
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

## Amendment 2 (2026-11-25): an attribute limit (LIB-SEC-007, slice 0077)

Slice 0075a found that one element with 100,000 attributes took about 214 s to read in a
debug build. The cause is roxmltree 0.21.2: it checks each attribute of an element against
the element's earlier ones (`src/parse.rs:1015`), so its time per element grows with the
square of the element's attributes, before the reader runs. One start tag within the
default size limit can hold about three million attributes. The reader itself is linear in
them (slice 0077: with the parser's check removed, 400,000 kept attributes on one Claim
read in 3.0 s debug, twice the time of 200,000).

- **D8. An attribute limit.** `Limits::with_max_attributes` adds a fifth limit, by default
  256: the attributes of one element, which is every attribute in its start tag except the first declaration of each prefix (`xmlns` or `xmlns:p`), which D1's bindings limit bounds. A declaration that repeats one in the same start tag counts: roxmltree 0.21.2 does not refuse a repeated `xmlns` or `xmlns:xml`, and each repeat lengthens every namespace lookup at and under the element. The
  pre-parse scan counts them at each start tag, after the depth, the bindings in scope and
  the copies. It refuses an element over the limit at its `<` with
  `ReadError::LimitExceeded`, `Limit::Attributes`, template `SACM-SEC-003/attributes`, in
  both modes. The parser then makes fewer than 256 comparisons per attribute, so a read at
  the defaults is linear in its input. A hostile 32 MiB input of 256-attribute elements
  reads in about 2 to 4 s in a release build, where ordinary content of that size takes
  about 1 s. The largest element in the corpus has 8 attributes.
- **D9. No parser fork.** roxmltree is not forked, vendored or patched. A `[patch]` does not
  reach users of the published crate, and a fork would make us maintain a security-critical
  parser for inputs no real file has. Its latest release, 0.21.2, is quadratic. Upstream
  knows (roxmltree issue 153, closed 2026-10-24): its unreleased master caps an element at
  65,535 attributes, which still costs about 2·10⁹ comparisons. If a release makes the check
  linear, the default can rise, and the limit stays.
- **D10. Writes.** `write` has no limit (D3). An element can be written with more
  attributes than it was read with: values read in element form (SACM23-XMI-014) and a
  generated `xmi:id`. Output of an element at the limit may need a raised limit to read
  back.

Consequences: a file with an element of more than 256 attributes (none is known) is refused
by default. An application that reads trusted files of that kind raises the limit and
accepts the parser's quadratic cost for those elements. The CHANGELOG entry of LIB-SEC-003,
not yet released, gains the limit.

Alternatives considered: bumping roxmltree (no release fixes it); a fork (see D9); counting
namespace declarations (rejected: they would collide with the 256-binding limit at a root
that declares many namespaces, and the writer adds declarations to kept elements; a repeated declaration is counted, since it binds nothing new); refusing
at the first attribute over the limit (rejected: the other scan limits locate at the `<`).

*Amendment 2 status: accepted (the maintainer, 2026-11-25: a limit, not a fork; default
256).*
