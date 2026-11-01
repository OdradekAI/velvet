use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crc32fast::hash;
use velvet_persistence::{
    DurabilityMode, DurabilityReport, DurabilityReportStore, JsonFileDurabilityReportStore,
    JsonFileRecoverySetStore, JsonFileStateSnapshotStore, RecoverySet, RecoverySetMember,
    RecoverySetStore, SnapshotError, StateSnapshot, StateSnapshotMetadata, StateSnapshotStore,
    CORE_RECOVERY_SET_ID,
};

fn temporary_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "velvet-release-qualification-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos()
    ))
}

fn snapshot(generation: u64, committed: bool) -> StateSnapshot {
    let payload = format!("layout-{generation}").into_bytes();
    StateSnapshot {
        metadata: StateSnapshotMetadata {
            owner_id: "layout".into(),
            scope: "workspace".into(),
            schema_version: 1,
            generation,
            created_at_ms: generation,
            checksum: hash(&payload),
            committed,
        },
        payload,
    }
}

fn recovery_set(generation: u64, snapshot: &StateSnapshot, committed: bool) -> RecoverySet {
    let mut members = BTreeMap::new();
    members.insert(
        "layout:workspace".into(),
        RecoverySetMember {
            owner_id: "layout".into(),
            scope: "workspace".into(),
            generation: snapshot.metadata.generation,
            schema_version: snapshot.metadata.schema_version,
            checksum: snapshot.metadata.checksum,
        },
    );
    RecoverySet {
        set_id: CORE_RECOVERY_SET_ID.into(),
        generation,
        created_at_ms: generation,
        members,
        committed,
    }
}

