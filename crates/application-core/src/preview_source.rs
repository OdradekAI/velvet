use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use velvet_asset_core::{AssetId, ContentFingerprint, ContentKindId};
use velvet_persistence::{AssetSourceLocationRecord, AssetSourceLocationStore, PersistenceError};
use velvet_preview_core::{
    PreviewAssetContent, PreviewAssetSource, PreviewContext, PreviewReadRequest, PreviewSourceError,
};

/// Resolves an opaque Asset identity to an application-owned source location.
/// The returned path is consumed only by the application source adapter and
/// never crosses the preview provider seam.
pub trait AssetSourceLocationResolver: Send + Sync {
    fn resolve(&self, asset_id: &AssetId) -> Result<PathBuf, PreviewSourceError>;

    /// Returns the source-owned content kind when the registry has one.
    /// Existing composition-only resolvers may leave this unset and continue
    /// using an explicit content kind supplied by the preview request.
    fn content_kind(
        &self,
        _asset_id: &AssetId,
    ) -> Result<Option<ContentKindId>, PreviewSourceError> {
        Ok(None)
    }

    /// Returns the source-owned identity of the complete content at this
    /// location. Resolvers that cannot provide one leave derived work
    /// uncacheable rather than guessing from an opaque path.
    fn fingerprint(
        &self,
        _asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        Ok(None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewAssetRegistration {
    pub asset_id: AssetId,
    pub source_path: PathBuf,
    pub content_kind: ContentKindId,
    pub fingerprint: Option<ContentFingerprint>,
    pub volume_identity: Option<String>,
    pub approved_root: Option<PathBuf>,
}

#[derive(Clone)]
struct StoredAssetSourceLocation {
    source_path: PathBuf,
    content_kind: ContentKindId,
    fingerprint: Option<ContentFingerprint>,
    volume_identity: Option<String>,
    approved_root: Option<PathBuf>,
}

struct AssetSourceLocationState {
    locations: BTreeMap<AssetId, StoredAssetSourceLocation>,
    approved_roots: BTreeSet<PathBuf>,
}

/// Rust-owned persistent mapping from opaque Asset identities to local files.
///
/// The registry owns registration and persistence; `FilePreviewAssetSource`
/// consumes its resolved path only inside the application core. The path is
/// never returned by the protocol or passed to a provider or plugin runtime.
pub struct PersistentAssetSourceLocationResolver {
    state: RwLock<AssetSourceLocationState>,
    store: Arc<dyn AssetSourceLocationStore>,
    load_error: Option<PreviewSourceError>,
}

impl PersistentAssetSourceLocationResolver {
    pub fn new(store: Arc<dyn AssetSourceLocationStore>) -> Self {
        let (locations, mut load_error) = match store.load() {
            Ok(records) => (
                records.into_iter().filter_map(record_to_location).collect(),
                None,
            ),
            Err(error) => (BTreeMap::new(), Some(map_persistence_error(error))),
        };
        let approved_roots = match store.load_approved_roots() {
            Ok(roots) => roots
                .into_iter()
                .map(PathBuf::from)
                .filter(|root| root.is_absolute())
                .collect(),
            Err(error) => {
                if load_error.is_none() {
                    load_error = Some(map_persistence_error(error));
                }
                BTreeSet::new()
            }
        };
        Self {
            state: RwLock::new(AssetSourceLocationState {
                locations,
                approved_roots,
            }),
            store,
            load_error,
        }
    }

    pub fn load_error(&self) -> Option<&PreviewSourceError> {
        self.load_error.as_ref()
    }

    pub fn approved_roots(&self) -> Result<Vec<PathBuf>, PreviewSourceError> {
        self.ensure_state_writable()?;
        self.state
            .read()
            .map(|state| state.approved_roots.iter().cloned().collect())
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))
    }

    pub fn add_approved_root(&self, root: impl AsRef<Path>) -> Result<PathBuf, PreviewSourceError> {
        self.ensure_state_writable()?;
        let root = canonical_approved_root(root.as_ref())?;
        let mut state = self
            .state
            .write()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?;
        if state.approved_roots.contains(&root) {
            return Ok(root);
        }
        state.approved_roots.insert(root.clone());
        if let Err(error) = self.store.save_state(
            &location_records(&state.locations),
            &root_strings(&state.approved_roots),
        ) {
            state.approved_roots.remove(&root);
            return Err(map_persistence_error(error));
        }
        Ok(root)
    }

    pub fn remove_approved_root(&self, root: impl AsRef<Path>) -> Result<bool, PreviewSourceError> {
        self.ensure_state_writable()?;
        let root = canonical_root_for_removal(root.as_ref())?;
        let mut state = self
            .state
            .write()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?;
        if !state.approved_roots.remove(&root) {
            return Ok(false);
        }
        if let Err(error) = self.store.save_state(
            &location_records(&state.locations),
            &root_strings(&state.approved_roots),
        ) {
            state.approved_roots.insert(root);
            return Err(map_persistence_error(error));
        }
        Ok(true)
    }

    pub fn register(
        &self,
        asset_id: AssetId,
        source_path: impl AsRef<Path>,
        content_kind: ContentKindId,
        fingerprint: Option<ContentFingerprint>,
    ) -> Result<(), PreviewSourceError> {
        self.register_with_approved_root(asset_id, source_path, content_kind, fingerprint, None)
    }

    pub fn register_with_approved_root(
        &self,
        asset_id: AssetId,
        source_path: impl AsRef<Path>,
        content_kind: ContentKindId,
        fingerprint: Option<ContentFingerprint>,
        approved_root: Option<&Path>,
    ) -> Result<(), PreviewSourceError> {
        self.ensure_state_writable()?;
        validate_asset_id(&asset_id)?;
        validate_content_kind(&content_kind)?;
        validate_fingerprint(&fingerprint)?;

        let source_path = fs::canonicalize(source_path.as_ref()).map_err(map_io_error)?;
        let metadata = fs::metadata(&source_path).map_err(map_io_error)?;
        if !metadata.is_file() {
            return Err(PreviewSourceError::NotFound);
        }
        let approved_root = approved_root.map(canonical_approved_root).transpose()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?;
        if let Some(root) = approved_root.as_deref() {
            if !state.approved_roots.contains(root) {
                return Err(PreviewSourceError::PermissionDenied);
            }
            ensure_path_is_within_root(&source_path, root)?;
        }
        let volume_identity = source_volume_identity(&metadata);

        let registration = StoredAssetSourceLocation {
            source_path,
            content_kind,
            fingerprint,
            volume_identity,
            approved_root,
        };
        let key = asset_id.clone();
        let previous = state.locations.insert(key.clone(), registration);
        if let Err(error) = self.store.save_state(
            &location_records(&state.locations),
            &root_strings(&state.approved_roots),
        ) {
            if let Some(previous) = previous {
                state.locations.insert(key, previous);
            } else {
                state.locations.remove(&key);
            }
            return Err(map_persistence_error(error));
        }
        Ok(())
    }

    pub fn remove(&self, asset_id: &AssetId) -> Result<bool, PreviewSourceError> {
        self.ensure_state_writable()?;
        validate_asset_id(asset_id)?;
        let mut state = self
            .state
            .write()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?;
        let Some(previous) = state.locations.remove(asset_id) else {
            return Ok(false);
        };
        if let Err(error) = self.store.save_state(
            &location_records(&state.locations),
            &root_strings(&state.approved_roots),
        ) {
            state.locations.insert(asset_id.clone(), previous);
            return Err(map_persistence_error(error));
        }
        Ok(true)
    }

    pub fn registration(
        &self,
        asset_id: &AssetId,
    ) -> Result<PreviewAssetRegistration, PreviewSourceError> {
        validate_asset_id(asset_id)?;
        let state = self
            .state
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?;
        let Some(location) = state.locations.get(asset_id) else {
            return Err(PreviewSourceError::NotFound);
        };
        Ok(PreviewAssetRegistration {
            asset_id: asset_id.clone(),
            source_path: location.source_path.clone(),
            content_kind: location.content_kind.clone(),
            fingerprint: location.fingerprint.clone(),
            volume_identity: location.volume_identity.clone(),
            approved_root: location.approved_root.clone(),
        })
    }

    fn ensure_state_writable(&self) -> Result<(), PreviewSourceError> {
        self.load_error.clone().map_or(Ok(()), Err)
    }
}

impl AssetSourceLocationResolver for PersistentAssetSourceLocationResolver {
    fn resolve(&self, asset_id: &AssetId) -> Result<PathBuf, PreviewSourceError> {
        validate_asset_id(asset_id)?;
        let location = self
            .state
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source state lock poisoned".into()))?
            .locations
            .get(asset_id)
            .cloned()
            .ok_or(PreviewSourceError::NotFound)?;
        let canonical = fs::canonicalize(&location.source_path).map_err(map_revalidation_error)?;
        if (location.volume_identity.is_some() || location.approved_root.is_some())
            && canonical != location.source_path
        {
            return Err(PreviewSourceError::Unavailable);
        }
        if let Some(expected_root) = location.approved_root.as_deref() {
            let root = fs::canonicalize(expected_root).map_err(map_revalidation_error)?;
            if root != expected_root {
                return Err(PreviewSourceError::Unavailable);
            }
            let state = self.state.read().map_err(|_| {
                PreviewSourceError::Failed("asset source state lock poisoned".into())
            })?;
            if !state.approved_roots.contains(&root) {
                return Err(PreviewSourceError::PermissionDenied);
            }
            ensure_path_is_within_root(&canonical, &root)?;
        }
        let metadata = fs::metadata(&canonical).map_err(map_revalidation_error)?;
        if !metadata.is_file() {
            return Err(PreviewSourceError::Unavailable);
        }
        if let Some(expected) = location.volume_identity.as_deref() {
            if source_volume_identity(&metadata).as_deref() != Some(expected) {
                return Err(PreviewSourceError::Unavailable);
            }
        }
        Ok(location.source_path.clone())
    }

