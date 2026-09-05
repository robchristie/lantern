use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEFAULT_BROWSER_IMAGE: &str = "localhost/lantern-browser-cdp:stable";
pub const INSTANCE_ROOT: &str = ".smoogle/lantern/browser-instances";
pub const MANAGED_LABEL_KEY: &str = "dev.lantern.managed";
pub const INSTANCE_ID_LABEL_KEY: &str = "dev.lantern.instance-id";
pub const PROFILE_NAME_LABEL_KEY: &str = "dev.lantern.profile-name";
pub const INSTANCE_NAME_PREFIX: &str = "lantern-browser";
pub const PERSISTENT_INSTANCE_ID_PREFIX: &str = "lantern-profile-";
const PROFILE_OPERATION_LOCK_FILE: &str = "operation.lock";

static PROFILE_RESERVATION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn storage_backend() -> &'static str {
    "json-files"
}

#[derive(Debug, Clone)]
pub struct BrowserRegistry {
    root: PathBuf,
}

impl BrowserRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn under_repo(repo_root: impl AsRef<Path>) -> Self {
        Self::new(repo_root.as_ref().join(INSTANCE_ROOT))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create_instance_layout(&self, id: &str) -> io::Result<BrowserInstanceLayout> {
        validate_instance_id(id)?;
        ensure_private_directory(&self.root)?;
        let instance_dir = self.root.join(id);
        fs::create_dir(&instance_dir)?;
        set_private_directory_permissions(&instance_dir)?;
        self.validate_instance_directory(id)?;
        let profile_dir = instance_dir.join("profile");
        fs::create_dir(&profile_dir)?;
        set_private_directory_permissions(&profile_dir)?;
        Ok(BrowserInstanceLayout {
            instance_dir,
            profile_dir,
        })
    }

    pub fn create_persistent_instance_layout(
        &self,
        id: &str,
        profile_dir: PathBuf,
    ) -> io::Result<BrowserInstanceLayout> {
        validate_instance_id(id)?;
        ensure_private_directory(&self.root)?;
        let instance_dir = self.root.join(id);
        ensure_private_directory(&instance_dir)?;
        self.validate_instance_directory(id)?;
        Ok(BrowserInstanceLayout {
            instance_dir,
            profile_dir,
        })
    }

    pub fn write_record_atomic(&self, record: &BrowserInstanceRecord) -> io::Result<()> {
        validate_instance_record(record, &record.id)?;
        ensure_private_directory(&self.root)?;
        let _lock = self.lock()?;
        self.write_record_atomic_unlocked(record)
    }

    pub fn update_record<F>(&self, id: &str, update: F) -> io::Result<BrowserInstanceRecord>
    where
        F: FnOnce(&mut BrowserInstanceRecord),
    {
        validate_instance_id(id)?;
        let _lock = self.lock()?;
        let mut record = self.read_record_unlocked(id)?;
        update(&mut record);
        record.updated_at_unix_ms = now_unix_ms();
        self.write_record_atomic_unlocked(&record)?;
        Ok(record)
    }

    pub fn read_record(&self, id: &str) -> io::Result<BrowserInstanceRecord> {
        validate_instance_id(id)?;
        self.read_record_unlocked(id)
    }

