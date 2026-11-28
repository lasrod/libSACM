//! Acceptance tests for hash-pinned fetching of the SACM 2.3 sources
//! (LIB-TOOL-002), XMI 2.5.1 (LIB-TOOL-020), its XMI.xsd (LIB-TOOL-021),
//! XML Schema Part 2 (LIB-TOOL-022), XML Schema Part 1 (LIB-TOOL-024), the
//! W3C XML Conformance Test Suite (LIB-SEC-006) and the redirects and HTTP
//! errors of the download (LIB-TOOL-012).
//!
//! `fetch_into` and `fetch_suite` are driven with fake downloaders (and fake
//! extractors), so no test touches the network. `curl_download` is driven
//! with the real `curl` on `file://` URLs and on a local HTTP server on
//! 127.0.0.1, and `tar_extract` with the real `tar` on local files. Every test
//! works in its own directory under the system temporary directory and removes
//! it afterwards.

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use std::fmt::Write as _;

use xtask::fetch::{
    ARTIFACTS, Artifact, Outcome, XMLCONF, XMLCONF_DEST, curl_download, extract_with, fetch_into,
    fetch_suite, fetch_suite_from, sha256_hex, tar_extract,
};

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
            // XML Schema Part 2 is the W3C's (LIB-TOOL-022).
            artifact.url.starts_with("https://www.omg.org/")
                || artifact.url.starts_with("https://www.w3.org/"),
            "LIB-TOOL-002: OMG or W3C source"
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

// ---------------------------------------------------------------------------
// LIB-TOOL-020 item 1: XMI 2.5.1 is pinned after the two SACM sources
// ---------------------------------------------------------------------------

