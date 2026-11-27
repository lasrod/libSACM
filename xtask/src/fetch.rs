//! Download the reference artifacts, pinned by SHA-256: the OMG SACM 2.3
//! sources, the XMI 2.5.1 specification they reference and its normative
//! XML Schema, XMI.xsd, XML Schema Part 2: Datatypes and XML Schema Part 1:
//! Structures ([`ARTIFACTS`]),
//! and the W3C XML Conformance Test Suite ([`XMLCONF`]).
//!
//! The artifacts are not redistributed in this repository; each contributor
//! fetches them into a git-ignored directory. Pinning the hash means every
//! agent and human reads the same bytes, and a silent upstream change is
//! reported rather than absorbed.
//!
//! The W3C suite is fetch-only. Its parts carry different terms: the W3C
//! Software License, "internal use" for James Clark's xmltest, and "All
//! rights reserved" for the IBM and Sun parts. So it is fetched at test time
//! into `third_party/xmlconf/` and extracted there, it is never committed,
//! and no case file is copied into the repository (LIB-SEC-006).
//!
//! The corpus of SACM files written by other tools ([`CORPUS`]) is fetch-only
//! too. Several of its files carry no licence, so `cargo xtask fetch-corpus`
//! fetches them into the git-ignored `third_party/corpus/`, they are never
//! committed, and nothing from them is copied into the repository
//! (LIB-COMPAT-015).
//!
//! [`fetch_into`] and [`fetch_suite`] hold the policy and take the downloader
//! (and the extractor) as parameters, so they can be exercised without the
//! network; [`fetch_each`] adds one line of output per artifact. [`cmd_fetch`],
//! [`cmd_fetch_suite`] and [`cmd_fetch_corpus`] run them with
//! [`curl_download`] and [`tar_extract`].

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::output_error;

/// Where `cargo xtask fetch-spec` puts the artifacts, relative to the
/// repository root. The directory is git-ignored.
pub const DEFAULT_DEST: &str = "third_party/sacm-2.3";

/// One pinned reference artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Artifact {
    /// File name under the destination directory.
    pub file_name: &'static str,
    /// Where the artifact is downloaded from.
    pub url: &'static str,
    /// Expected SHA-256 of the file, as lowercase hex.
    pub sha256: &'static str,
    /// Human-readable description, written by [`fetch_each`].
    pub what: &'static str,
}

/// The reference artifacts: the SACM 2.3 sources in source-priority order,
/// then XMI 2.5.1, which they reference, and its XMI.xsd (LIB-TOOL-021), and
/// XML Schema Part 2 (LIB-TOOL-022), whose boolean, `NCName` and ID, IDREFS,
/// list and whiteSpace rules the crate relies on, and XML Schema Part 1
/// (LIB-TOOL-024), whose `skip` wildcard and ID table it relies on.
pub const ARTIFACTS: &[Artifact] = &[
    Artifact {
        file_name: "SACM-2.3-formal-23-05-08.pdf",
        url: "https://www.omg.org/spec/SACM/2.3/PDF",
        sha256: "8004a2ee2e3469552ae199deea22e44ec86a1d4769948bdce4782c7220de3012",
        what: "SACM 2.3 formal specification (formal/23-05-08)",
    },
    Artifact {
        file_name: "SACM-2.3-ptc-22-03-13.xml",
        url: "https://www.omg.org/spec/SACM/20220301/SACM.xml",
        sha256: "4bc12020fe38018fe24d39e56ec04f2fc9190fe7cf1161d85452fda5f3fbeb1e",
        what: "SACM 2.3 normative machine-readable metamodel (ptc/22-03-13)",
    },
    Artifact {
        file_name: "XMI-2.5.1-formal-15-06-07.pdf",
        url: "https://www.omg.org/spec/XMI/2.5.1/PDF",
        sha256: "2bf97c670f128affc9ed5dc202ef748101afd0eee8ee662e8763da6092c80c1b",
        what: "XMI 2.5.1 formal specification (formal/15-06-07)",
    },
    Artifact {
        file_name: "XMI-2.5.1-XMI.xsd",
        url: "https://www.omg.org/spec/XMI/20131001/XMI.xsd",
        sha256: "7b228718d35a15a8e2fb99f078d36eee4b23b7a2cd405e7ac81b9d27bf2fab5d",
        what: "XMI 2.5.1 normative XML Schema (XMI.xsd)",
    },
    Artifact {
        file_name: "XMLSchema-2-REC-20041028-datatypes.html",
        url: "https://www.w3.org/TR/2004/REC-xmlschema-2-20041028/datatypes.html",
        sha256: "980de872aa2c50013d5202176eb85eef3f183a7df635fdcb6b4aa34c1c12eb72",
        what: "XML Schema Part 2: Datatypes Second Edition (W3C Recommendation 28 October 2004)",
    },
    Artifact {
        file_name: "XMLSchema-1-REC-20041028-structures.html",
        url: "https://www.w3.org/TR/2004/REC-xmlschema-1-20041028/structures.html",
        sha256: "92bfd116316332b1153941436430afbdb2040e036d5fb2d21d4c8fba36a7765a",
        what: "XML Schema Part 1: Structures Second Edition (W3C Recommendation 28 October 2004)",
    },
];

