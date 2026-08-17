//! Machine-wide recorded history about the executable that may install assets.
//!
//! Adapter install records answer which files Estigia created. These records
//! answer a different question: which binary versions a cooperating installer
//! recorded on this machine. They are not authenticated against a malicious
//! same-user writer. Keeping them under `~/.estigia/lifecycle` lets them survive
//! uninstall.

use std::cmp::Ordering;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

const SCHEMA: u8 = 3;
static STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The machine-wide lifecycle paths.
#[derive(Debug, Clone)]
pub struct StateRoot {
    root: PathBuf,
}

impl StateRoot {
    /// Resolves lifecycle state below a user's home directory.
    pub fn under(home: &Path) -> Self {
        Self {
            root: home.join(".estigia").join("lifecycle"),
        }
    }

    /// Resolves lifecycle state for this process.
    pub fn current() -> Result<Self, StateError> {
        crate::paths::home_dir()
            .map(|home| Self::under(&home))
            .map_err(|error| StateError::Home(error.to_string()))
    }

    /// Immutable records keyed by bytes observed through an executable pathname.
    pub fn provenance(&self) -> PathBuf {
        self.root.join("provenance")
    }

    /// Immutable records keyed by canonical semantic version.
    pub fn releases(&self) -> PathBuf {
        self.root.join("releases")
    }

    /// Records an installer candidate after checking it against readable history.
    ///
    /// Identity comes from the candidate pathname and this compiled build, not
    /// from installer arguments. Local records are cooperative history rather
    /// than authentication against another same-user process.
    pub fn record_installer_install(&self, candidate: &Path) -> Result<(), StateError> {
        let version = compiled_version()?;
        if let Some(high_water) = self.high_water()?
            && version.cmp_precedence(&high_water).is_lt()
        {
            return Err(StateError::Downgrade {
                candidate: version,
                high_water,
            });
        }
        let observed_path_sha256 = observed_path_sha256(candidate)?;
        let record = ProvenanceRecord {
            schema: SCHEMA,
            observed_path_sha256: observed_path_sha256.clone(),
            version: version.clone(),
            asset_set_sha256: compiled_asset_set_sha256(),
        };
        publish_immutable(
            &self
                .provenance()
                .join(format!("{observed_path_sha256}.json")),
            &record,
        )?;
        publish_immutable(
            &self.releases().join(format!("{version}.json")),
            &ReleaseRecord {
                schema: SCHEMA,
                version,
            },
        )
        .map_err(|source| StateError::ReleaseAfterProvenance(Box::new(source)))
    }

    /// The greatest canonical release recorded on this machine.
    pub fn high_water(&self) -> Result<Option<Version>, StateError> {
        let entries = match std::fs::read_dir(self.releases()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StateError::Read(self.releases(), error)),
        };
        let mut highest: Option<Version> = None;
        for entry in entries {
            let path = entry
                .map_err(|error| StateError::Read(self.releases(), error))?
                .path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let record: ReleaseRecord = read_record(&path)?;
            validate_schema(record.schema, &path)?;
            require_canonical_release(&record.version)?;
            let expected = format!("{}.json", record.version);
            if path.file_name().and_then(|name| name.to_str()) != Some(expected.as_str()) {
                return Err(StateError::NonCanonical(path));
            }
            if highest
                .as_ref()
                .is_none_or(|current| record.version.cmp_precedence(current).is_gt())
            {
                highest = Some(record.version);
            }
        }
        Ok(highest)
    }

    fn provenance_for(
        &self,
        observed_path_sha256: &str,
    ) -> Result<Option<ProvenanceRecord>, StateError> {
        let path = self
            .provenance()
            .join(format!("{observed_path_sha256}.json"));
        let record: ProvenanceRecord = match read_optional(&path)? {
            Some(record) => record,
            None => return Ok(None),
        };
        validate_schema(record.schema, &path)?;
        if record.observed_path_sha256 != observed_path_sha256 {
            return Err(StateError::KeyMismatch(path));
        }
        if record.version != compiled_version()?
            || record.asset_set_sha256 != compiled_asset_set_sha256()
        {
            return Err(StateError::CompiledIdentityMismatch(path));
        }
        require_canonical_release(&record.version)?;
        Ok(Some(record))
    }
}

