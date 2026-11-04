use ed25519_dalek::VerifyingKey;
use velvet_application_core::ReleaseTrustPolicy;
use velvet_workbench_protocol::ReleaseChannel;

/// Pinned public root for the preview channel. The corresponding private key
/// is intentionally kept outside the repository and release build machines.
pub const PREVIEW_RELEASE_ROOT_KEY_ID: &str = "velvet-preview-root-1";
const PREVIEW_RELEASE_ROOT_KEY_BYTES: [u8; 32] = [
    0x35, 0xfd, 0x9b, 0x0f, 0xb4, 0x99, 0xd1, 0xc3, 0x38, 0x10, 0xd2, 0xab, 0x88, 0x73,
    0xd7, 0xd5, 0x41, 0xd5, 0x61, 0xd2, 0x8c, 0x71, 0xa6, 0xa3, 0x4e, 0xc3, 0x55, 0xc2,
    0xf5, 0x21, 0x1d, 0x2b,
];

pub fn preview_release_trust_policy() -> ReleaseTrustPolicy {
    let root_key = VerifyingKey::from_bytes(&PREVIEW_RELEASE_ROOT_KEY_BYTES)
        .expect("the pinned preview release root must be a valid Ed25519 key");
    ReleaseTrustPolicy::new(PREVIEW_RELEASE_ROOT_KEY_ID, root_key, ReleaseChannel::Preview)
}

/// Pinned publisher-metadata root for the preview channel. It is intentionally
/// separate from the application release root: an extension publisher keyset
/// can rotate without granting authority over application artifacts.
pub const PREVIEW_PLUGIN_PUBLISHER_ROOT_KEY_ID: &str = "velvet-preview-plugin-root-1";
const PREVIEW_PLUGIN_PUBLISHER_ROOT_KEY_BYTES: [u8; 32] = [
    0x0b, 0xde, 0xfc, 0x08, 0x57, 0x49, 0x59, 0xef, 0x30, 0x7e, 0xb5, 0x2c, 0x5f, 0x28,
    0xf2, 0x11, 0x92, 0x60, 0xb3, 0x2a, 0xad, 0x21, 0x51, 0x2c, 0xc7, 0x65, 0xf4, 0xc6,
    0x37, 0x06, 0x6d, 0xf0,
];

pub fn preview_plugin_publisher_root() -> (&'static str, VerifyingKey) {
    let root_key = VerifyingKey::from_bytes(&PREVIEW_PLUGIN_PUBLISHER_ROOT_KEY_BYTES)
        .expect("the pinned preview publisher root must be a valid Ed25519 key");
    (PREVIEW_PLUGIN_PUBLISHER_ROOT_KEY_ID, root_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publisher_root_is_pinned_separately_from_release_root() {
        let (publisher_id, publisher_key) = preview_plugin_publisher_root();
        assert_eq!(publisher_id, PREVIEW_PLUGIN_PUBLISHER_ROOT_KEY_ID);
        assert_ne!(publisher_id, PREVIEW_RELEASE_ROOT_KEY_ID);
        assert_ne!(publisher_key.as_bytes(), &PREVIEW_RELEASE_ROOT_KEY_BYTES);
    }
}
