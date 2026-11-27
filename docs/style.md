# Documentation style

A human must be able to read it. Write for a user who wants one answer and will leave
when they have it.

## Rules

- **Lead with the point.** A bullet opens with its key word or fact in bold, then the detail.
- **Most important first.** What the thing is for, then how to start, then the exceptions.
- **Keep it short.** No prose paragraph or list item runs over four lines.
- **Leave out the obvious.** Do not restate a signature or a name. Say what the code cannot.
- **Structure over sentences.** A list for parallel points, a table for anything with
  two or more columns of facts, one complete example over three partial ones.
- **Link sparingly.** The [functionality matrix](matrix/functionality-matrix.md) is the
  inventory; a document need not list everything. Link the one place that holds the detail.
- **Claim only what is verified.** State in the present tense only what the matrix shows
  as `verified`. Name planned work as planned, or leave it out.
- **SACM terms only.** Use the standard's names for its concepts.
- **No diagrams in rustdoc.** They do not render there. Mermaid is fine in Markdown that
  GitHub renders, where it clearly helps.

## Example

Bad: the point is buried, and one sentence carries four facts.

> An operation is atomic: a refused one changes nothing, and `edit::check` answers
> whether `apply` would accept an operation, and `edit::preview` what it would do, without
> changing the document, while a delete cascades only where its policies allow.

Good: each fact is one bullet that opens with its key word.

> - **Atomic.** A refused operation changes nothing, and no edit leaves a reference dangling.
> - **Ask first.** `edit::check` says whether an operation would be accepted;
>   `edit::preview` says what it would do.
> - **Deletes.** A delete cascades only where its policies allow; each refuses by default.

## Where the rules apply

| Applies to | Notes |
|---|---|
| The crate root (`crates/sacm/src/lib.rs`) | What the crate is for, a table of entry points, one complete example, the key rules. |
| Module and item rustdoc | A one-line summary of what it is for, then only what the signature does not say. |
| `README.md` and the two crate READMEs | Short; they point to the crate root and do not repeat it. |
| [architecture.md](architecture.md) and [sacm/](sacm/) | Generated tables stay as generated. |
| The user guide and the SACM primer | Planned; they follow this guide when written. |

Exempt, because they are history and stay as written:

- **Slice and verification records** in [slices/](slices/) and [verification/](verification/).
- **Accepted ADRs** in [adr/](adr/).

## Checks

- **By test.** `cargo test -p xtask --test docs_style` checks the four-line rule in the
  entry documents, that rustdoc holds no diagram, and that the entry documents link the matrix.
- **By review.** The verifier reads each document against this guide.