/// The W3C XML Conformance Test Suite 20130923, which the LIB-SEC-006 test
/// runs through `sacm::xmi::read`. It is fetched only; see the module docs.
pub const XMLCONF: Artifact = Artifact {
    file_name: "xmlts20130923.tar.gz",
    url: "https://www.w3.org/XML/Test/xmlts20130923.tar.gz",
    sha256: "9b61db9f5dbffa545f4b8d78422167083a8568c59bd1129f94138f936cf6fc1f",
    what: "W3C XML Conformance Test Suite 20130923",
};

/// Where `cargo xtask fetch-spec` puts and extracts [`XMLCONF`], relative to
/// the repository root. The directory is git-ignored.
pub const XMLCONF_DEST: &str = "third_party/xmlconf";

/// The file whose presence under the suite's destination shows that the
/// archive has been extracted: the suite's master catalog.
const XMLCONF_MARKER: &str = "xmlconf/xmlconf.xml";

/// Where `cargo xtask fetch-corpus` puts [`CORPUS`], relative to the
/// repository root. The directory is git-ignored.
pub const CORPUS_DEST: &str = "third_party/corpus";

/// SACM files that tools other than this crate wrote, each at a URL pinned to
/// a commit, which the LIB-COMPAT-015 test reads in both modes. They are
/// fetched only; see the module docs. `docs/compat/corpus.md` lists their
/// licences, writers and namespaces, and the report of the test.
pub const CORPUS: &[Artifact] = &[
    Artifact {
        file_name: "af-examples-kitchen-blender.sacm",
        url: "https://raw.githubusercontent.com/lasrod/assurance-forge-examples/7ec4add92d1892ace8c09509d5e7c92b7a65b903/projects/kitchen-blender/arguments/main.sacm",
        sha256: "8be670f7ec47eccbb94ab3ec056f13bda769766ddf3799905d9eb3c49d528ee1",
        what: "Assurance Forge example project kitchen-blender",
    },
    Artifact {
        file_name: "af-examples-gsn-pattern-decorators.sacm",
        url: "https://raw.githubusercontent.com/lasrod/assurance-forge-examples/7ec4add92d1892ace8c09509d5e7c92b7a65b903/projects/gsn-pattern-decorators/arguments/main.sacm",
        sha256: "79e2781449a0cbe80bff89ca55a0db0e6e0a77088d479317213246f799fe2f4a",
        what: "Assurance Forge example project gsn-pattern-decorators",
    },
    Artifact {
        file_name: "af-testdata-sample.sacm.xml",
        url: "https://raw.githubusercontent.com/lasrod/assurance-forge/c1f62854e5e78c1225315588868a693c67564c3a/testdata/sacm/sample.sacm.xml",
        sha256: "bdd63b30cc19f31deedf2bb898010b322127fbefd2bff2ce5ec5bd99c0bf0711",
        what: "Assurance Forge test data, legacy app dialect: sample",
    },
    Artifact {
        file_name: "af-testdata-oasc-ja.xml",
        url: "https://raw.githubusercontent.com/lasrod/assurance-forge/c1f62854e5e78c1225315588868a693c67564c3a/testdata/sacm/oasc-ja.xml",
        sha256: "64890f19b4dbe6b18ec6535a009013c51e8953441df8cc78d32e3c9d6501964f",
        what: "Assurance Forge test data, legacy app dialect: OASC in English and Japanese",
    },
    Artifact {
        file_name: "af-testdata-safety-case-sanitized-strict.sacm.xml",
        url: "https://raw.githubusercontent.com/lasrod/assurance-forge/c1f62854e5e78c1225315588868a693c67564c3a/testdata/sacm/safety-case-sanitized-strict.sacm.xml",
        sha256: "8d1ca006fbee2758b4012a8abb08625d39e59a9f36ff8bcff76d9e02f5587645",
        what: "Assurance Forge test data, legacy app dialect: a sanitized safety case",
    },
    Artifact {
        file_name: "oasc-open-autonomy-safety-case.sacm.xml",
        url: "https://raw.githubusercontent.com/EdgeCaseResearch/oasc/abf48ef74d07ff485ef759c5f6f5390953ca45c6/open-autonomy-safety-case.sacm.xml",
        sha256: "6e4a4cd48a50986bddfb30a0dec2cf6929f05fd70ec0c39976f055096dcc0e8e",
        what: "Edge Case Research Open Autonomy Safety Case, SACM 2.2 XML",
    },
    Artifact {
        file_name: "aceditor-hbs-default.sacm2",
        url: "https://raw.githubusercontent.com/LuisFelipeAN/aceditor-mbac-results/2bdad068b033f84b893400a9a7ce9fb040649554/HBS/default.sacm2",
        sha256: "d3a9afd14b14d57adaf103428a7362e74dfce7cd55bfb7e59cd48cbe5c91e2dc",
        what: "ACEditor MBAC output, HBS case study",
    },
    Artifact {
        file_name: "epsilon-sacm-example.model",
        url: "https://raw.githubusercontent.com/epsilonlabs/SACM-UML-Profile/f7222ed99140522f41be1426b39e0e1abab53618/gsn.metamodel/model/SACM_Example.model",
        sha256: "7fa6931eeeee087ce98ce2e75c9a1b5cbfaee09442654a37706984dc98b7a516",
        what: "Epsilon SACM 2.0 EMF example",
    },
    Artifact {
        file_name: "epsilon-untitled.model",
        url: "https://raw.githubusercontent.com/epsilonlabs/SACM-UML-Profile/f7222ed99140522f41be1426b39e0e1abab53618/sacm.emf/model/untitled.model",
        sha256: "66570d374408655cc8c207d755e9cdbd0d4c1582049ae6a58bd07bcf0a209955",
        what: "Epsilon SACM 2.0 EMF model with xmi:id",
    },
    Artifact {
        file_name: "justification-factory-sample-sacm.xmi",
        url: "https://raw.githubusercontent.com/JustificationFactory/JustificationDiagramModelToSACM/12abf127f6bea45fce56e8ee4154e7baecf17ae1/SACM-generated/sample-sacm.xmi",
        sha256: "76e1a3d8fe7a352c884088e0fdfa87a74f736e17df331aa7c291db9e745c4571",
        what: "JustificationFactory ATL output, sample",
    },
    Artifact {
        file_name: "justification-factory-sacm-derivation.xmi",
        url: "https://raw.githubusercontent.com/JustificationFactory/JustificationDiagramModelToSACM/12abf127f6bea45fce56e8ee4154e7baecf17ae1/SACM-generated/sacm-derivation.xmi",
        sha256: "0d4b9f1155cac46594c0d8dfc93558cbad1974e49198d5005ca672ac43031222",
        what: "JustificationFactory ATL output, derivation",
    },
    Artifact {
        file_name: "sysmline-easy-example.assurancecase",
        url: "https://raw.githubusercontent.com/Ruizhe-Yang/SysMLine/2658f087c5cd303019aa1df5a17b26bc4ded2346/dut.cs.sysmline/model/SACM/EasyExample.assurancecase",
        sha256: "3286379e9e1d472bc634d7a79b2ded3bb9298455a330a3aadbaf1644e3898598",
        what: "SysMLine EMF SACM 2.2 model with GSN types",
    },
    Artifact {
        file_name: "sysmini-quan.assurancecase",
        url: "https://raw.githubusercontent.com/Ruizhe-Yang/SysMini/313d44ed25b46e8c9af7f737359a87d87fbb7c10/org.omg.sysmini/model/SACM/quan.assurancecase",
        sha256: "0aaeec3ff8b6833dfae68f8974472b41d4d69450843e31156b8d72a2d67921a7",
        what: "SysMini EMF SACM 2.2 model with artifacts",
    },
    Artifact {
        file_name: "mobstr-safetycase.integration",
        url: "https://raw.githubusercontent.com/panorama-research/mobstr-dataset/14f86e018ee6c0ce2f0a36b9ff25ae776e2c12dd/org.panorama-research.mobstr.safetycase/mobstr-safetycase.integration",
        sha256: "e2bb673a97fd0dc11dc3e8ecf19f18b288c1bcafb38ed2788d8eb36277d947c3",
        what: "MobSTr ODE container with a SACM assurance case",
    },
    Artifact {
        file_name: "deis-etcs.model",
        url: "https://raw.githubusercontent.com/DEIS-Project-EU/DDI-Scripting-Tools/a234b7f25500080a421685d4692e8a5b20033b2e/Examples/ETCS/etcs.model",
        sha256: "72d2f64261188c267b7617c71de8bcb46dcd989d39d4e33643e584e08457a6a9",
        what: "DEIS ETCS ODE container with SACM argumentation",
    },
];

