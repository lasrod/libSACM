//! Acceptance tests for hash-pinned fetching of the SACM 2.3 sources
//! (LIB-TOOL-002).
//!
//! `fetch_into` is driven with a fake downloader, so no test touches the
//! network. `curl_download` is driven with the real `curl` on `file://` URLs
//! only. Every test works in its own directory under the system temporary
//! directory and removes it afterwards.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use std::fmt::Write as _;

use xtask::fetch::{ARTIFACTS, Artifact, Outcome, curl_download, fetch_into, sha256_hex};

/// The FIPS 180-2 test input and its SHA-256, so the pins in these tests do
/// not depend on the hashing under test.
const ABC: &[u8] = b"abc";
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

const ARTIFACT: Artifact = Artifact {
    file_name: "reference.bin",
    url: "https://example.invalid/reference",
    sha256: ABC_SHA256,
    what: "test artifact",
};

const OTHER: Artifact = Artifact {
    file_name: "other.bin",
    url: "https://example.invalid/other",
    sha256: ABC_SHA256,
    what: "second test artifact",
};

/// A fresh directory, removed on drop.
#[derive(Debug)]
struct TempDir(PathBuf);

impl TempDir {
    fn new(test: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path =
            std::env::temp_dir().join(format!("xtask-{test}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("LIB-TOOL-002: create {}: {e}", path.display()));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Names of the entries in the directory, sorted.
    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------
// Ported from the inline tests in xtask/src/fetch.rs
// ---------------------------------------------------------------------------

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
        sha256_hex(ABC),
        ABC_SHA256,
        "LIB-TOOL-002: SHA-256 of 'abc' (FIPS 180-2 test vector)"
    );
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "LIB-TOOL-002: SHA-256 of the empty input"
    );
}

// ---------------------------------------------------------------------------
// Acceptance criterion 2: hash before move; a mismatch deletes the partial file
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_002_fresh_download_matching_the_pin_is_moved_into_place() {
    let dir = TempDir::new("lib-tool-002-fresh");
    let final_path = dir.file("reference.bin");
    let mut calls: Vec<(String, PathBuf)> = Vec::new();
    let mut final_existed_during_download = false;

    let result = fetch_into(dir.path(), &[ARTIFACT], &mut |url: &str, out: &Path| {
        calls.push((url.to_owned(), out.to_path_buf()));
        final_existed_during_download |= final_path.exists();
        fs::write(out, ABC).map_err(|e| e.to_string())
    });

    assert_eq!(
        result,
        Ok(vec![Outcome::Fetched]),
        "LIB-TOOL-002: a matching download is Fetched"
    );
    assert_eq!(
        calls,
        [(ARTIFACT.url.to_owned(), dir.file("reference.bin.part"))],
        "LIB-TOOL-002: downloaded once, from the pinned URL, to <name>.part"
    );
    assert!(
        !final_existed_during_download,
        "LIB-TOOL-002: nothing is at the final path until the hash is verified"
    );
    assert_eq!(
        fs::read(&final_path).ok().as_deref(),
        Some(ABC),
        "LIB-TOOL-002: the verified bytes are in place"
    );
    assert_eq!(
        dir.entries(),
        ["reference.bin"],
        "LIB-TOOL-002: no .part file is left behind"
    );
}

#[test]
fn lib_tool_002_hash_mismatch_deletes_the_partial_file_and_fails() {
    let dir = TempDir::new("lib-tool-002-mismatch");

    let result = fetch_into(dir.path(), &[ARTIFACT], &mut |_url: &str, out: &Path| {
        fs::write(out, b"changed upstream").map_err(|e| e.to_string())
    });

    let err = result.expect_err("LIB-TOOL-002: a hash mismatch is an error");
    assert!(
        err.contains(ABC_SHA256),
        "LIB-TOOL-002: the error reports the pinned hash, got {err:?}"
    );
    assert!(
        err.contains(&sha256_hex(b"changed upstream")),
        "LIB-TOOL-002: the error reports the hash actually received, got {err:?}"
    );
    assert_eq!(
        dir.entries(),
        Vec::<String>::new(),
        "LIB-TOOL-002: neither a .part file nor a final file is left"
    );
}

#[test]
fn lib_tool_002_downloader_error_deletes_the_partial_file_and_fails() {
    let dir = TempDir::new("lib-tool-002-download-error");

    let result = fetch_into(dir.path(), &[ARTIFACT], &mut |_url: &str, out: &Path| {
        // A transfer that dies half way leaves a partial file behind.
        fs::write(out, b"ab").map_err(|e| e.to_string())?;
        Err("connection reset".to_owned())
    });

    let err = result.expect_err("LIB-TOOL-002: a failed download is an error");
    assert!(
        err.contains(ARTIFACT.url) || err.contains("connection reset"),
        "LIB-TOOL-002: the error says what failed, got {err:?}"
    );
    assert_eq!(
        dir.entries(),
        Vec::<String>::new(),
        "LIB-TOOL-002: neither a .part file nor a final file is left"
    );
}

// ---------------------------------------------------------------------------
// Acceptance criterion 3: existing files
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_002_existing_file_with_pinned_hash_is_not_downloaded_again() {
    let dir = TempDir::new("lib-tool-002-present");
    fs::write(dir.file("reference.bin"), ABC).expect("LIB-TOOL-002: write fixture");
    let mut calls = 0_usize;

    let result = fetch_into(dir.path(), &[ARTIFACT], &mut |_url: &str, _out: &Path| {
        calls += 1;
        Err("must not be called".to_owned())
    });

    assert_eq!(
        result,
        Ok(vec![Outcome::AlreadyPresent]),
        "LIB-TOOL-002: a file with the pinned hash is AlreadyPresent"
    );
    assert_eq!(calls, 0, "LIB-TOOL-002: the downloader is never called");
    assert_eq!(
        fs::read(dir.file("reference.bin")).ok().as_deref(),
        Some(ABC),
        "LIB-TOOL-002: the file is untouched"
    );
}

#[test]
fn lib_tool_002_existing_file_with_wrong_hash_is_an_error_and_left_untouched() {
    let dir = TempDir::new("lib-tool-002-tampered");
    fs::write(dir.file("reference.bin"), b"local edit").expect("LIB-TOOL-002: write fixture");
    let mut calls = 0_usize;

    let result = fetch_into(dir.path(), &[ARTIFACT], &mut |_url: &str, out: &Path| {
        calls += 1;
        fs::write(out, ABC).map_err(|e| e.to_string())
    });

    let err = result.expect_err("LIB-TOOL-002: a file with another hash is an error");
    assert!(
        err.contains("reference.bin"),
        "LIB-TOOL-002: the error names the file, got {err:?}"
    );
    assert_eq!(calls, 0, "LIB-TOOL-002: the downloader is not called");
    assert_eq!(
        fs::read(dir.file("reference.bin")).ok().as_deref(),
        Some(&b"local edit"[..]),
        "LIB-TOOL-002: the existing file is left untouched"
    );
    assert_eq!(
        dir.entries(),
        ["reference.bin"],
        "LIB-TOOL-002: no .part file is created"
    );
}

#[test]
fn lib_tool_002_outcomes_follow_artifact_order() {
    let dir = TempDir::new("lib-tool-002-order");
    fs::write(dir.file("reference.bin"), ABC).expect("LIB-TOOL-002: write fixture");
    let mut urls: Vec<String> = Vec::new();

    let result = fetch_into(
        dir.path(),
        &[ARTIFACT, OTHER],
        &mut |url: &str, out: &Path| {
            urls.push(url.to_owned());
            fs::write(out, ABC).map_err(|e| e.to_string())
        },
    );

    assert_eq!(
        result,
        Ok(vec![Outcome::AlreadyPresent, Outcome::Fetched]),
        "LIB-TOOL-002: one outcome per artifact, in order"
    );
    assert_eq!(
        urls,
        [OTHER.url],
        "LIB-TOOL-002: only the missing artifact is downloaded"
    );
    assert_eq!(
        dir.entries(),
        ["other.bin", "reference.bin"],
        "LIB-TOOL-002: both artifacts in place, no .part file"
    );
}

// ---------------------------------------------------------------------------
// curl_download, with the real `curl` on `file://` URLs
// ---------------------------------------------------------------------------
//
// `curl` is a prerequisite of `cargo xtask fetch-spec`, and it ships with the
// Linux CI runners and with Windows 10 and later. These tests do not skip when
// it is missing: they fail and say so, because a skipped test would leave
// `curl_download` untested without anyone noticing.

/// A `file://` URL for the absolute path `path`. Bytes other than unreserved
/// characters, `/` and `:` are percent-encoded; Windows separators become `/`.
fn file_url(path: &Path) -> String {
    let text = path
        .to_str()
        .unwrap_or_else(|| panic!("LIB-TOOL-002: {} is not UTF-8", path.display()))
        .replace('\\', "/");
    let mut url = String::from("file://");
    if !text.starts_with('/') {
        // `C:/...` becomes `file:///C:/...`.
        url.push('/');
    }
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
            url.push(char::from(byte));
        } else {
            let _ = write!(url, "%{byte:02X}");
        }
    }
    url
}