/// How the running release compares with recorded machine history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// This executable has no matching installer record.
    SourceOrUnrecorded,
    /// Its release is the greatest release recorded here.
    Current,
    /// A greater release has already been recorded here.
    DowngradeBlocked,
    /// This recorded release is newer than the existing history.
    AheadOfRecorded,
    /// Provenance is recorded and no release history has been recorded yet.
    RecordedNoHistory,
    /// Evidence exists but cannot be read safely.
    Unknown,
}

impl Relation {
    /// Compares one recorded running version with an optional high-water mark.
    pub fn between(running: &Version, high_water: Option<&Version>) -> Self {
        let Some(high_water) = high_water else {
            return Self::RecordedNoHistory;
        };
        match running.cmp_precedence(high_water) {
            Ordering::Less => Self::DowngradeBlocked,
            Ordering::Equal => Self::Current,
            Ordering::Greater => Self::AheadOfRecorded,
        }
    }
}

/// Recorded identity for the compiled payload this process can deploy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    /// No digest-keyed installer record exists.
    SourceOrUnrecorded,
    /// A record matches the observed pathname bytes and this compiled payload.
    InstallerRecorded {
        /// Release version bound to the observed pathname digest.
        version: Version,
    },
    /// A record exists but cannot be read safely or does not match this process.
    Unknown,
}

/// Local public-release knowledge for this network-free slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublicRelease {
    /// No remote was queried, so absence is not reported as currency.
    Unavailable {
        /// Whether a public release source was queried.
        checked: bool,
        /// Why no latest public release can be asserted.
        reason: &'static str,
    },
}

/// Complete read-only lifecycle inventory for one executable.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// The executable inspected.
    pub executable: Executable,
    /// Installer-recorded or unrecorded provenance.
    pub provenance: Provenance,
    /// Relation to recorded machine history.
    pub relation: Relation,
    /// Greatest release recorded here.
    pub high_water: Option<Version>,
    /// Why lifecycle evidence could not be read.
    pub state_error: Option<String>,
    /// What is known about the latest public release.
    pub public_release: PublicRelease,
}

impl Status {
    /// Inspects an executable and never changes lifecycle state.
    pub fn inspect_executable(state: &StateRoot, executable: PathBuf) -> Self {
        let digest = observed_path_sha256(&executable);
        let public_release = PublicRelease::Unavailable {
            checked: false,
            reason: "no public Release is configured for local verification; no network request was made",
        };
        let sha256 = match digest {
            Ok(digest) => digest,
            Err(error) => {
                return Self {
                    executable: Executable {
                        path: executable,
                        observed_path_sha256: None,
                        compiled_version: env!("CARGO_PKG_VERSION"),
                        asset_set_sha256: compiled_asset_set_sha256(),
                    },
                    provenance: Provenance::Unknown,
                    relation: Relation::Unknown,
                    high_water: None,
                    state_error: Some(error.to_string()),
                    public_release,
                };
            }
        };
        let executable = Executable {
            path: executable,
            observed_path_sha256: Some(sha256.clone()),
            compiled_version: env!("CARGO_PKG_VERSION"),
            asset_set_sha256: compiled_asset_set_sha256(),
        };
        let provenance = state.provenance_for(&sha256);
        let high_water = state.high_water();
        match (provenance, high_water) {
            (Ok(None), Ok(high_water)) => Self {
                executable,
                provenance: Provenance::SourceOrUnrecorded,
                relation: Relation::SourceOrUnrecorded,
                high_water,
                state_error: None,
                public_release,
            },
            (Ok(Some(record)), Ok(high_water)) => Self {
                executable,
                relation: Relation::between(&record.version, high_water.as_ref()),
                provenance: Provenance::InstallerRecorded {
                    version: record.version,
                },
                high_water,
                state_error: None,
                public_release,
            },
            (provenance, high_water) => {
                let state_error = provenance
                    .as_ref()
                    .err()
                    .or_else(|| high_water.as_ref().err())
                    .map_or_else(
                        || "lifecycle state is unreadable".to_owned(),
                        ToString::to_string,
                    );
                Self {
                    executable,
                    provenance: Provenance::Unknown,
                    relation: Relation::Unknown,
                    high_water: high_water.ok().flatten(),
                    state_error: Some(state_error),
                    public_release,
                }
            }
        }
    }

    /// Inspects the process executable and the current user's state.
    pub fn current() -> Result<Self, StateError> {
        let state = StateRoot::current()?;
        let executable = std::env::current_exe().map_err(StateError::Executable)?;
        Ok(Self::inspect_executable(&state, executable))
    }
}

