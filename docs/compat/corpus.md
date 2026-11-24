# Real SACM files

What the reader does with SACM files that other tools wrote (LIB-COMPAT-015), and what that
shows (LIB-COMPAT-005). The files are
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
  tolerant read: read; SACM-XMI-003 warning 17, SACM-XMI-004 info 5, SACM-XMI-008 warning 1, SACM-XMI-012 warning 5
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: none
  tolerant write: reads back equal

af-testdata-oasc-ja.xml
  tolerant read: read; SACM-MULT-001 warning 1, SACM-XMI-003 warning 195, SACM-XMI-004 info 8, SACM-XMI-008 warning 1, SACM-XMI-012 warning 8
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: none
  tolerant write: reads back equal

af-testdata-safety-case-sanitized-strict.sacm.xml
  tolerant read: read; SACM-XMI-002 warning 1, SACM-XMI-003 warning 473, SACM-XMI-004 info 17, SACM-XMI-012 warning 17
  strict read: refused Unsupported SACM-XMI-002 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: none
  tolerant write: reads back equal

oasc-open-autonomy-safety-case.sacm.xml
  tolerant read: read; SACM-XMI-003 warning 189, SACM-XMI-004 info 7, SACM-XMI-008 warning 1, SACM-XMI-012 warning 7
  strict read: refused Unsupported SACM-XMI-008 at line 15, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: none
  tolerant write: reads back equal

aceditor-hbs-default.sacm2
  tolerant read: read; SACM-XMI-003 warning 20, SACM-XMI-004 info 81, SACM-XMI-008 warning 1, SACM-XMI-010 warning 35, SACM-XMI-011 info 1, SACM-XMI-013 warning 12, SACM-XMI-015 warning 1, SACM-XMI-017 warning 1
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: SACM-MULT-001 error 32, SACM-MULT-001 warning 2
  tolerant write: reads back equal

epsilon-sacm-example.model
  tolerant read: read; SACM-XMI-003 warning 39, SACM-XMI-004 info 1, SACM-XMI-008 warning 1, SACM-XMI-011 info 1, SACM-XMI-015 warning 1
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: SACM-MULT-001 error 1
  tolerant write: reads back equal

epsilon-untitled.model
  tolerant read: read; SACM-XMI-003 warning 4, SACM-XMI-008 warning 1, SACM-XMI-011 info 1, SACM-XMI-012 warning 1, SACM-XMI-015 warning 1
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: none
  tolerant write: reads back equal

justification-factory-sample-sacm.xmi
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 7, column 3
  strict read: refused Unsupported SACM-XMI-015 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)

justification-factory-sacm-derivation.xmi
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 9, column 3
  strict read: refused Unsupported SACM-XMI-015 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)

sysmline-easy-example.assurancecase
  tolerant read: read; SACM-XMI-003 warning 1, SACM-XMI-004 info 1, SACM-XMI-008 warning 1, SACM-XMI-011 info 1, SACM-XMI-015 warning 1
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: SACM-MULT-001 error 1
  tolerant write: reads back equal

sysmini-quan.assurancecase
  tolerant read: read; SACM-XMI-003 warning 2, SACM-XMI-004 info 7, SACM-XMI-008 warning 1, SACM-XMI-010 warning 1, SACM-XMI-011 info 1, SACM-XMI-015 warning 1
  strict read: refused Unsupported SACM-XMI-008 at line 2, column 1 (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)
  validate: SACM-ABS-001 error 2, SACM-MULT-001 error 3
  tolerant write: reads back equal

mobstr-safetycase.integration
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 2, column 1
  strict read: refused InvalidRoot SACM-XMI-001 at line 2, column 1

deis-etcs.model
  tolerant read: refused InvalidRoot SACM-XMI-001 at line 2, column 1
  strict read: refused InvalidRoot SACM-XMI-001 at line 2, column 1