#[test]
fn power_loss_boundary_keeps_last_committed_snapshot_and_recovery_pointer() {
    let root = temporary_root("power-loss");
    let snapshot_root = root.join("snapshots");
    let recovery_root = root.join("recovery-sets");

    let store = JsonFileStateSnapshotStore::new(&snapshot_root);
    store
        .stage(&snapshot(1, false))
        .expect("first stage should work");
    store
        .commit("layout", "workspace", 1)
        .expect("first commit should work");
    let set_store = JsonFileRecoverySetStore::new(&recovery_root);
    set_store
        .stage(&recovery_set(1, &snapshot(1, true), false))
        .expect("first recovery set should stage");
    set_store
        .commit(CORE_RECOVERY_SET_ID, 1)
        .expect("first recovery set should commit");

    // Simulate a process/power loss after staged bytes are durable but before
    // the owner commit and recovery-index publication steps.
    store
        .stage(&snapshot(2, false))
        .expect("second stage should work");
    set_store
        .stage(&recovery_set(2, &snapshot(2, true), false))
        .expect("second recovery set should stage");
    drop(store);
    drop(set_store);

    let reopened = JsonFileStateSnapshotStore::new(&snapshot_root);
    assert_eq!(
        reopened
            .load_latest("layout", "workspace")
            .expect("reopen should read the prior committed snapshot")
            .expect("generation one should remain authoritative")
            .metadata
            .generation,
        1
    );
    assert!(snapshot_root.join("layout/workspace/2.stage.json").exists());
    let reopened_sets = JsonFileRecoverySetStore::new(&recovery_root);
    assert_eq!(
        reopened_sets
            .load_latest(CORE_RECOVERY_SET_ID)
            .expect("reopen should read the prior recovery pointer")
            .expect("generation one should remain authoritative")
            .generation,
        1
    );
    assert!(recovery_root.join("core/2.stage.json").exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn abrupt_process_termination_keeps_the_previous_generation() {
    let root = temporary_root("abrupt-process");
    let ready = root.join("child-ready");
    fs::create_dir_all(&root).expect("qualification root should exist");
    let mut child = Command::new(std::env::current_exe().expect("test binary should exist"))
        .args(["--exact", "power_loss_child", "--nocapture"])
        .env("VELVET_RELEASE_POWER_LOSS_ROOT", &root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("child qualification process should start");
    for _ in 0..100 {
        if ready.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        ready.exists(),
        "child should reach the staged-write boundary"
    );
    child.kill().expect("child should be terminable");
    let status = child.wait().expect("child status should be available");
    assert!(!status.success(), "the child should end abnormally");

    let reopened = JsonFileStateSnapshotStore::new(root.join("snapshots"));
    assert_eq!(
        reopened
            .load_latest("layout", "workspace")
            .expect("reopen should read the prior generation")
            .expect("prior generation should remain authoritative")
            .metadata
            .generation,
        1
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn power_loss_child() {
    let Ok(root) = std::env::var("VELVET_RELEASE_POWER_LOSS_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let store = JsonFileStateSnapshotStore::new(root.join("snapshots"));
    store
        .stage(&snapshot(1, false))
        .expect("child stage one should work");
    store
        .commit("layout", "workspace", 1)
        .expect("child commit one should work");
    store
        .stage(&snapshot(2, false))
        .expect("child stage two should work");
    fs::write(root.join("child-ready"), b"ready").expect("child ready marker should write");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn unreadable_snapshot_directory_is_reported_without_default_overwrite() {
    let root = temporary_root("unreadable-directory");
    let directory = root.join("layout");
    fs::create_dir_all(&directory).expect("owner directory should exist");
    let occupied_path = directory.join("workspace");
    fs::write(&occupied_path, b"preserve this evidence").expect("fixture should write");

    let store = JsonFileStateSnapshotStore::new(&root);
    assert!(matches!(
        store.load_latest("layout", "workspace"),
        Err(SnapshotError::Unavailable(_))
    ));
    assert!(matches!(
        store.stage(&snapshot(1, false)),
        Err(SnapshotError::Unavailable(_))
    ));
    assert_eq!(
        fs::read(&occupied_path).expect("unreadable-directory evidence should remain"),
        b"preserve this evidence"
    );

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn acl_and_readonly_snapshot_directory_are_typed_unavailable() {
    use std::os::unix::fs::PermissionsExt;

    let root = temporary_root("acl-readonly");
    let store = JsonFileStateSnapshotStore::new(&root);
    store
        .stage(&snapshot(1, false))
        .expect("initial stage should work");
    store
        .commit("layout", "workspace", 1)
        .expect("initial commit should work");

    let scope = root.join("layout/workspace");
    let restore_permissions = || {
        fs::set_permissions(&scope, fs::Permissions::from_mode(0o700))
            .expect("snapshot directory permissions should be restorable");
    };

    fs::set_permissions(&scope, fs::Permissions::from_mode(0o000))
        .expect("snapshot directory should become unreadable");
    let unreadable = store.load_latest("layout", "workspace");
    if unreadable.is_ok() {
        // Root and other privilege-bypassing test runners can read mode-000
        // directories. The native harness records that limitation separately;
        // this test must not claim an ACL denial that the OS did not enforce.
        eprintln!("ACL probe skipped: the test process can bypass mode-000 permissions");
        restore_permissions();
        let _ = fs::remove_dir_all(root);
        return;
    }
    assert!(matches!(unreadable, Err(SnapshotError::Unavailable(_))));

    restore_permissions();
    fs::set_permissions(&scope, fs::Permissions::from_mode(0o555))
        .expect("snapshot directory should become read-only");
    let readonly_write = store.stage(&snapshot(2, false));
    if readonly_write.is_ok() {
        eprintln!(
            "read-only probe skipped: the test process can bypass directory write permissions"
        );
    } else {
        assert!(matches!(readonly_write, Err(SnapshotError::Unavailable(_))));
    }

    restore_permissions();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn durability_report_release_matrix_matches_platform_capabilities() {
    let atomic = DurabilityReport::for_mode(DurabilityMode::AtomicRenameOnly);
    assert!(atomic.atomic_replace);
    assert!(!atomic.file_sync);
    assert!(!atomic.directory_sync);
    assert!(!atomic.crash_durable);

    let required = DurabilityReport::for_mode(DurabilityMode::RequireFsync);
    assert!(required.atomic_replace);
    if cfg!(unix) {
        assert!(required.file_sync);
        assert!(required.directory_sync);
        assert!(required.crash_durable);
    } else {
        assert!(!required.file_sync);
        assert!(!required.directory_sync);
        assert!(!required.crash_durable);
        assert!(required.detail.contains("unavailable"));
    }

    let root = temporary_root("durability-report");
    let path = root.join("durability-report.json");
    let store = JsonFileDurabilityReportStore::new(&path);
    store
        .save(&required)
        .expect("report should publish atomically");
    assert_eq!(
        store.load().expect("report should round-trip"),
        Some(required.clone())
    );
    let mut unsupported = required;
    unsupported.schema_version += 1;
    fs::write(
        &path,
        serde_json::to_vec(&unsupported).expect("unsupported report should encode"),
    )
    .expect("unsupported report should write");
    assert!(store.load().is_err());
    let _ = fs::remove_dir_all(root);
}
