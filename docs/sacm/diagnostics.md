# Diagnostics catalog

Machine-readable diagnostic codes emitted by the `sacm` crate. **Codes are stable API**:
once released, a code's meaning never changes and a retired code is never reused.

Every diagnostic carries: `code`, `severity` (info / warning / error), the matrix row
and clause it enforces, affected element ids, an optional source location
(line/column), and a human-readable message. See decision D-DIAG.

> **Status.** The codes below are *reserved*: they are adopted from the assurance-forge
> C++ SACM library (same author) so that the two implementations report the same
> finding with the same code. A code becomes live when the matrix row that emits it is
> verified; until then the crate does not emit it. Tool-specific wording from that
> project ("Assurance Forge", ODE containers) has been kept where it explains the code.
> The `sacm::diagnostic::Diagnostic` type exists (LIB-DIAG-001). SACM-MULT-001 is emitted by
> `sacm::validation::validate` (SACM23-VAL-018). SACM-CMD-001, SACM-CMD-004, SACM-CMD-005 and SACM-ID-002 are
> produced as the `EditError::code()` of a refused edit (LIB-EDIT-001).
> SACM-XML-001 (rows LIB-SEC-002 and LIB-SEC-005), SACM-SEC-001 (LIB-SEC-001) and
> SACM-XMI-001 (SACM23-XMI-001) are produced, as the `ReadError::code()` of a refusal by
> `sacm::xmi::read`.

Adding a code: append the row here in the same change that adds its constant to the
crate, and name the matrix row that owns it.

## XML and security

| Code | Severity | Meaning |
| --- | --- | --- |
| SACM-XML-001 | Error | The input is not well-formed XML 1.0 with namespaces, is truncated, is not valid UTF-8 or UTF-16, declares ASCII but holds non-ASCII, or has a UTF-16 byte order mark but declares an encoding other than UTF-16, UTF-16LE or UTF-16BE. UTF-16 with a byte order mark is read. Fix: correct the XML at the reported location. `Code::XML_MALFORMED`, the code of `ReadError::Malformed` (LIB-SEC-002, LIB-SEC-005, SACM23-XMI-018). |
| SACM-SEC-001 | Error | The input has a DOCTYPE, which may declare entities; it is refused before it is read. Fix: remove the DOCTYPE, because SACM XMI does not need one. `Code::XML_DOCTYPE_REJECTED`, the code of `ReadError::Doctype` (LIB-SEC-001). |

## XMI structure

| Code | Severity | Meaning |
| --- | --- | --- |
| SACM-XMI-001 | Error | An interchange root (the document element, or an element child of the XMI element outside the XMI namespace) is not named as one of the twelve SACM package kinds, or the XMI element holds no root. Fix: make the document element, or each child of `xmi:XMI`, a SACM 2.3 package such as `sacm:AssuranceCasePackage` or `sacm:ArgumentPackage`; a single package needs no `xmi:XMI` element. `Code::XMI_INVALID_ROOT`, the code of `ReadError::InvalidRoot` and `ReadError::NoPackage` (SACM23-XMI-001). |
| SACM-XMI-002 | Error (strict) / Warning (tolerant) | Namespace URI is not a pinned SACM 2.3 namespace. |
| SACM-XMI-003 | Error (strict) / Warning (tolerant) | Unknown element; preserved in tolerant mode, rejected in strict mode. |
| SACM-XMI-004 | Error (strict) / Warning (tolerant) | Element lacks an `xmi:id`/`id`; generated in tolerant mode. |
| SACM-XMI-005 | Error (strict) | Containment role has an abstract declared type and the child carries no type attribute (`xmi:type`, or `xsi:type` on import). |
| SACM-XMI-006 | Error | Strict save refused: document contains preserved unknown content or compatibility-only data (LIB-COMPAT-003). |
| SACM-XMI-007 | Warning | External (cross-document) `href` reference is not supported; kept as opaque text in compatibility mode only. |
| SACM-XMI-008 | Warning | Document declares a SACM revision other than 2.3; loaded in compatibility mode. Distinct from SACM-XMI-002, which means the namespace itself is unrecognized. |
| SACM-XMI-009 | Warning | The document's root is not a SACM interchange package, but SACM packages were found inside it (an ODE `DDIPackage`, say). A tolerant load reads the SACM out and states that the file does not conform, that content outside the SACM packages is not represented, and that saving writes conformant SACM rather than the original container. Strict refuses the document (SACM-XMI-001). |
| SACM-XMI-010 | Error (strict) / Warning (tolerant) | An `xsi:type` attribute is used; XMI 2.5.1 §9.5.2 rule 2g allows it only if `org.omg.xmi.useSchemaExtensions` is true, and SACM sets no such tag. The document is read as if it were `xmi:type`. Fix: write `xmi:type`. `Code::XMI_XSI_TYPE` (LIB-COMPAT-001, ADR 0022). |
| SACM-XMI-011 | Info | An `xmi:version` attribute on the `xmi:XMI` element or a root is present and ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3). Nothing is lost and nothing needs fixing. To silence it, remove the attribute, or save the document again with `xmi::write`, which does not write it. `Code::XMI_VERSION_IGNORED` (LIB-COMPAT-001, ADR 0022). |