    pub fn list_records(&self) -> io::Result<Vec<BrowserInstanceRecord>> {
        let mut records: Vec<BrowserInstanceRecord> = Vec::new();
        match fs::symlink_metadata(&self.root) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(records),
            Err(source) => return Err(source),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "browser instance root is not a directory",
                ));
            }
            Ok(_) => {}
        }
        match fs::read_dir(&self.root) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    let name = entry.file_name();
                    let Some(id) = name.to_str() else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "browser instance id is not UTF-8",
                        ));
                    };
                    if id == ".lock" {
                        continue;
                    }
                    validate_instance_id(id)?;
                    self.validate_instance_directory(id)?;
                    records.push(self.read_record_unlocked(id)?);
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(source),
        }

        records.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(records)
    }

    pub fn remove_instance_dir(&self, id: &str) -> io::Result<()> {
        validate_instance_id(id)?;
        let _lock = self.lock()?;
        let path = match self.validate_instance_directory(id) {
            Ok(path) => path,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => return Err(source),
        };
        match fs::remove_dir_all(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(source),
        }
    }

    fn read_record_unlocked(&self, id: &str) -> io::Result<BrowserInstanceRecord> {
        validate_instance_id(id)?;
        let instance_dir = self.validate_instance_directory(id)?;
        let record_path = instance_dir.join("record.json");
        validate_regular_file_in_directory(&record_path, &instance_dir, "browser instance record")?;
        let body = fs::read_to_string(record_path)?;
        let record: BrowserInstanceRecord = serde_json::from_str(&body).map_err(invalid_data)?;
        validate_instance_record(&record, id)?;
        Ok(record)
    }

    fn write_record_atomic_unlocked(&self, record: &BrowserInstanceRecord) -> io::Result<()> {
        validate_instance_record(record, &record.id)?;
        let instance_dir = self.root.join(&record.id);
        ensure_private_directory(&instance_dir)?;
        self.validate_instance_directory(&record.id)?;
        let final_path = instance_dir.join("record.json");
        let tmp_path = instance_dir.join("record.json.tmp");
        reject_symlink_or_non_file_if_present(&final_path, "browser instance record")?;
        let body = serde_json::to_vec_pretty(record).map_err(invalid_data)?;
        write_private_file(&tmp_path, &body)?;
        fs::rename(tmp_path, final_path)
    }

    fn validate_instance_directory(&self, id: &str) -> io::Result<PathBuf> {
        validate_instance_id(id)?;
        let root_metadata = fs::symlink_metadata(&self.root)?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "browser instance root is not a directory",
            ));
        }
        let instance_dir = self.root.join(id);
        let metadata = fs::symlink_metadata(&instance_dir)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "browser instance path is not a directory",
            ));
        }
        let root = fs::canonicalize(&self.root)?;
        let canonical = fs::canonicalize(&instance_dir)?;
        if canonical.parent() != Some(root.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "browser instance path escapes its root",
            ));
        }
        Ok(instance_dir)
    }

    fn lock(&self) -> io::Result<RegistryLock> {
        ensure_private_directory(&self.root)?;
        let lock_dir = self.root.join(".lock");
        let started = SystemTime::now();

        loop {
            match fs::create_dir(&lock_dir) {
                Ok(()) => {
                    return Ok(RegistryLock { lock_dir });
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                    if started.elapsed().unwrap_or_default() > Duration::from_secs(10) {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "timed out waiting for browser registry lock",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(source) => return Err(source),
            }
        }
    }
}

#[derive(Debug)]
pub struct BrowserInstanceLayout {
    pub instance_dir: PathBuf,
    pub profile_dir: PathBuf,
}

struct RegistryLock {
    lock_dir: PathBuf,
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.lock_dir);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserInstanceRecord {
    pub schema_version: u8,
    pub id: String,
    pub name: String,
    pub runtime: RuntimeKind,
    pub image: String,
    pub container_id: Option<String>,
    pub status: BrowserInstanceStatus,
    pub endpoint: Option<String>,
    pub cdp_host_port: Option<u16>,
    pub novnc_url: Option<String>,
    pub novnc_host_port: Option<u16>,
    pub vnc_host_port: Option<u16>,
    pub profile_dir: PathBuf,
    #[serde(default)]
    pub profile_kind: BrowserProfileKind,
    #[serde(default)]
    pub profile_name: Option<String>,
    #[serde(default)]
    pub profile_reservation_id: Option<String>,
    #[serde(default)]
    pub host_gateway: Option<String>,
    pub created_at_unix_ms: u128,
    pub updated_at_unix_ms: u128,
}

impl BrowserInstanceRecord {
    pub fn pending(
        id: String,
        name: String,
        runtime: RuntimeKind,
        image: String,
        profile_dir: PathBuf,
        profile_kind: BrowserProfileKind,
        profile_name: Option<String>,
        profile_reservation_id: Option<String>,
    ) -> Self {
        let now = now_unix_ms();
        Self {
            schema_version: 1,
            id,
            name,
            runtime,
            image,
            container_id: None,
            status: BrowserInstanceStatus::Starting,
            endpoint: None,
            cdp_host_port: None,
            novnc_url: None,
            novnc_host_port: None,
            vnc_host_port: None,
            profile_dir,
            profile_kind,
            profile_name,
            profile_reservation_id,
            host_gateway: None,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserProfileKind {
    #[default]
    Disposable,
    Persistent,
}

impl BrowserProfileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disposable => "disposable",
            Self::Persistent => "persistent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserProfileAttachment {
    pub instance_id: String,
    pub container_name: String,
    pub runtime: RuntimeKind,
    #[serde(default)]
    pub reservation_id: String,
    #[serde(default)]
    pub state: BrowserProfileAttachmentState,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserProfileAttachmentState {
    #[default]
    Active,
    Stopping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserProfileRecord {
    pub schema_version: u8,
    pub name: String,
    pub attachment: Option<BrowserProfileAttachment>,
    pub created_at_unix_ms: u128,
    pub updated_at_unix_ms: u128,
}

impl BrowserProfileRecord {
    fn new(name: String) -> Self {
        let now = now_unix_ms();
        Self {
            schema_version: 1,
            name,
            attachment: None,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowserProfileRegistry {
    root: PathBuf,
}

#[derive(Debug)]
pub struct BrowserProfileOperationLock {
    file: File,
}

impl Drop for BrowserProfileOperationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl BrowserProfileRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn create(&self, name: &str) -> io::Result<BrowserProfileRecord> {
        validate_profile_name(name)?;
        ensure_private_directory(&self.root)?;
        let _lock = ProfileRegistryLock::acquire(&self.root)?;
        let profile_dir = self.profile_dir(name);
        match fs::symlink_metadata(&profile_dir) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "profile exists",
                ));
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(source),
        }
        fs::create_dir(&profile_dir)?;
        let create_result = (|| {
            set_private_directory_permissions(&profile_dir)?;
            let data_dir = profile_dir.join("data");
            fs::create_dir(&data_dir)?;
            set_private_directory_permissions(&data_dir)?;
            write_private_file(&profile_dir.join(PROFILE_OPERATION_LOCK_FILE), b"")?;
            let record = BrowserProfileRecord::new(name.to_owned());
            self.write_record_atomic_unlocked(&record)?;
            Ok(record)
        })();
        if create_result.is_err() {
            let _ = fs::remove_dir_all(&profile_dir);
        }
        create_result
    }

    pub fn read(&self, name: &str) -> io::Result<BrowserProfileRecord> {
        validate_profile_name(name)?;
        self.validate_profile_directory(name)?;
        self.read_record_unlocked(name)
    }

    pub fn list(&self) -> io::Result<Vec<BrowserProfileRecord>> {
        match fs::symlink_metadata(&self.root) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(source),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "profile root is not a directory",
                ));
            }
            Ok(_) => {}
        }

        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "profile name is not UTF-8",
                ));
            };
            if name == ".lock" {
                continue;
            }
            validate_profile_name(name)?;
            self.validate_profile_directory(name)?;
            records.push(self.read_record_unlocked(name)?);
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    pub fn data_dir(&self, name: &str) -> io::Result<PathBuf> {
        validate_profile_name(name)?;
        let profile_dir = self.validate_profile_directory(name)?;
        let data_dir = profile_dir.join("data");
        let metadata = fs::symlink_metadata(&data_dir)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile data path is not a directory",
            ));
        }
        let root = fs::canonicalize(&self.root)?;
        let data_dir = fs::canonicalize(data_dir)?;
        if !data_dir.starts_with(root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile data path escapes its root",
            ));
        }
        Ok(data_dir)
    }

    pub fn persistent_instance_id(&self, name: &str) -> io::Result<String> {
        persistent_instance_id(&self.root, name)
    }

    pub fn try_operation_lock(&self, name: &str) -> io::Result<BrowserProfileOperationLock> {
        validate_profile_name(name)?;
        ensure_private_directory(&self.root)?;
        let _registry_lock = ProfileRegistryLock::acquire(&self.root)?;
        let profile_dir = self.validate_profile_directory(name)?;
        let lock_path = profile_dir.join(PROFILE_OPERATION_LOCK_FILE);
        match fs::symlink_metadata(&lock_path) {
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                write_private_file(&lock_path, b"")?;
            }
            Err(source) => return Err(source),
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "profile operation lock is not a regular file",
                ));
            }
            Ok(_) => {}
        }
        validate_regular_file_in_directory(&lock_path, &profile_dir, "profile operation lock")?;
        let file = OpenOptions::new().read(true).write(true).open(&lock_path)?;
        file.try_lock_exclusive()?;
        Ok(BrowserProfileOperationLock { file })
    }

    pub fn compare_and_swap_attachment(
        &self,
        name: &str,
        expected: Option<&BrowserProfileAttachment>,
        replacement: Option<BrowserProfileAttachment>,
    ) -> io::Result<BrowserProfileRecord> {
        validate_profile_name(name)?;
        if let Some(attachment) = replacement.as_ref() {
            validate_profile_attachment(attachment, io::ErrorKind::InvalidInput)?;
        }
        ensure_private_directory(&self.root)?;
        let _lock = ProfileRegistryLock::acquire(&self.root)?;
        self.validate_profile_directory(name)?;
        let mut record = self.read_record_unlocked(name)?;
        if record.attachment.as_ref() != expected {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "profile attachment changed",
            ));
        }
        record.attachment = replacement;
        record.updated_at_unix_ms = now_unix_ms();
        self.write_record_atomic_unlocked(&record)?;
        Ok(record)
    }

    pub fn release_if_owner(
        &self,
        name: &str,
        expected: &BrowserProfileAttachment,
    ) -> io::Result<bool> {
        validate_profile_name(name)?;
        validate_profile_attachment(expected, io::ErrorKind::InvalidInput)?;
        ensure_private_directory(&self.root)?;
        let _lock = ProfileRegistryLock::acquire(&self.root)?;
        self.validate_profile_directory(name)?;
        let mut record = self.read_record_unlocked(name)?;
        if record.attachment.as_ref() != Some(expected) {
            return Ok(false);
        }
        record.attachment = None;
        record.updated_at_unix_ms = now_unix_ms();
        self.write_record_atomic_unlocked(&record)?;
        Ok(true)
    }

    pub fn delete(&self, name: &str) -> io::Result<()> {
        validate_profile_name(name)?;
        ensure_private_directory(&self.root)?;
        let _lock = ProfileRegistryLock::acquire(&self.root)?;
        let profile_dir = self.validate_profile_directory(name)?;
        let record = self.read_record_unlocked(name)?;
        if record.attachment.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "profile is attached",
            ));
        }
        let root = fs::canonicalize(&self.root)?;
        let canonical = fs::canonicalize(&profile_dir)?;
        if canonical.parent() != Some(root.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile path escapes its root",
            ));
        }
        fs::remove_dir_all(profile_dir)
    }

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn validate_profile_directory(&self, name: &str) -> io::Result<PathBuf> {
        let profile_dir = self.profile_dir(name);
        let metadata = fs::symlink_metadata(&profile_dir)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile path is not a directory",
            ));
        }
        let root = fs::canonicalize(&self.root)?;
        let canonical = fs::canonicalize(&profile_dir)?;
        if canonical.parent() != Some(root.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile path escapes its root",
            ));
        }
        Ok(profile_dir)
    }

    fn read_record_unlocked(&self, name: &str) -> io::Result<BrowserProfileRecord> {
        let profile_dir = self.profile_dir(name);
        let record_path = profile_dir.join("profile.json");
        let metadata = fs::symlink_metadata(&record_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile record is not a regular file",
            ));
        }
        let canonical = fs::canonicalize(&record_path)?;
        if canonical.parent() != Some(fs::canonicalize(profile_dir)?.as_path()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile record escapes its directory",
            ));
        }
        let body = fs::read_to_string(record_path)?;
        let record: BrowserProfileRecord = serde_json::from_str(&body).map_err(invalid_data)?;
        if record.schema_version != 1 || record.name != name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "profile record identity does not match its directory",
            ));
        }
        if let Some(attachment) = record.attachment.as_ref() {
            validate_profile_attachment(attachment, io::ErrorKind::InvalidData)?;
        }
        Ok(record)
    }

    fn write_record_atomic_unlocked(&self, record: &BrowserProfileRecord) -> io::Result<()> {
        let profile_dir = self.profile_dir(&record.name);
        let final_path = profile_dir.join("profile.json");
        let tmp_path = profile_dir.join("profile.json.tmp");
        reject_symlink_or_non_file_if_present(&final_path, "profile record")?;
        let body = serde_json::to_vec_pretty(record).map_err(invalid_data)?;
        write_private_file(&tmp_path, &body)?;
        fs::rename(tmp_path, final_path)
    }
}

