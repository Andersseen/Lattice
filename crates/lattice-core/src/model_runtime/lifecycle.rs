use super::discovery::{
    probe_model_runtime, run_bounded_command, unix_timestamp_now, CommandProbeError,
    ModelRuntimeAvailability, RuntimeDaemonStatus, RuntimeProbeResult, RuntimeServerStatus,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub const START_MODEL_RUNTIME_COMMAND: &str = "start_model_runtime";
pub const STOP_MODEL_RUNTIME_COMMAND: &str = "stop_model_runtime";
pub const CANCEL_MODEL_RUNTIME_OPERATION_COMMAND: &str = "cancel_model_runtime_operation";

/// Total bound for an interactive start operation: daemon spawn/confirm
/// plus, if needed, server spawn/confirm. Set well above the CLI's own
/// observed ~60s wake-failure timeout (see design.md "Real CLI behavior").
pub const START_MODEL_RUNTIME_DEADLINE: Duration = Duration::from_secs(90);
/// An already-owned resource shutting down is not expected to be slow.
pub const STOP_MODEL_RUNTIME_DEADLINE: Duration = Duration::from_secs(10);
/// Best-effort stop attempted from the application-exit hook; short so it
/// never meaningfully delays shutdown.
pub const SHUTDOWN_STOP_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartModelRuntimeRequest {
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopModelRuntimeRequest {
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CancelModelRuntimeOperationRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum RuntimeOwnership {
    #[serde(rename = "owned")]
    Owned {
        daemon_pid: u32,
        executable_fingerprint: String,
        owned_since_unix_seconds: u64,
    },
    #[serde(rename = "attached")]
    Attached,
    #[serde(rename = "unknown")]
    #[default]
    Unknown,
}

impl RuntimeOwnership {
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            Self::Owned { .. } => "owned",
            Self::Attached => "attached",
            Self::Unknown => "unknown",
        }
    }

    pub fn owned_daemon_pid(&self) -> Option<u32> {
        match self {
            Self::Owned { daemon_pid, .. } => Some(*daemon_pid),
            _ => None,
        }
    }

    pub fn owned_executable_fingerprint(&self) -> Option<&str> {
        match self {
            Self::Owned {
                executable_fingerprint,
                ..
            } => Some(executable_fingerprint),
            _ => None,
        }
    }

    pub fn owned_since_unix_seconds(&self) -> Option<u64> {
        match self {
            Self::Owned {
                owned_since_unix_seconds,
                ..
            } => Some(*owned_since_unix_seconds),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeOperationOutcome {
    Started,
    AlreadyRunning,
    Stopped,
    AlreadyStopped,
    Refused,
    Cancelled,
    TimedOut,
    Failed,
}

/// Everything a lifecycle operation needs that is not itself part of the
/// pure computation: the approved executable, the ownership record as
/// currently persisted, and a shared flag the caller can set to cancel an
/// in-flight wait. Kept separate from `ModelRuntimeStatus` because start/stop
/// are operations on a resource, not a settings read/write.
pub struct RuntimeLifecycleInput<'a> {
    pub executable_path: &'a Path,
    pub persisted_ownership: &'a RuntimeOwnership,
    pub cancel: &'a AtomicBool,
    pub deadline: Duration,
}

pub struct RuntimeLifecycleResult {
    pub probe: RuntimeProbeResult,
    pub ownership: RuntimeOwnership,
    pub outcome: RuntimeOperationOutcome,
}

pub fn start_model_runtime(input: &RuntimeLifecycleInput<'_>) -> RuntimeLifecycleResult {
    let started_at = Instant::now();
    let before = probe_model_runtime(input.executable_path);

    if !matches!(
        before.availability,
        ModelRuntimeAvailability::Stopped
            | ModelRuntimeAvailability::Running
            | ModelRuntimeAvailability::Unreachable
    ) {
        return RuntimeLifecycleResult {
            ownership: input.persisted_ownership.clone(),
            outcome: RuntimeOperationOutcome::Failed,
            probe: before,
        };
    }

    if before.daemon.status == RuntimeDaemonStatus::Running {
        let ownership = if ownership_matches(&before, input.persisted_ownership) {
            input.persisted_ownership.clone()
        } else {
            RuntimeOwnership::Attached
        };

        if !matches!(ownership, RuntimeOwnership::Owned { .. })
            || before.server.status == RuntimeServerStatus::Running
        {
            return RuntimeLifecycleResult {
                ownership,
                outcome: RuntimeOperationOutcome::AlreadyRunning,
                probe: before,
            };
        }

        return start_server_and_finish(input, ownership, started_at);
    }

    let remaining = remaining_budget(started_at, input.deadline);
    if remaining.is_zero() {
        return RuntimeLifecycleResult {
            ownership: RuntimeOwnership::Unknown,
            outcome: RuntimeOperationOutcome::TimedOut,
            probe: before,
        };
    }

    match run_bounded_command(
        input.executable_path,
        &["daemon", "up", "--json"],
        remaining,
        Some(input.cancel),
    ) {
        Ok(_) => {}
        Err(CommandProbeError::Cancelled) => {
            return RuntimeLifecycleResult {
                ownership: RuntimeOwnership::Unknown,
                outcome: RuntimeOperationOutcome::Cancelled,
                probe: probe_model_runtime(input.executable_path),
            };
        }
        Err(CommandProbeError::Timeout) => {
            return RuntimeLifecycleResult {
                ownership: RuntimeOwnership::Unknown,
                outcome: RuntimeOperationOutcome::TimedOut,
                probe: probe_model_runtime(input.executable_path),
            };
        }
        Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            return RuntimeLifecycleResult {
                ownership: RuntimeOwnership::Unknown,
                outcome: RuntimeOperationOutcome::Failed,
                probe: probe_model_runtime(input.executable_path),
            };
        }
    }

    let after_daemon = probe_model_runtime(input.executable_path);
    if after_daemon.daemon.status != RuntimeDaemonStatus::Running {
        return RuntimeLifecycleResult {
            ownership: RuntimeOwnership::Unknown,
            outcome: RuntimeOperationOutcome::Failed,
            probe: after_daemon,
        };
    }

    let Some(daemon_pid) = after_daemon.daemon.pid else {
        return RuntimeLifecycleResult {
            ownership: RuntimeOwnership::Unknown,
            outcome: RuntimeOperationOutcome::Failed,
            probe: after_daemon,
        };
    };
    let Some(executable_fingerprint) = after_daemon
        .approved
        .as_ref()
        .map(|approval| approval.executable_fingerprint.clone())
    else {
        return RuntimeLifecycleResult {
            ownership: RuntimeOwnership::Unknown,
            outcome: RuntimeOperationOutcome::Failed,
            probe: after_daemon,
        };
    };

    let ownership = RuntimeOwnership::Owned {
        daemon_pid,
        executable_fingerprint,
        owned_since_unix_seconds: unix_timestamp_now(),
    };

    if after_daemon.server.status == RuntimeServerStatus::Running {
        return RuntimeLifecycleResult {
            ownership,
            outcome: RuntimeOperationOutcome::Started,
            probe: after_daemon,
        };
    }

    start_server_and_finish(input, ownership, started_at)
}

fn start_server_and_finish(
    input: &RuntimeLifecycleInput<'_>,
    ownership: RuntimeOwnership,
    started_at: Instant,
) -> RuntimeLifecycleResult {
    let remaining = remaining_budget(started_at, input.deadline);
    if remaining.is_zero() {
        return RuntimeLifecycleResult {
            ownership,
            outcome: RuntimeOperationOutcome::TimedOut,
            probe: probe_model_runtime(input.executable_path),
        };
    }

    let outcome = match run_bounded_command(
        input.executable_path,
        &["server", "start", "--bind", "127.0.0.1"],
        remaining,
        Some(input.cancel),
    ) {
        Ok(_) => None,
        Err(CommandProbeError::Cancelled) => Some(RuntimeOperationOutcome::Cancelled),
        Err(CommandProbeError::Timeout) => Some(RuntimeOperationOutcome::TimedOut),
        Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            Some(RuntimeOperationOutcome::Failed)
        }
    };

    if let Some(outcome) = outcome {
        return RuntimeLifecycleResult {
            ownership,
            outcome,
            probe: probe_model_runtime(input.executable_path),
        };
    }

    let after_server = probe_model_runtime(input.executable_path);
    let outcome = if after_server.server.status == RuntimeServerStatus::Running {
        RuntimeOperationOutcome::Started
    } else {
        RuntimeOperationOutcome::Failed
    };

    RuntimeLifecycleResult {
        ownership,
        outcome,
        probe: after_server,
    }
}

pub fn stop_model_runtime(input: &RuntimeLifecycleInput<'_>) -> RuntimeLifecycleResult {
    let started_at = Instant::now();
    let before = probe_model_runtime(input.executable_path);

    if before.daemon.status != RuntimeDaemonStatus::Running {
        return RuntimeLifecycleResult {
            ownership: RuntimeOwnership::Unknown,
            outcome: RuntimeOperationOutcome::AlreadyStopped,
            probe: before,
        };
    }

    if !ownership_matches(&before, input.persisted_ownership) {
        let ownership = if input.persisted_ownership.owned_daemon_pid().is_some() {
            RuntimeOwnership::Unknown
        } else {
            input.persisted_ownership.clone()
        };

        return RuntimeLifecycleResult {
            ownership,
            outcome: RuntimeOperationOutcome::Refused,
            probe: before,
        };
    }

    if before.server.status == RuntimeServerStatus::Running {
        let remaining = remaining_budget(started_at, input.deadline);
        if remaining.is_zero() {
            return RuntimeLifecycleResult {
                ownership: input.persisted_ownership.clone(),
                outcome: RuntimeOperationOutcome::TimedOut,
                probe: before,
            };
        }

        if let Some(outcome) = run_stop_command(
            input.executable_path,
            &["server", "stop"],
            remaining,
            input.cancel,
        ) {
            return RuntimeLifecycleResult {
                ownership: input.persisted_ownership.clone(),
                outcome,
                probe: probe_model_runtime(input.executable_path),
            };
        }
    }

    let remaining = remaining_budget(started_at, input.deadline);
    if remaining.is_zero() {
        return RuntimeLifecycleResult {
            ownership: input.persisted_ownership.clone(),
            outcome: RuntimeOperationOutcome::TimedOut,
            probe: probe_model_runtime(input.executable_path),
        };
    }

    if let Some(outcome) = run_stop_command(
        input.executable_path,
        &["daemon", "down"],
        remaining,
        input.cancel,
    ) {
        return RuntimeLifecycleResult {
            ownership: input.persisted_ownership.clone(),
            outcome,
            probe: probe_model_runtime(input.executable_path),
        };
    }

    let after = probe_model_runtime(input.executable_path);
    let outcome = if after.daemon.status == RuntimeDaemonStatus::Running {
        RuntimeOperationOutcome::Failed
    } else {
        RuntimeOperationOutcome::Stopped
    };
    let ownership = if outcome == RuntimeOperationOutcome::Stopped {
        RuntimeOwnership::Unknown
    } else {
        input.persisted_ownership.clone()
    };

    RuntimeLifecycleResult {
        ownership,
        outcome,
        probe: after,
    }
}

/// Runs one mutating stop-side command and translates a non-success result
/// into a terminal outcome. Returns `None` when the command completed and
/// the caller should proceed to the next step / final probe.
fn run_stop_command(
    executable_path: &Path,
    args: &[&str],
    deadline: Duration,
    cancel: &AtomicBool,
) -> Option<RuntimeOperationOutcome> {
    match run_bounded_command(executable_path, args, deadline, Some(cancel)) {
        Ok(_) => None,
        Err(CommandProbeError::Cancelled) => Some(RuntimeOperationOutcome::Cancelled),
        Err(CommandProbeError::Timeout) => Some(RuntimeOperationOutcome::TimedOut),
        Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            Some(RuntimeOperationOutcome::Failed)
        }
    }
}