/// The two SACM 2.3 sources as LIB-TOOL-002 pinned them. LIB-TOOL-020 must
/// leave both unchanged, in this order, at the head of the list.
const SACM_ARTIFACTS: [Artifact; 2] = [
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

/// LIB-TOOL-021 appends XMI.xsd after the XMI 2.5.1 specification, so the
/// specification is pinned at index 2 rather than as the last artifact. The
/// name is kept because the matrix cites it.
#[test]
fn lib_tool_020_artifacts_end_with_the_xmi_2_5_1_pin() {
    let Some(xmi) = ARTIFACTS.get(2) else {
        panic!(
            "LIB-TOOL-020: ARTIFACTS lists the two SACM sources, then XMI 2.5.1 at index 2, \
             got {:?}",
            ARTIFACTS.iter().map(|a| a.file_name).collect::<Vec<_>>()
        );
    };
    assert_eq!(
        (xmi.file_name, xmi.url, xmi.sha256),
        (
            "XMI-2.5.1-formal-15-06-07.pdf",
            "https://www.omg.org/spec/XMI/2.5.1/PDF",
            "2bf97c670f128affc9ed5dc202ef748101afd0eee8ee662e8763da6092c80c1b",
        ),
        "LIB-TOOL-020: the artifact at index 2 is XMI 2.5.1 (formal/15-06-07) with its file \
         name, URL and SHA-256 pin"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-021: XMI.xsd is pinned after the XMI 2.5.1 specification
// ---------------------------------------------------------------------------

/// The first three artifacts as LIB-TOOL-002 and LIB-TOOL-020 pinned them.
const FIRST_THREE_ARTIFACTS: [Artifact; 3] = [
    SACM_ARTIFACTS[0],
    SACM_ARTIFACTS[1],
    Artifact {
        file_name: "XMI-2.5.1-formal-15-06-07.pdf",
        url: "https://www.omg.org/spec/XMI/2.5.1/PDF",
        sha256: "2bf97c670f128affc9ed5dc202ef748101afd0eee8ee662e8763da6092c80c1b",
        what: "XMI 2.5.1 formal specification (formal/15-06-07)",
    },
];

/// The XMI 2.5.1 normative XML Schema, exactly as LIB-TOOL-021 states it.
const XMI_XSD: Artifact = Artifact {
    file_name: "XMI-2.5.1-XMI.xsd",
    url: "https://www.omg.org/spec/XMI/20131001/XMI.xsd",
    sha256: "7b228718d35a15a8e2fb99f078d36eee4b23b7a2cd405e7ac81b9d27bf2fab5d",
    what: "XMI 2.5.1 normative XML Schema (XMI.xsd)",
};

/// Renamed from `lib_tool_021_artifacts_has_four_entries` in slice 0072:
/// LIB-TOOL-022 appends a fifth artifact, so LIB-TOOL-021 asks for at least
/// four.
#[test]
fn lib_tool_021_artifacts_has_at_least_four_entries() {
    assert!(
        ARTIFACTS.len() >= 4,
        "LIB-TOOL-021: ARTIFACTS lists the two SACM sources, XMI 2.5.1 and XMI.xsd, got {:?}",
        ARTIFACTS.iter().map(|a| a.file_name).collect::<Vec<_>>()
    );
}

#[test]
fn lib_tool_021_the_fourth_artifact_is_the_xmi_xsd_pin() {
    assert_eq!(
        ARTIFACTS.get(3),
        Some(&XMI_XSD),
        "LIB-TOOL-021: the fourth artifact is XMI.xsd with exactly the file name, https URL, \
         SHA-256 pin and description of the acceptance"
    );
}

#[test]
fn lib_tool_021_the_first_three_artifacts_are_unchanged() {
    assert!(
        ARTIFACTS.len() >= 3,
        "LIB-TOOL-021: ARTIFACTS still lists the first three sources, got {} entries",
        ARTIFACTS.len()
    );
    assert_eq!(
        ARTIFACTS[..3],
        FIRST_THREE_ARTIFACTS,
        "LIB-TOOL-021: the SACM sources and the XMI 2.5.1 specification keep their file names, \
         URLs, pins, descriptions and order, ahead of XMI.xsd"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-022: XML Schema Part 2 is pinned after XMI.xsd
// ---------------------------------------------------------------------------

/// The first four artifacts as LIB-TOOL-002, LIB-TOOL-020 and LIB-TOOL-021
/// pinned them.
const FIRST_FOUR_ARTIFACTS: [Artifact; 4] = [
    FIRST_THREE_ARTIFACTS[0],
    FIRST_THREE_ARTIFACTS[1],
    FIRST_THREE_ARTIFACTS[2],
    XMI_XSD,
];

/// XML Schema Part 2: Datatypes Second Edition, exactly as LIB-TOOL-022
/// states it.
const XSD_PART_2: Artifact = Artifact {
    file_name: "XMLSchema-2-REC-20041028-datatypes.html",
    url: "https://www.w3.org/TR/2004/REC-xmlschema-2-20041028/datatypes.html",
    sha256: "980de872aa2c50013d5202176eb85eef3f183a7df635fdcb6b4aa34c1c12eb72",
    what: "XML Schema Part 2: Datatypes Second Edition (W3C Recommendation 28 October 2004)",
};

/// Renamed from `lib_tool_022_artifacts_has_five_entries` in slice 0088:
/// LIB-TOOL-024 appends a sixth artifact, so LIB-TOOL-022 asks for at least
/// five.
#[test]
fn lib_tool_022_artifacts_has_at_least_five_entries() {
    assert!(
        ARTIFACTS.len() >= 5,
        "LIB-TOOL-022: ARTIFACTS lists the two SACM sources, XMI 2.5.1, XMI.xsd and XML Schema \
         Part 2, got {:?}",
        ARTIFACTS.iter().map(|a| a.file_name).collect::<Vec<_>>()
    );
}

#[test]
fn lib_tool_022_the_fifth_artifact_is_the_xml_schema_part_2_pin() {
    assert_eq!(
        ARTIFACTS.get(4),
        Some(&XSD_PART_2),
        "LIB-TOOL-022: the fifth artifact is XML Schema Part 2 with exactly the file name, URL, \
         SHA-256 pin and description of the acceptance"
    );
}

#[test]
fn lib_tool_022_the_first_four_artifacts_are_unchanged() {
    assert!(
        ARTIFACTS.len() >= 4,
        "LIB-TOOL-022: ARTIFACTS still lists the first four sources, got {} entries",
        ARTIFACTS.len()
    );
    assert_eq!(
        ARTIFACTS[..4],
        FIRST_FOUR_ARTIFACTS,
        "LIB-TOOL-022: the SACM sources, XMI 2.5.1 and XMI.xsd keep their file names, URLs, \
         pins, descriptions and order, ahead of XML Schema Part 2"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-024: XML Schema Part 1 is pinned after XML Schema Part 2
// ---------------------------------------------------------------------------

/// The first five artifacts as LIB-TOOL-002, LIB-TOOL-020, LIB-TOOL-021 and
/// LIB-TOOL-022 pinned them.
const FIRST_FIVE_ARTIFACTS: [Artifact; 5] = [
    FIRST_FOUR_ARTIFACTS[0],
    FIRST_FOUR_ARTIFACTS[1],
    FIRST_FOUR_ARTIFACTS[2],
    FIRST_FOUR_ARTIFACTS[3],
    XSD_PART_2,
];

/// XML Schema Part 1: Structures Second Edition, exactly as LIB-TOOL-024
/// states it, whose `skip` wildcard (§3.10.1) and ID table (§3.15.5)
/// SACM23-XMI-031 quotes.
const XSD_PART_1: Artifact = Artifact {
    file_name: "XMLSchema-1-REC-20041028-structures.html",
    url: "https://www.w3.org/TR/2004/REC-xmlschema-1-20041028/structures.html",
    sha256: "92bfd116316332b1153941436430afbdb2040e036d5fb2d21d4c8fba36a7765a",
    what: "XML Schema Part 1: Structures Second Edition (W3C Recommendation 28 October 2004)",
};

#[test]
fn lib_tool_024_artifacts_has_six_entries() {
    assert_eq!(
        ARTIFACTS.len(),
        6,
        "LIB-TOOL-024: ARTIFACTS lists the two SACM sources, XMI 2.5.1, XMI.xsd, XML Schema \
         Part 2 and XML Schema Part 1, got {:?}",
        ARTIFACTS.iter().map(|a| a.file_name).collect::<Vec<_>>()
    );
}

#[test]
fn lib_tool_024_the_sixth_artifact_is_the_xml_schema_part_1_pin() {
    assert_eq!(
        ARTIFACTS.get(5),
        Some(&XSD_PART_1),
        "LIB-TOOL-024: the sixth artifact is XML Schema Part 1 with exactly the file name, W3C \
         URL, SHA-256 pin and description of the acceptance"
    );
}

#[test]
fn lib_tool_024_the_first_five_artifacts_are_unchanged() {
    assert!(
        ARTIFACTS.len() >= 5,
        "LIB-TOOL-024: ARTIFACTS still lists the first five sources, got {} entries",
        ARTIFACTS.len()
    );
    assert_eq!(
        ARTIFACTS[..5],
        FIRST_FIVE_ARTIFACTS,
        "LIB-TOOL-024: the SACM sources, XMI 2.5.1, XMI.xsd and XML Schema Part 2 keep their \
         file names, URLs, pins, descriptions and order, ahead of XML Schema Part 1"
    );
}

#[test]
fn lib_tool_020_sacm_artifacts_are_unchanged_and_come_first() {
    assert!(
        ARTIFACTS.len() >= 2,
        "LIB-TOOL-020: ARTIFACTS still lists both SACM sources, got {} entries",
        ARTIFACTS.len()
    );
    assert_eq!(
        ARTIFACTS[..2],
        SACM_ARTIFACTS,
        "LIB-TOOL-020: the two SACM sources keep their file names, URLs, pins and descriptions, \
         ahead of XMI 2.5.1 (ADR 0003 source priority)"
    );
}

// ---------------------------------------------------------------------------
// LIB-SEC-006 item 1: the W3C XML Conformance Test Suite
// ---------------------------------------------------------------------------
//
// `fetch_suite_from` is driven with a test pin (the `abc` vector), a fake
// downloader and a fake extractor, so no test needs the real archive or the
// network. `fetch_suite` is `fetch_suite_from` with `XMLCONF`; one test
// drives it directly, offline, through a planted file with the wrong hash.
// `tar_extract` runs the real `tar` on an archive the test builds.

/// A suite archive pinned to the `abc` test vector.
const SUITE: Artifact = Artifact {
    file_name: "suite.tar.gz",
    url: "https://example.invalid/suite.tar.gz",
    sha256: ABC_SHA256,
    what: "test suite archive",
};

/// Where a test's suite goes: `<temp>/third_party/xmlconf`, not yet created.
fn suite_dest(dir: &TempDir) -> PathBuf {
    dir.path().join("third_party").join("xmlconf")
}

/// Names of the entries of `path`, sorted; empty when it does not exist.
fn entries_of(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Writes `<dir>/xmlconf/xmlconf.xml`, as extracting the archive would.
fn plant_master(dir: &Path) -> Result<(), String> {
    let suite = dir.join("xmlconf");
    fs::create_dir_all(&suite).map_err(|e| e.to_string())?;
    fs::write(suite.join("xmlconf.xml"), b"<TESTSUITE/>").map_err(|e| e.to_string())
}

/// A destination holding the pinned `SUITE` archive, and the extracted
/// suite's `xmlconf/xmlconf.xml` when `extracted`.
fn present_suite(dir: &TempDir, extracted: bool) -> PathBuf {
    let dest = suite_dest(dir);
    fs::create_dir_all(&dest).expect("LIB-SEC-006: create the destination");
    fs::write(dest.join(SUITE.file_name), ABC).expect("LIB-SEC-006: plant the archive");
    if extracted {
        plant_master(&dest).expect("LIB-SEC-006: plant xmlconf/xmlconf.xml");
    }
    dest
}

#[test]
fn lib_sec_006_the_suite_pin_is_the_w3c_20130923_archive() {
    assert_eq!(
        (XMLCONF.file_name, XMLCONF.url, XMLCONF.sha256),
        (
            "xmlts20130923.tar.gz",
            "https://www.w3.org/XML/Test/xmlts20130923.tar.gz",
            "9b61db9f5dbffa545f4b8d78422167083a8568c59bd1129f94138f936cf6fc1f",
        ),
        "LIB-SEC-006: the suite is pinned by file name, w3.org URL and SHA-256"
    );
    assert!(
        XMLCONF.sha256.len() == 64
            && XMLCONF
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "LIB-SEC-006: the pin is 64 lowercase hex digits"
    );
    assert!(
        !XMLCONF.what.trim().is_empty(),
        "LIB-SEC-006: the suite has a description"
    );
    assert_eq!(
        XMLCONF_DEST, "third_party/xmlconf",
        "LIB-SEC-006: the suite goes to the git-ignored third_party/xmlconf"
    );
    assert!(
        !ARTIFACTS.iter().any(|a| a.url == XMLCONF.url),
        "LIB-SEC-006: the suite is not one of the OMG ARTIFACTS"
    );
}

#[test]
fn lib_sec_006_third_party_xmlconf_is_git_ignored() {
    let path = xtask::repo_root().join(".gitignore");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("LIB-SEC-006: read {}: {e}", path.display()));
    assert!(
        text.lines()
            .any(|line| line.trim() == "/third_party/xmlconf/"),
        "LIB-SEC-006: .gitignore lists /third_party/xmlconf/, so the suite is never committed"
    );
}

#[test]
fn lib_sec_006_fetch_suite_downloads_verifies_and_then_extracts() {
    let dir = TempDir::new("lib-sec-006-fresh");
    let dest = suite_dest(&dir);
    let final_path = dest.join(SUITE.file_name);
    let mut downloads: Vec<(String, PathBuf)> = Vec::new();
    let mut extracts: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut verified_before_extract = false;

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |url: &str, out: &Path| {
            downloads.push((url.to_owned(), out.to_path_buf()));
            fs::write(out, ABC).map_err(|e| e.to_string())
        },
        &mut |archive: &Path, into: &Path| {
            extracts.push((archive.to_path_buf(), into.to_path_buf()));
            // Only the verified archive is extracted: at its final path, with
            // the pinned bytes, and no .part file beside it.
            verified_before_extract =
                fs::read(archive).is_ok_and(|b| b == ABC) && entries_of(into) == [SUITE.file_name];
            plant_master(into)
        },
    );

    assert_eq!(
        result,
        Ok(Outcome::Fetched),
        "LIB-SEC-006: a fresh, matching download is Fetched"
    );
    assert_eq!(
        downloads,
        [(
            SUITE.url.to_owned(),
            dest.join(format!("{}.part", SUITE.file_name))
        )],
        "LIB-SEC-006: downloaded once, from the pinned URL, to <name>.part"
    );
    assert_eq!(
        extracts,
        [(final_path.clone(), dest.clone())],
        "LIB-SEC-006: the archive is extracted once, into the destination"
    );
    assert!(
        verified_before_extract,
        "LIB-SEC-006: tar runs only on the verified archive at its final path"
    );
    assert_eq!(
        fs::read(&final_path).ok().as_deref(),
        Some(ABC),
        "LIB-SEC-006: the verified archive is kept in place"
    );
    assert_eq!(
        entries_of(&dest),
        [SUITE.file_name, "xmlconf"],
        "LIB-SEC-006: the archive and the extracted suite, no .part file"
    );
}

#[test]
fn lib_sec_006_fetch_suite_hash_mismatch_deletes_the_partial_file_and_does_not_extract() {
    let dir = TempDir::new("lib-sec-006-mismatch");
    let dest = suite_dest(&dir);
    let mut extracts = 0_usize;

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |_url: &str, out: &Path| {
            fs::write(out, b"changed upstream").map_err(|e| e.to_string())
        },
        &mut |_archive: &Path, _into: &Path| {
            extracts += 1;
            Ok(())
        },
    );

    let err = result.expect_err("LIB-SEC-006: a hash mismatch is an error");
    assert!(
        err.contains(ABC_SHA256) && err.contains(&sha256_hex(b"changed upstream")),
        "LIB-SEC-006: the error reports the pinned and the received hash, got {err:?}"
    );
    assert_eq!(
        extracts, 0,
        "LIB-SEC-006: an unverified archive is never extracted"
    );
    assert_eq!(
        entries_of(&dest),
        Vec::<String>::new(),
        "LIB-SEC-006: neither a .part file nor an archive is left"
    );
}

#[test]
fn lib_sec_006_fetch_suite_downloader_error_deletes_the_partial_file_and_does_not_extract() {
    let dir = TempDir::new("lib-sec-006-download-error");
    let dest = suite_dest(&dir);
    let mut extracts = 0_usize;

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |_url: &str, out: &Path| {
            // A transfer that dies half way leaves a partial file behind.
            fs::write(out, b"ab").map_err(|e| e.to_string())?;
            Err("connection reset".to_owned())
        },
        &mut |_archive: &Path, _into: &Path| {
            extracts += 1;
            Ok(())
        },
    );

    let err = result.expect_err("LIB-SEC-006: a failed download is an error");
    assert!(
        err.contains("connection reset"),
        "LIB-SEC-006: the error carries the downloader's message, got {err:?}"
    );
    assert_eq!(extracts, 0, "LIB-SEC-006: nothing is extracted");
    assert_eq!(
        entries_of(&dest),
        Vec::<String>::new(),
        "LIB-SEC-006: neither a .part file nor an archive is left"
    );
}

#[test]
fn lib_sec_006_fetch_suite_changes_nothing_when_the_archive_and_the_suite_are_present() {
    let dir = TempDir::new("lib-sec-006-present");
    let dest = present_suite(&dir, true);
    let (mut downloads, mut extracts) = (0_usize, 0_usize);

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |_url: &str, _out: &Path| {
            downloads += 1;
            Err("must not be called".to_owned())
        },
        &mut |_archive: &Path, _into: &Path| {
            extracts += 1;
            Err("must not be called".to_owned())
        },
    );

    assert_eq!(
        result,
        Ok(Outcome::AlreadyPresent),
        "LIB-SEC-006: a matching archive and an extracted suite are AlreadyPresent"
    );
    assert_eq!(
        (downloads, extracts),
        (0, 0),
        "LIB-SEC-006: nothing is downloaded or extracted"
    );
    assert_eq!(
        fs::read(dest.join(SUITE.file_name)).ok().as_deref(),
        Some(ABC),
        "LIB-SEC-006: the archive is untouched"
    );
    assert_eq!(
        fs::read(dest.join("xmlconf/xmlconf.xml")).ok().as_deref(),
        Some(&b"<TESTSUITE/>"[..]),
        "LIB-SEC-006: the extracted suite is untouched"
    );
}

#[test]
fn lib_sec_006_fetch_suite_re_extracts_when_the_master_catalog_is_missing() {
    // No xmlconf/ at all, and an xmlconf/ without xmlconf.xml.
    for partial_suite in [false, true] {
        let dir = TempDir::new("lib-sec-006-re-extract");
        let dest = present_suite(&dir, false);
        if partial_suite {
            fs::create_dir_all(dest.join("xmlconf/sun"))
                .expect("LIB-SEC-006: plant a partial suite");
        }
        let mut downloads = 0_usize;
        let mut extracts: Vec<(PathBuf, PathBuf)> = Vec::new();

        let result = fetch_suite_from(
            &dest,
            &SUITE,
            &mut |_url: &str, _out: &Path| {
                downloads += 1;
                Err("must not be called".to_owned())
            },
            &mut |archive: &Path, into: &Path| {
                extracts.push((archive.to_path_buf(), into.to_path_buf()));
                plant_master(into)
            },
        );

        assert_eq!(
            result,
            Ok(Outcome::Fetched),
            "LIB-SEC-006: re-extracting a present archive is Fetched (partial suite: \
             {partial_suite})"
        );
        assert_eq!(
            downloads, 0,
            "LIB-SEC-006: a present archive with the pinned hash is not downloaded again"
        );
        assert_eq!(
            extracts,
            [(dest.join(SUITE.file_name), dest.clone())],
            "LIB-SEC-006: without xmlconf/xmlconf.xml the archive is extracted again \
             (partial suite: {partial_suite})"
        );
        assert_eq!(
            fs::read(dest.join(SUITE.file_name)).ok().as_deref(),
            Some(ABC),
            "LIB-SEC-006: the archive is untouched"
        );
    }
}

#[test]
fn lib_sec_006_fetch_suite_extracts_a_fresh_download_even_when_the_marker_is_present() {
    // A left-over xmlconf/xmlconf.xml without the archive: the archive just
    // downloaded is extracted over it, so the suite matches the pin.
    let dir = TempDir::new("lib-sec-006-stale-marker");
    let dest = suite_dest(&dir);
    plant_master(&dest).expect("LIB-SEC-006: plant xmlconf/xmlconf.xml");
    let mut downloads = 0_usize;
    let mut extracts: Vec<(PathBuf, PathBuf)> = Vec::new();

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |_url: &str, out: &Path| {
            downloads += 1;
            fs::write(out, ABC).map_err(|e| e.to_string())
        },
        &mut |archive: &Path, into: &Path| {
            extracts.push((archive.to_path_buf(), into.to_path_buf()));
            plant_master(into)
        },
    );

    assert_eq!(
        (result, downloads),
        (Ok(Outcome::Fetched), 1),
        "LIB-SEC-006: a missing archive is downloaded once and the outcome is Fetched, even \
         with the marker present"
    );
    assert_eq!(
        extracts,
        [(dest.join(SUITE.file_name), dest.clone())],
        "LIB-SEC-006: a freshly downloaded archive is extracted once, marker or not"
    );
}

#[test]
fn lib_sec_006_fetch_suite_refuses_an_existing_archive_with_the_wrong_hash() {
    // `fetch_suite` itself, with the real pin: a planted file with another
    // hash fails before anything is downloaded, so this runs offline. With
    // and without an extracted suite beside it: the hash is always checked.
    for with_suite in [false, true] {
        let dir = TempDir::new("lib-sec-006-tampered");
        let dest = suite_dest(&dir);
        fs::create_dir_all(&dest).expect("LIB-SEC-006: create the destination");
        fs::write(dest.join(XMLCONF.file_name), b"local edit").expect("LIB-SEC-006: plant junk");
        if with_suite {
            plant_master(&dest).expect("LIB-SEC-006: plant xmlconf/xmlconf.xml");
        }
        let (mut downloads, mut extracts) = (0_usize, 0_usize);

        let result = fetch_suite(
            &dest,
            &mut |_url: &str, out: &Path| {
                downloads += 1;
                fs::write(out, b"x").map_err(|e| e.to_string())
            },
            &mut |_archive: &Path, _into: &Path| {
                extracts += 1;
                Ok(())
            },
        );

        let Err(err) = result else {
            panic!(
                "LIB-SEC-006: an archive with another hash is an error (suite beside it: \
                 {with_suite}), got {result:?}"
            );
        };
        assert!(
            err.contains(XMLCONF.file_name)
                && err.contains(&sha256_hex(b"local edit"))
                && err.contains(XMLCONF.sha256),
            "LIB-SEC-006: the error names the archive, its hash and the XMLCONF pin, got {err:?}"
        );
        assert_eq!(
            (downloads, extracts),
            (0, 0),
            "LIB-SEC-006: nothing is downloaded or extracted (suite beside it: {with_suite})"
        );
        assert_eq!(
            fs::read(dest.join(XMLCONF.file_name)).ok().as_deref(),
            Some(&b"local edit"[..]),
            "LIB-SEC-006: the existing archive is left untouched"
        );
        let expected: &[&str] = if with_suite {
            &["xmlconf", XMLCONF.file_name]
        } else {
            &[XMLCONF.file_name]
        };
        assert_eq!(
            entries_of(&dest),
            expected,
            "LIB-SEC-006: no .part file is created"
        );
    }
}

#[test]
fn lib_sec_006_fetch_suite_propagates_an_extractor_error() {
    let dir = TempDir::new("lib-sec-006-extract-error");
    let dest = suite_dest(&dir);

    let result = fetch_suite_from(
        &dest,
        &SUITE,
        &mut |_url: &str, out: &Path| fs::write(out, ABC).map_err(|e| e.to_string()),
        &mut |_archive: &Path, _into: &Path| Err("tar exit status: 2".to_owned()),
    );

    let err = result.expect_err("LIB-SEC-006: an extractor error is an error");
    assert!(
        err.contains("tar exit status: 2"),
        "LIB-SEC-006: the extractor's message is passed on, got {err:?}"
    );
}

#[test]
fn lib_sec_006_fetch_suite_retries_after_a_partly_failed_extraction() {
    // An extraction that writes xmlconf/xmlconf.xml and then fails must not
    // leave the marker behind: the next call would take the half-extracted
    // suite for a complete one and return AlreadyPresent.
    let dir = TempDir::new("lib-sec-006-partial-extract");
    let dest = suite_dest(&dir);
    let mut download = |_url: &str, out: &Path| -> Result<(), String> {
        fs::write(out, ABC).map_err(|e| e.to_string())
    };

    let first = fetch_suite_from(
        &dest,
        &SUITE,
        &mut download,
        &mut |_archive: &Path, into: &Path| {
            plant_master(into)?;
            Err("tar exit status: 2".to_owned())
        },
    );
    assert!(
        first
            .as_ref()
            .is_err_and(|e| e.contains("tar exit status: 2")),
        "LIB-SEC-006: an extraction that fails after writing the marker is an error, got \
         {first:?}"
    );

    let mut extracts = 0_usize;
    let second = fetch_suite_from(
        &dest,
        &SUITE,
        &mut download,
        &mut |_archive: &Path, into: &Path| {
            extracts += 1;
            plant_master(into)
        },
    );
    assert_eq!(
        (second, extracts),
        (Ok(Outcome::Fetched), 1),
        "LIB-SEC-006: after a failed extraction the next call extracts again and is Fetched, \
         not AlreadyPresent"
    );
    assert_eq!(
        fs::read(dest.join("xmlconf/xmlconf.xml")).ok().as_deref(),
        Some(&b"<TESTSUITE/>"[..]),
        "LIB-SEC-006: the re-extracted suite is in place"
    );
}

#[test]
fn lib_sec_006_tar_extract_of_a_missing_archive_is_an_error_naming_tar() {
    // The message must start with the command, so a "tar" elsewhere in the
    // path (the system temporary directory may hold one) cannot satisfy it.
    let dir = TempDir::new("lib-sec-006-extract-absent");
    let archive = dir.file("absent.tgz");

    let outcome = catch_unwind(AssertUnwindSafe(|| tar_extract(&archive, dir.path())));

    let Ok(result) = outcome else {
        panic!("LIB-SEC-006: tar_extract panicked on a missing archive instead of returning Err");
    };
    let Err(message) = result else {
        panic!("LIB-SEC-006: tar_extract of a missing archive must fail, got Ok");
    };
    assert!(
        message.starts_with("tar -xzf "),
        "LIB-SEC-006: the error starts with the command tar ran, got {message:?}"
    );
}

#[test]
fn lib_sec_006_extract_with_a_program_that_cannot_run_is_an_error_naming_it() {
    let dir = TempDir::new("lib-sec-006-extract-with-missing-program");
    let program = "libsacm-no-such-extractor";
    let archive = dir.file("absent.tgz");

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        extract_with(program, &archive, dir.path())
    }));

    let Ok(result) = outcome else {
        panic!("LIB-SEC-006: extract_with panicked on a missing program instead of returning Err");
    };
    let Err(message) = result else {
        panic!("LIB-SEC-006: extract_with({program:?}, ..) must fail, got Ok");
    };
    assert!(
        message.starts_with(&format!("could not run {program}")),
        "LIB-SEC-006: a program that cannot be run is an error naming it, got {message:?}"
    );
}

