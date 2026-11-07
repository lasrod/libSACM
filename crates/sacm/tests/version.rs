//! Tests for crate and standard version reporting (LIB-API-001).

#[test]
fn lib_api_001_reports_library_and_sacm_versions() {
    assert_eq!(
        sacm::VERSION,
        env!("CARGO_PKG_VERSION"),
        "LIB-API-001: VERSION must be the published crate version"
    );
    assert_eq!(
        sacm::SACM_VERSION,
        "2.3",
        "LIB-API-001: the implemented standard revision is SACM 2.3"
    );
}