struct ProfileRegistryLock {
    lock_dir: PathBuf,
}

impl ProfileRegistryLock {
    fn acquire(root: &Path) -> io::Result<Self> {
        let lock_dir = root.join(".lock");
        let started = SystemTime::now();
        loop {
            match fs::create_dir(&lock_dir) {
                Ok(()) => {
                    set_private_directory_permissions(&lock_dir)?;
                    return Ok(Self { lock_dir });
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                    if started.elapsed().unwrap_or_default() > Duration::from_secs(10) {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "timed out waiting for profile registry lock",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(source) => return Err(source),
            }
        }
    }
}

impl Drop for ProfileRegistryLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.lock_dir);
    }
}

pub fn validate_profile_name(name: &str) -> io::Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 47
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid profile name",
        ))
    }
}

pub fn persistent_instance_id(profile_root: &Path, profile_name: &str) -> io::Result<String> {
    validate_profile_name(profile_name)?;
    let canonical_root = fs::canonicalize(profile_root)?;
    let mut hasher = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(canonical_root.as_os_str().as_bytes());
    }
    #[cfg(not(unix))]
    hasher.update(canonical_root.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let namespace = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        "{PERSISTENT_INSTANCE_ID_PREFIX}{namespace}-{profile_name}"
    ))
}

pub fn validate_instance_id(id: &str) -> io::Result<()> {
    validate_bounded_identifier(id, "invalid browser instance id")
}

