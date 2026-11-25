# ADR 0044 — The EMF dialect and foreign subclasses

- Status: accepted (the maintainer, 2026-11-19: M6 decisions Q6 and Q7; 2026-11-23: this ADR,
  and the slice 0075 questions Q1 and Q2, both as recommended).
- Date: 2026-11-23
- Matrix rows: LIB-COMPAT-002, LIB-COMPAT-017; LIB-COMPAT-005 and LIB-CLI-004 use them.
- Amends: ADR 0016 D1, ADR 0041 D6.
- Amended: amendment 1 (2026-11-25, an attribute in both XMI namespaces).

## Context

Most SACM tools are built on EMF, which writes XMI in its own dialect. It binds `xmi` to
`http://www.omg.org/XMI`, not to XMI 2.5.1's `http://www.omg.org/spec/XMI/20131001`, and
names same-document targets by fragment paths (`//@argumentPackage.0/@argumentElement.3`, `/1`)
instead of `xmi:id`s. XMI 2.5.1 says "Each version of XMI is unambiguously identified by its
unique namespace URI" (§7.5.3), and has a reference name "the target element's XML ID"
(§7.10.2). So the dialect is not SACM 2.3 XMI. Until now both modes refused it with
SACM-XMI-015, so no file of the fetched corpus that comes from ACEditor, Epsilon or the EMF
SACM implementation opened.

The same tools type SACM roles with classes of their own metamodels, GSN above all
(`http://acwg.org/3.0/gsn`). ptc/22-03-13 sets no `allowMetamodelExtension`, so a foreign
class in a SACM role is not valid against the SACM XML Schema (rule 4f, Annex B.2).

## Decision

- **D1. The EMF XMI namespace, in tolerant mode only.**
  - A tolerant read reads the `XMI` element and the attributes `id`, `type`, `idref`,
    `version`, `uuid` and `label` in `http://www.omg.org/XMI` as XMI 2.5.1's. It reports one
    SACM-XMI-015 Warning per document, at the first name in that namespace.
  - What it reads is held as XMI 2.5.1's, so both saves write XMI 2.5.1.
  - Any other name in that namespace is content that is not SACM 2.3 (SACM-XMI-003), kept as
    written.
  - A strict read refuses, as `Unsupported` SACM-XMI-015, any name in
    `http://www.omg.org/XMI` outside the content of an `xmi:Extension` or of a Difference's
    `target`, which XMI.xsd leaves open and both modes keep as written. Names in that
    content are no ids, and the alias does not apply to them (slice 0075a verification,
    2026-11-24: XMI.xsd lets an Extension hold any element and a `target` any element and
    attribute, so a valid document may carry them).
  - An `XMI` element in any other namespace, or none, is refused in both modes, as a limit
    of this version.
- **D2. EMF fragment paths, in tolerant mode only.**
  - A tolerant read reads a same-document reference target written as an EMF fragment path
    as the id of the element it names. One SACM-XMI-017 Warning per document says how many
    paths were read, and whether some were kept.
  - The targets considered are a reference end's token, the fragment of a same-document
    `href`, and an ExpressionLangString's `expression`. A token that equals an `xmi:id` is that
    id.
  - Only EMF's default grammar is read. A path names an element read as an element, the root
    of a resolved same-file proxy, or a preserved element that has an id. A path that names
    nothing is kept as written, and `validate` reports it (SACM-REF-001).
  - Paths inside preserved content are never rewritten. A strict read resolves no path.
  - The scan and the resolution are linear in the input.
- **D3. Foreign subclasses, declared by the application, in tolerant mode only.**
  - `ReadOptions::with_subclass(namespace, name, kind)` declares that a foreign class is a
    subclass of the SACM 2.3 class `kind`.
  - A tolerant read then reads a child element typed with that class, under a role that
    admits `kind`, as an element of `kind`. It holds the class as preserved content
    (`SacmElement::subclass()`), with the attributes and children SACM lacks.
  - A tolerant save writes the class back as the element's `xmi:type`; a strict save refuses
    the element, as for any preserved content.
  - A declaration never retypes a class of the root's SACM family, a root or a proxy. A
    strict read ignores the declarations.
  - A root's type that names a class the application declared a subclass of the root's own
    class is read as the root's subclass; the root's class still comes from its element name
    (the maintainer, 2026-11-24, slice 0075b Q1).
  - The crate names no foreign class (D-TERM): an application gives its own declarations.
- **D4.** A subclass type is reported as SACM-XMI-003 (`subclass-type-kept`), one per element,
  not under a new code: it is content SACM 2.3 does not have, kept.
- **`sacm convert`** (LIB-CLI-004, ADR 0023 amendment 2) reads tolerantly, writes strictly,
  reloads strictly and compares. It writes its output only when nothing is lost. It refuses a
  file that keeps content SACM 2.3 lacks. Discarding such content is LIB-EDIT-016 (deferred;
  the maintainer, 2026-11-23).
- **Two corpus files** hold loose `ArgumentReasoning` and `Assertion` elements beside their
  packages. They stay refused (SACM-XMI-001), recorded under LIB-COMPAT-014 (deferred; the
  maintainer, 2026-11-23).

## Alternatives

- **Reading the EMF namespace strictly:** it is not XMI 2.5.1, and strict mode reads only
  the standard (Q6).
- **Resolving paths inside preserved content:** that would rewrite content kept as written.
- **EMF `gid` or key-predicate identity:** no file needs it.
- **One declaration per namespace, or a callback:** less exact, and harder to compare and
  hash.
- **Normalising EMF `xmi:Extension`:** the binding rules get tangled, and no file has one.

## Consequences

- Five files of the fetched corpus, from three tool families, read tolerantly and write back
  without loss (LIB-COMPAT-005).
- The API change is additive: `ReadOptions::with_subclass` and `subclass`,
  `SacmElement::subclass`, `model::Subclass`, `Code::XMI_FRAGMENT_PATH`, the new templates and
  the `convert` subcommand.
- D-STRICT: EMF's XMI namespace and fragment paths (LIB-COMPAT-002), and an application's
  declared subclasses (LIB-COMPAT-017), are read in tolerant mode only.

## Amendment 1 (2026-11-25): an attribute in both XMI namespaces (LIB-COMPAT-019, slice 0076)

Under D1: when an element carries `id`, `type`, `idref`, `version`, `uuid` or `label` in
both XMI namespaces, XMI 2.5.1's is read, and EMF's is content in another namespace, kept as
written (SACM-XMI-003). XMI 2.5.1 §7.5.3 (p. 10): "Each version of XMI is unambiguously
identified by its unique namespace URI"; with XMI 2.5.1's attribute present, the alias has
nothing to stand in for. An element that keeps `type` in EMF's namespace or `xsi:type`, and no type in XMI
2.5.1's, is written with its own `xmi:type` (only a root keeps an `xsi:type`). This replaces slice 0075a's "the first in
document order". D3's rule that no declaration applies to an element typed in both
namespaces stays.

*Amendment 1 status: accepted (lead ruling, 2026-11-25: it follows §7.5.3 and keeps every
verified test; the maintainer may veto).*
