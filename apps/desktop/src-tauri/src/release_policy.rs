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