pub fn generate_profile_reservation_id() -> String {
    format!(
        "reservation-{}-{}-{}",
        now_unix_ms(),
        std::process::id(),
        PROFILE_RESERVATION_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn validate_reservation_id(id: &str) -> io::Result<()> {
    validate_bounded_identifier(id, "invalid browser profile reservation id")
}

fn validate_bounded_identifier(value: &str, message: &'static str) -> io::Result<()> {
    let valid = !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidInput, message))
    }
}

fn validate_profile_attachment(
    attachment: &BrowserProfileAttachment,
    error_kind: io::ErrorKind,
) -> io::Result<()> {
    validate_instance_id(&attachment.instance_id)
        .map_err(|_| io::Error::new(error_kind, "profile attachment identity is invalid"))?;
    validate_reservation_id(&attachment.reservation_id)
        .map_err(|_| io::Error::new(error_kind, "profile attachment identity is invalid"))?;
    if attachment.container_name != attachment.instance_id {
        return Err(io::Error::new(
            error_kind,
            "profile attachment identity is invalid",
        ));
    }
    Ok(())
}

fn validate_instance_record(record: &BrowserInstanceRecord, expected_id: &str) -> io::Result<()> {
    validate_instance_id(expected_id)?;
    validate_instance_id(&record.id)?;
    if record.schema_version != 1 || record.id != expected_id || record.name != expected_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "browser instance record identity does not match its directory",
        ));
    }
    if let Some(hostname) = record.host_gateway.as_deref() {
        validate_host_gateway_hostname(hostname)?;
    }
    match record.profile_kind {
        BrowserProfileKind::Disposable => {
            if record.profile_name.is_some() || record.profile_reservation_id.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "disposable browser record has persistent profile identity",
                ));
            }
        }
        BrowserProfileKind::Persistent => {
            let profile_name = record.profile_name.as_deref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "persistent browser profile is missing",
                )
            })?;
            validate_profile_name(profile_name).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "persistent browser profile is invalid",
                )
            })?;
            let reservation_id = record.profile_reservation_id.as_deref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "persistent browser reservation is missing",
                )
            })?;
            validate_reservation_id(reservation_id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "persistent browser reservation is invalid",
                )
            })?;
        }
    }
    Ok(())
}

pub fn validate_host_gateway_hostname(hostname: &str) -> io::Result<()> {
    if hostname.is_empty()
        || hostname.len() > 253
        || !hostname.is_ascii()
        || hostname.parse::<IpAddr>().is_ok()
        || looks_like_legacy_ipv4(hostname)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "host gateway must be a bounded DNS hostname",
        ));
    }

    for label in hostname.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "host gateway must be a bounded DNS hostname",
            ));
        }
    }
    Ok(())
}

fn looks_like_legacy_ipv4(hostname: &str) -> bool {
    hostname.split('.').all(|component| {
        !component.is_empty()
            && (component.bytes().all(|byte| byte.is_ascii_digit())
                || component
                    .strip_prefix("0x")
                    .or_else(|| component.strip_prefix("0X"))
                    .is_some_and(|digits| {
                        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
                    }))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostGatewayHostname(String);

impl HostGatewayHostname {
    pub fn parse(hostname: impl Into<String>) -> io::Result<Self> {
        let hostname = hostname.into().to_ascii_lowercase();
        validate_host_gateway_hostname(&hostname)?;
        Ok(Self(hostname))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserInstanceStatus {
    Starting,
    Running,
    Stopped,
    Missing,
    Error,
}

impl BrowserInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Missing => "missing",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Podman,
    Docker,
}

impl RuntimeKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "podman" => Some(Self::Podman),
            "docker" => Some(Self::Docker),
            _ => None,
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        }
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserGraphicsMode {
    Disabled,
    SwiftShader,
    Gpu,
}

impl BrowserGraphicsMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "swiftshader" | "software" => Some(Self::SwiftShader),
            "gpu" => Some(Self::Gpu),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SwiftShader => "swiftshader",
            Self::Gpu => "gpu",
        }
    }
}

impl fmt::Display for BrowserGraphicsMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl RuntimeCommand {
    pub fn new(runtime: RuntimeKind, args: Vec<String>) -> Self {
        Self {
            program: runtime.program().to_owned(),
            args,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowserRunSpec {
    pub id: String,
    pub name: String,
    pub image: String,
    pub profile_dir: PathBuf,
    pub profile_name: Option<String>,
    pub host_gateway: Option<HostGatewayHostname>,
    pub graphics: BrowserGraphicsMode,
}

pub fn browser_run_command(runtime: RuntimeKind, spec: &BrowserRunSpec) -> RuntimeCommand {
    let mut args = vec![
        "run".to_owned(),
        "-d".to_owned(),
        "--name".to_owned(),
        spec.name.clone(),
        "--label".to_owned(),
        format!("{MANAGED_LABEL_KEY}=true"),
        "--label".to_owned(),
        format!("{INSTANCE_ID_LABEL_KEY}={}", spec.id),
        "-p".to_owned(),
        "127.0.0.1::9222".to_owned(),
        "-p".to_owned(),
        "127.0.0.1::5900".to_owned(),
        "-p".to_owned(),
        "127.0.0.1::6080".to_owned(),
        "-e".to_owned(),
        "CDP_PORT=9222".to_owned(),
        "-e".to_owned(),
        "VNC_PORT=5900".to_owned(),
        "-e".to_owned(),
        "NOVNC_PORT=6080".to_owned(),
        "-e".to_owned(),
        format!("CHROME_GRAPHICS={}", spec.graphics.as_str()),
        "-v".to_owned(),
        profile_volume_arg(runtime, &spec.profile_dir),
        spec.image.clone(),
    ];

    if let Some(profile_name) = &spec.profile_name {
        args.splice(
            8..8,
            [
                "--label".to_owned(),
                format!("{PROFILE_NAME_LABEL_KEY}={profile_name}"),
            ],
        );
    }

    if let Some(hostname) = &spec.host_gateway {
        args.splice(
            args.len() - 1..args.len() - 1,
            [
                "--add-host".to_owned(),
                format!("{}:host-gateway", hostname.as_str()),
            ],
        );
    }

    if runtime == RuntimeKind::Podman {
        args.insert(2, "--replace".to_owned());
        args.insert(3, "--userns=keep-id:uid=1000,gid=1000".to_owned());
    } else if runtime == RuntimeKind::Docker {
        args.splice(
            args.len() - 1..args.len() - 1,
            [
                "--user".to_owned(),
                profile_owner_user_arg(&spec.profile_dir),
                "-e".to_owned(),
                "HOME=/tmp".to_owned(),
                "-e".to_owned(),
                "CHROME_NO_SANDBOX=1".to_owned(),
            ],
        );
    }

    RuntimeCommand::new(runtime, args)
}

pub fn browser_stop_command(runtime: RuntimeKind, name: &str) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec![
            "stop".to_owned(),
            "--time".to_owned(),
            "10".to_owned(),
            name.to_owned(),
        ],
    )
}

pub fn browser_rm_command(runtime: RuntimeKind, name: &str) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec!["rm".to_owned(), "-f".to_owned(), name.to_owned()],
    )
}