fn ownership_matches(probe: &RuntimeProbeResult, persisted: &RuntimeOwnership) -> bool {
    let RuntimeOwnership::Owned {
        daemon_pid,
        executable_fingerprint,
        ..
    } = persisted
    else {
        return false;
    };

    let Some(current_pid) = probe.daemon.pid else {
        return false;
    };
    let Some(approval) = &probe.approved else {
        return false;
    };

    current_pid == *daemon_pid && approval.executable_fingerprint == *executable_fingerprint
}

fn remaining_budget(started_at: Instant, deadline: Duration) -> Duration {
    deadline.saturating_sub(started_at.elapsed())
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{
        start_model_runtime, stop_model_runtime, RuntimeLifecycleInput, RuntimeOperationOutcome,
        RuntimeOwnership,
    };
    use std::{
        error::Error,
        fs,
        net::TcpListener,
        path::{Path, PathBuf},
        sync::atomic::AtomicBool,
        time::Duration,
    };
    use tempfile::{tempdir, TempDir};

    #[test]
    fn starts_daemon_and_server_from_stopped() -> Result<(), Box<dyn Error>> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let port = listener.local_addr()?.port();
        let fixture = lifecycle_fixture(&format!(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1 $2" in
  "daemon up") touch "$state_dir/daemon"; echo '{{"status":"running","pid":4242,"isDaemon":true}}';;
  "daemon status") if [ -f "$state_dir/daemon" ]; then echo '{{"status":"running","pid":4242,"isDaemon":true}}'; else echo '{{"status":"not-running"}}'; fi;;
  "server start") touch "$state_dir/server"; echo "started";;
  "server status") if [ -f "$state_dir/server" ]; then echo '{{"running":true,"port":{port}}}'; else echo '{{"running":false}}'; fi;;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#
        ))?;

        let cancel = AtomicBool::new(false);
        let result = start_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &RuntimeOwnership::Unknown,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::Started);
        assert!(matches!(
            result.ownership,
            RuntimeOwnership::Owned {
                daemon_pid: 4242,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn already_running_daemon_reports_attached_without_spawning() -> Result<(), Box<dyn Error>> {
        let fixture = lifecycle_fixture(
            r#"case "$1 $2" in
  "daemon status") echo '{"status":"running","pid":7777,"isDaemon":true}';;
  "server status") echo '{"running":false}';;
  "daemon up") echo "UNEXPECTED SPAWN" >&2; exit 1;;
  "server start") echo "UNEXPECTED SPAWN" >&2; exit 1;;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = start_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &RuntimeOwnership::Unknown,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::AlreadyRunning);
        assert_eq!(result.ownership, RuntimeOwnership::Attached);
        Ok(())
    }

    #[test]
    fn stop_refuses_attached_daemon() -> Result<(), Box<dyn Error>> {
        let fixture = lifecycle_fixture(
            r#"case "$1 $2" in
  "daemon status") echo '{"status":"running","pid":9999,"isDaemon":true}';;
  "server status") echo '{"running":false}';;
  "daemon down") echo "UNEXPECTED STOP" >&2; exit 1;;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &RuntimeOwnership::Attached,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::Refused);
        assert_eq!(result.ownership, RuntimeOwnership::Attached);
        Ok(())
    }

    #[test]
    fn stop_refuses_when_pid_no_longer_matches_owned_record() -> Result<(), Box<dyn Error>> {
        let fixture = lifecycle_fixture(
            r#"case "$1 $2" in
  "daemon status") echo '{"status":"running","pid":5,"isDaemon":true}';;
  "server status") echo '{"running":false}';;
  "daemon down") echo "UNEXPECTED STOP" >&2; exit 1;;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let stale = RuntimeOwnership::Owned {
            daemon_pid: 999,
            executable_fingerprint: "stale".to_string(),
            owned_since_unix_seconds: 0,
        };
        let result = stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &stale,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::Refused);
        assert_eq!(result.ownership, RuntimeOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn stop_owned_daemon_succeeds() -> Result<(), Box<dyn Error>> {
        let fixture = lifecycle_fixture(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1 $2" in
  "daemon status") if [ -f "$state_dir/daemon" ]; then echo '{"status":"running","pid":321,"isDaemon":true}'; else echo '{"status":"not-running"}'; fi;;
  "server status") echo '{"running":false}';;
  "daemon down") rm -f "$state_dir/daemon"; echo "stopped";;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;
        fs::create_dir_all(
            fixture
                .path
                .parent()
                .ok_or("fixture has no parent")?
                .join("state"),
        )?;
        fs::write(
            fixture
                .path
                .parent()
                .ok_or("fixture has no parent")?
                .join("state")
                .join("daemon"),
            b"",
        )?;

        let cancel = AtomicBool::new(false);
        let owned = RuntimeOwnership::Owned {
            daemon_pid: 321,
            executable_fingerprint: probe_fingerprint(&fixture.path)?,
            owned_since_unix_seconds: 0,
        };
        let result = stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &owned,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::Stopped);
        assert_eq!(result.ownership, RuntimeOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn stop_already_stopped_daemon_is_idempotent() -> Result<(), Box<dyn Error>> {
        let fixture = lifecycle_fixture(
            r#"case "$1 $2" in
  "daemon status") echo '{"status":"not-running"}';;
  "server status") echo '{"running":false}';;
  *)
    case "$1" in
      --version) echo "lms v0.0.47";;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &fixture.path,
            persisted_ownership: &RuntimeOwnership::Unknown,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, RuntimeOperationOutcome::AlreadyStopped);
        Ok(())
    }

    fn probe_fingerprint(path: &Path) -> Result<String, Box<dyn Error>> {
        use super::super::discovery::executable_fingerprint;
        Ok(executable_fingerprint(path)?.ok_or("missing fingerprint")?)
    }

    struct LifecycleFixture {
        _directory: TempDir,
        path: PathBuf,
    }

    fn lifecycle_fixture(script: &str) -> Result<LifecycleFixture, Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lms-fixture");
        fs::write(&path, format!("#!/bin/sh\n{script}"))?;
        make_executable(&path)?;
        Ok(LifecycleFixture {
            _directory: directory,
            path,
        })
    }

    fn make_executable(path: &Path) -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }
}
