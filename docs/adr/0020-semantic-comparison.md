# 0020. Semantic comparison

- Status: accepted (the maintainer, 2026-11-13)
- Date: 2026-11-13
- Matrix rows: LIB-RT-001. LIB-RT-002, LIB-RT-003, LIB-RT-004, LIB-EDIT-002 and LIB-CLI-003
  use it.

## Context

D-RT guarantees that load, save and load gives an equal model. The edit rows need "the
document is unchanged". Both need a public way to say whether two documents hold the same
SACM content and, if not, where they differ. The assurance-forge C++ library has
`compare::semantic_compare` (`libs/sacm/include/sacm/compare/semantic_compare.h`).

## Decision

- `sacm::compare::semantic_compare(left, right) -> Vec<Difference>`. An empty result means
  equal. `Document: PartialEq + Eq` means exactly that.
- **Matching.** Elements are matched by id across the whole document (D-ID), so a moved
  element is one `Parent` difference.
- **What is compared.** Every feature a `Document` holds:
  - presence;
  - kind and parent;
  - name;
  - `assertionDeclaration`;
  - the order of the roots;
  - the order of each element's children: the relative order of the children both sides
    hold, across all roles (ADR 0010 D2).

  Values are compared exactly, so a missing name differs from an empty one (ADR 0012, 0013).
- **What is not compared.** The generated-id counters are not compared: they are not SACM
  content and XMI does not carry them. Formatting, attribute order and prefixes are not
  held by a `Document` at all.
- **Order.** Order is compared, although SACM 2.3 marks no role ordered, so that equal
  documents write equal bytes (D-RT). Order differences are separate variants that a caller
  can ignore. MultiLangString values are compared in order (LIB-RT-001); no element holds
  one yet, and the first row that adds one adds its variant.
- **The `Difference` type.** It is a `#[non_exhaustive]` enum with one variant per feature,
  listed in a fixed order:
  1. root order;
  2. then each element of `left` in pre-order, giving either left-only, or its kind,
     parent, name, declaration and child order;
  3. then the right-only elements, in pre-order.

  Every element on one side only is reported, not just the top of a subtree. `Display`
  gives one line, which is for people and is not API.
- **Later rows.** Each row that adds a model feature adds a variant and a test. An
  exhaustive destructure in the model makes a forgotten feature fail to compile.

**Differences from C++ (`compare/semantic_compare.h`, `.cpp`):**
- C1. A typed `Difference` enum, not `SemanticDifference { path, category, message }`
  strings.
- C2. Elements are matched across the document, not within their parent (`.cpp:213-232`).
- C3. Order is compared; C++ pairs children regardless of order and sorts MultiLangString
  values (`.cpp:20-38`).
- C4. A missing name differs from an empty one; C++ treats an empty name as no name
  (`.cpp:82-87`).
- C5. Every one-side element is reported, not only the top of a missing subtree
  (`.cpp:220-231`).
- C6. An element whose kind differs is still compared on its other features
  (`.cpp:162-170`).
- C7. `Document` implements `PartialEq`/`Eq`; C++ has no `operator==`.

## Consequences

- Two equal documents may generate different next ids after a future delete; the `Document`
  docs say so, and LIB-EDIT-002 checks the next id on its own.
- A caller who wants order-free equality filters out the two order variants. An option for
  it can be added later without a break.