#[test]
fn lib_sec_006_tar_extract_unpacks_an_archive_into_the_directory() {
    // Build `small.tar.gz` holding `xmlconf/xmlconf.xml` with the system tar,
    // by relative names from the temporary directory, then remove the source
    // so only extraction can bring it back.
    let dir = TempDir::new("lib-sec-006-tar-extract");
    let bytes = b"<TESTSUITE PROFILE=\"tar_extract\"/>\n";
    plant_master(dir.path()).expect("LIB-SEC-006: write the archive's content");
    fs::write(dir.file("xmlconf/xmlconf.xml"), bytes).expect("LIB-SEC-006: write the content");
    let status = std::process::Command::new("tar")
        .args(["-czf", "small.tar.gz", "xmlconf"])
        .current_dir(dir.path())
        .status();
    assert!(
        status.as_ref().is_ok_and(std::process::ExitStatus::success),
        "LIB-SEC-006: the system tar builds the test archive, got {status:?}"
    );
    fs::remove_dir_all(dir.file("xmlconf")).expect("LIB-SEC-006: remove the archived source");
    let archive = dir.file("small.tar.gz");
    let archived = fs::read(&archive).expect("LIB-SEC-006: read the test archive");

    let result = tar_extract(&archive, dir.path());

    assert_eq!(
        result,
        Ok(()),
        "LIB-SEC-006: the system tar extracts the archive"
    );
    assert_eq!(
        fs::read(dir.file("xmlconf/xmlconf.xml")).ok().as_deref(),
        Some(&bytes[..]),
        "LIB-SEC-006: the archive is extracted into the directory, as xmlconf/xmlconf.xml"
    );
    assert_eq!(
        fs::read(&archive).ok(),
        Some(archived),
        "LIB-SEC-006: the archive is kept"
    );
}