/// What [`fetch_into`] did for one artifact, or [`fetch_suite`] for the suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The file was already present with the pinned hash, and for the suite
    /// already extracted; nothing was downloaded or extracted.
    AlreadyPresent,
    /// The file was downloaded, its hash verified, and moved into place; for
    /// the suite, the archive was downloaded or the suite extracted again.
    Fetched,
}

/// The SHA-256 of `bytes`, as 64 lowercase hex digits.
///
/// ```
/// assert_eq!(
///     xtask::fetch::sha256_hex(b"abc"),
///     "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
/// );
/// ```
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Make every artifact present in `dest` with its pinned hash.
///
/// For each artifact, in order:
///
/// - an existing file with the pinned hash is [`Outcome::AlreadyPresent`] and
///   `download` is not called;
/// - an existing file with another hash is an error, and the file is left
///   untouched;
/// - otherwise `download(url, dest/<file_name>.part)` is called. On a download
///   error or a hash mismatch the `.part` file is deleted and the call fails;
///   on a match the `.part` file is renamed to `dest/<file_name>`
///   ([`Outcome::Fetched`]). Nothing appears at the final path before its
///   hash is verified.
///
/// Processing stops at the first error; artifacts already handled stay in
/// place.
///
/// # Errors
///
/// Returns a message when `dest` cannot be created, a file cannot be read or
/// renamed, an existing file has the wrong hash (naming the file), `download`
/// fails (naming the URL and the downloader's message), or a download does not
/// match its pin (naming the received and the pinned hash).
pub fn fetch_into(
    dest: &Path,
    artifacts: &[Artifact],
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
) -> Result<Vec<Outcome>, String> {
    fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut outcomes = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let path = dest.join(artifact.file_name);
        if path.exists() {
            let actual = sha256_file(&path)?;
            if actual == artifact.sha256 {
                outcomes.push(Outcome::AlreadyPresent);
                continue;
            }
            return Err(format!(
                "{} exists but its SHA-256 is {actual}, expected {}; delete it to re-fetch",
                path.display(),
                artifact.sha256
            ));
        }
        let partial = dest.join(format!("{}.part", artifact.file_name));
        let fetched = download_verified(artifact, &partial, download).and_then(|()| {
            fs::rename(&partial, &path).map_err(|e| format!("rename {}: {e}", partial.display()))
        });
        if let Err(message) = fetched {
            // Best effort: the download may not have created the file at all.
            let _ = fs::remove_file(&partial);
            return Err(message);
        }
        outcomes.push(Outcome::Fetched);
    }
    Ok(outcomes)
}