## Identity and references

| Code | Severity | Meaning |
| --- | --- | --- |
| SACM-ID-001 | Error | Duplicate element ID. |
| SACM-ID-002 | Error | Invalid element ID syntax: a new element's id is not an XML NCName (XMI 2.5.1 §7.6.1 `xsd:ID`). `Code::ID_INVALID`, `EditError::InvalidId` (LIB-EDIT-001). Fix: use a name that starts with a letter or `_` and has no spaces or `:`, or leave the id out to have one generated. |
| SACM-ID-003 | Error | Duplicate `gid` (clause 8.2: "unique within the scope of the model instance"). Distinct from SACM-ID-001: `gid` is SACM's own model-global identifier rather than the XMI serialization id, and it is the handle third-party tools key on, so a document can be clean on one and broken on the other. An absent or empty `gid` is not an identity and is not compared. |
| SACM-REF-001 | Error | Dangling reference: target ID does not resolve. |
| SACM-REF-002 | Error | Reference resolves to an element of the wrong type. |
| SACM-REF-003 | Warning | Reference resolves to an element the document carries only as preserved compatibility content, so it cannot be type-checked. Distinct from SACM-REF-001: the target is present in the source, it is merely untyped. Conflating them reports an intact argument as structurally broken. |

## Validation

| Code | Severity | Meaning |
| --- | --- | --- |
| SACM-ENUM-001 | Error | Invalid enumeration literal (e.g. `assertionDeclaration`). |
| SACM-MULT-001 | Error | Multiplicity violation: a property held more or fewer times than its multiplicity allows. For example, a ModelElement with no `name` (clause 8.6 `name:LangString[1]`; SACM23-VAL-018, `Code::MULTIPLICITY_VIOLATION`) or, once SACM23-XMI-021 is built, with two, an `AssertedRelationship` with no source (clause 11.13 `source[1..*]`) or not exactly one target (`target[1]`), or a binding with fewer than two participant packages. |
| SACM-CITE-001 | Error | `isCitation` is true but `citedElement` is absent (or vice-versa constraints per clause 8.2). |
| SACM-ABS-001 | Error / Warning | An `isAbstract`/`abstractForm` combination the standard rules out. Error: an abstract element using `abstractForm` (clause 8.2 gives it to concrete elements), or a concrete `Expression` referencing abstract `ExpressionElement`s (clause 10.10 OCL). Warning: the referred `abstractForm` target is not abstract, or is a different type — clause 8.2 words both with "should". |
| SACM-EXPR-001 | Warning | An `ExpressionLangString` carries both an `expression` reference and literal `content`. Clause 8.4: "If expression is not empty, then +content should be empty." A warning because the clause says "should", but a real one: the two carry the same meaning twice and a reader has no rule for which wins. |
| SACM-PKG-001 | Error / Warning | Content the owning package clause does not allow. Error: an `ArgumentPackage` that nests an `ArgumentPackage` but contains something else (clause 11.4), interface or binding content that is not a citation (clauses 11.5, 11.6, 12.4, 12.5), or an `AssuranceCasePackageInterface` holding a non-interface sub-package (clause 9.3 OCL). Warning: an interface citation pointing outside the package it implements, and a binding named as another binding's participant — see the recorded resolutions in [decisions.md](decisions.md) (D-IFACE-CITE, D-BIND-PART), since the parallel clauses disagree. |

## Commands

| Code | Severity | Meaning |
| --- | --- | --- |
| SACM-CMD-001 | Error | Operation target not found. `Code::CMD_TARGET_NOT_FOUND`, `EditError::ParentNotFound` (LIB-EDIT-001): a create operation names a parent id that is not in the document. Fix: create the parent first, or correct its id (ids are compared exactly). |
| SACM-CMD-002 | Error | Delete rejected: element is referenced and policy is `RejectIfReferenced`. |
| SACM-CMD-003 | Error | Preview expired: the document changed after the preview was taken. |
| SACM-CMD-004 | Error | Create rejected: caller-provided ID already exists. `Code::CMD_DUPLICATE_ID`, `EditError::DuplicateId` (LIB-EDIT-001). Ids are unique across the whole document. Fix: choose another id, or leave it out to have one generated. |
| SACM-CMD-005 | Error | Invalid parent: the target cannot contain the requested child kind. `Code::CMD_INVALID_PARENT`, `EditError::InvalidParent` (LIB-EDIT-001): the parent's class has no composite role for the new kind, such as a Claim directly in an AssuranceCasePackage. Fix: choose a parent that can hold the kind (a Claim goes in an ArgumentPackage). |
| SACM-CMD-006 | Error | Delete rejected: package is not empty and policy is `RejectIfNonEmpty`. |
| SACM-CMD-007 | Error | Delete rejected: external (cross-package) references exist and policy is `RejectIfExternalReferencesExist`. |

