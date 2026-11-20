# 0035. Tolerant read opens any file; strict save is schema only

- Status: accepted (the maintainer, 2026-11-16); amended by [ADR 0042](0042-xmi-metadata-and-same-file-proxies.md) (XMI metadata is read, not preserved)
- Date: 2026-11-16
- Matrix rows: LIB-COMPAT-001, LIB-COMPAT-003, LIB-COMPAT-007, LIB-API-011, SACM23-VAL-006,
  SACM23-XMI-008, LIB-API-006. It amends ADRs 0015, 0021, 0022, 0030 and 0033.

## Context

An independent review (2026-11-16) found two problems with the default behaviour:
- **Reads:** a tolerant read refused a whole file for one bad value, such as
  `assertionDeclaration="Assumed"`, or for common XMI metadata, so an editor user could not
  open the file to fix it.
- **Saves:** a strict save ran full validation, so the default save failed on any draft with
  an open error.

The standard asks for neither:
- SACM 2.3 clause 2 asks for import and export of "XMI documents that conform with the SACM
  XML Schema".
- XMI 2.5.1 §7.8.2 says "By default, XMI 2 produces schemas that ignore multiplicities
  also", so a missing name or target is still schema-valid.

The maintainer wants a library that is simply usable.

## Decision

- **D1. A tolerant read returns a document for every well-formed input with SACM 2.3
  package roots.**
  - It refuses only:
    - a DOCTYPE;
    - malformed XML;
    - `InvalidRoot` and `NoPackage`;
    - `DuplicateId`;
    - `Unsupported` for an encoding, a parser limit, a root outside SACM 2.3, an `XMI` element
      outside XMI 2.5.1, or an XMI-namespace document element.
  - Every other refusal of a strict read is kept as preserved content at its construct, with
    one Warning at the strict refusal's location:
    - a bad literal (SACM-ENUM-001; the model holds the default);
    - a repeated value (SACM-MULT-001; the first is read and the rest are preserved);
    - an untyped child under an abstract role (SACM-XMI-005);
    - an empty `xmi:id` (SACM-ID-002; the element gets a generated id);
    - content not read yet (D2).
  - The full table is in `xmi/refusal.rs` (`Unread::tolerant`) and LIB-COMPAT-001.
- **D2. Conforming content this release does not read yet is kept by a tolerant read with
  SACM-XMI-014.** That covers:
  - the kinds, ends and XMI metadata `Interim` rows name;
  - a cross-document `href`, which gets SACM-XMI-007.

  It is never SACM-XMI-003, so valid SACM is never called defective. A strict read still
  refuses it as `Unsupported`.
- **D3. A document holds preserved content outside its elements.** This covers wrapper
  attributes, unread roots, XMI-namespace wrapper children and text, placed among the roots.
  `Document::has_preserved_content` and `Difference::DocumentPreservedContent` expose it.
- **D4. A strict write checks schema validity only.** It refuses, in this order:
  1. `NoRoot`;
  2. `PreservedContent`;
  3. `Invalid`:
     - an `xmi:id` that is not an NCName, SACM-ID-002 (XSD Part 2 §3.3.8);
     - a reference to no `xmi:id` the output writes, SACM-REF-001 (XSD Part 1 §3.3.4; XMI
       rule 4i);
  4. unrepresentable characters.

  It never runs `validate`; an application calls it for model checks.
- **D5. A strict read is unchanged.**

## Consequences

- An editor opens real files and shows the Warnings. It saves a draft strictly when the draft holds no preserved content; a file that holds some, such as an `xsi:schemaLocation`, saves in tolerant mode until the rows that read it are built.
- A tolerant read now continues past points where it used to stop, so its paths need
  mutation and property testing.
- `sacm validate --tolerant` on a file with a bad literal reports Warnings and exits 0. Since ADR 0023 amendment 1, `sacm validate` reads strictly by default and exits 1 for such a file.

## Alternatives considered

- **Keep refusing conforming content not read yet (option A).** Rejected by the maintainer:
  most real files hold Artifacts, so most would stay unreadable until M5.
- **Keep save-time validation.** Rejected: it makes the default save fail on drafts, and the
  standard asks only for schema conformance.

## Amendment 1 (2026-11-18): no root is kept as document content

SACM23-TERM-007, SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003 (slice 0067).
- **Unread roots no longer arise.** Since slice 0067 every SACM 2.3 package kind is read as a root, so D3's "unread roots" do not occur. The document's own preserved content is the wrapper's attributes, its XMI-namespace children and text.
- **`WriteError::NoRoot`** is a document with no root element; the writer no longer looks for a preserved root.
- **D2's Interim rows:** "the kinds, ends … `Interim` rows name" now covers no kind or end; XMI metadata and cross-document `href`s remain.

*Amendment 1 status: accepted (the maintainer, 2026-11-18).*