/// Download `artifact` to `partial` and check its hash. The caller removes
/// `partial` on failure.
fn download_verified(
    artifact: &Artifact,
    partial: &Path,
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
) -> Result<(), String> {
    download(artifact.url, partial)
        .map_err(|e| format!("download failed: {}: {e}", artifact.url))?;
    let actual = sha256_file(partial)?;
    if actual != artifact.sha256 {
        return Err(format!(
            "{} changed upstream: SHA-256 {actual}, pinned {}. Review the new \
             revision before updating the pin in xtask/src/fetch.rs",
            artifact.url, artifact.sha256
        ));
    }
    Ok(())
}

/// Make the W3C XML Conformance Test Suite present and extracted in `dest`.
///
/// The archive [`XMLCONF`] is fetched by [`fetch_into`], with the same hash
/// and partial-file policy. Only once its hash matches is
/// `extract(dest/<archive>, dest)` called, and then only when the archive was
/// just downloaded or `dest/xmlconf/xmlconf.xml` is missing. With both the
/// archive and that file present, nothing is downloaded or extracted
/// ([`Outcome::AlreadyPresent`]); otherwise the result is
/// [`Outcome::Fetched`].
///
/// # Errors
///
/// Returns the error of [`fetch_into`], in which case `extract` is not called,
/// or the error of `extract`. After a failed extraction the verified archive
/// stays in place and `dest/xmlconf/xmlconf.xml` is removed (best effort),
/// even if the extraction wrote it, so the next call extracts again.
pub fn fetch_suite(
    dest: &Path,
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
    extract: &mut dyn FnMut(&Path, &Path) -> Result<(), String>,
) -> Result<Outcome, String> {
    fetch_suite_from(dest, &XMLCONF, download, extract)
}

