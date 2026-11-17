# Real SACM files

What the reader does with SACM files that other tools wrote (LIB-COMPAT-015). The files are
fetched, never committed: `cargo xtask fetch-corpus` puts them in `third_party/corpus/`,
pinned by SHA-256 in `xtask/src/fetch.rs` (`CORPUS`). Several have no licence, so nothing
from them is copied into this repository. `cargo test -p xtask --test corpus` reads each file
in both modes, validates and writes back what a tolerant read opens, and checks that its
report equals the one below.

## Files

| # | File | Source (repository @ commit) | Licence | Written by | Namespace | Why it is here |
|---|---|---|---|---|---|---|
| 1 | `af-examples-kitchen-blender.sacm` | lasrod/assurance-forge-examples @ 7ec4add92d18 | MIT | Assurance Forge (C++ writer) | SACM `20220301`, XMI `20131001` | The only real-size file found in the SACM 2.3 namespace; has a TerminologyPackage, ArtifactReferences and gids |
| 2 | `af-examples-gsn-pattern-decorators.sacm` | lasrod/assurance-forge-examples @ 7ec4add92d18 | MIT | Assurance Forge | SACM `20220301`, XMI `20131001` | `isAbstract`, `needsSupport`; reads strictly |
| 3 | `af-testdata-sample.sacm.xml` | lasrod/assurance-forge @ c1f62854e5e7 | MIT | Assurance Forge legacy app dialect (project-authored) | `http://www.omg.org/spec/SACM/2.2/Argumentation` | The smallest example of the legacy dialect |
| 4 | `af-testdata-oasc-ja.xml` | lasrod/assurance-forge @ c1f62854e5e7 | MIT | Assurance Forge legacy dialect | `http://www.omg.org/spec/SACM/2.2/Argumentation` | Names in English and Japanese, non-ASCII UTF-8 |
| 5 | `af-testdata-safety-case-sanitized-strict.sacm.xml` | lasrod/assurance-forge @ c1f62854e5e7 | MIT | Assurance Forge legacy dialect | `http://www.omg.org/spec/SACM/2.3` (undated, SACM-like) | The structure of a real safety case (sanitized); an unrecognised SACM-like namespace |
| 6 | `oasc-open-autonomy-safety-case.sacm.xml` | EdgeCaseResearch/oasc @ abf48ef74d07 | none stated ("provided for reference purposes") | AI-generated in Edge Case Research's repository; no SACM tool | `http://www.omg.org/spec/SACM/2.2/Argumentation` | A published third-party SACM rendering of a real template case. Assurance Forge's copy is byte-identical, so it is not listed separately |
| 7 | `aceditor-hbs-default.sacm2` | LuisFelipeAN/aceditor-mbac-results @ 2bdad068b033 | none declared | ACEditor MBAC (EMF/Sirius SACM 2.1 editor) | `http://SACM_ACEditor/sacm/2.1`, XMI `http://www.omg.org/XMI` | Assurance Forge's C++ corpus called it the most useful file found |
| 8 | `epsilon-sacm-example.model` | epsilonlabs/SACM-UML-Profile @ f7222ed99140 | none declared | EMF / Epsilon (York), SACM 2.0 metamodel | `http://www.omg.org/spec/SACM/2.0/Argumentation`, encoding ASCII | SACM 2.0 shapes: `//@argumentAsset.N` paths, `ArtefactElementCitation`, `gid`, no `xmi:id` |
| 9 | `epsilon-untitled.model` | epsilonlabs/SACM-UML-Profile @ f7222ed99140 | none declared | EMF / Epsilon | `http://www.omg.org/spec/SACM/2.0/Argumentation` | The same dialect, with EMF UUID `xmi:id`s and id references |
| 10 | `justification-factory-sample-sacm.xmi` | JustificationFactory/JustificationDiagramModelToSACM @ 12abf127f6be | Apache-2.0 | ATL/EMF transformation (Justification Diagram to SACM) | `http://www.omg.org/spec/SACM/2.0/Argumentation`, ISO-8859-1 | Model-transformation output: `/N` references, `xsi:type`, a declared Latin-1 encoding |
| 11 | `justification-factory-sacm-derivation.xmi` | JustificationFactory/JustificationDiagramModelToSACM @ 12abf127f6be | Apache-2.0 | ATL/EMF transformation | `http://www.omg.org/spec/SACM/2.0/Argumentation`, ISO-8859-1 | `abstractForm`, `isAbstract` |
| 12 | `sysmline-easy-example.assurancecase` | Ruizhe-Yang/SysMLine @ 2658f087c5cd | EPL-2.0 | EMF with wrwei/SACM (the EMF implementation of SACM) | `http://omg.sacm/2.2/*` per package, and GSN `http://acwg.org/3.0/gsn` | The EMF reference-implementation dialect, with GSN types layered by `xsi:type` |
| 13 | `sysmini-quan.assurancecase` | Ruizhe-Yang/SysMini @ 313d44ed25b4 | none declared | EMF with wrwei/SACM | `http://omg.sacm/2.2/*` per package, GSN, and `artifact_` | The same dialect, with Artifacts and code in implementationConstraints |
| 14 | `mobstr-safetycase.integration` | panorama-research/mobstr-dataset @ 14f86e018ee6 | EPL-2.0 | EMF, DEIS ODE (mergedODE) | root `integration_:DDIPackage`; SACM in `http://www.deis-project.eu/ode/mergedODE/sacm/*` | SACM inside a foreign container root |
| 15 | `deis-etcs.model` | DEIS-Project-EU/DDI-Scripting-Tools @ a234b7f25500 | MIT | EMF, DEIS ODE | root `DDIPackage`; `http://omg.sacm/2.0/argumentation` | A container that carries other models too |

