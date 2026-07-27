use std::{
    fmt,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_BROWSER_IMAGE: &str = "localhost/lantern-browser-cdp:stable";
pub const INSTANCE_ROOT: &str = ".smoogle/lantern/browser-instances";
pub const MANAGED_LABEL_KEY: &str = "dev.lantern.managed";
pub const INSTANCE_ID_LABEL_KEY: &str = "dev.lantern.instance-id";
pub const INSTANCE_NAME_PREFIX: &str = "lantern-browser";

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
        fs::create_dir_all(&self.root)?;
        let instance_dir = self.root.join(id);
        fs::create_dir(&instance_dir)?;
        let profile_dir = instance_dir.join("profile");
        fs::create_dir(&profile_dir)?;
        Ok(BrowserInstanceLayout {
            instance_dir,
            profile_dir,
        })
    }

    pub fn write_record_atomic(&self, record: &BrowserInstanceRecord) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        let _lock = self.lock()?;
        self.write_record_atomic_unlocked(record)
    }

    pub fn update_record<F>(&self, id: &str, update: F) -> io::Result<BrowserInstanceRecord>
    where
        F: FnOnce(&mut BrowserInstanceRecord),
    {
        let _lock = self.lock()?;
        let mut record = self.read_record_unlocked(id)?;
        update(&mut record);
        record.updated_at_unix_ms = now_unix_ms();
        self.write_record_atomic_unlocked(&record)?;
        Ok(record)
    }

    pub fn read_record(&self, id: &str) -> io::Result<BrowserInstanceRecord> {
        self.read_record_unlocked(id)
    }

    pub fn list_records(&self) -> io::Result<Vec<BrowserInstanceRecord>> {
        let mut records: Vec<BrowserInstanceRecord> = Vec::new();
        match fs::read_dir(&self.root) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    if !entry.file_type()?.is_dir() {
                        continue;
                    }
                    let path = entry.path().join("record.json");
                    if !path.exists() {
                        continue;
                    }
                    let body = fs::read_to_string(path)?;
                    records.push(serde_json::from_str(&body).map_err(invalid_data)?);
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(source),
        }

        records.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(records)
    }

    pub fn remove_instance_dir(&self, id: &str) -> io::Result<()> {
        let path = self.root.join(id);
        match fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(source),
        }
    }

    fn read_record_unlocked(&self, id: &str) -> io::Result<BrowserInstanceRecord> {
        let body = fs::read_to_string(self.root.join(id).join("record.json"))?;
        serde_json::from_str(&body).map_err(invalid_data)
    }

    fn write_record_atomic_unlocked(&self, record: &BrowserInstanceRecord) -> io::Result<()> {
        let instance_dir = self.root.join(&record.id);
        fs::create_dir_all(&instance_dir)?;
        let final_path = instance_dir.join("record.json");
        let tmp_path = instance_dir.join("record.json.tmp");
        let body = serde_json::to_vec_pretty(record).map_err(invalid_data)?;
        fs::write(&tmp_path, body)?;
        fs::rename(tmp_path, final_path)
    }

    fn lock(&self) -> io::Result<RegistryLock> {
        fs::create_dir_all(&self.root)?;
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
    #[serde(default)]
    pub graphics: BrowserGraphicsMode,
    #[serde(default)]
    pub gpu_device: Option<String>,
    pub container_id: Option<String>,
    pub status: BrowserInstanceStatus,
    pub endpoint: Option<String>,
    pub cdp_host_port: Option<u16>,
    pub novnc_url: Option<String>,
    pub novnc_host_port: Option<u16>,
    pub vnc_host_port: Option<u16>,
    pub profile_dir: PathBuf,
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
        graphics: BrowserGraphicsMode,
        gpu_device: Option<String>,
    ) -> Self {
        let now = now_unix_ms();
        Self {
            schema_version: 1,
            id,
            name,
            runtime,
            image,
            graphics,
            gpu_device,
            container_id: None,
            status: BrowserInstanceStatus::Starting,
            endpoint: None,
            cdp_host_port: None,
            novnc_url: None,
            novnc_host_port: None,
            vnc_host_port: None,
            profile_dir,
            created_at_unix_ms: now,
            updated_at_unix_ms: now,
        }
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
    WebGpu,
}

impl Default for BrowserGraphicsMode {
    fn default() -> Self {
        Self::Disabled
    }
}

impl BrowserGraphicsMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "disabled" => Some(Self::Disabled),
            "swiftshader" | "software" => Some(Self::SwiftShader),
            "gpu" => Some(Self::Gpu),
            "webgpu" => Some(Self::WebGpu),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SwiftShader => "swiftshader",
            Self::Gpu => "gpu",
            Self::WebGpu => "webgpu",
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
    pub graphics: BrowserGraphicsMode,
    pub gpu_device: Option<String>,
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

    if let Some(device) = &spec.gpu_device {
        args.splice(
            args.len() - 1..args.len() - 1,
            ["--device".to_owned(), device.clone()],
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
            graphics: BrowserGraphicsMode::Disabled,
            gpu_device: None,
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
            graphics: BrowserGraphicsMode::SwiftShader,
            gpu_device: None,
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
            BrowserGraphicsMode::Disabled,
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
    fn hardware_webgpu_command_passes_explicit_device_and_mode() {
        let spec = BrowserRunSpec {
            id: "lantern-browser-webgpu".to_owned(),
            name: "lantern-browser-webgpu".to_owned(),
            image: DEFAULT_BROWSER_IMAGE.to_owned(),
            profile_dir: PathBuf::from("/tmp/lantern-webgpu-profile"),
            graphics: BrowserGraphicsMode::WebGpu,
            gpu_device: Some("nvidia.com/gpu=0".to_owned()),
        };

        let command = browser_run_command(RuntimeKind::Podman, &spec);
        let device_position = command
            .args
            .iter()
            .position(|argument| argument == "--device")
            .expect("hardware mode passes a runtime device");

        assert_eq!(command.args[device_position + 1], "nvidia.com/gpu=0");
        assert!(command.args.contains(&"CHROME_GRAPHICS=webgpu".to_owned()));
        assert_eq!(command.args.last(), Some(&DEFAULT_BROWSER_IMAGE.to_owned()));
    }

    #[test]
    fn old_instance_records_default_to_disabled_graphics_without_a_device() {
        let record: BrowserInstanceRecord = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "id": "legacy",
                "name": "legacy",
                "runtime": "podman",
                "image": "browser:old",
                "container_id": null,
                "status": "stopped",
                "endpoint": null,
                "cdp_host_port": null,
                "novnc_url": null,
                "novnc_host_port": null,
                "vnc_host_port": null,
                "profile_dir": "/tmp/legacy",
                "created_at_unix_ms": 1,
                "updated_at_unix_ms": 1
            }"#,
        )
        .expect("legacy record remains readable");

        assert_eq!(record.graphics, BrowserGraphicsMode::Disabled);
        assert_eq!(record.gpu_device, None);
    }
}
