//! Download the reference artifacts, pinned by SHA-256: the OMG SACM 2.3
//! sources and the XMI 2.5.1 specification they reference ([`ARTIFACTS`]),
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
//! [`fetch_into`] and [`fetch_suite`] hold the policy and take the downloader
//! (and the extractor) as parameters, so they can be exercised without the
//! network; [`cmd_fetch`] and [`cmd_fetch_suite`] run them with
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
    /// Human-readable description, written by [`cmd_fetch`].
    pub what: &'static str,
}

/// The reference artifacts: the SACM 2.3 sources in source-priority order,
/// then XMI 2.5.1, which they reference.
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
/// or the error of `extract`. The verified archive stays in place after a
/// failed extraction, so the next call extracts again.
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
    extract(&dest.join(artifact.file_name), dest)?;
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
    let name = archive.strip_prefix(dir).unwrap_or(archive);
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(name)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("could not run tar (is it installed?): {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "tar -xzf {} in {} failed: {status}",
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
    // One artifact at a time, so each line is written as soon as it is known.
    for artifact in ARTIFACTS {
        let outcomes = fetch_into(dest, std::slice::from_ref(artifact), &mut curl_download)?;
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
    cmd_fetch_suite(suite_dest, out)
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
