# ADR 0037 — Undo is the application's, on whole-document copies

- Status: accepted (the maintainer, 2026-11-17)
- Row: LIB-EDIT-011 (Q-UNDO)
- Related: ADR 0019, ADR 0020, ADR 0031, ADR 0032, ADR 0036

## Context

Editors built on the crate need undo and redo. GitHub issue #61 lists what an application
needs:
- one user action, even a batch, is one step;
- accepting a draft document into the main one is one step;
- the ids generated after an undo are specified;
- the history is per document and in memory;
- memory use stays bounded for thousands of elements.

The standard says nothing about editing.

How comparable libraries do it (2026-11-17 survey):

| System | Where the history lives | Mechanism |
|---|---|---|
| EMF | the application's editing domain | a `BasicCommandStack` of commands, or `ChangeRecorder` / `ChangeDescription` |
| GMF, Sirius | the framework | EMF Transaction and `IOperationHistory` |
| ProseMirror | a separate package, `prosemirror-history` | inverted steps |
| Slate | a separate package, `slate-history` | inverse operations |
| Lexical | a separate package, `@lexical/history` | snapshots of the immutable state |
| Rust `undo`, `undo_2` | the application | commands the application implements |
| Yjs | the core library, because it is multi-user | selective undo; ids are never reused |
| Automerge | userland | built-in undo was removed for 1.0 |
| Redux, Immer | the application | snapshots or inverse patches |

Model libraries give the application one of two building blocks: reversible change records,
or cheap whole-state copies. The history itself is a separate, optional layer that the
application configures.

Measured on 2026-11-17 (release build, a flat package of claims with one AssertedInference
per 10 claims):

| Elements | Heap per copy | `Document::clone` |
|---|---|---|
| 1,101 | 670 KiB | 0.26–0.29 ms |
| 5,501 | 3.3 MiB | 1.4–2.3 ms |
| 22,001 | 13.2 MiB | 8–9 ms |

## Decision

1. **Undo and redo are the application's,** on whole-document copies. The application
   takes a `Document::clone` before each successful `edit::apply`, `edit::apply_preview`,
   `edit::batch` or `edit::apply_all`, and restores it with `std::mem::replace`. So one
   operation, one batch, or an accepted draft (`CopyElements` in a batch) is one step. The
   `edit` module docs show this as a tested recipe.
2. **What a restored copy is:**
   - It is `==` to the document before the change, and it writes the same XMI, preserved
     content included.
   - It has the revision and the generated-id counters it had. So the next create generates
     the id an undone create had (D-ID). An application that needs ids unique across its
     history gives its own (ADR 0031).
   - A preview taken before an undone change applies again after the undo.
3. **Cost:** a step costs one clone, in time and memory in proportion to the document. The
   application chooses the depth.
4. **Nothing new in the crate:** no history type and no inverse operations.

## Consequences

- No public API is added, so nothing has to stay stable for undo in 0.1.
- An undo is always exact, vendor content included.
- An application that sends changes as operations, like a web client, still sends its
  forward operations. An undo is a restored copy, not a sendable operation.

## Why not inverse operations

Inverse operations built from today's operations cannot restore a document exactly:
1. No operation can re-create preserved content.
2. No operation sets the `xmi:id` of a name, `content`, `key` or `value`. `SetContent::remove`
   and shorter value lists free those ids.
3. Creates append, so restoring an element's place needs a reorder, and preserved placement
   on top of that.
4. The one operation that could re-create a subtree exactly, `CopyElements`, has no serde
   form (ADR 0030 amendment 2).

A lossless version would need a private change record holding the detached subtrees, as
EMF's `ChangeDescription` does, and an inverse for every operation, now and for each
operation added later. Even then there would be no multi-user undo, which needs selective
undo and a collaboration design.

## Alternatives considered

- **A built-in `edit::History` type.** Not now: it saves an application about 30 lines, and
  applications keep their own policy for grouping, depth and audit. It can be added later
  without a break, with the same id rule.
- **Recorded inverse changes, or per-user undo.** Not now: see above. It can be added later
  without a break, as an opaque reversal record (ADR 0019 amendment 2), if multi-user
  editing becomes a requirement. The maintainer is unsure whether it will.
- **Cheaper copies through structural sharing inside `Document`.** Internal, with no API
  change. It can be done later as a performance row.