    fn content_kind(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentKindId>, PreviewSourceError> {
        Ok(Some(self.registration(asset_id)?.content_kind))
    }

    fn fingerprint(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        Ok(self.registration(asset_id)?.fingerprint)
    }
}

fn record_to_location(
    record: AssetSourceLocationRecord,
) -> Option<(AssetId, StoredAssetSourceLocation)> {
    let asset_id = AssetId(record.asset_id);
    let content_kind = ContentKindId(record.content_kind);
    let source_path = PathBuf::from(record.source_path);
    let fingerprint = record.fingerprint.map(ContentFingerprint);
    let approved_root = record.approved_root.map(PathBuf::from);
    if validate_asset_id(&asset_id).is_err()
        || validate_content_kind(&content_kind).is_err()
        || !source_path.is_absolute()
        || validate_fingerprint(&fingerprint).is_err()
        || approved_root
            .as_deref()
            .is_some_and(|root| !root.is_absolute())
    {
        return None;
    }
    Some((
        asset_id,
        StoredAssetSourceLocation {
            source_path,
            content_kind,
            fingerprint,
            volume_identity: record.volume_identity,
            approved_root,
        },
    ))
}

fn location_records(
    locations: &BTreeMap<AssetId, StoredAssetSourceLocation>,
) -> Vec<AssetSourceLocationRecord> {
    locations
        .iter()
        .map(|(asset_id, location)| AssetSourceLocationRecord {
            asset_id: asset_id.0.clone(),
            source_path: location.source_path.to_string_lossy().into_owned(),
            content_kind: location.content_kind.0.clone(),
            fingerprint: location
                .fingerprint
                .as_ref()
                .map(|fingerprint| fingerprint.0.clone()),
            volume_identity: location.volume_identity.clone(),
            approved_root: location
                .approved_root
                .as_ref()
                .map(|root| root.to_string_lossy().into_owned()),
        })
        .collect()
}

fn validate_asset_id(asset_id: &AssetId) -> Result<(), PreviewSourceError> {
    if asset_id.0.trim().is_empty() {
        Err(PreviewSourceError::InvalidRequest(
            "asset id cannot be empty".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_content_kind(content_kind: &ContentKindId) -> Result<(), PreviewSourceError> {
    if content_kind.0.trim().is_empty() {
        Err(PreviewSourceError::InvalidRequest(
            "content kind cannot be empty".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_fingerprint(
    fingerprint: &Option<ContentFingerprint>,
) -> Result<(), PreviewSourceError> {
    if fingerprint
        .as_ref()
        .is_some_and(|fingerprint| fingerprint.0.trim().is_empty())
    {
        Err(PreviewSourceError::InvalidRequest(
            "fingerprint cannot be empty".into(),
        ))
    } else {
        Ok(())
    }
}

fn canonical_approved_root(path: &Path) -> Result<PathBuf, PreviewSourceError> {
    let root = fs::canonicalize(path).map_err(map_io_error)?;
    let metadata = fs::metadata(&root).map_err(map_io_error)?;
    if !metadata.is_dir() {
        return Err(PreviewSourceError::InvalidRequest(
            "approved source root must be a directory".into(),
        ));
    }
    Ok(root)
}

fn canonical_root_for_removal(path: &Path) -> Result<PathBuf, PreviewSourceError> {
    if path.exists() {
        return canonical_approved_root(path);
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_| PreviewSourceError::Unavailable)
    }
}

fn root_strings(roots: &BTreeSet<PathBuf>) -> Vec<String> {
    roots
        .iter()
        .map(|root| root.to_string_lossy().into_owned())
        .collect()
}

fn ensure_path_is_within_root(path: &Path, root: &Path) -> Result<(), PreviewSourceError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(PreviewSourceError::PermissionDenied)
    }
}

fn source_volume_identity(metadata: &fs::Metadata) -> Option<String> {
    #[cfg(unix)]
    {
        Some(format!("unix-dev:{}", metadata.dev()))
    }
    #[cfg(windows)]
    {
        let _ = metadata;
        None
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        None
    }
}

fn map_revalidation_error(error: io::Error) -> PreviewSourceError {
    match error.kind() {
        io::ErrorKind::NotFound => PreviewSourceError::Unavailable,
        io::ErrorKind::PermissionDenied => PreviewSourceError::PermissionDenied,
        _ => PreviewSourceError::Failed("asset source could not be revalidated".into()),
    }
}

fn map_persistence_error(error: PersistenceError) -> PreviewSourceError {
    match error {
        PersistenceError::Unavailable => {
            PreviewSourceError::Failed("asset source location store is unavailable".into())
        }
        PersistenceError::CorruptState => {
            PreviewSourceError::Failed("asset source location state is corrupt".into())
        }
        PersistenceError::CorruptOwner { .. } => {
            PreviewSourceError::Failed("asset source location state is corrupt".into())
        }
    }
}

/// Read-only source adapter used by the current skeleton and contract tests.
///
/// `ApplicationCore` owns the source instance. The adapter deliberately stores
/// only typed content keyed by opaque `AssetId`; no path, database handle or
/// credential crosses the preview seam. A future Asset source-location/store
/// adapter can replace this implementation without changing `preview-core`.
#[derive(Default)]
pub struct InMemoryPreviewAssetSource {
    assets: RwLock<BTreeMap<AssetId, PreviewAssetRecord>>,
}

struct PreviewAssetRecord {
    content: PreviewAssetContent,
    fingerprint: Option<ContentFingerprint>,
}

impl InMemoryPreviewAssetSource {
    pub fn insert_text(&self, asset_id: AssetId, text: impl Into<String>) {
        self.insert(
            asset_id,
            PreviewAssetContent::Text {
                text: text.into(),
                truncated: false,
            },
        );
    }

    pub fn insert_bytes(&self, asset_id: AssetId, bytes: impl Into<Vec<u8>>) {
        self.insert(
            asset_id,
            PreviewAssetContent::Bytes {
                bytes: bytes.into(),
                truncated: false,
            },
        );
    }

    pub fn insert(&self, asset_id: AssetId, content: PreviewAssetContent) {
        if let Ok(mut assets) = self.assets.write() {
            assets.insert(
                asset_id,
                PreviewAssetRecord {
                    content,
                    fingerprint: None,
                },
            );
        }
    }

    pub fn insert_with_fingerprint(
        &self,
        asset_id: AssetId,
        content: PreviewAssetContent,
        fingerprint: ContentFingerprint,
    ) {
        if let Ok(mut assets) = self.assets.write() {
            assets.insert(
                asset_id,
                PreviewAssetRecord {
                    content,
                    fingerprint: Some(fingerprint),
                },
            );
        }
    }

    pub fn remove(&self, asset_id: &AssetId) -> Option<PreviewAssetContent> {
        self.assets
            .write()
            .ok()
            .and_then(|mut assets| assets.remove(asset_id).map(|record| record.content))
    }
}

impl PreviewAssetSource for InMemoryPreviewAssetSource {
    fn read(
        &self,
        context: &PreviewContext,
        request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError> {
        let assets = self
            .assets
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source lock poisoned".into()))?;
        let Some(content) = assets.get(&context.asset_id) else {
            return Err(PreviewSourceError::NotFound);
        };
        Ok(bound_content(&content.content, request.max_bytes()))
    }

    fn fingerprint(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        let assets = self
            .assets
            .read()
            .map_err(|_| PreviewSourceError::Failed("asset source lock poisoned".into()))?;
        let Some(record) = assets.get(asset_id) else {
            return Err(PreviewSourceError::NotFound);
        };
        Ok(record.fingerprint.clone())
    }
}

/// Bounded filesystem adapter for trusted application composition.
///
/// The resolver owns source-location policy (roots, permissions and identity
/// mapping); this adapter owns bounded reads and typed content conversion.
/// Providers still receive only `PreviewAssetContent`.
pub struct FilePreviewAssetSource {
    locations: Arc<dyn AssetSourceLocationResolver>,
}

impl FilePreviewAssetSource {
    pub fn new(locations: Arc<dyn AssetSourceLocationResolver>) -> Self {
        Self { locations }
    }
}

impl PreviewAssetSource for FilePreviewAssetSource {
    fn read(
        &self,
        context: &PreviewContext,
        request: PreviewReadRequest,
    ) -> Result<PreviewAssetContent, PreviewSourceError> {
        let path = self.locations.resolve(&context.asset_id)?;
        let file = File::open(path).map_err(map_io_error)?;
        let max_bytes = request.max_bytes();
        let mut bytes = Vec::with_capacity(max_bytes.saturating_add(1));
        file.take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(map_io_error)?;
        let truncated = bytes.len() > max_bytes;
        if truncated {
            bytes.truncate(max_bytes);
        }

        if is_text_content_kind(&context.content_kind.0) {
            if let Some(text) = decode_bounded_utf8(&bytes) {
                return Ok(PreviewAssetContent::Text { text, truncated });
            }
        }

        Ok(PreviewAssetContent::Bytes { bytes, truncated })
    }

    fn fingerprint(
        &self,
        asset_id: &AssetId,
    ) -> Result<Option<ContentFingerprint>, PreviewSourceError> {
        self.locations.fingerprint(asset_id)
    }
}

fn map_io_error(error: io::Error) -> PreviewSourceError {
    match error.kind() {
        io::ErrorKind::NotFound => PreviewSourceError::NotFound,
        io::ErrorKind::PermissionDenied => PreviewSourceError::PermissionDenied,
        _ => PreviewSourceError::Failed(error.to_string()),
    }
}

fn is_text_content_kind(content_kind: &str) -> bool {
    content_kind.starts_with("text/")
        || matches!(
            content_kind,
            "application/json"
                | "application/javascript"
                | "application/xml"
                | "application/yaml"
                | "application/x-yaml"
                | "application/toml"
        )
}

fn decode_bounded_utf8(bytes: &[u8]) -> Option<String> {
    let mut end = bytes.len();
    while end > 0 {
        if let Ok(text) = std::str::from_utf8(&bytes[..end]) {
            return Some(text.to_owned());
        }
        end -= 1;
    }
    if bytes.is_empty() {
        Some(String::new())
    } else {
        None
    }
}

fn bound_content(content: &PreviewAssetContent, max_bytes: usize) -> PreviewAssetContent {
    match content {
        PreviewAssetContent::Text { text, truncated } => {
            if text.len() <= max_bytes {
                return PreviewAssetContent::Text {
                    text: text.clone(),
                    truncated: *truncated,
                };
            }

            let mut end = max_bytes;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            PreviewAssetContent::Text {
                text: text[..end].to_string(),
                truncated: true,
            }
        }
        PreviewAssetContent::Bytes { bytes, truncated } => PreviewAssetContent::Bytes {
            bytes: bytes[..bytes.len().min(max_bytes)].to_vec(),
            truncated: *truncated || bytes.len() > max_bytes,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use velvet_persistence::InMemoryAssetSourceLocationStore;

    fn context(asset_id: &str) -> PreviewContext {
        PreviewContext {
            asset_id: AssetId(asset_id.into()),
            content_kind: velvet_asset_core::ContentKindId("text/plain".into()),
            intent: velvet_preview_core::PreviewIntent::Quick,
            surface: velvet_preview_core::PreviewSurface::Workspace,
        }
    }

    #[test]
    fn reads_typed_content_with_a_byte_bound_and_utf8_boundary() {
        let source = InMemoryPreviewAssetSource::default();
        source.insert_text(AssetId("asset-1".into()), "你好 preview");

        let content = source
            .read(&context("asset-1"), PreviewReadRequest::new(6))
            .expect("the source should find the asset");
        assert_eq!(
            content,
            PreviewAssetContent::Text {
                text: "你好".into(),
                truncated: true,
            }
        );
    }

    #[test]
    fn reports_missing_assets_without_exposing_storage_details() {
        let source = InMemoryPreviewAssetSource::default();
        assert_eq!(
            source.read(&context("missing"), PreviewReadRequest::default()),
            Err(PreviewSourceError::NotFound)
        );
        assert_eq!(
            source.fingerprint(&AssetId("missing".into())),
            Err(PreviewSourceError::NotFound)
        );
    }

    #[test]
    fn returns_source_owned_fingerprints_and_clears_them_on_unknown_updates() {
        let source = InMemoryPreviewAssetSource::default();
        source.insert_with_fingerprint(
            AssetId("asset-1".into()),
            PreviewAssetContent::Text {
                text: "v1".into(),
                truncated: false,
            },
            ContentFingerprint("sha256:v1".into()),
        );
        assert_eq!(
            source.fingerprint(&AssetId("asset-1".into())),
            Ok(Some(ContentFingerprint("sha256:v1".into())))
        );

        source.insert_text(AssetId("asset-1".into()), "v2");
        assert_eq!(source.fingerprint(&AssetId("asset-1".into())), Ok(None));
    }

    struct MapAssetLocations(BTreeMap<AssetId, PathBuf>);

    impl AssetSourceLocationResolver for MapAssetLocations {
        fn resolve(&self, asset_id: &AssetId) -> Result<PathBuf, PreviewSourceError> {
            self.0
                .get(asset_id)
                .cloned()
                .ok_or(PreviewSourceError::NotFound)
        }
    }

    #[test]
    fn file_source_reads_bounded_text_without_exposing_the_path() {
        let path = std::env::temp_dir().join(format!(
            "velvet-preview-source-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::write(&path, "你好 preview").expect("test source should be writable");

        let mut locations = BTreeMap::new();
        locations.insert(AssetId("asset-1".into()), path.clone());
        let source = FilePreviewAssetSource::new(Arc::new(MapAssetLocations(locations)));
        let content = source
            .read(&context("asset-1"), PreviewReadRequest::new(6))
            .expect("the file source should read the mapped asset");
        assert_eq!(
            content,
            PreviewAssetContent::Text {
                text: "你好".into(),
                truncated: true,
            }
        );

        fs::remove_file(path).expect("test source should be cleaned up");
    }

    #[test]
    fn persistent_locations_round_trip_and_remove_without_exposing_paths() {
        let path = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-source-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::write(&path, "persistent preview").expect("test source should be writable");

        let store = Arc::new(InMemoryAssetSourceLocationStore::default());
        let resolver = PersistentAssetSourceLocationResolver::new(store.clone());
        resolver
            .register(
                AssetId("asset-1".into()),
                &path,
                ContentKindId("text/plain".into()),
                Some(ContentFingerprint("sha256:asset-1".into())),
            )
            .expect("the source location should be registered");

        let source = FilePreviewAssetSource::new(Arc::new(resolver));
        assert_eq!(
            source
                .read(&context("asset-1"), PreviewReadRequest::default())
                .expect("the persisted source should be readable"),
            PreviewAssetContent::Text {
                text: "persistent preview".into(),
                truncated: false,
            }
        );

        let restored = PersistentAssetSourceLocationResolver::new(store);
        assert_eq!(
            restored
                .content_kind(&AssetId("asset-1".into()))
                .expect("the content kind should be restored"),
            Some(ContentKindId("text/plain".into()))
        );
        assert_eq!(
            restored
                .fingerprint(&AssetId("asset-1".into()))
                .expect("the fingerprint should be restored"),
            Some(ContentFingerprint("sha256:asset-1".into()))
        );
        assert!(restored
            .remove(&AssetId("asset-1".into()))
            .expect("the source location should be removable"));
        assert_eq!(
            restored.resolve(&AssetId("asset-1".into())),
            Err(PreviewSourceError::NotFound)
        );

        fs::remove_file(path).expect("test source should be cleaned up");
    }

    #[test]
    fn persistent_location_load_failure_is_isolated_until_the_store_recovers() {
        struct FailingStore;

        impl AssetSourceLocationStore for FailingStore {
            fn load(&self) -> Result<Vec<AssetSourceLocationRecord>, PersistenceError> {
                Err(PersistenceError::CorruptState)
            }

            fn save(
                &self,
                _locations: &[AssetSourceLocationRecord],
            ) -> Result<(), PersistenceError> {
                Err(PersistenceError::Unavailable)
            }
        }

        let resolver = PersistentAssetSourceLocationResolver::new(Arc::new(FailingStore));
        assert!(resolver.load_error().is_some());
        assert_eq!(
            resolver.resolve(&AssetId("missing".into())),
            Err(PreviewSourceError::NotFound)
        );
    }

    #[cfg(unix)]
    #[test]
    fn disconnected_registered_source_is_unavailable_and_retryable() {
        let path = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-disconnect-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::write(&path, "persistent preview").expect("test source should be writable");

        let resolver = PersistentAssetSourceLocationResolver::new(Arc::new(
            InMemoryAssetSourceLocationStore::default(),
        ));
        resolver
            .register(
                AssetId("asset-disconnected".into()),
                &path,
                ContentKindId("text/plain".into()),
                None,
            )
            .expect("the source location should be registered");
        fs::remove_file(&path).expect("source should be disconnected");

        assert_eq!(
            resolver.resolve(&AssetId("asset-disconnected".into())),
            Err(PreviewSourceError::Unavailable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn volume_identity_mismatch_does_not_rebind_an_asset() {
        let path = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-volume-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::write(&path, "persistent preview").expect("test source should be writable");
        let store = Arc::new(InMemoryAssetSourceLocationStore::default());
        store
            .save(&[AssetSourceLocationRecord {
                asset_id: "asset-volume".into(),
                source_path: fs::canonicalize(&path)
                    .expect("source path should canonicalize")
                    .to_string_lossy()
                    .into_owned(),
                content_kind: "text/plain".into(),
                fingerprint: None,
                volume_identity: Some("unix-dev:other-volume".into()),
                approved_root: None,
            }])
            .expect("volume fixture should persist");
        let resolver = PersistentAssetSourceLocationResolver::new(store);

        assert_eq!(
            resolver.resolve(&AssetId("asset-volume".into())),
            Err(PreviewSourceError::Unavailable)
        );
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_path_revalidation_rejects_symlink_replacement() {
        let root = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-symlink-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("source root should be writable");
        let path = root.join("asset.txt");
        let replacement = root.join("replacement.txt");
        fs::write(&path, "original").expect("source should be writable");
        fs::write(&replacement, "replacement").expect("replacement should be writable");

        let resolver = PersistentAssetSourceLocationResolver::new(Arc::new(
            InMemoryAssetSourceLocationStore::default(),
        ));
        resolver
            .register(
                AssetId("asset-symlink".into()),
                &path,
                ContentKindId("text/plain".into()),
                None,
            )
            .expect("the source location should be registered");
        fs::remove_file(&path).expect("original source should be removed");
        std::os::unix::fs::symlink(&replacement, &path).expect("replacement symlink should exist");

        assert_eq!(
            resolver.resolve(&AssetId("asset-symlink".into())),
            Err(PreviewSourceError::Unavailable)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn approved_root_rejects_escape_and_revalidates_root_identity() {
        let base = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-approved-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        let approved = base.join("approved");
        let outside = base.join("outside");
        fs::create_dir_all(&approved).expect("approved root should be writable");
        fs::create_dir_all(&outside).expect("outside root should be writable");
        let inside_file = approved.join("inside.txt");
        let outside_file = outside.join("outside.txt");
        fs::write(&inside_file, "inside").expect("inside source should be writable");
        fs::write(&outside_file, "outside").expect("outside source should be writable");

        let resolver = PersistentAssetSourceLocationResolver::new(Arc::new(
            InMemoryAssetSourceLocationStore::default(),
        ));
        resolver
            .add_approved_root(&approved)
            .expect("approved root should be registered");
        resolver
            .register_with_approved_root(
                AssetId("asset-approved".into()),
                &inside_file,
                ContentKindId("text/plain".into()),
                None,
                Some(&approved),
            )
            .expect("source inside the approved root should register");
        assert_eq!(
            resolver
                .registration(&AssetId("asset-approved".into()))
                .expect("registration should exist")
                .approved_root,
            Some(fs::canonicalize(&approved).expect("root should canonicalize"))
        );
        assert_eq!(
            resolver.register_with_approved_root(
                AssetId("asset-outside".into()),
                &outside_file,
                ContentKindId("text/plain".into()),
                None,
                Some(&approved),
            ),
            Err(PreviewSourceError::PermissionDenied)
        );

        let moved_root = base.join("approved-moved");
        fs::rename(&approved, &moved_root).expect("approved root should be movable");
        assert_eq!(
            resolver.resolve(&AssetId("asset-approved".into())),
            Err(PreviewSourceError::Unavailable)
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn approved_root_registry_revocation_preserves_source_identity() {
        let base = std::env::temp_dir().join(format!(
            "velvet-persistent-preview-approved-registry-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("source root should be writable");
        let source = base.join("asset.txt");
        fs::write(&source, "registry").expect("source should be writable");

        let store = Arc::new(InMemoryAssetSourceLocationStore::default());
        let resolver = PersistentAssetSourceLocationResolver::new(store.clone());
        let canonical_root = resolver
            .add_approved_root(&base)
            .expect("root should be approved");
        resolver
            .register_with_approved_root(
                AssetId("asset-registry".into()),
                &source,
                ContentKindId("text/plain".into()),
                None,
                Some(&base),
            )
            .expect("source should use the approved root");
        assert_eq!(
            resolver.approved_roots().expect("roots should load"),
            vec![canonical_root]
        );
        assert!(resolver
            .remove_approved_root(&base)
            .expect("root should be removable"));
        assert_eq!(
            resolver.resolve(&AssetId("asset-registry".into())),
            Err(PreviewSourceError::PermissionDenied)
        );
        assert!(resolver
            .add_approved_root(&base)
            .expect("root should be re-approvable")
            .is_absolute());
        assert!(resolver.resolve(&AssetId("asset-registry".into())).is_ok());
        let restored = PersistentAssetSourceLocationResolver::new(store);
        assert_eq!(
            restored
                .approved_roots()
                .expect("roots should survive restart")
                .len(),
            1
        );

        let _ = fs::remove_dir_all(base);
    }
}