The files come from 9 repositories and 6 tool families other than this crate (Assurance
Forge, ACEditor, Epsilon/EMF, ATL, the EMF SACM implementation, DEIS ODE); 10 of the 15 are
not Assurance Forge's. No public file from another tool uses the SACM 2.3 namespace.

## Report

<!-- corpus-report:start -->
```text
corpus: 15 files

af-examples-kitchen-blender.sacm
  tolerant read: read; SACM-XMI-010 warning 112, SACM-XMI-011 info 1
  strict read: read; SACM-XMI-010 error 112, SACM-XMI-011 info 1
  validate: SACM-MULT-001 warning 40
  tolerant write: reads back equal

af-examples-gsn-pattern-decorators.sacm
  tolerant read: read; SACM-XMI-010 warning 2, SACM-XMI-011 info 1
  strict read: read; SACM-XMI-010 error 2, SACM-XMI-011 info 1
  validate: none
  tolerant write: reads back equal

af-testdata-sample.sacm.xml
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

af-testdata-oasc-ja.xml
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

af-testdata-safety-case-sanitized-strict.sacm.xml
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

oasc-open-autonomy-safety-case.sacm.xml
  tolerant read: refused Unsupported SACM-XMI-002 at line 15, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 15, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

aceditor-hbs-default.sacm2
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

epsilon-sacm-example.model
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

epsilon-untitled.model
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

justification-factory-sample-sacm.xmi
  tolerant read: refused Unsupported SACM-XML-002 at line 1, column 31 (a limit of this version)
  strict read: refused Unsupported SACM-XML-002 at line 1, column 31 (a limit of this version)

justification-factory-sacm-derivation.xmi
  tolerant read: refused Unsupported SACM-XML-002 at line 1, column 31 (a limit of this version)
  strict read: refused Unsupported SACM-XML-002 at line 1, column 31 (a limit of this version)

sysmline-easy-example.assurancecase
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

sysmini-quan.assurancecase
  tolerant read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 is to read it in tolerant mode)

mobstr-safetycase.integration
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 2, column 1
  strict read: refused InvalidRoot SACM-XMI-001 at line 2, column 1

deis-etcs.model
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 2, column 1
  strict read: refused InvalidRoot SACM-XMI-001 at line 2, column 1

read tolerantly: 2 of 15; read strictly: 2 of 15; tolerant write reads back equal: 2 of 2
```
<!-- corpus-report:end -->

## Findings

| Finding | Files | Row or note |
|---|---|---|
| A TerminologyPackage inside an AssuranceCasePackage, with a Category and six Terms: read since slice 0062, with each Term's `value`, `category` and `externalReference` as written; the two ArtifactReferences that name Terms validate | 1 | SACM23-TERM-001 to SACM23-TERM-004 |
| 112 + 2 `xsi:type` (SACM-XMI-010; an Error when strict) | 1, 2 | By design (LIB-COMPAT-001). Assurance Forge writer issue: write `xmi:type` |
| `xmi:version="2.0"` (SACM-XMI-011, Info) | 1, 2 | By design. Assurance Forge writer issue |
| 40 Claims with two `description`s (SACM-MULT-001, Warning) | 1 | SACM23-VAL-011. Assurance Forge writer issue |
| A root in another SACM revision's namespace, or a SACM-like one | 3–9, 12, 13 | LIB-COMPAT-004 (M6) |
| The Assurance Forge legacy app dialect (`id=`, `name=`, class-named children) | 3–6 | LIB-COMPAT-012 (M6, after LIB-COMPAT-004) |
| EMF dialects (XMI `http://www.omg.org/XMI`, per-package namespaces, `//@role.N` paths, gid identity) | 7–13 | LIB-COMPAT-002 (M6) |
| The declared encoding ISO-8859-1 | 10, 11 | LIB-COMPAT-016 (candidate, M6) |
| An ODE `DDIPackage` container root (SACM-XMI-001) | 14, 15 | LIB-COMPAT-009 (M6) |
| GSN classes layered on SACM by `xsi:type` (`gsn_:Goal`) | 12, 13 | Unknown until LIB-COMPAT-002 and LIB-COMPAT-004 read the namespace |
| Tolerant write-back | 1, 2 | Reads back equal: no loss found |

## Adding a file

1. Pin a raw URL to a commit; record its licence, tool and namespace here.
2. Add it to `CORPUS` with its SHA-256; run `cargo xtask fetch-corpus` and the test.
3. Paste the printed report; map every new refusal, Error or loss to a row or a note.
