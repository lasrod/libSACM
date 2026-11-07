# SACM 2.3 XMI — serialization conventions

Clause 2 (Conformance) requires import/export of "XMI documents that conform with the
SACM XML Schema produced by applying XMI rules to the normative MOF metamodel". The
specification does not print that schema, and the machine-readable model carries no
namespace URI, so several conventions are **project pins**, not normative values. They
are recorded here and in `docs/sacm/metamodel-inventory.md`; correcting one later is a
single-constant change plus golden-file refresh.

## Pinned conventions (strict export)

| Concern | Convention |
|---|---|
| SACM namespace | `http://www.omg.org/spec/SACM/20220301` (OMG version URI for 2.3), prefix `sacm` |
| XMI namespace | `http://www.omg.org/spec/XMI/20131001`, prefix `xmi`, `xmi:version="2.0"` |
| XSI namespace | `http://www.w3.org/2001/XMLSchema-instance`, prefix `xsi`, declared only when `xsi:type` is emitted |
| Root | One interchange package element — `sacm:AssuranceCasePackage`, `sacm:ArgumentPackage`, `sacm:ArtifactPackage` or `sacm:TerminologyPackage` |
| Identity | Every element carries `xmi:id`; ids are preserved exactly; generated ids are deterministic |
| Containment | Child elements named by their containment role (`argumentPackage`, `argumentElement`, `artifactElement`, `terminologyElement`, `description`, `name`, ...) |
| Concrete type | `xsi:type="sacm:<Class>"` whenever a role's declared type is abstract (e.g. `argumentElement` holding a `Claim`) |
| References | Attribute named by the reference end, value = space-separated `xmi:id`s (IDREFS), e.g. `target="c1"`, `source="e1 e2"` |
| Enumerations | Literal spelled as in the model (`needsSupport`); default values may be omitted |
| Booleans | `true` / `false` |
| Order | Deterministic: containment in document order; attributes in a fixed order |

## Import tolerance (tolerant mode only)

- Namespace prefixes are irrelevant; matching is by namespace URI + local name.
- Accepted namespace families: any URI containing `/spec/SACM/`, the EMF reference
  implementation's per-package `http://omg.sacm/2.2/<package>` URIs, and the fixture
  URI `http://example.org/sacm/2.3`. Other URIs: warning (tolerant) / error (strict).
- An `xmi:XMI` wrapper around the root package is accepted.
- References may also appear as child elements or `href="#id"` forms; external
  (cross-document) hrefs are preserved as opaque text with a warning.
- Machine-readable-model spellings are accepted on import: `ArtifactAssertedRelationship`
  for `ArtifactAssetRelationship`, `occurece` for `Event.date`, `argumentationElement`
  for `argumentElement`. Output always uses the resolved spelling.
- Missing `xmi:id`: generated, with a warning. Unknown elements: preserved with a
  warning; strict save then refuses the document rather than drop them.
- Other SACM revisions: loaded with a warning; saved as 2.3.

## Safety

- DOCTYPE declarations and entities are rejected outright (no external entity
  resolution, no entity expansion), with diagnostic SACM-SEC-001.
- Malformed XML produces SACM-XML-001 with a location, never a panic.
- Depth and size limits guard against resource exhaustion (row LIB-SEC-003).

## Round trip

- **Semantic** round trip is the guarantee: load → save → load yields an equal model.
- **Textual** round trip is not: whitespace, attribute order and prefixes may change.
- **Determinism** is guaranteed: the same model always saves to the same bytes.