pub fn browser_port_command(
    runtime: RuntimeKind,
    name: &str,
    container_port: u16,
) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec![
            "port".to_owned(),
            name.to_owned(),
            format!("{container_port}/tcp"),
        ],
    )
}

pub fn browser_inspect_status_command(runtime: RuntimeKind, name: &str) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec![
            "inspect".to_owned(),
            "--format".to_owned(),
            "{{.State.Status}}".to_owned(),
            name.to_owned(),
        ],
    )
}

pub fn browser_ps_managed_command(runtime: RuntimeKind) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec![
            "ps".to_owned(),
            "-a".to_owned(),
            "--filter".to_owned(),
            format!("label={MANAGED_LABEL_KEY}=true"),
            "--format".to_owned(),
            "{{.Names}}".to_owned(),
        ],
    )
}

pub fn browser_ps_all_command(runtime: RuntimeKind) -> RuntimeCommand {
    RuntimeCommand::new(
        runtime,
        vec![
            "ps".to_owned(),
            "-a".to_owned(),
            "--format".to_owned(),
            "{{.Names}}".to_owned(),
        ],
    )
}

pub fn parse_runtime_status(value: &str) -> BrowserInstanceStatus {
    match value.trim() {
        "created" | "initialized" | "configured" => BrowserInstanceStatus::Starting,
        "running" => BrowserInstanceStatus::Running,
        "exited" | "stopped" | "dead" | "removing" => BrowserInstanceStatus::Stopped,
        _ => BrowserInstanceStatus::Error,
    }
}

pub fn parse_published_port(value: &str) -> Option<u16> {
    value
        .lines()
        .filter_map(|line| {
            line.rsplit_once(':')
                .and_then(|(_, port)| port.trim().parse().ok())
        })
        .next()
}

pub fn generate_instance_id() -> String {
    format!(
        "{}-{}-{}",
        INSTANCE_NAME_PREFIX,
        now_unix_ms(),
        std::process::id()
    )
}

pub fn instance_name(id: &str) -> String {
    id.to_owned()
}

fn profile_volume_arg(runtime: RuntimeKind, profile_dir: &Path) -> String {
    let suffix = match runtime {
        RuntimeKind::Podman => ":Z",
        RuntimeKind::Docker => "",
    };
    format!("{}:/profile{suffix}", profile_dir.display())
}

fn profile_owner_user_arg(profile_dir: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if let Ok(metadata) = fs::metadata(profile_dir) {
            return format!("{}:{}", metadata.uid(), metadata.gid());
        }
    }

    "1000:1000".to_owned()
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "state path is not a directory",
            ));
        }
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => fs::create_dir_all(path)?,
        Err(source) => return Err(source),
    }
    set_private_directory_permissions(path)
}

fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private_file(path: &Path, body: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(source),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "private temporary file already exists",
            ));
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    use std::io::Write;
    file.write_all(body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.sync_all()
}

fn reject_symlink_or_non_file_if_present(path: &Path, description: &str) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(source),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{description} is not a regular file"),
            ))
        }
        Ok(_) => Ok(()),
    }
}

fn validate_regular_file_in_directory(
    path: &Path,
    directory: &Path,
    description: &str,
) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} is not a regular file"),
        ));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(fs::canonicalize(directory)?.as_path()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} escapes its directory"),
        ));
    }
    Ok(())
}

pub fn ensure_file_exists(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
}