/// [`fetch_suite`] for another pinned `artifact`, so that the policy can be
/// exercised offline with an archive of known bytes.
///
/// The policy is the same: `artifact` is fetched by [`fetch_into`], and only
/// once its hash matches is `extract(dest/<file_name>, dest)` called, when
/// the archive was just downloaded or `dest/xmlconf/xmlconf.xml` is missing.
/// That file shows the extraction for every artifact. With both present the
/// result is [`Outcome::AlreadyPresent`]; otherwise [`Outcome::Fetched`].
///
/// # Errors
///
/// As [`fetch_suite`].
pub fn fetch_suite_from(
    dest: &Path,
    artifact: &Artifact,
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
    extract: &mut dyn FnMut(&Path, &Path) -> Result<(), String>,
) -> Result<Outcome, String> {
    let outcomes = fetch_into(dest, std::slice::from_ref(artifact), download)?;
    let extracted = dest.join(XMLCONF_MARKER).is_file();
    if outcomes == [Outcome::AlreadyPresent] && extracted {
        return Ok(Outcome::AlreadyPresent);
    }
    if let Err(error) = extract(&dest.join(artifact.file_name), dest) {
        // An extraction that wrote the marker and then failed must not pass
        // for a complete one next time. Best effort: the error reported is
        // the extraction's.
        let _ = fs::remove_file(dest.join(XMLCONF_MARKER));
        return Err(error);
    }
    Ok(Outcome::Fetched)
}

/// Extract the gzip-compressed tar `archive` into `dir` with the system
/// `tar`, as `tar -xzf <archive>` run in `dir`.
///
/// The archive is named by its path under `dir` (or as given, if it is not
/// under `dir`), because GNU tar reads a name such as `C:\archive.tar.gz` as
/// a remote host.
///
/// # Errors
///
/// Returns a message when `tar` cannot be run (naming it) or exits
/// unsuccessfully (naming the archive, `dir` and the exit status).
pub fn tar_extract(archive: &Path, dir: &Path) -> Result<(), String> {
    extract_with("tar", archive, dir)
}

/// Extract the gzip-compressed tar `archive` into `dir` with `program`, as
/// `<program> -xzf <archive>` run in `dir`.
///
/// [`tar_extract`] is this with `"tar"`. The archive is named as
/// [`tar_extract`] names it.
///
/// # Errors
///
/// Returns a message starting `could not run <program>` when `program` cannot
/// be run, or `<program> -xzf <archive> in <dir> failed: <status>` when it
/// exits unsuccessfully.
pub fn extract_with(program: &str, archive: &Path, dir: &Path) -> Result<(), String> {
    let name = archive.strip_prefix(dir).unwrap_or(archive);
    let status = Command::new(program)
        .arg("-xzf")
        .arg(name)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("could not run {program} (is it installed?): {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} -xzf {} in {} failed: {status}",
            name.display(),
            dir.display()
        ))
    }
}