#[test]
fn lib_tool_002_curl_download_writes_the_bytes_at_the_url_to_the_output_path() {
    let dir = TempDir::new("lib-tool-002-curl-copy");
    // Every byte value, and line endings that a text-mode copy would change.
    let bytes: Vec<u8> = (0..=u8::MAX).chain(*b"\r\n\n\r\x1a").collect();
    let source = dir.file("source.bin");
    fs::write(&source, &bytes).expect("LIB-TOOL-002: write the source file");
    let out = dir.file("out.bin");
    let url = file_url(&source);

    let result = curl_download(&url, &out);

    assert_eq!(
        result,
        Ok(()),
        "LIB-TOOL-002: curl_download({url:?}) succeeds; an error saying that curl could \
         not be run means curl is not installed, which this test does not accept"
    );
    assert_eq!(
        fs::read(&out).ok(),
        Some(bytes),
        "LIB-TOOL-002: curl_download writes exactly the bytes at the URL to {}",
        out.display()
    );
}

#[test]
fn lib_tool_002_curl_download_reports_a_failed_transfer_as_an_error() {
    let dir = TempDir::new("lib-tool-002-curl-missing");
    let url = file_url(&dir.file("missing.bin"));
    let out = dir.file("out.bin");

    let result = curl_download(&url, &out);

    // `curl <exit status>`: `exit status: 37` on Unix, `exit code: 37` on
    // Windows. "could not run curl" would mean curl is missing, not that it
    // reported the failure.
    let Err(message) = result else {
        panic!("LIB-TOOL-002: curl_download({url:?}) of a missing file must fail, got Ok");
    };
    assert!(
        message.starts_with("curl exit"),
        "LIB-TOOL-002: a transfer that curl reports as failed is an error naming curl's \
         exit status, got {message:?}"
    );
}
