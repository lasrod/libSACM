//! The namespaces the reader and the writer share: one pin each (D-NS-PIN;
//! ADR 0017 W5 keeps them crate-private). No `xmi:version` is pinned: the XMI
//! namespace identifies the version (XMI 2.5.1 §7.5.3; ADR 0017 amendment 3).
//!
//! And the classes of a root's namespace (LIB-COMPAT-004; ADR 0041 D4, D5).
//! No SACM revision sets a `Package::URI` (spec defect D7), so files use many
//! namespaces: a root in the pinned one is SACM 2.3; one in a recognised
//! older revision's, or in one whose URI contains `sacm` in any letter case,
//! is read as SACM 2.3 by a tolerant read; SACM 2.4 is refused
//! (LIB-COMPAT-018); anything else is not SACM.

/// The pinned SACM 2.3 namespace (D-NS-PIN).
pub(super) const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
pub(super) const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The prefix written for [`SACM_NS`].
pub(super) const SACM_PREFIX: &str = "sacm";
/// The prefix written for [`XMI_NS`].
pub(super) const XMI_PREFIX: &str = "xmi";

/// The older SACM revisions recognised, by the base of their namespace, with
/// the revision each is: OMG's dated URIs of SACM 2.0, 2.1 and 2.2 (the
/// `emof.xml` of formal/17-08-43, /19-02-35 and /21-02-18), the undated forms
/// of the same, the forms of the Eclipse and `ACEditor` tools.
const REVISIONS: [(&str, &str); 10] = [
    ("http://www.omg.org/spec/SACM/20170843", "2.0"),
    ("http://www.omg.org/spec/SACM/20190235", "2.1"),
    ("http://www.omg.org/spec/SACM/20210218", "2.2"),
    ("http://www.omg.org/spec/SACM/2.0", "2.0"),
    ("http://www.omg.org/spec/SACM/2.1", "2.1"),
    ("http://www.omg.org/spec/SACM/2.2", "2.2"),
    ("http://omg.sacm/2.0", "2.0"),
    ("http://omg.sacm/2.1", "2.1"),
    ("http://omg.sacm/2.2", "2.2"),
    ("http://SACM_ACEditor/sacm/2.1", "2.1"),
];

/// The bases of SACM 2.4: Beta1's dated URI and the undated one.
const SACM_24: [&str; 2] = [
    "http://www.omg.org/spec/SACM/20260504",
    "http://www.omg.org/spec/SACM/2.4",
];

/// The undated base of SACM 2.3, read as a SACM-like namespace with its
/// packages.
const UNDATED_23: &str = "http://www.omg.org/spec/SACM/2.3";

/// The class of a root's namespace (ADR 0041 D4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RootNamespace<'u> {
    /// The pinned SACM 2.3 namespace.
    Pinned,
    /// A recognised older revision's, SACM `revision`.
    OtherRevision {
        revision: &'static str,
        family: Family<'u>,
    },
    /// A namespace whose URI contains `sacm` in any letter case.
    SacmLike { family: Family<'u> },
    /// SACM 2.4's.
    Sacm24,
    /// No namespace, or one that does not name SACM.
    NotSacm,
}

impl<'u> RootNamespace<'u> {
    /// The family of a root of this class: its SACM namespaces for types.
    pub(super) const fn family(self) -> Option<Family<'u>> {
        match self {
            Self::Pinned => Some(PINNED),
            Self::OtherRevision { family, .. } | Self::SacmLike { family } => Some(family),
            Self::Sacm24 | Self::NotSacm => None,
        }
    }
}

/// The namespaces that count as a root's SACM namespace (ADR 0041 D5): the
/// base URI and, with `packages`, the base followed by `/` and a package
/// name of ASCII letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Family<'u> {
    base: &'u str,
    packages: bool,
}

/// The family of a root in the pinned namespace: that URI only.
pub(super) const PINNED: Family<'static> = Family {
    base: SACM_NS,
    packages: false,
};

