//! The namespaces the reader and the writer share: one pin each (D-NS-PIN;
//! ADR 0017 W5 keeps them crate-private). No `xmi:version` is pinned: the XMI
//! namespace identifies the version (XMI 2.5.1 §7.5.3; ADR 0017 amendment 3).

/// The pinned SACM 2.3 namespace (D-NS-PIN).
pub(super) const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
pub(super) const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The prefix written for [`SACM_NS`].
pub(super) const SACM_PREFIX: &str = "sacm";
/// The prefix written for [`XMI_NS`].
pub(super) const XMI_PREFIX: &str = "xmi";
