# 0036. Moving and copying elements, and finding referrers

- Status: accepted (the maintainer, 2026-11-17)
- Date: 2026-11-17
- Matrix rows: LIB-EDIT-017, LIB-EDIT-018, LIB-API-014, LIB-API-015. It amends ADRs 0029
  and 0030.

## Context

The independent review of 2026-11-16 asked for consumer needs before M4 and M5. An editor
needs to:
- move an element by drag and drop;
- accept a draft built in another document (GitHub issue #61);
- answer "what refers to this element?" without walking every element;
- show a generic property panel of flags and reference ends.

The standard does not define edits. Two facts bound the design:
- XMI 2.5.1 §7.6.1 (p. 14): "XML semantics require the values of this attribute to be unique
  within an XML document; however, the value is not required to be globally unique." So a
  copy may keep a source id only when it is free in the target.
- formal/23-05-08 §8.2 (p. 18): "gid:String[0..1] - a unique identifier that is unique
  within the scope of the model instance". A copy repeats gids, and `validate` reports that
  as SACM-ID-003. That is a finding, not a refusal (D-VAL-LAYER).

The C++ library has no move and no copy.

## Decision

- **D1. `edit::MoveElement`** moves an element and its whole subtree, with its preserved
  content, either under another parent (`new`) or to the roots (`root`).
  - **Position:** after the last child or root, or at a position (`with_position`). The
    position is the element's index among the new parent's children after the move, from 0
    to the number of the others.
  - **What does not change:** ids, references, features and the generated-id counters.
  - **What changes:** the parent, the role (recomputed from the containment table) and the
    order.
  - **Preserved children** of either parent keep their place after the typed child they
    followed.
  - **Refusals, in order:**
    - `ElementNotFound`, then `ParentNotFound`;
    - `InvalidParent`, or `InvalidRoot` for a root that is not a package (both
      SACM-CMD-005);
    - `ParentInSubtree` (SACM-CMD-005);
    - `PositionOutOfRange` (SACM-CMD-009).
  - **`changed_ids` follow LIB-EDIT-003.** They list the element when its parent changes,
    and any parent whose child order or preserved placement changes, in pre-order. A move to
    the place the element already holds changes nothing.
- **D2. `edit::CopyElements`** copies a list of elements of a source `Document`, each with
  everything it contains, under a target parent or as roots.
  - **A list:** a draft is several sibling elements that reference each other.
  - **The snapshot:** the source is captured when the operation is built (`ElementNotFound`).
    An element listed twice, or inside another listed element, is copied once.
  - **Ids:**
    - An element keeps its id when that id is an NCName and free in the target. Free means
      no element, part or preserved content holds it, and no reference names it.
    - Otherwise the element gets a generated id that avoids every id the copy keeps or
      names.
    - A part id (name, `content`, `key`, `location`, `value`) is kept when free, and dropped
      otherwise.
    - Preserved content is copied verbatim, so its ids must be free, and its references
      must not name an id the copy changes (`DuplicateId`).
  - **References:** a reference to a copied element follows its new id, and every other
    reference is kept as written. The copy is then checked as a create would be.
  - **The result:** the `Mutation` lists the created ids in pre-order. `source_ids()` gives
    the matching source ids in the same order.
- **D3. `Document::referrers(id)`** lists every typed reference that names `id`, giving the
  holder, the end and the position.
  - **Order:** holder pre-order, then `ReferenceEnd::ALL` order, then position.
  - **What is listed:** dangling ids too. References inside preserved content are not typed,
    so they are not listed.
  - **The index:** the document keeps one, updated by every edit, so the cost grows with the
    number of referrers and their depth, not with the document's size.
- **D4. Batch net ids** also take the before and after parent of every changed id. A move
  away and back that changes a parent's order is then reported.

## Amendments

- **ADR 0030 amendment 2:** `Operation::CopyElements` is not serialized. It holds a
  document snapshot, which has no JSON form (ADR 0030 rejects serializing `Document`).
  Serializing anything that holds one is a serde error, and the tag does not deserialize.
- **ADR 0029 amendment 1:** these become public:
  - the readers `SacmElement::flag(Flag)` and `SacmElement::references(ReferenceEnd)`;
  - `Flag::ALL` and `ReferenceEnd::ALL`, as `&'static` slices so that a later variant is
    additive;
  - `applies_to(kind)`;
  - `ReferenceEnd::bounds(kind)`. Bounds depend on the owner: §11.13 (p. 40)
    "target:ArgumentAsset[1]" and §12.14 (p. 50) "target:ArtifactAsset[1..*]".

## Consequences

- **Easier:** an editor can drag and drop, paste a draft, and show referrers and property
  panels with no hand-written tables.
- **The index:** each node carries a sibling slot, and each id use carries its holders. That
  adds memory per reference.
- **Serde:** `CopyElements` is the one operation that a JSON client cannot send. Such a
  client builds the draft itself and sends creates.

## Alternatives considered

- **Copy one element:** a draft is a set of sibling elements, so one element is not enough.
- **Copy refuses on any id clash:** that is useless for pasting into the same document.
- **Serializing the copy with an embedded document:** that would make JSON a second model
  format.
- **A move reports both parents:** that is wrong for LIB-EDIT-003, since `semantic_compare`
  reports `Parent` on the element.
- **Referrers by a full scan:** the cost would grow with the document's size, which is too
  slow for an editor on large cases.
