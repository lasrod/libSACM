//! Download the OMG reference artifacts, pinned by SHA-256: the SACM 2.3
//! sources and the XMI 2.5.1 specification they reference.
//!
//! The artifacts are OMG documents and are not redistributed in this
//! repository; each contributor fetches them into a git-ignored directory.
//! Pinning the hash means every agent and human reads the same bytes, and a
//! silent upstream change is reported rather than absorbed.
//!
//! [`fetch_into`] holds the policy and takes the downloader as a parameter, so
//! it can be exercised without the network; [`cmd_fetch`] runs it with
//! [`curl_download`].

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

/// What [`fetch_into`] did for one artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The file was already present with the pinned hash; nothing was downloaded.
    AlreadyPresent,
    /// The file was downloaded, its hash verified, and moved into place.
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
/// [`curl_download`], writing one line per artifact to `out`.
///
/// # Errors
///
/// Returns the first error of [`fetch_into`], or an error when writing to
/// `out` fails.
pub fn cmd_fetch(dest: &Path, out: &mut dyn Write) -> Result<(), String> {
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
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(sha256_hex(&bytes))
}