impl Family<'_> {
    /// Whether `uri` is in the family.
    pub(super) fn contains(self, uri: &str) -> bool {
        uri == self.base || (self.packages && is_package_of(uri, self.base))
    }
}

/// Whether `uri` is `base` followed by `/` and one or more ASCII letters.
fn is_package_of(uri: &str, base: &str) -> bool {
    uri.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|package| {
            !package.is_empty() && package.bytes().all(|b| b.is_ascii_alphabetic())
        })
}

/// Whether `uri` is `base`, alone or with a package.
fn is_of(uri: &str, base: &str) -> bool {
    uri == base || is_package_of(uri, base)
}

/// The class of the namespace `uri` of a root, `None` for no namespace, in
/// this order: none or empty, the pinned one, SACM 2.4, the recognised
/// revisions, the undated SACM 2.3 with its packages, any URI containing
/// `sacm` in any letter case, and anything else.
pub(super) fn classify(uri: Option<&str>) -> RootNamespace<'_> {
    let Some(uri) = uri.filter(|uri| !uri.is_empty()) else {
        return RootNamespace::NotSacm;
    };
    if uri == SACM_NS {
        return RootNamespace::Pinned;
    }
    if SACM_24.iter().any(|base| is_of(uri, base)) {
        return RootNamespace::Sacm24;
    }
    if let Some(&(base, revision)) = REVISIONS.iter().find(|(base, _)| is_of(uri, base)) {
        return RootNamespace::OtherRevision {
            revision,
            family: Family {
                base,
                packages: true,
            },
        };
    }
    if is_of(uri, UNDATED_23) {
        return RootNamespace::SacmLike {
            family: Family {
                base: UNDATED_23,
                packages: true,
            },
        };
    }
    if uri.to_ascii_lowercase().contains("sacm") {
        return RootNamespace::SacmLike {
            family: Family {
                base: uri,
                packages: false,
            },
        };
    }
    RootNamespace::NotSacm
}

#[cfg(test)]
mod tests {
    use super::{RootNamespace, classify};

    #[test]
    fn a_package_is_one_or_more_ascii_letters_after_a_slash() {
        let older = |uri| matches!(classify(Some(uri)), RootNamespace::OtherRevision { .. });
        assert!(older("http://omg.sacm/2.2/argumentation"));
        assert!(!older("http://omg.sacm/2.2/"));
        assert!(!older("http://omg.sacm/2.2/arg1"));
        assert!(!older("http://omg.sacm/2.22"));
        assert_eq!(classify(Some("")), RootNamespace::NotSacm);
        assert_eq!(classify(None), RootNamespace::NotSacm);
    }

    #[test]
    fn a_family_holds_its_base_and_its_packages_only() {
        let Some(family) = classify(Some("http://omg.sacm/2.2/assurancecase")).family() else {
            panic!("an older revision has a family");
        };
        assert!(family.contains("http://omg.sacm/2.2"));
        assert!(family.contains("http://omg.sacm/2.2/argumentation"));
        assert!(!family.contains("http://omg.sacm/2.1/argumentation"));
        assert!(!family.contains("http://omg.sacm/2.2/"));
        let Some(exact) = classify(Some("urn:example:sacm")).family() else {
            panic!("a SACM-like namespace has a family");
        };
        assert!(exact.contains("urn:example:sacm"));
        assert!(!exact.contains("urn:example:sacm/x"));
        // The undated SACM 2.3 is SACM-like, with its packages.
        let Some(undated) =
            classify(Some("http://www.omg.org/spec/SACM/2.3/Argumentation")).family()
        else {
            panic!("the undated SACM 2.3 has a family");
        };
        assert!(undated.contains("http://www.omg.org/spec/SACM/2.3/Base"));
        assert!(undated.contains("http://www.omg.org/spec/SACM/2.3"));
    }
}