/// Path and raceable inventory digest of bytes reopened through that pathname.
#[derive(Debug, Clone, Serialize)]
pub struct Executable {
    /// Resolved process path.
    pub path: PathBuf,
    /// SHA-256 of observed pathname bytes, absent when the path could not be read.
    ///
    /// This does not identify the mapped running image: `current_exe` supplies a
    /// pathname and another process may replace that path before it is opened.
    pub observed_path_sha256: Option<String>,
    /// Version compiled into the running process.
    pub compiled_version: &'static str,
    /// Digest of the embedded skill and agent-definition assets in this process.
    pub asset_set_sha256: String,
}

/// A lifecycle-state read or immutable publication failure.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// Home could not be resolved.
    #[error("could not resolve lifecycle home: {0}")]
    Home(String),
    /// The executable pathname or the bytes observed through it could not be read.
    #[error("could not inspect the current executable pathname: {0}")]
    Executable(#[source] std::io::Error),
    /// A state path could not be read.
    #[error("could not read {path}: {source}", path = .0.display(), source = .1)]
    Read(PathBuf, #[source] std::io::Error),
    /// A state record is malformed.
    #[error("could not parse {path}: {source}", path = .0.display(), source = .1)]
    Malformed(PathBuf, #[source] serde_json::Error),
    /// A state record has an unsupported schema.
    #[error("{} has an unsupported lifecycle schema", .0.display())]
    Schema(PathBuf),
    /// The filename and record identity disagree.
    #[error("{} does not match its immutable key", .0.display())]
    KeyMismatch(PathBuf),
    /// A provenance record names a different compiled deployable payload.
    #[error("{} does not match this build's compiled version and embedded assets", .0.display())]
    CompiledIdentityMismatch(PathBuf),
    /// A release filename is not its canonical SemVer spelling.
    #[error("{} is not keyed by canonical SemVer", .0.display())]
    NonCanonical(PathBuf),
    /// The candidate is older than readable machine history.
    #[error("installer candidate {candidate} is below recorded high-water {high_water}")]
    Downgrade {
        /// Canonical version compiled into the candidate.
        candidate: Version,
        /// Greatest canonical release already recorded.
        high_water: Version,
    },
    /// Build metadata is not a canonical release identity.
    #[error(
        "release {0} contains SemVer build metadata, which is not a canonical release identity"
    )]
    BuildMetadata(Version),
    /// The package version embedded by Cargo is not SemVer.
    #[error("compiled package version {0:?} is not valid SemVer: {1}")]
    CompiledVersion(String, #[source] semver::Error),
    /// A record path is a symlink or another non-regular file.
    #[error("{} is not a regular lifecycle record", .0.display())]
    InvalidRecordType(PathBuf),
    /// Immutable state could not be written.
    #[error("could not publish {path}: {source}", path = .0.display(), source = .1)]
    Write(PathBuf, #[source] std::io::Error),
    /// Existing immutable evidence disagrees with a later writer.
    #[error("{} already contains different immutable evidence", .0.display())]
    Conflict(PathBuf),
    /// Provenance landed, but release publication did not complete.
    #[error("candidate provenance was recorded, but release history did not advance: {0}")]
    ReleaseAfterProvenance(Box<StateError>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceRecord {
    schema: u8,
    observed_path_sha256: String,
    version: Version,
    asset_set_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseRecord {
    schema: u8,
    version: Version,
}

fn observed_path_sha256(path: &Path) -> Result<String, StateError> {
    let mut file = std::fs::File::open(path).map_err(StateError::Executable)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(StateError::Executable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn require_canonical_release(version: &Version) -> Result<(), StateError> {
    if version.build.is_empty() {
        Ok(())
    } else {
        Err(StateError::BuildMetadata(version.clone()))
    }
}

fn compiled_version() -> Result<Version, StateError> {
    let version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| {
        StateError::CompiledVersion(env!("CARGO_PKG_VERSION").to_owned(), error)
    })?;
    require_canonical_release(&version)?;
    Ok(version)
}

fn compiled_asset_set_sha256() -> String {
    let mut hasher = sha2::Sha256::new();
    hash_typed(&mut hasher, b"estigia-embedded-asset-set-v1", b"");
    let collections = [
        (b"skill".as_slice(), crate::skill::FILES),
        (
            b"agent-definitions".as_slice(),
            crate::skill::AGENT_DEFINITIONS,
        ),
    ];
    hasher.update((collections.len() as u64).to_be_bytes());
    for (set, files) in collections {
        hash_typed(&mut hasher, b"collection", set);
        hasher.update((files.len() as u64).to_be_bytes());
        for file in files {
            hash_typed(&mut hasher, b"asset-path", file.path.as_bytes());
            hash_typed(&mut hasher, b"asset-contents", file.contents.as_bytes());
        }
    }
    hex_digest(hasher.finalize())
}

fn hash_typed(hasher: &mut sha2::Sha256, tag: &[u8], bytes: &[u8]) {
    hasher.update((tag.len() as u64).to_be_bytes());
    hasher.update(tag);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_schema(schema: u8, path: &Path) -> Result<(), StateError> {
    if schema == SCHEMA {
        Ok(())
    } else {
        Err(StateError::Schema(path.to_path_buf()))
    }
}

fn read_record<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, StateError> {
    let mut file = open_record(path).map_err(|error| map_record_open(path, error))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| StateError::Read(path.into(), error))?;
    serde_json::from_str(&text).map_err(|error| StateError::Malformed(path.into(), error))
}

fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, StateError> {
    let mut file = match open_record(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(map_record_open(path, error)),
    };
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| StateError::Read(path.into(), error))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| StateError::Malformed(path.into(), error))
}

fn open_record(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "lifecycle record is not a regular file",
        ));
    }
    Ok(file)
}

fn map_record_open(path: &Path, error: std::io::Error) -> StateError {
    if error.kind() == std::io::ErrorKind::InvalidData
        || error.raw_os_error() == Some(platform_symlink_error())
    {
        StateError::InvalidRecordType(path.into())
    } else {
        StateError::Read(path.into(), error)
    }
}

#[cfg(unix)]
fn platform_symlink_error() -> i32 {
    libc::ELOOP
}

#[cfg(not(unix))]
fn platform_symlink_error() -> i32 {
    -1
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn publish_immutable<T>(path: &Path, record: &T) -> Result<(), StateError>
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq,
{
    if let Some(existing) = read_optional::<T>(path)? {
        return if existing == *record {
            Ok(())
        } else {
            Err(StateError::Conflict(path.to_path_buf()))
        };
    }
    let parent = path.parent().ok_or_else(|| {
        StateError::Write(
            path.to_path_buf(),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "record has no parent"),
        )
    })?;
    std::fs::create_dir_all(parent).map_err(|error| StateError::Write(path.into(), error))?;
    let mut text = serde_json::to_vec_pretty(record)
        .map_err(|error| StateError::Write(path.into(), std::io::Error::other(error)))?;
    text.push(b'\n');
    let (staged, mut file) = create_unique_stage(path)?;
    if let Err(error) = file.write_all(&text).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&staged);
        return Err(StateError::Write(staged, error));
    }
    drop(file);
    match std::fs::hard_link(&staged, path) {
        Ok(()) => {
            let _ = std::fs::remove_file(staged);
            sync_parent_best_effort(parent);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(staged);
            match read_optional::<T>(path)? {
                Some(existing) if existing == *record => Ok(()),
                Some(_) => Err(StateError::Conflict(path.to_path_buf())),
                None => Err(StateError::Write(path.to_path_buf(), error)),
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(staged);
            Err(StateError::Write(path.to_path_buf(), error))
        }
    }
}

fn create_unique_stage(path: &Path) -> Result<(PathBuf, std::fs::File), StateError> {
    let parent = path.parent().ok_or_else(|| {
        StateError::Write(
            path.into(),
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "record has no parent"),
        )
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            StateError::Write(
                path.into(),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "record has no UTF-8 filename",
                ),
            )
        })?;
    for _ in 0..64 {
        let sequence = STAGE_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let staged = parent.join(format!(".{name}.{}.{}.stage", std::process::id(), sequence));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
        {
            Ok(file) => return Ok((staged, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StateError::Write(staged, error)),
        }
    }
    Err(StateError::Write(
        path.into(),
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique publication stage",
        ),
    ))
}

fn sync_parent_best_effort(parent: &Path) {
    if let Ok(directory) = std::fs::File::open(parent) {
        let _ = directory.sync_all();
    }
}