read tolerantly: 11 of 15; read strictly: 2 of 15; tolerant write reads back equal: 11 of 11
```
<!-- corpus-report:end -->

## Findings

| Finding | Files | Row or note |
|---|---|---|
| A TerminologyPackage inside an AssuranceCasePackage, with a Category and six Terms: read since slice 0062, with each Term's `value`, `category` and `externalReference` as written; the two ArtifactReferences that name Terms validate | 1 | SACM23-TERM-001 to SACM23-TERM-004 |
| 112 + 2 `xsi:type` (SACM-XMI-010; an Error when strict) | 1, 2 | By design (LIB-COMPAT-001). Assurance Forge writer issue: write `xmi:type` |
| `xmi:version="2.0"` (SACM-XMI-011, Info) | 1, 2 | By design. Assurance Forge writer issue |
| 40 Claims with two `description`s (SACM-MULT-001, Warning) | 1 | SACM23-VAL-011. Assurance Forge writer issue |
| A root in another SACM revision's namespace, or a SACM-like one: read since slice 0072, with one SACM-XMI-008 (3, 4, 6–9, 12, 13) or SACM-XMI-002 (5) Warning; a strict read refuses it | 3–9, 12, 13 | LIB-COMPAT-004 |
| The Assurance Forge legacy app dialect (`id=`, `name=`, class-named children) | 3–6 | LIB-COMPAT-012 (deferred: only SACM and GSN are read, the maintainer, 2026-11-19) |
| EMF dialects: the XMI namespace `http://www.omg.org/XMI` and `//@role.N` and `/N` fragment paths, read since slice 0075a with one SACM-XMI-015 Warning per file (7–9, 12, 13) and one SACM-XMI-017 for file 7's 35 paths, all read; a strict read refuses the XMI namespace. File 8's paths sit in kept SACM 2.0 `argumentAsset` children, so none is read. gid identity is not read | 7–13 | LIB-COMPAT-002 |
| The declared encoding ISO-8859-1: read since slice 0072. With the XMI 2.0 `xmi:XMI` element read since slice 0075a, a tolerant read refuses the files next for their loose `ArgumentReasoning` and `Assertion` roots beside the packages (SACM-XMI-001 at line 7 and 9); a strict read refuses the XMI 2.0 namespace (SACM-XMI-015) | 10, 11 | LIB-COMPAT-016; LIB-COMPAT-014 (deferred, the maintainer, 2026-11-23) |
| An ODE `DDIPackage` container root (SACM-XMI-001) | 14, 15 | LIB-COMPAT-009 (M6) |
| GSN classes layered on SACM by `xsi:type` (`gsn_:Goal`): without declarations, kept whole as preserved content (SACM-XMI-003); read since slice 0075b as the SACM classes an application declares them subclasses of (see "What LIB-COMPAT-005 claims") | 12, 13 | LIB-COMPAT-017 |
| Tolerant write-back | 1–9, 12, 13 | Reads back equal: no loss found |
| Tolerant write-back after a renamed prefix: until slice 0075a, a root attribute whose `xmi` prefix was bound to XMI 2.0's `http://www.omg.org/XMI` was written as `ns1:…`, and each holder of preserved content read back with one namespace declaration more. Resolved: the XMI 2.0 `xmi:version` and `xmi:id` are read as XMI 2.5.1's, so no prefix is renamed | 8, 9, 12, 13 | LIB-COMPAT-002 |

## What LIB-COMPAT-005 claims

Files that tools independent of this crate wrote are read, checked and written back without
loss. The evidence is five files of this corpus, from three tool families: ACEditor (7),
Epsilon (8, 9), and the EMF SACM implementation wrwei/SACM as SysMLine and SysMini use it
(12, 13). `cargo test -p xtask --test corpus` checks every fact below (`lib_compat_005_*`);
without the corpus, those tests are skipped.

For each of the five files:
- a tolerant read opens it, with one SACM-XMI-015 Warning for EMF's XMI namespace;
- `validate` finds no reference that names nothing (no SACM-REF-001);
- a tolerant write reads back with no difference (`compare::semantic_compare`).

Files 12 and 13 type SACM roles with GSN classes of `http://acwg.org/3.0/gsn`. The tests read
them with the declarations an application would give (`ReadOptions::with_subclass`,
LIB-COMPAT-017):
- Module as ArgumentPackage;
- Goal, Context, Assumption, Justification and AwayGoal as Claim;
- Strategy as ArgumentReasoning;
- Solution as ArtifactReference;
- SupportedBy as AssertedInference;
- InContextOf as AssertedContext.

Without them, a tolerant read keeps each such element whole (the report above).

| # | Read with | Elements read | Of them, GSN subclasses | EMF paths read | Kept whole | Strict write |
|---|---|---|---|---|---|---|
| 7 | default options | 81 | — | 35 of 35 | none | refused: 20 elements hold preserved content |
| 8 | default options | 1 | — | none | 39 SACM 2.0 `argumentAsset` | refused: 1 element |
| 9 | default options | 1 | — | none | 4 SACM 2.0 `argumentAsset` | refused: 1 element |
| 12 | the declarations | 139 | 46 | 46 of 46 | none | refused: 46 elements |
| 13 | the declarations | 75 | 24 | 23 of 23 | 1 `artifactProperty` | refused: 25 elements |

Two families, ACEditor and wrwei/SACM, read as SACM 2.3 elements. The Epsilon files hold SACM
2.0 shapes: they open and write back without loss, but are kept, not read.

It does not claim:
- **Conformance.** These files are not SACM 2.3 XMI, and tolerant mode reads them
  (D-STRICT). None writes in strict mode: each keeps content SACM 2.3 lacks, so
  `sacm convert` refuses it (discarding that content is LIB-EDIT-016, deferred).
- **Validity.** Only dangling references are checked. `validate` reports other Errors on
  files 7, 8, 12 and 13.
- **Files 1 to 6:** Assurance Forge, by this crate's author, and a file no tool wrote.
  **Files 10 and 11:** refused, LIB-COMPAT-014. **Files 14 and 15:** refused, LIB-COMPAT-009.

No third-party file is committed (D-CORPUS). The hand-written fixtures
`crates/sacm/tests/data/compat/emf-dialect.xmi` and `emf-subclasses.xmi` reproduce these
dialects in the committed tests.

## Adding a file

1. Pin a raw URL to a commit; record its licence, tool and namespace here.
2. Add it to `CORPUS` with its SHA-256; run `cargo xtask fetch-corpus` and the test.
3. Paste the printed report; map every new refusal, Error or loss to a row or a note.