/// Download `url` to `out` with the `curl` command-line tool, following
/// redirects and failing on HTTP errors.
///
/// # Errors
///
/// Returns a message when `curl` cannot be run or exits unsuccessfully. `out`
/// may then hold a partial file; [`fetch_into`] removes it.
pub fn curl_download(url: &str, out: &Path) -> Result<(), String> {
    let status = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(out)
        .arg(url)
        .status()
        .map_err(|e| format!("could not run curl (is it installed?): {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("curl {status}"))
    }
}

/// `cargo xtask fetch-spec`: fetch [`ARTIFACTS`] into `dest` with
/// [`curl_download`], then the W3C suite into `suite_dest` as
/// [`cmd_fetch_suite`] does, writing one line per artifact to `out`.
///
/// # Errors
///
/// Returns the first error of [`fetch_into`] or [`cmd_fetch_suite`], or an
/// error when writing to `out` fails. The suite is not fetched when an OMG
/// artifact fails.
pub fn cmd_fetch(dest: &Path, suite_dest: &Path, out: &mut dyn Write) -> Result<(), String> {
    fetch_each(dest, ARTIFACTS, &mut curl_download, out)?;
    cmd_fetch_suite(suite_dest, out)
}

/// Make every artifact present in `dest` with its pinned hash, as
/// [`fetch_into`] does, one artifact at a time, writing a line to `out` for
/// each as soon as it is handled: `ok       <dest>/<file_name> (<what>)` for
/// one already present with its pin, `fetched  <dest>/<file_name> (<what>)`
/// for one just downloaded and verified.
///
/// With every artifact present with its pin, `download` is never called, so
/// the call needs no network.
///
/// ```
/// use std::path::Path;
/// use xtask::fetch::{Artifact, fetch_each};
///
/// let dir = std::env::temp_dir().join(format!("fetch-each-doc-{}", std::process::id()));
/// # let _ = std::fs::remove_dir_all(&dir);
/// let abc = Artifact {
///     file_name: "abc.txt",
///     url: "https://example.org/abc.txt",
///     sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
///     what: "three letters",
/// };
/// let mut download = |_url: &str, part: &Path| std::fs::write(part, b"abc").map_err(|e| e.to_string());
/// let mut out = Vec::new();
/// fetch_each(&dir, &[abc], &mut download, &mut out)?;
/// fetch_each(&dir, &[abc], &mut download, &mut out)?;
/// let text = String::from_utf8_lossy(&out).into_owned();
/// let lines: Vec<&str> = text.lines().collect();
/// assert!(lines[0].starts_with("fetched  ") && lines[0].ends_with("abc.txt (three letters)"));
/// assert!(lines[1].starts_with("ok       "));
/// # std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// Returns the first error of [`fetch_into`], after the lines of the
/// artifacts handled before it, which stay in place, or an error when writing
/// to `out` fails.
pub fn fetch_each(
    dest: &Path,
    artifacts: &[Artifact],
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    // One artifact at a time, so each line is written as soon as it is known.
    for artifact in artifacts {
        let outcomes = fetch_into(dest, std::slice::from_ref(artifact), download)?;
        let label = match outcomes.first() {
            Some(Outcome::AlreadyPresent) => "ok      ",
            Some(Outcome::Fetched) => "fetched ",
            None => continue,
        };
        writeln!(
            out,
            "{label} {} ({})",
            dest.join(artifact.file_name).display(),
            artifact.what
        )
        .map_err(output_error)?;
    }
    Ok(())
}

/// `cargo xtask fetch-corpus`: fetch [`CORPUS`] into `dest` with
/// [`fetch_each`] and [`curl_download`], writing one line per file to `out`.
///
/// # Errors
///
/// As [`fetch_each`]: processing stops at the first file that cannot be
/// fetched or does not match its pin.
pub fn cmd_fetch_corpus(dest: &Path, out: &mut dyn Write) -> Result<(), String> {
    fetch_each(dest, CORPUS, &mut curl_download, out)
}

/// `cargo xtask fetch-spec --only xmlconf`: fetch and extract [`XMLCONF`]
/// into `dest` with [`fetch_suite`], [`curl_download`] and [`tar_extract`],
/// writing one line to `out`.
///
/// # Errors
///
/// Returns the error of [`fetch_suite`], or an error when writing to `out`
/// fails.
pub fn cmd_fetch_suite(dest: &Path, out: &mut dyn Write) -> Result<(), String> {
    let label = match fetch_suite(dest, &mut curl_download, &mut tar_extract)? {
        Outcome::AlreadyPresent => "ok      ",
        Outcome::Fetched => "fetched ",
    };
    writeln!(
        out,
        "{label} {} ({})",
        dest.join(XMLCONF.file_name).display(),
        XMLCONF.what
    )
    .map_err(output_error)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(sha256_hex(&bytes))
}
