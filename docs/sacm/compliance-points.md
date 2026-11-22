# SACM 2.3 compliance points

Clause 2 of formal/23-05-08 defines conformance as the ability to import and export
XMI documents conforming to the SACM XML Schema, organized into compliance points by
unit of interchange. No compliance point is claimed yet.

| Compliance point | Unit of interchange | Mandatory | Matrix row | Claimed |
|---|---|---|---|---|
| Assurance Case Model | `SACM::AssuranceCasePackage` | yes | SACM23-CP-001 | no |
| Argumentation Model | `Argumentation::ArgumentPackage` | no | SACM23-CP-002 | no |
| Artifact Model | `Artifact::ArtifactPackage` | no | SACM23-CP-003 | no |
| Terminology Model | `Terminology::TerminologyPackage` | no | SACM23-CP-004 | no |
| SACM UML Profile | UML models with the SACM profile | no | SACM23-CP-005 | out of scope |

## What claiming a point means here

A compliance point row depends on every model, XMI, validation and round-trip row
for the classes in its unit of interchange. It may be marked `verified` only when all
of them are verified, and then it means:

- every conforming document of that unit can be read without loss, in strict mode;
- a strict write gives a document valid against the SACM XML Schema (ADR 0043), apart
  from XMI metadata that arrived invalid and is written as read; it does not run
  `validate` (LIB-COMPAT-007);
- load → save → load is semantically lossless for everything in the unit;
- the constraints of the governing clauses are checked and reported as diagnostics.

It does **not** mean that every third-party dialect is accepted (that is tolerant mode,
rows `LIB-COMPAT-*`), nor that the namespace URI is normative (decision D-NS-PIN).

The `README.md` and crate docs may only state a compliance claim the matrix supports.
