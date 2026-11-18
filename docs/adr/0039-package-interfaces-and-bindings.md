# ADR 0039 — Package interfaces and bindings: ends, creates and deletes

- Status: accepted (the maintainer, 2026-11-18).
- Date: 2026-11-18
- Rows: SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013, LIB-EDIT-019
- Related: ADR 0026, ADR 0029 amendment 1, ADR 0031, ADR 0034 amendment 2, ADR 0036; the M5 decisions Q1, Q6, Q7 and Q9 (the maintainer, 2026-11-18)

## Context

- §11.5, §11.6, §12.4 and §12.5 give each interface `implements:<package>[1]` and each binding `participantPackage:<package>[2..*]`.
- ptc/22-03-13 gives each package `interface:<package>Interface[0..*]`.
- The same three ends recur in four families, each typed to the holder's family.
- They are the first required ends held by a container, and the first with a lower bound above 1.
- The design brief is `target/slice-0066-design.md` (git-ignored).

## Decision

- **D1. Three ends.**
  - There are three `ReferenceEnd` variants, `Interface`, `Implements` and `ParticipantPackage`, one per name.
  - Each end has a table of (declaring class, target type) pairs. Its owner, type, `applies_to` and `bounds` come from the first pair whose class admits the holder's kind.
  - Slice 0066 declares the Argumentation and Artifact pairs; SACM23-TERM-007/-008 and SACM23-AC-002/-003 add theirs.
  - In write and compare order they come after `TerminologyGroupMember` and before `Expression`.
- **D2. `interface` and `implements` are kept as written.** They are never synced or checked against each other; `Document::referrers` finds a package's interfaces.
- **D3. Bounds.**
  - A lower bound counts distinct ids; an upper bound counts every id.
  - This holds in edits and copies, where `TooFewReferences.given` is the distinct count, and in validation from SACM23-VAL-014.
- **D4. Creates.**
  - There is one create per kind, `root(name, ends)` or `new(parent, name, ends)`.
  - The JSON fields are `implements` and `participant_packages`.
- **D5 (amends ADR 0031). Deletes.**
  - Under both non-reject reference policies, an `implements` or `participantPackage` id is scrubbed.
  - A holder that loses an id and is left below the bound is deleted with its contents, and the fixpoint continues.
  - The package policy guards that holder: a fourth check, after the dangling references, refuses a non-empty one as `NotEmpty` naming it.
  - A holder already below its bound that loses no id is untouched.
- **D6. Copy and move.** A copy follows copied ids and checks bounds as D3 says; a move changes no reference.

## Consequences

- An end can apply to several unrelated classes, so crate code asks `owner_on(kind)` and `target_on(kind)`, never a kind-free owner.
- A delete may remove an interface or binding the caller did not name; a preview lists it.
- `AddReference` cannot bring a binding read with one participant to two one step at a time; `SetReferences` with two ids repairs it.

## Alternatives considered

- Six variants named after the families: rejected by Q9.
- Syncing `interface` and `implements`: rejected by Q1, because the model keeps two associations.
- Leaving a holder below its bound, or refusing the delete: rejected by Q6.
- Counting occurrences: rejected by Q7, because `"a a"` is one package.