// ---------------------------------------------------------------------------
// LIB-COMPAT-015: the corpus of SACM files written by other tools
// ---------------------------------------------------------------------------

use xtask::fetch::{CORPUS, fetch_each};

/// Where the corpus URLs point: raw files of a repository at a commit.
const RAW_PREFIX: &str = "https://raw.githubusercontent.com/";

/// Whether `text` is all lowercase hex digits.
fn lowercase_hex(text: &str) -> bool {
    text.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn lib_compat_015_the_corpus_pins_are_well_formed_unique_and_pinned_to_commits() {
    assert_eq!(CORPUS.len(), 15, "LIB-COMPAT-015: CORPUS has 15 entries");
    let mut names = std::collections::BTreeSet::new();
    let mut repositories = std::collections::BTreeSet::new();
    for artifact in CORPUS {
        let name = artifact.file_name;
        assert!(
            artifact.sha256.len() == 64 && lowercase_hex(artifact.sha256),
            "LIB-COMPAT-015: the pin of {name} is not 64 lowercase hex digits: {:?}",
            artifact.sha256
        );
        assert!(
            !name.is_empty()
                && !name.contains(['/', '\\'])
                && !name.contains("..")
                && !name.starts_with('.'),
            "LIB-COMPAT-015: {name:?} is not a plain file name"
        );
        assert!(
            names.insert(name),
            "LIB-COMPAT-015: the file name {name} is used twice"
        );
        assert!(
            !artifact.what.is_empty(),
            "LIB-COMPAT-015: {name} has no description"
        );
        let segments: Vec<&str> = artifact
            .url
            .strip_prefix(RAW_PREFIX)
            .map(|path| path.split('/').collect())
            .unwrap_or_default();
        let [owner, repository, commit, rest @ ..] = segments.as_slice() else {
            panic!(
                "LIB-COMPAT-015: the URL of {name} is not {RAW_PREFIX}<owner>/<repo>/<commit>/...: {}",
                artifact.url
            );
        };
        assert!(
            !owner.is_empty()
                && !repository.is_empty()
                && !rest.is_empty()
                && rest.iter().all(|segment| !segment.is_empty()),
            "LIB-COMPAT-015: the URL of {name} does not name an owner, a repository and a file: {}",
            artifact.url
        );
        assert!(
            commit.len() == 40 && lowercase_hex(commit),
            "LIB-COMPAT-015: the URL of {name} is not pinned to a 40-digit lowercase commit: {}",
            artifact.url
        );
        repositories.insert((*owner, *repository));
    }
    assert!(
        repositories.len() >= 2,
        "LIB-COMPAT-015: the corpus comes from fewer than two repositories: {repositories:?}"
    );
    assert!(
        repositories.iter().any(|(owner, _)| *owner != "lasrod"),
        "LIB-COMPAT-015: no corpus repository is outside lasrod/: {repositories:?}"
    );
}

/// The `fetch_each` lines for [`ARTIFACT`] and [`OTHER`] in `dir`, each
/// with `label`.
fn fetch_each_lines(dir: &TempDir, label: &str) -> String {
    format!(
        "{label} {} ({})\n{label} {} ({})\n",
        dir.file(ARTIFACT.file_name).display(),
        ARTIFACT.what,
        dir.file(OTHER.file_name).display(),
        OTHER.what
    )
}

#[test]
fn lib_compat_015_fetch_each_writes_a_line_per_file_and_needs_no_network_when_all_are_present() {
    let dir = TempDir::new("lib-compat-015-fetch-each");
    let mut urls: Vec<String> = Vec::new();
    let mut out = Vec::new();

    let first = fetch_each(
        dir.path(),
        &[ARTIFACT, OTHER],
        &mut |url: &str, path: &Path| {
            urls.push(url.to_owned());
            fs::write(path, ABC).map_err(|e| e.to_string())
        },
        &mut out,
    );

    assert_eq!(first, Ok(()), "LIB-COMPAT-015: the first run fetches both");
    assert_eq!(
        urls,
        [ARTIFACT.url, OTHER.url],
        "LIB-COMPAT-015: each missing file is downloaded once, in order"
    );
    assert_eq!(
        String::from_utf8_lossy(&out),
        fetch_each_lines(&dir, "fetched "),
        "LIB-COMPAT-015: the first run writes one `fetched` line per file"
    );
    assert_eq!(
        dir.entries(),
        ["other.bin", "reference.bin"],
        "LIB-COMPAT-015: both files are in place and no .part file is left"
    );

    let mut calls = 0_usize;
    let mut again = Vec::new();
    let second = fetch_each(
        dir.path(),
        &[ARTIFACT, OTHER],
        &mut |_url: &str, _path: &Path| {
            calls += 1;
            Err("LIB-COMPAT-015: no download when every file is present".to_owned())
        },
        &mut again,
    );

    assert_eq!(
        second,
        Ok(()),
        "LIB-COMPAT-015: the second run succeeds with no network"
    );
    assert_eq!(calls, 0, "LIB-COMPAT-015: the second run downloads nothing");
    assert_eq!(
        String::from_utf8_lossy(&again),
        fetch_each_lines(&dir, "ok      "),
        "LIB-COMPAT-015: the second run writes one `ok` line per file"
    );
}

#[test]
fn lib_compat_015_fetch_each_names_the_file_on_a_hash_mismatch_and_leaves_no_partial_file() {
    // A download that does not match its pin, after one that does.
    let dir = TempDir::new("lib-compat-015-mismatch");
    let changed: &[u8] = b"changed upstream";
    let mut out = Vec::new();
    let result = fetch_each(
        dir.path(),
        &[ARTIFACT, OTHER],
        &mut |url: &str, path: &Path| {
            let bytes = if url == OTHER.url { changed } else { ABC };
            fs::write(path, bytes).map_err(|e| e.to_string())
        },
        &mut out,
    );
    let err = result.expect_err("LIB-COMPAT-015: a download that does not match its pin fails");
    assert!(
        err.contains(OTHER.url) && err.contains(ABC_SHA256) && err.contains(&sha256_hex(changed)),
        "LIB-COMPAT-015: the error names the URL, the pinned hash and the received hash, got \
         {err:?}"
    );
    assert_eq!(
        dir.entries(),
        ["reference.bin"],
        "LIB-COMPAT-015: the file fetched before stays; neither a .part file nor the bad file is \
         left"
    );
    assert_eq!(
        String::from_utf8_lossy(&out),
        format!(
            "fetched  {} ({})\n",
            dir.file(ARTIFACT.file_name).display(),
            ARTIFACT.what
        ),
        "LIB-COMPAT-015: only the file fetched before the error has a line"
    );

    // An existing file with another hash.
    let dir = TempDir::new("lib-compat-015-tampered");
    let local: &[u8] = b"local edit";
    let path = dir.file(ARTIFACT.file_name);
    fs::write(&path, local).expect("LIB-COMPAT-015: write fixture");
    let mut calls = 0_usize;
    let mut out = Vec::new();
    let result = fetch_each(
        dir.path(),
        &[ARTIFACT],
        &mut |_url: &str, part: &Path| {
            calls += 1;
            fs::write(part, ABC).map_err(|e| e.to_string())
        },
        &mut out,
    );
    let err = result.expect_err("LIB-COMPAT-015: an existing file with another hash fails");
    assert!(
        err.contains(&path.display().to_string()) && err.contains(&sha256_hex(local)),
        "LIB-COMPAT-015: the error names the file and its hash, got {err:?}"
    );
    assert_eq!(calls, 0, "LIB-COMPAT-015: nothing is downloaded");
    assert_eq!(
        fs::read(&path).ok().as_deref(),
        Some(local),
        "LIB-COMPAT-015: the existing file is byte-identical afterwards"
    );
    assert_eq!(
        dir.entries(),
        [ARTIFACT.file_name],
        "LIB-COMPAT-015: no .part file is created"
    );
    assert!(
        out.is_empty(),
        "LIB-COMPAT-015: a refused file has no line, wrote {:?}",
        String::from_utf8_lossy(&out)
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-012: curl_download against a local HTTP server
// ---------------------------------------------------------------------------
//
// A std-only HTTP/1.1 server on 127.0.0.1 answers each request with
// `Connection: close` and records the path of each request, in order. The
// real `curl` talks to it, so `--location` and `--fail` are exercised as
// `cargo xtask fetch-spec` uses them.
//
// When `http_proxy` is set, whether curl sends a request for 127.0.0.1 to the
// proxy depends on its version and on `no_proxy`; CI sets no proxy.

use std::io::{BufRead as _, BufReader, ErrorKind, Write as _};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The body the server answers `/file` with.
const PINNED_BYTES: &[u8] = b"the pinned bytes\n";

/// The body of the server's error answers, which must never be saved.
const ERROR_PAGE: &[u8] = b"<html>error page</html>\n";

/// How long the server waits for its requests before it stops.
const SERVER_DEADLINE: Duration = Duration::from_secs(30);

/// A local HTTP server that answers a fixed number of requests.
struct Server {
    base: String,
    handle: JoinHandle<Vec<String>>,
}

impl Server {
    /// Start a server that stops after `requests` requests or after
    /// [`SERVER_DEADLINE`], whichever comes first.
    fn start(requests: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| panic!("LIB-TOOL-012: bind 127.0.0.1:0: {e}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("LIB-TOOL-012: the server's address: {e}"));
        listener
            .set_nonblocking(true)
            .unwrap_or_else(|e| panic!("LIB-TOOL-012: a non-blocking listener: {e}"));
        let handle = thread::spawn(move || serve(&listener, requests));
        Self {
            base: format!("http://{address}"),
            handle,
        }
    }

    /// The URL of `path` on this server.
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// Wait for the server to stop, and return the paths it was asked for.
    fn paths(self) -> Vec<String> {
        self.handle
            .join()
            .unwrap_or_else(|_| panic!("LIB-TOOL-012: the server thread panicked"))
    }
}

/// Answer up to `requests` requests on `listener`, and return their paths.
fn serve(listener: &TcpListener, requests: usize) -> Vec<String> {
    let deadline = Instant::now() + SERVER_DEADLINE;
    let mut paths = Vec::new();
    while paths.len() < requests && Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(path) = answer(stream) {
                    paths.push(path);
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
    paths
}

/// The status line, `Location` and body the server answers `path` with.
fn route(path: &str) -> (&'static str, Option<&'static str>, &'static [u8]) {
    match path {
        "/file" => ("200 OK", None, PINNED_BYTES),
        "/moved" => ("302 Found", Some("/file"), b""),
        "/moved-to-missing" => ("301 Moved Permanently", Some("/missing"), b""),
        "/broken" => ("500 Internal Server Error", None, ERROR_PAGE),
        _ => ("404 Not Found", None, ERROR_PAGE),
    }
}

/// Read one request from `stream`, answer it, and return its path.
fn answer(mut stream: TcpStream) -> Option<String> {
    // An accepted socket may inherit the listener's non-blocking mode.
    stream.set_nonblocking(false).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    // The headers, up to the blank line.
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header).ok()?;
        if read == 0 || header.trim_end().is_empty() {
            break;
        }
    }
    let path = request_line.split_whitespace().nth(1)?.to_owned();
    let (status, location, body) = route(&path);
    let location = location.map_or_else(String::new, |to| format!("Location: {to}\r\n"));
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n{location}\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body).ok()?;
    stream.flush().ok()?;
    let _ = stream.shutdown(Shutdown::Write);
    Some(path)
}

/// Whether `out` is absent or empty: no error page was saved.
fn nothing_saved(out: &Path) -> bool {
    fs::read(out).map_or(true, |bytes| bytes.is_empty())
}

#[test]
fn lib_tool_012_curl_download_follows_a_redirect() {
    let dir = TempDir::new("lib-tool-012-redirect");
    let out = dir.file("out.bin");
    let server = Server::start(2);

    let result = curl_download(&server.url("/moved"), &out);
    let paths = server.paths();

    assert_eq!(
        result,
        Ok(()),
        "LIB-TOOL-012: a download through a 302 redirect succeeds"
    );
    assert_eq!(
        fs::read(&out).ok().as_deref(),
        Some(PINNED_BYTES),
        "LIB-TOOL-012: the bytes at the redirect's target are saved"
    );
    assert_eq!(
        paths,
        ["/moved", "/file"],
        "LIB-TOOL-012: curl followed the redirect (`--location`)"
    );
}

#[test]
fn lib_tool_012_curl_download_fails_on_an_http_error_status() {
    let dir = TempDir::new("lib-tool-012-error-status");
    let server = Server::start(2);

    for (path, status) in [("/missing", 404), ("/broken", 500)] {
        let out = dir.file(&format!("out{status}.bin"));
        let result = curl_download(&server.url(path), &out);
        let Err(message) = result else {
            panic!("LIB-TOOL-012: a download answered with {status} must fail, got Ok");
        };
        assert!(
            message.starts_with("curl "),
            "LIB-TOOL-012: a {status} is an error naming curl's exit status (not \"could not \
             run curl\", which means curl is missing), got {message:?}"
        );
        let saved = fs::read(&out).unwrap_or_default();
        assert!(
            saved.is_empty(),
            "LIB-TOOL-012: the {status} error page is not saved (`--fail`), {} holds {:?}",
            out.display(),
            String::from_utf8_lossy(&saved)
        );
    }
    assert_eq!(
        server.paths(),
        ["/missing", "/broken"],
        "LIB-TOOL-012: each download asked the server once"
    );
}

#[test]
fn lib_tool_012_curl_download_fails_when_a_redirect_ends_in_an_error() {
    let dir = TempDir::new("lib-tool-012-redirect-error");
    let out = dir.file("out.bin");
    let server = Server::start(2);

    let result = curl_download(&server.url("/moved-to-missing"), &out);
    let paths = server.paths();

    assert_eq!(
        paths,
        ["/moved-to-missing", "/missing"],
        "LIB-TOOL-012: curl followed the 301 to the missing file"
    );
    let Err(message) = result else {
        panic!("LIB-TOOL-012: a redirect to a 404 must fail, got Ok");
    };
    assert!(
        message.starts_with("curl "),
        "LIB-TOOL-012: a redirect to a 404 is an error naming curl's exit status, got \
         {message:?}"
    );
    assert!(
        nothing_saved(&out),
        "LIB-TOOL-012: the error page behind the redirect is not saved"
    );
}

/// An artifact at `url` pinned to `sha256`, leaked so that it is `'static`
/// as [`Artifact`] requires.
fn served_artifact(file_name: &'static str, url: String, sha256: String) -> Artifact {
    Artifact {
        file_name,
        url: Box::leak(url.into_boxed_str()),
        sha256: Box::leak(sha256.into_boxed_str()),
        what: "LIB-TOOL-012 served artifact",
    }
}

#[test]
fn lib_tool_012_fetch_into_with_curl_keeps_no_error_page() {
    let dir = TempDir::new("lib-tool-012-fetch-into");
    let server = Server::start(3);
    let pin = sha256_hex(PINNED_BYTES);

    let missing = served_artifact("missing.bin", server.url("/missing"), pin.clone());
    let refused = fetch_into(dir.path(), &[missing], &mut curl_download);
    assert!(
        refused.is_err(),
        "LIB-TOOL-012: fetch_into of a 404 fails, got {refused:?}"
    );
    assert!(
        !dir.file("missing.bin").exists() && !dir.file("missing.bin.part").exists(),
        "LIB-TOOL-012: neither a file nor a partial file is left for a 404: {:?}",
        dir.entries()
    );

    let moved = served_artifact("moved.bin", server.url("/moved"), pin);
    let fetched = fetch_into(dir.path(), &[moved], &mut curl_download);
    assert_eq!(
        fetched,
        Ok(vec![Outcome::Fetched]),
        "LIB-TOOL-012: the pinned bytes behind a redirect are fetched"
    );
    assert_eq!(
        fs::read(dir.file("moved.bin")).ok().as_deref(),
        Some(PINNED_BYTES),
        "LIB-TOOL-012: the destination holds the pinned bytes"
    );
    assert_eq!(
        dir.entries(),
        ["moved.bin"],
        "LIB-TOOL-012: only the fetched file is in the directory"
    );
    assert_eq!(
        server.paths(),
        ["/missing", "/moved", "/file"],
        "LIB-TOOL-012: the server was asked for the 404, then the redirect and its target"
    );
}