pub fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn invalid_data(source: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_run_command_uses_loopback_random_ports_labels_and_isolated_profile() {
        let spec = BrowserRunSpec {
            id: "lantern-browser-test".to_owned(),
            name: "lantern-browser-test".to_owned(),
            image: DEFAULT_BROWSER_IMAGE.to_owned(),
            profile_dir: PathBuf::from("/tmp/lantern-profile"),
            profile_name: None,
            host_gateway: None,
            graphics: BrowserGraphicsMode::Disabled,
        };

        let command = browser_run_command(RuntimeKind::Podman, &spec);

        assert_eq!(command.program, "podman");
        assert!(
            command
                .args
                .contains(&"--userns=keep-id:uid=1000,gid=1000".to_owned())
        );
        assert!(command.args.contains(&"127.0.0.1::9222".to_owned()));
        assert!(command.args.contains(&"127.0.0.1::5900".to_owned()));
        assert!(command.args.contains(&"127.0.0.1::6080".to_owned()));
        assert!(
            command
                .args
                .contains(&"CHROME_GRAPHICS=disabled".to_owned())
        );
        assert!(command.args.contains(&format!("{MANAGED_LABEL_KEY}=true")));
        assert!(
            command
                .args
                .contains(&format!("{INSTANCE_ID_LABEL_KEY}=lantern-browser-test"))
        );
        assert!(
            command
                .args
                .contains(&"/tmp/lantern-profile:/profile:Z".to_owned())
        );
    }

    #[test]
    fn runtime_run_command_maps_only_the_validated_host_gateway_hostname() {
        let spec = BrowserRunSpec {
            id: "lantern-browser-test".to_owned(),
            name: "lantern-browser-test".to_owned(),
            image: DEFAULT_BROWSER_IMAGE.to_owned(),
            profile_dir: PathBuf::from("/tmp/lantern-profile"),
            profile_name: None,
            host_gateway: Some(
                HostGatewayHostname::parse("lv426.yutani.tech")
                    .expect("host gateway should validate"),
            ),
        };

        for runtime in [RuntimeKind::Podman, RuntimeKind::Docker] {
            let command = browser_run_command(runtime, &spec);
            let position = command
                .args
                .iter()
                .position(|argument| argument == "--add-host")
                .expect("host mapping flag should be present");
            assert_eq!(
                command.args.get(position + 1).map(String::as_str),
                Some("lv426.yutani.tech:host-gateway")
            );
        }
    }

    #[test]
    fn host_gateway_hostname_is_strictly_bounded() {
        for valid in ["app.example.test", "lv426.yutani.tech", "host-1.test"] {
            validate_host_gateway_hostname(valid).expect("hostname should be valid");
        }
        for invalid in [
            "",
            "127.0.0.1",
            "127.1",
            "127.0.1",
            "2130706433",
            "0177.0.0.1",
            "0x7f000001",
            "http://app.example.test",
            "app_example.test",
            ".example.test",
            "example.test.",
            "-app.example.test",
            "app-.example.test",
        ] {
            assert!(
                validate_host_gateway_hostname(invalid).is_err(),
                "{invalid} should be rejected"
            );
        }
        assert!(validate_host_gateway_hostname(&format!("{}.test", "a".repeat(64))).is_err());
        assert!(validate_host_gateway_hostname(&format!("{}.test", "a".repeat(249))).is_err());
        assert_eq!(
            HostGatewayHostname::parse("LV426.YUTANI.TECH")
                .expect("hostname should canonicalise")
                .as_str(),
            "lv426.yutani.tech"
        );
    }

    #[test]
    fn docker_volume_command_omits_podman_selinux_suffix() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-docker-command-test-{}",
            now_unix_ms()
        ));
        let registry = BrowserRegistry::new(&root);
        let layout = registry
            .create_instance_layout("lantern-browser-test")
            .expect("layout should be created");
        let spec = BrowserRunSpec {
            id: "lantern-browser-test".to_owned(),
            name: "lantern-browser-test".to_owned(),
            image: DEFAULT_BROWSER_IMAGE.to_owned(),
            profile_dir: layout.profile_dir.clone(),
            profile_name: None,
            host_gateway: None,
            graphics: BrowserGraphicsMode::SwiftShader,
        };

        let command = browser_run_command(RuntimeKind::Docker, &spec);

        assert_eq!(command.program, "docker");
        assert!(
            command
                .args
                .contains(&format!("{}:/profile", layout.profile_dir.display()))
        );
        assert!(command.args.contains(&"--user".to_owned()));
        assert!(command.args.contains(&"HOME=/tmp".to_owned()));
        assert!(command.args.contains(&"CHROME_NO_SANDBOX=1".to_owned()));
        assert!(
            command
                .args
                .contains(&"CHROME_GRAPHICS=swiftshader".to_owned())
        );
    }

    #[test]
    fn parses_published_loopback_port() {
        assert_eq!(parse_published_port("127.0.0.1:43791\n"), Some(43791));
    }

    #[test]
    fn registry_round_trips_record() {
        let root =
            std::env::temp_dir().join(format!("lantern-browser-registry-test-{}", now_unix_ms()));
        let registry = BrowserRegistry::new(&root);
        let layout = registry
            .create_instance_layout("lantern-browser-test")
            .expect("layout should be created");
        let record = BrowserInstanceRecord::pending(
            "lantern-browser-test".to_owned(),
            "lantern-browser-test".to_owned(),
            RuntimeKind::Podman,
            DEFAULT_BROWSER_IMAGE.to_owned(),
            layout.profile_dir,
            BrowserProfileKind::Disposable,
            None,
            None,
        );

        registry
            .write_record_atomic(&record)
            .expect("record should be written");

        let records = registry.list_records().expect("records should list");
        assert_eq!(records, vec![record]);

        fs::remove_dir_all(root).expect("test registry should be removed");
    }

    #[test]
    fn legacy_instance_record_without_host_gateway_remains_readable() {
        let record = BrowserInstanceRecord::pending(
            "lantern-browser-test".to_owned(),
            "lantern-browser-test".to_owned(),
            RuntimeKind::Podman,
            DEFAULT_BROWSER_IMAGE.to_owned(),
            PathBuf::from("/tmp/lantern-profile"),
            BrowserProfileKind::Disposable,
            None,
            None,
        );
        let mut value = serde_json::to_value(&record).expect("record should serialise");
        value
            .as_object_mut()
            .expect("record should be an object")
            .remove("host_gateway");

        let parsed: BrowserInstanceRecord =
            serde_json::from_value(value).expect("legacy record should parse");

        assert_eq!(parsed.host_gateway, None);
        assert_eq!(parsed.id, record.id);
    }

    #[test]
    fn browser_registry_rejects_record_identity_drift_and_traversal_removal() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-registry-identity-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-registry-victim-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserRegistry::new(&root);
        let layout = registry
            .create_instance_layout("safe-instance")
            .expect("layout should be created");
        let record = BrowserInstanceRecord::pending(
            "../../victim".to_owned(),
            "../../victim".to_owned(),
            RuntimeKind::Podman,
            DEFAULT_BROWSER_IMAGE.to_owned(),
            layout.profile_dir,
            BrowserProfileKind::Disposable,
            None,
            None,
        );
        fs::write(
            root.join("safe-instance/record.json"),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep"), "keep").unwrap();

        assert_eq!(
            registry.list_records().unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            registry
                .remove_instance_dir("../../victim")
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "keep");

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn browser_registry_rejects_symlink_paths_and_temporary_files() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-registry-symlink-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-registry-symlink-outside-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("linked-instance")).unwrap();
        let registry = BrowserRegistry::new(&root);
        assert_eq!(
            registry.list_records().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            registry
                .remove_instance_dir("linked-instance")
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );

        fs::remove_file(root.join("linked-instance")).unwrap();
        let layout = registry
            .create_instance_layout("safe-instance")
            .expect("layout should be created");
        let record = BrowserInstanceRecord::pending(
            "safe-instance".to_owned(),
            "safe-instance".to_owned(),
            RuntimeKind::Podman,
            DEFAULT_BROWSER_IMAGE.to_owned(),
            layout.profile_dir,
            BrowserProfileKind::Disposable,
            None,
            None,
        );
        let outside_file = outside.join("outside-record");
        fs::write(&outside_file, "unchanged").unwrap();
        symlink(&outside_file, root.join("safe-instance/record.json.tmp")).unwrap();
        assert_eq!(
            registry.write_record_atomic(&record).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&outside_file).unwrap(), "unchanged");
        fs::remove_file(root.join("safe-instance/record.json.tmp")).unwrap();
        registry.write_record_atomic(&record).unwrap();
        let outside_record = outside.join("outside-valid-record");
        fs::rename(root.join("safe-instance/record.json"), &outside_record).unwrap();
        symlink(&outside_record, root.join("safe-instance/record.json")).unwrap();
        assert_eq!(
            registry.list_records().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn persistent_profile_registry_creates_private_root_data_and_record() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-registry-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);

        let created = registry
            .create("geometis-review")
            .expect("profile should be created");
        assert_eq!(created.name, "geometis-review");
        assert_eq!(created.attachment, None);
        assert_eq!(
            registry
                .data_dir("geometis-review")
                .expect("data directory should resolve"),
            root.join("geometis-review/data")
        );
        assert_eq!(
            registry.list().expect("profiles should list"),
            vec![created]
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(root.join("geometis-review/data"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(root.join("geometis-review/profile.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(root.join("geometis-review/operation.lock"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        fs::remove_dir_all(root).expect("test registry should be removed");
    }

    #[test]
    fn persistent_profile_operation_lock_fences_concurrent_lifecycle_commands() {
        use std::sync::{Arc, Barrier};

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-operation-lock-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry.create("review").unwrap();
        let first = registry.try_operation_lock("review").unwrap();
        let attempted = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker = {
            let registry = registry.clone();
            let attempted = Arc::clone(&attempted);
            let release = Arc::clone(&release);
            std::thread::spawn(move || {
                let blocked = registry.try_operation_lock("review");
                attempted.wait();
                release.wait();
                let acquired = registry.try_operation_lock("review");
                (blocked, acquired)
            })
        };
        attempted.wait();
        drop(first);
        release.wait();
        let (blocked, acquired) = worker.join().unwrap();
        assert_eq!(blocked.unwrap_err().kind(), io::ErrorKind::WouldBlock);
        assert!(acquired.is_ok());

        registry.delete("review").unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_profile_registry_rejects_invalid_names_and_duplicates() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-name-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);

        for name in [
            "",
            "../escape",
            "daily profile",
            ".",
            "profile/name",
            "a23456789012345678901234567890123456789012345678",
        ] {
            assert_eq!(
                registry.create(name).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        registry
            .create("review")
            .expect("profile should be created");
        assert_eq!(
            registry.create("review").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );

        fs::remove_dir_all(root).expect("test registry should be removed");
    }

    #[test]
    fn persistent_instance_ids_are_stable_and_namespaced_by_state_root() {
        let base = std::env::temp_dir().join(format!(
            "lantern-browser-profile-instance-id-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let first = BrowserProfileRegistry::new(base.join("first/browser-profiles"));
        let second = BrowserProfileRegistry::new(base.join("second/browser-profiles"));
        first.create("review").unwrap();
        second.create("review").unwrap();

        let first_id = first.persistent_instance_id("review").unwrap();
        let second_id = second.persistent_instance_id("review").unwrap();
        assert_eq!(first_id, first.persistent_instance_id("review").unwrap());
        assert_ne!(first_id, second_id);
        assert!(first_id.starts_with(PERSISTENT_INSTANCE_ID_PREFIX));
        assert!(second_id.starts_with(PERSISTENT_INSTANCE_ID_PREFIX));
        assert!(first_id.len() <= 80);
        assert!(second_id.len() <= 80);

        fs::remove_dir_all(base).expect("test registries should be removed");
    }

    #[test]
    fn persistent_profile_attachment_is_compare_and_swap_and_delete_is_explicit() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-attachment-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry
            .create("review")
            .expect("profile should be created");
        let first = BrowserProfileAttachment {
            instance_id: "review-browser".to_owned(),
            container_name: "review-browser".to_owned(),
            runtime: RuntimeKind::Podman,
            reservation_id: "reservation-first".to_owned(),
            state: BrowserProfileAttachmentState::Active,
        };
        let second = BrowserProfileAttachment {
            instance_id: "other-browser".to_owned(),
            container_name: "other-browser".to_owned(),
            runtime: RuntimeKind::Docker,
            reservation_id: "reservation-second".to_owned(),
            state: BrowserProfileAttachmentState::Active,
        };
        let invalid = BrowserProfileAttachment {
            instance_id: "invalid-browser".to_owned(),
            container_name: "different-container".to_owned(),
            runtime: RuntimeKind::Podman,
            reservation_id: "reservation-invalid".to_owned(),
            state: BrowserProfileAttachmentState::Active,
        };

        assert_eq!(
            registry
                .compare_and_swap_attachment("review", None, Some(invalid))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );

        registry
            .compare_and_swap_attachment("review", None, Some(first.clone()))
            .expect("first attachment should reserve");
        assert_eq!(
            registry
                .compare_and_swap_attachment("review", None, Some(second.clone()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            registry.delete("review").unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(
            !registry
                .release_if_owner("review", &second)
                .expect("non-owner release should be a no-op")
        );
        assert!(
            registry
                .release_if_owner("review", &first)
                .expect("owner should release")
        );
        registry
            .delete("review")
            .expect("released profile should delete");
        assert!(!root.join("review").exists());

        fs::remove_dir_all(root).expect("test registry should be removed");
    }

    #[test]
    fn persistent_profile_reservation_token_prevents_same_identity_aba_release() {
        use std::sync::{Arc, Barrier};

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-aba-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry.create("review").unwrap();
        let initial = BrowserProfileAttachment {
            instance_id: "review-browser".to_owned(),
            container_name: "review-browser".to_owned(),
            runtime: RuntimeKind::Podman,
            reservation_id: "reservation-initial".to_owned(),
            state: BrowserProfileAttachmentState::Active,
        };
        registry
            .compare_and_swap_attachment("review", None, Some(initial.clone()))
            .unwrap();

        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for suffix in ["one", "two"] {
            let registry = registry.clone();
            let barrier = Arc::clone(&barrier);
            let initial = initial.clone();
            workers.push(std::thread::spawn(move || {
                let replacement = BrowserProfileAttachment {
                    reservation_id: format!("reservation-{suffix}"),
                    ..initial.clone()
                };
                barrier.wait();
                registry.compare_and_swap_attachment("review", Some(&initial), Some(replacement))
            }));
        }
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| result
                    .as_ref()
                    .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock))
                .count(),
            1
        );
        assert!(!registry.release_if_owner("review", &initial).unwrap());
        let winner = registry.read("review").unwrap().attachment.unwrap();
        assert!(registry.release_if_owner("review", &winner).unwrap());

        registry.delete("review").unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stopping_reservation_blocks_a_same_identity_restart() {
        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-stopping-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry.create("review").unwrap();
        let active = BrowserProfileAttachment {
            instance_id: "review-browser".to_owned(),
            container_name: "review-browser".to_owned(),
            runtime: RuntimeKind::Podman,
            reservation_id: "reservation-active".to_owned(),
            state: BrowserProfileAttachmentState::Active,
        };
        registry
            .compare_and_swap_attachment("review", None, Some(active.clone()))
            .unwrap();
        let stopping = BrowserProfileAttachment {
            state: BrowserProfileAttachmentState::Stopping,
            ..active.clone()
        };
        registry
            .compare_and_swap_attachment("review", Some(&active), Some(stopping.clone()))
            .unwrap();
        let restart = BrowserProfileAttachment {
            reservation_id: "reservation-restart".to_owned(),
            ..active.clone()
        };

        assert_eq!(
            registry
                .compare_and_swap_attachment("review", Some(&active), Some(restart))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            registry.read("review").unwrap().attachment,
            Some(stopping.clone())
        );

        assert!(!registry.release_if_owner("review", &active).unwrap());
        assert!(registry.release_if_owner("review", &stopping).unwrap());
        registry.delete("review").unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn persistent_profile_registry_rejects_symlink_profile_directory() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-symlink-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-profile-symlink-outside-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("review")).unwrap();
        let registry = BrowserProfileRegistry::new(&root);

        assert_eq!(
            registry.read("review").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            registry.delete("review").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(outside.exists());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn persistent_profile_registry_rejects_symlink_record_and_invalid_attachment_identity() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-record-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-profile-record-outside-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry
            .create("review")
            .expect("profile should be created");
        fs::rename(root.join("review/profile.json"), &outside).unwrap();
        symlink(&outside, root.join("review/profile.json")).unwrap();
        assert_eq!(
            registry.read("review").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_file(root.join("review/profile.json")).unwrap();
        fs::rename(&outside, root.join("review/profile.json")).unwrap();

        let invalid = BrowserProfileRecord {
            schema_version: 1,
            name: "review".to_owned(),
            attachment: Some(BrowserProfileAttachment {
                instance_id: "review-browser".to_owned(),
                container_name: "different-container".to_owned(),
                runtime: RuntimeKind::Podman,
                reservation_id: "reservation-invalid".to_owned(),
                state: BrowserProfileAttachmentState::Active,
            }),
            created_at_unix_ms: now_unix_ms(),
            updated_at_unix_ms: now_unix_ms(),
        };
        registry
            .write_record_atomic_unlocked(&invalid)
            .expect("invalid fixture should be written");
        assert_eq!(
            registry.read("review").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn persistent_profile_registry_does_not_follow_temporary_record_symlinks() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-temp-symlink-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-profile-temp-symlink-outside-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        let record = registry.create("review").unwrap();
        fs::write(&outside, "unchanged").unwrap();
        symlink(&outside, root.join("review/profile.json.tmp")).unwrap();

        assert_eq!(
            registry
                .write_record_atomic_unlocked(&record)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), "unchanged");

        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn persistent_profile_registry_rejects_symlink_operation_lock() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lantern-browser-profile-operation-symlink-test-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "lantern-browser-profile-operation-symlink-outside-{}-{}",
            now_unix_ms(),
            std::process::id()
        ));
        let registry = BrowserProfileRegistry::new(&root);
        registry.create("review").unwrap();
        fs::write(&outside, "unchanged").unwrap();
        fs::remove_file(root.join("review/operation.lock")).unwrap();
        symlink(&outside, root.join("review/operation.lock")).unwrap();

        assert_eq!(
            registry.try_operation_lock("review").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read_to_string(&outside).unwrap(), "unchanged");

        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn persistent_runtime_command_labels_profile_without_changing_loopback_ports() {
        let spec = BrowserRunSpec {
            id: "geometis-review".to_owned(),
            name: "geometis-review".to_owned(),
            image: DEFAULT_BROWSER_IMAGE.to_owned(),
            profile_dir: PathBuf::from("/tmp/geometis-review-profile"),
            profile_name: Some("geometis-review".to_owned()),
            host_gateway: None,
        };

        let command = browser_run_command(RuntimeKind::Podman, &spec);

        assert!(
            command
                .args
                .contains(&format!("{PROFILE_NAME_LABEL_KEY}=geometis-review"))
        );
        assert!(command.args.contains(&"127.0.0.1::9222".to_owned()));
        assert!(command.args.contains(&"127.0.0.1::5900".to_owned()));
        assert!(command.args.contains(&"127.0.0.1::6080".to_owned()));
    }

    #[test]
    fn all_container_listing_is_unfiltered_for_missing_status_proof() {
        let command = browser_ps_all_command(RuntimeKind::Podman);

        assert_eq!(command.program, "podman");
        assert_eq!(command.args, ["ps", "-a", "--format", "{{.Names}}"]);
        assert!(!command.args.iter().any(|argument| argument == "--filter"));
    }
}
