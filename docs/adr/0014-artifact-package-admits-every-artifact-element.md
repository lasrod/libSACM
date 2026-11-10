# 0014. An ArtifactPackage admits every ArtifactElement

- Status: accepted (the maintainer decided on 2026-11-10)
- Date: 2026-11-10
- Matrix rows: LIB-API-008; inherited by SACM23-ART-001, SACM23-XMI-016

## Context

SACM 2.3 §12.2 types the ArtifactPackage role as
"artifactElement:Base::ArtifactElement[0..*] (composition)", and ptc/22-03-13 agrees.
Every ModelElement kind is an ArtifactElement, so an AssuranceCasePackage, an
ArgumentPackage, a Claim or a Term inside an ArtifactPackage conforms to the SACM XML
Schema.

The assurance-forge C++ library narrows the role to the 12 artifact-package kinds
(`io/name_tables.cpp:124`, `kind_is_artifact_element_in_artifact_package`). That
narrowing is not documented, and its reader drops a child that no role admits.

Clause 2 requires strict import of every document that conforms to the schema
(D-STRICT), so the narrowing would refuse conforming documents.

## Decision

- The containment table (LIB-API-008) admits every ArtifactElement kind, all 34
  ModelElement kinds, under ArtifactPackage, ArtifactPackageInterface and
  ArtifactPackageBinding, as §12.2 and ptc type the role.
- No warning is given for a non-artifact kind there. If a content rule is wanted later,
  it belongs to validation (D-VAL-LAYER), not to insertion.
- **Deviation from C++:** this replaces the narrowing in `name_tables.cpp:124`. The
  other C++ containment rules match the metamodel and are followed.

## Consequences

- Every document that is schema-valid in this respect loads in both modes.
- Code walking an ArtifactPackage must not assume that its children are artifact kinds.

## Alternatives considered

- **Follow C++: admit only the artifact-package kinds.** Rejected: strict import would
  refuse schema-conforming documents, which breaks clause 2.
