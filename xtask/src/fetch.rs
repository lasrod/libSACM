//! Download the OMG SACM 2.3 reference artifacts, pinned by SHA-256.
//!
//! The artifacts are OMG documents and are not redistributed in this
//! repository; each contributor fetches them into a git-ignored directory.
//! Pinning the hash means every agent and human reads the same bytes, and a
//! silent upstream change is reported rather than absorbed.

use std::fs;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

pub(crate) const DEFAULT_DEST: &str = "third_party/sacm-2.3";

/// One pinned reference artifact.
pub(crate) struct Artifact {
    pub(crate) file_name: &'static str,
    pub(crate) url: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) what: &'static str,
}

/// The normative SACM 2.3 sources, in source-priority order.
pub(crate) const ARTIFACTS: &[Artifact] = &[
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
];

pub(crate) fn cmd_fetch(dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    for artifact in ARTIFACTS {
        let path = dest.join(artifact.file_name);
        if path.exists() {
            let actual = sha256_file(&path)?;
            if actual == artifact.sha256 {
                println!("ok       {} ({})", path.display(), artifact.what);
                continue;
            }
            return Err(format!(
                "{} exists but its SHA-256 is {actual}, expected {}; delete it to re-fetch",
                path.display(),
                artifact.sha256
            ));
        }
        let partial = dest.join(format!("{}.part", artifact.file_name));
        let status = Command::new("curl")
            .args([
                "--location",
                "--fail",
                "--silent",
                "--show-error",
                "--output",
            ])
            .arg(&partial)
            .arg(artifact.url)
            .status()
            .map_err(|e| format!("could not run curl (is it installed?): {e}"))?;
        if !status.success() {
            let _ = fs::remove_file(&partial);
            return Err(format!("download failed: {}", artifact.url));
        }
        let actual = sha256_file(&partial)?;
        if actual != artifact.sha256 {
            let _ = fs::remove_file(&partial);
            return Err(format!(
                "{} changed upstream: SHA-256 {actual}, pinned {}. Review the new \
                 revision before updating the pin in xtask/src/fetch.rs",
                artifact.url, artifact.sha256
            ));
        }
        fs::rename(&partial, &path).map_err(|e| format!("rename {}: {e}", partial.display()))?;
        println!("fetched  {} ({})", path.display(), artifact.what);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    Ok(hex(&Sha256::digest(&bytes)))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_tool_002_pins_are_well_formed_and_unique() {
        let mut names = std::collections::HashSet::new();
        for artifact in ARTIFACTS {
            assert_eq!(
                artifact.sha256.len(),
                64,
                "LIB-TOOL-002: {} pin",
                artifact.file_name
            );
            assert!(
                artifact
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "LIB-TOOL-002: {} pin must be lowercase hex",
                artifact.file_name
            );
            assert!(
                artifact.url.starts_with("https://www.omg.org/"),
                "LIB-TOOL-002: OMG source"
            );
            assert!(
                names.insert(artifact.file_name),
                "LIB-TOOL-002: duplicate file name"
            );
        }
    }

    #[test]
    fn lib_tool_002_hashes_known_input() {
        assert_eq!(
            hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "LIB-TOOL-002: SHA-256 of 'abc' (FIPS 180-2 test vector)"
        );
    }
}
