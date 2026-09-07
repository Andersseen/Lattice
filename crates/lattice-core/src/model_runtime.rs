use crate::AppError;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const GET_MODEL_RUNTIME_STATUS_COMMAND: &str = "get_model_runtime_status";
pub const CONFIGURE_MODEL_RUNTIME_COMMAND: &str = "configure_model_runtime";
pub const PROBE_MODEL_RUNTIME_COMMAND: &str = "probe_model_runtime";

pub const DEFAULT_RUNTIME_MESSAGE: &str = "No runtime executable configured.";
const PROBE_REQUIRED_MESSAGE: &str = "Approve a runtime probe to check this executable.";
const EXECUTABLE_CHANGED_MESSAGE: &str = "Runtime executable changed; approve a new probe.";
const EXECUTABLE_MISSING_MESSAGE: &str = "Runtime executable could not be found.";
const SUPPORTED_MESSAGE: &str = "Runtime discovery completed.";
const UNSUPPORTED_MESSAGE: &str = "Runtime CLI version is not supported.";
const UNKNOWN_MESSAGE: &str = "Runtime status could not be determined.";
const UNREACHABLE_MESSAGE: &str = "Runtime server reported a port that is not reachable.";
const STOPPED_MESSAGE: &str = "Runtime daemon or server is not running.";
const MAX_EXECUTABLE_PATH_LENGTH: usize = 4096;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const TCP_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const MIN_SUPPORTED_CLI_VERSION: Version = Version {
    major: 0,
    minor: 0,
    patch: 47,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelRuntimeAvailability {
    Missing,
    Unsupported,
    Stopped,
    Running,
    Unreachable,
    Unknown,
}

impl ModelRuntimeAvailability {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unsupported => "unsupported",
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Unreachable => "unreachable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeDaemonStatus {
    Running,
    NotRunning,
    Unknown,
}

impl RuntimeDaemonStatus {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::NotRunning => "notRunning",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeServerStatus {
    Running,
    Stopped,
    Unreachable,
    Unknown,
}

impl RuntimeServerStatus {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Unreachable => "unreachable",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbeApproval {
    pub executable_fingerprint: String,
    pub cli_version: String,
    pub checked_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDaemonObservation {
    pub status: RuntimeDaemonStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_daemon: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl Default for RuntimeDaemonObservation {
    fn default() -> Self {
        Self {
            status: RuntimeDaemonStatus::Unknown,
            pid: None,
            is_daemon: None,
            version: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeServerObservation {
    pub status: RuntimeServerStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

impl Default for RuntimeServerObservation {
    fn default() -> Self {
        Self {
            status: RuntimeServerStatus::Unknown,
            port: None,
            endpoint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRuntimeStatus {
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<String>,
    pub availability: ModelRuntimeAvailability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved: Option<RuntimeProbeApproval>,
    pub daemon: RuntimeDaemonObservation,
    pub server: RuntimeServerObservation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked_unix_seconds: Option<u64>,
    pub message: String,
}

impl Default for ModelRuntimeStatus {
    fn default() -> Self {
        Self {
            revision: 1,
            executable_path: None,
            availability: ModelRuntimeAvailability::Missing,
            cli_version: None,
            approved: None,
            daemon: RuntimeDaemonObservation::default(),
            server: RuntimeServerObservation::default(),
            last_checked_unix_seconds: None,
            message: DEFAULT_RUNTIME_MESSAGE.to_string(),
        }
    }
}

impl ModelRuntimeStatus {
    pub fn configured(revision: u64, executable_path: String) -> Self {
        Self {
            revision,
            executable_path: Some(executable_path),
            availability: ModelRuntimeAvailability::Unknown,
            message: PROBE_REQUIRED_MESSAGE.to_string(),
            ..Self::default()
        }
    }

    pub fn with_current_file_state(mut self) -> Self {
        let Some(executable_path) = self.executable_path.as_deref() else {
            return Self::default_with_revision(self.revision);
        };

        let path = Path::new(executable_path);
        let Ok(current_fingerprint) = executable_fingerprint(path) else {
            self.availability = ModelRuntimeAvailability::Unknown;
            self.message = UNKNOWN_MESSAGE.to_string();
            return self;
        };

        let Some(current_fingerprint) = current_fingerprint else {
            self.availability = ModelRuntimeAvailability::Missing;
            self.message = EXECUTABLE_MISSING_MESSAGE.to_string();
            return self;
        };

        if let Some(approval) = &self.approved {
            if approval.executable_fingerprint != current_fingerprint {
                self.availability = ModelRuntimeAvailability::Unknown;
                self.message = EXECUTABLE_CHANGED_MESSAGE.to_string();
            }
        }

        self
    }

    fn default_with_revision(revision: u64) -> Self {
        Self {
            revision,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureModelRuntimeRequest {
    pub expected_revision: u64,
    pub executable_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeModelRuntimeRequest {
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    fn is_supported(self) -> bool {
        (self.major, self.minor, self.patch)
            >= (
                MIN_SUPPORTED_CLI_VERSION.major,
                MIN_SUPPORTED_CLI_VERSION.minor,
                MIN_SUPPORTED_CLI_VERSION.patch,
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeResult {
    pub availability: ModelRuntimeAvailability,
    pub cli_version: Option<String>,
    pub approved: Option<RuntimeProbeApproval>,
    pub daemon: RuntimeDaemonObservation,
    pub server: RuntimeServerObservation,
    pub last_checked_unix_seconds: u64,
    pub message: String,
}

pub fn validate_runtime_executable_path(raw_path: &str) -> Result<PathBuf, AppError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_EXECUTABLE_PATH_LENGTH {
        return Err(AppError::invalid_runtime(
            "Runtime executable path must be an absolute path.",
        ));
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(AppError::invalid_runtime(
            "Runtime executable path must be an absolute path.",
        ));
    }

    Ok(path)
}

pub fn probe_model_runtime(executable_path: &Path) -> RuntimeProbeResult {
    let checked_at = unix_timestamp_now();
    let fingerprint = match executable_fingerprint(executable_path) {
        Ok(Some(fingerprint)) => fingerprint,
        Ok(None) => {
            return RuntimeProbeResult {
                availability: ModelRuntimeAvailability::Missing,
                cli_version: None,
                approved: None,
                daemon: RuntimeDaemonObservation::default(),
                server: RuntimeServerObservation::default(),
                last_checked_unix_seconds: checked_at,
                message: EXECUTABLE_MISSING_MESSAGE.to_string(),
            };
        }
        Err(_) => {
            return RuntimeProbeResult {
                availability: ModelRuntimeAvailability::Unknown,
                cli_version: None,
                approved: None,
                daemon: RuntimeDaemonObservation::default(),
                server: RuntimeServerObservation::default(),
                last_checked_unix_seconds: checked_at,
                message: UNKNOWN_MESSAGE.to_string(),
            };
        }
    };

    let version_output = match run_probe_command(executable_path, &["--version"]) {
        Ok(output) if output.status.success() => output.combined_text(),
        Ok(_) | Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            return unknown_probe_result(checked_at);
        }
        Err(CommandProbeError::Timeout) => {
            return unknown_probe_result(checked_at);
        }
    };
    let Some((cli_version, parsed_version)) = first_semver(&version_output) else {
        return unknown_probe_result(checked_at);
    };

    if !parsed_version.is_supported() {
        return RuntimeProbeResult {
            availability: ModelRuntimeAvailability::Unsupported,
            cli_version: Some(cli_version.clone()),
            approved: Some(RuntimeProbeApproval {
                executable_fingerprint: fingerprint,
                cli_version,
                checked_at_unix_seconds: checked_at,
            }),
            daemon: RuntimeDaemonObservation::default(),
            server: RuntimeServerObservation::default(),
            last_checked_unix_seconds: checked_at,
            message: UNSUPPORTED_MESSAGE.to_string(),
        };
    }

    let daemon = probe_daemon(executable_path).unwrap_or_default();
    let mut server = probe_server(executable_path).unwrap_or_default();
    if server.status == RuntimeServerStatus::Running {
        server = with_loopback_health(server);
    }

    let availability = runtime_availability(&daemon, &server);
    let message = match availability {
        ModelRuntimeAvailability::Running => SUPPORTED_MESSAGE,
        ModelRuntimeAvailability::Stopped => STOPPED_MESSAGE,
        ModelRuntimeAvailability::Unreachable => UNREACHABLE_MESSAGE,
        ModelRuntimeAvailability::Unknown => UNKNOWN_MESSAGE,
        ModelRuntimeAvailability::Missing => EXECUTABLE_MISSING_MESSAGE,
        ModelRuntimeAvailability::Unsupported => UNSUPPORTED_MESSAGE,
    };

    RuntimeProbeResult {
        availability,
        cli_version: Some(cli_version.clone()),
        approved: Some(RuntimeProbeApproval {
            executable_fingerprint: fingerprint,
            cli_version,
            checked_at_unix_seconds: checked_at,
        }),
        daemon,
        server,
        last_checked_unix_seconds: checked_at,
        message: message.to_string(),
    }
}

pub fn availability_from_storage(value: &str) -> Result<ModelRuntimeAvailability, AppError> {
    match value {
        "missing" => Ok(ModelRuntimeAvailability::Missing),
        "unsupported" => Ok(ModelRuntimeAvailability::Unsupported),
        "stopped" => Ok(ModelRuntimeAvailability::Stopped),
        "running" => Ok(ModelRuntimeAvailability::Running),
        "unreachable" => Ok(ModelRuntimeAvailability::Unreachable),
        "unknown" => Ok(ModelRuntimeAvailability::Unknown),
        _ => Err(AppError::storage_unavailable(
            "Lattice could not read model runtime status.",
        )),
    }
}

pub fn daemon_status_from_storage(value: &str) -> Result<RuntimeDaemonStatus, AppError> {
    match value {
        "running" => Ok(RuntimeDaemonStatus::Running),
        "notRunning" => Ok(RuntimeDaemonStatus::NotRunning),
        "unknown" => Ok(RuntimeDaemonStatus::Unknown),
        _ => Err(AppError::storage_unavailable(
            "Lattice could not read model runtime status.",
        )),
    }
}

pub fn server_status_from_storage(value: &str) -> Result<RuntimeServerStatus, AppError> {
    match value {
        "running" => Ok(RuntimeServerStatus::Running),
        "stopped" => Ok(RuntimeServerStatus::Stopped),
        "unreachable" => Ok(RuntimeServerStatus::Unreachable),
        "unknown" => Ok(RuntimeServerStatus::Unknown),
        _ => Err(AppError::storage_unavailable(
            "Lattice could not read model runtime status.",
        )),
    }
}

pub fn executable_fingerprint(path: &Path) -> Result<Option<String>, AppError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(AppError::storage_unavailable(
                "Lattice could not inspect runtime executable.",
            ));
        }
    };

    if !metadata.is_file() {
        return Ok(None);
    }

    let canonical = fs::canonicalize(path).map_err(|_| {
        AppError::storage_unavailable("Lattice could not inspect runtime executable.")
    })?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());

    Ok(Some(format!(
        "{}:{}:{}",
        canonical.display(),
        metadata.len(),
        modified
    )))
}

fn probe_daemon(executable_path: &Path) -> Result<RuntimeDaemonObservation, CommandProbeError> {
    let output = run_probe_command(executable_path, &["daemon", "status", "--json"])?;
    if !output.status.success() {
        return Ok(RuntimeDaemonObservation::default());
    }

    let daemon: LmsDaemonStatus =
        serde_json::from_str(&output.stdout).map_err(|_| CommandProbeError::Io)?;

    Ok(match daemon.status.as_str() {
        "running" => RuntimeDaemonObservation {
            status: RuntimeDaemonStatus::Running,
            pid: daemon.pid,
            is_daemon: daemon.is_daemon,
            version: daemon.version,
        },
        "not-running" => RuntimeDaemonObservation {
            status: RuntimeDaemonStatus::NotRunning,
            ..RuntimeDaemonObservation::default()
        },
        _ => RuntimeDaemonObservation::default(),
    })
}

fn probe_server(executable_path: &Path) -> Result<RuntimeServerObservation, CommandProbeError> {
    let output = run_probe_command(executable_path, &["server", "status", "--json", "--quiet"])?;
    if !output.status.success() {
        return Ok(RuntimeServerObservation::default());
    }

    let server: LmsServerStatus =
        serde_json::from_str(&output.stdout).map_err(|_| CommandProbeError::Io)?;

    if !server.running {
        return Ok(RuntimeServerObservation {
            status: RuntimeServerStatus::Stopped,
            ..RuntimeServerObservation::default()
        });
    }

    let Some(port) = server.port else {
        return Ok(RuntimeServerObservation::default());
    };

    Ok(RuntimeServerObservation {
        status: RuntimeServerStatus::Running,
        port: Some(port),
        endpoint: Some(format!("http://127.0.0.1:{port}")),
    })
}

fn with_loopback_health(mut server: RuntimeServerObservation) -> RuntimeServerObservation {
    let Some(port) = server.port else {
        return RuntimeServerObservation::default();
    };

    let address = SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&address, TCP_TIMEOUT).is_err() {
        server.status = RuntimeServerStatus::Unreachable;
    }

    server
}

fn runtime_availability(
    daemon: &RuntimeDaemonObservation,
    server: &RuntimeServerObservation,
) -> ModelRuntimeAvailability {
    if server.status == RuntimeServerStatus::Unreachable {
        return ModelRuntimeAvailability::Unreachable;
    }

    if daemon.status == RuntimeDaemonStatus::Running
        && server.status == RuntimeServerStatus::Running
    {
        return ModelRuntimeAvailability::Running;
    }

    if daemon.status == RuntimeDaemonStatus::Unknown
        || server.status == RuntimeServerStatus::Unknown
    {
        return ModelRuntimeAvailability::Unknown;
    }

    if daemon.status == RuntimeDaemonStatus::NotRunning
        || server.status == RuntimeServerStatus::Stopped
    {
        return ModelRuntimeAvailability::Stopped;
    }

    ModelRuntimeAvailability::Unknown
}

fn unknown_probe_result(checked_at: u64) -> RuntimeProbeResult {
    RuntimeProbeResult {
        availability: ModelRuntimeAvailability::Unknown,
        cli_version: None,
        approved: None,
        daemon: RuntimeDaemonObservation::default(),
        server: RuntimeServerObservation::default(),
        last_checked_unix_seconds: checked_at,
        message: UNKNOWN_MESSAGE.to_string(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LmsDaemonStatus {
    status: String,
    pid: Option<u32>,
    is_daemon: Option<bool>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LmsServerStatus {
    running: bool,
    port: Option<u16>,
}

#[derive(Debug)]
struct CommandOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl CommandOutput {
    fn combined_text(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandProbeError {
    Spawn,
    Timeout,
    Io,
}

fn run_probe_command(
    executable_path: &Path,
    args: &[&str],
) -> Result<CommandOutput, CommandProbeError> {
    let stdout_path = create_output_path("lattice-runtime-probe-stdout")?;
    let stderr_path = create_output_path("lattice-runtime-probe-stderr")?;
    let stdout = File::create(&stdout_path).map_err(|_| CommandProbeError::Io)?;
    let stderr = File::create(&stderr_path).map_err(|_| CommandProbeError::Io)?;

    let mut child = Command::new(executable_path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|_| CommandProbeError::Spawn)?;

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| CommandProbeError::Io)? {
            break status;
        }

        if started.elapsed() >= COMMAND_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            return Err(CommandProbeError::Timeout);
        }

        thread::sleep(Duration::from_millis(20));
    };

    let stdout = read_capped_output(&stdout_path)?;
    let stderr = read_capped_output(&stderr_path)?;
    let _ = fs::remove_file(&stdout_path);
    let _ = fs::remove_file(&stderr_path);

    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

fn create_output_path(prefix: &str) -> Result<PathBuf, CommandProbeError> {
    let base = std::env::temp_dir();
    let process_id = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());

    for attempt in 0..16 {
        let path = base.join(format!("{prefix}-{process_id}-{nanos}-{attempt}.log"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(CommandProbeError::Io),
        }
    }

    Err(CommandProbeError::Io)
}

fn read_capped_output(path: &Path) -> Result<String, CommandProbeError> {
    let file = File::open(path).map_err(|_| CommandProbeError::Io)?;
    let mut output = Vec::new();
    file.take(MAX_OUTPUT_BYTES + 1)
        .read_to_end(&mut output)
        .map_err(|_| CommandProbeError::Io)?;

    if output.len() > MAX_OUTPUT_BYTES as usize {
        output.truncate(MAX_OUTPUT_BYTES as usize);
    }

    Ok(String::from_utf8_lossy(&output).to_string())
}

fn first_semver(text: &str) -> Option<(String, Version)> {
    let bytes = text.as_bytes();
    for index in 0..bytes.len() {
        if !bytes[index].is_ascii_digit() {
            continue;
        }

        if let Some((version_text, version)) = parse_semver_from(&text[index..]) {
            return Some((version_text, version));
        }
    }

    None
}

fn parse_semver_from(text: &str) -> Option<(String, Version)> {
    let mut end = 0;
    for character in text.chars() {
        if character.is_ascii_digit() || character == '.' || character == '+' || character == '-' {
            end += character.len_utf8();
        } else {
            break;
        }
    }

    let candidate = &text[..end];
    let core = candidate
        .split_once('+')
        .map_or(candidate, |(core, _)| core)
        .split_once('-')
        .map_or(candidate, |(core, _)| core);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some((
        candidate.to_string(),
        Version {
            major,
            minor,
            patch,
        },
    ))
}

fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{
        executable_fingerprint, first_semver, probe_model_runtime,
        validate_runtime_executable_path, ModelRuntimeAvailability, RuntimeDaemonStatus,
        RuntimeServerStatus,
    };
    use std::{
        error::Error,
        fs,
        net::TcpListener,
        path::{Path, PathBuf},
    };
    use tempfile::{tempdir, TempDir};

    #[test]
    fn rejects_relative_runtime_executable_path() {
        let error = validate_runtime_executable_path("bin/lms")
            .err()
            .map(|error| error.code);

        assert_eq!(error, Some("runtime.invalid"));
    }

    #[test]
    fn parses_documented_lms_semver() {
        let parsed = first_semver("lms is LM Studio's CLI utility. (v0.0.47)");

        assert_eq!(
            parsed,
            Some((
                "0.0.47".to_string(),
                super::Version {
                    major: 0,
                    minor: 0,
                    patch: 47
                }
            ))
        );
    }

    #[test]
    fn missing_runtime_executable_reports_missing_without_probe() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let missing = directory.path().join("missing-lms");
        let status = probe_model_runtime(&missing);

        assert_eq!(status.availability, ModelRuntimeAvailability::Missing);
        assert!(status.approved.is_none());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn supported_runtime_with_reachable_server_reports_running() -> Result<(), Box<dyn Error>> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let port = listener.local_addr()?.port();
        let fixture = executable_fixture(&format!(
            r#"case "$1" in
  --version) echo "lms v0.0.47";;
  daemon) echo '{{"status":"running","pid":12345,"isDaemon":true,"version":"0.4.4+1"}}';;
  server) echo '{{"running":true,"port":{port}}}';;
  *) exit 2;;
esac
"#
        ))?;

        let status = probe_model_runtime(&fixture.path);

        assert_eq!(status.availability, ModelRuntimeAvailability::Running);
        assert_eq!(status.cli_version, Some("0.0.47".to_string()));
        assert_eq!(status.daemon.status, RuntimeDaemonStatus::Running);
        assert_eq!(status.server.status, RuntimeServerStatus::Running);
        assert_eq!(status.server.port, Some(port));
        assert!(status.approved.is_some());
        drop(listener);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unsupported_cli_version_reports_unsupported() -> Result<(), Box<dyn Error>> {
        let fixture = executable_fixture(
            r#"case "$1" in
  --version) echo "lms v0.0.1";;
  *) exit 2;;
esac
"#,
        )?;

        let status = probe_model_runtime(&fixture.path);

        assert_eq!(status.availability, ModelRuntimeAvailability::Unsupported);
        assert_eq!(status.cli_version, Some("0.0.1".to_string()));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn malformed_daemon_json_reports_unknown() -> Result<(), Box<dyn Error>> {
        let fixture = executable_fixture(
            r#"case "$1" in
  --version) echo "lms v0.0.47";;
  daemon) echo 'not-json';;
  server) echo '{"running":false}';;
  *) exit 2;;
esac
"#,
        )?;

        let status = probe_model_runtime(&fixture.path);

        assert_eq!(status.availability, ModelRuntimeAvailability::Unknown);
        assert_eq!(status.daemon.status, RuntimeDaemonStatus::Unknown);
        assert_eq!(status.server.status, RuntimeServerStatus::Stopped);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn running_server_with_closed_port_reports_unreachable() -> Result<(), Box<dyn Error>> {
        let port = 9;
        let fixture = executable_fixture(&format!(
            r#"case "$1" in
  --version) echo "lms v0.0.47";;
  daemon) echo '{{"status":"running","pid":12345,"isDaemon":true}}';;
  server) echo '{{"running":true,"port":{port}}}';;
  *) exit 2;;
esac
"#
        ))?;

        let status = probe_model_runtime(&fixture.path);

        assert_eq!(status.availability, ModelRuntimeAvailability::Unreachable);
        assert_eq!(status.server.status, RuntimeServerStatus::Unreachable);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn timed_out_probe_reports_unknown() -> Result<(), Box<dyn Error>> {
        let fixture = executable_fixture(
            r#"case "$1" in
  --version) sleep 5;;
  *) exit 2;;
esac
"#,
        )?;

        let status = probe_model_runtime(&fixture.path);

        assert_eq!(status.availability, ModelRuntimeAvailability::Unknown);
        assert!(status.approved.is_none());
        Ok(())
    }

    #[test]
    fn fingerprint_changes_when_file_changes() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("runtime");
        fs::write(&path, b"one")?;
        let first = executable_fingerprint(&path)?.ok_or("missing first fingerprint")?;
        fs::write(&path, b"two two")?;
        let second = executable_fingerprint(&path)?.ok_or("missing second fingerprint")?;

        assert_ne!(first, second);
        Ok(())
    }

    #[cfg(unix)]
    struct ExecutableFixture {
        _directory: TempDir,
        path: PathBuf,
    }

    #[cfg(unix)]
    fn executable_fixture(script: &str) -> Result<ExecutableFixture, Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lms-fixture");
        fs::write(&path, format!("#!/bin/sh\n{script}"))?;
        make_executable(&path)?;
        Ok(ExecutableFixture {
            _directory: directory,
            path,
        })
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }
}
