use super::discovery::{run_bounded_command, unix_timestamp_now, CommandProbeError};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub const GET_MODEL_SLOT_STATUS_COMMAND: &str = "get_model_slot_status";
pub const LOAD_MODEL_COMMAND: &str = "load_model";
pub const UNLOAD_MODEL_COMMAND: &str = "unload_model";
pub const CANCEL_MODEL_OPERATION_COMMAND: &str = "cancel_model_operation";

/// Provisional: wide enough to cover cold-start weight loading for a small
/// candidate model, not just process spawn. Pending confirmation against a
/// real cold-load measurement (see docs/verification/0.7-installed-model-management.md);
/// adjust here and in design.md together if real timing disagrees.
pub const LOAD_MODEL_DEADLINE: Duration = Duration::from_secs(120);
/// Releasing an already-loaded model is not expected to be slow.
pub const UNLOAD_MODEL_DEADLINE: Duration = Duration::from_secs(10);

/// Fixed identifier Lattice assigns to the one model it manages, so a
/// follow-up `ps --json` probe can unambiguously confirm a load Lattice
/// itself requested versus any other loaded model.
const LATTICE_MANAGED_MODEL_IDENTIFIER: &str = "lattice-managed";
const LIST_MODELS_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct GetModelSlotStatusRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadModelRequest {
    pub expected_revision: u64,
    pub model_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnloadModelRequest {
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CancelModelOperationRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDescriptor {
    pub model_key: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    pub is_llm: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedModelObservation {
    pub identifier: String,
    pub model_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ModelLoadOwnership {
    #[serde(rename = "owned")]
    Owned {
        identifier: String,
        model_key: String,
        loaded_since_unix_seconds: u64,
    },
    #[serde(rename = "attached")]
    Attached {
        identifier: String,
        model_key: String,
    },
    #[serde(rename = "unknown")]
    #[default]
    Unknown,
}

impl ModelLoadOwnership {
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            Self::Owned { .. } => "owned",
            Self::Attached { .. } => "attached",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelOperationOutcome {
    Loaded,
    AlreadyLoaded,
    Unloaded,
    AlreadyUnloaded,
    Refused,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSlotStatus {
    pub revision: u64,
    pub installed: Vec<ModelDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loaded: Option<LoadedModelObservation>,
    pub ownership: ModelLoadOwnership,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_operation: Option<ModelOperationOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked_unix_seconds: Option<u64>,
    pub message: String,
}

/// Everything a load/unload operation needs beyond the pure computation: the
/// approved executable, the ownership record as currently persisted, and a
/// shared cancellation flag independent of 0.6's runtime cancellation flag.
pub struct ModelOperationInput<'a> {
    pub executable_path: &'a Path,
    pub persisted_ownership: &'a ModelLoadOwnership,
    pub cancel: &'a AtomicBool,
    pub deadline: Duration,
}

pub struct ModelOperationResult {
    pub loaded: Option<LoadedModelObservation>,
    pub ownership: ModelLoadOwnership,
    pub outcome: ModelOperationOutcome,
}

/// Lists models present on disk. Fails closed to an empty list on any
/// command, timeout, or malformed-JSON error rather than panicking or
/// guessing: an empty result and a genuinely empty catalog are
/// indistinguishable at this layer, matching 0.5's `probe_daemon`/
/// `probe_server` fail-closed convention for equally undocumented JSON.
pub fn list_installed_models(executable_path: &Path) -> Vec<ModelDescriptor> {
    run_bounded_command(
        executable_path,
        &["ls", "--json"],
        LIST_MODELS_TIMEOUT,
        None,
    )
    .ok()
    .filter(|output| output.status.success())
    .and_then(|output| serde_json::from_str::<Vec<LmsModelEntry>>(&output.stdout).ok())
    .map(|entries| entries.into_iter().map(ModelDescriptor::from).collect())
    .unwrap_or_default()
}

/// Lists models currently loaded in the runtime. Always a fresh call; there
/// is no cached-only read, exactly like 0.5's `probe_model_runtime`.
pub fn list_loaded_models(executable_path: &Path) -> Vec<LoadedModelObservation> {
    run_bounded_command(
        executable_path,
        &["ps", "--json"],
        LIST_MODELS_TIMEOUT,
        None,
    )
    .ok()
    .filter(|output| output.status.success())
    .and_then(|output| serde_json::from_str::<Vec<LmsLoadedEntry>>(&output.stdout).ok())
    .map(|entries| {
        entries
            .into_iter()
            .map(LoadedModelObservation::from)
            .collect()
    })
    .unwrap_or_default()
}

/// Fresh loaded-slot observation reconciled against `persisted_ownership`:
/// a match restores `Owned`, any other loaded model is `Attached`, and no
/// loaded model at all is `Unknown` (nothing to own or attach).
pub fn observe_loaded_slot(
    executable_path: &Path,
    persisted_ownership: &ModelLoadOwnership,
) -> (Option<LoadedModelObservation>, ModelLoadOwnership) {
    let Some(first) = list_loaded_models(executable_path).into_iter().next() else {
        return (None, ModelLoadOwnership::Unknown);
    };

    let ownership = if ownership_matches(persisted_ownership, &first) {
        persisted_ownership.clone()
    } else {
        ModelLoadOwnership::Attached {
            identifier: first.identifier.clone(),
            model_key: first.model_key.clone(),
        }
    };

    (Some(first), ownership)
}

/// Loads `model_key` into the one Lattice-managed slot. Refuses without
/// spawning anything when the slot is already occupied (by this exact model,
/// reported `AlreadyLoaded`; by anything else, `Refused`). Success, failure
/// and identity are always taken from a follow-up `ps --json` probe, never
/// from `load`'s own exit code or stdout — neither is documented at all.
pub fn load_model(input: &ModelOperationInput<'_>, model_key: &str) -> ModelOperationResult {
    let started_at = Instant::now();
    let (loaded, ownership) = observe_loaded_slot(input.executable_path, input.persisted_ownership);

    if let Some(loaded) = loaded {
        let outcome = match &ownership {
            ModelLoadOwnership::Owned {
                model_key: owned_key,
                ..
            } if owned_key == model_key => ModelOperationOutcome::AlreadyLoaded,
            _ => ModelOperationOutcome::Refused,
        };
        return ModelOperationResult {
            loaded: Some(loaded),
            ownership,
            outcome,
        };
    }

    let remaining = remaining_budget(started_at, input.deadline);
    if remaining.is_zero() {
        return terminal_result(input, ModelOperationOutcome::TimedOut);
    }

    match run_bounded_command(
        input.executable_path,
        &[
            "load",
            model_key,
            "--identifier",
            LATTICE_MANAGED_MODEL_IDENTIFIER,
            "-y",
        ],
        remaining,
        Some(input.cancel),
    ) {
        Ok(_) => {}
        Err(CommandProbeError::Cancelled) => {
            return terminal_result(input, ModelOperationOutcome::Cancelled)
        }
        Err(CommandProbeError::Timeout) => {
            return terminal_result(input, ModelOperationOutcome::TimedOut)
        }
        Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            return terminal_result(input, ModelOperationOutcome::Failed)
        }
    }

    let (loaded, ownership) = observe_loaded_slot(input.executable_path, input.persisted_ownership);
    match &loaded {
        Some(observation)
            if observation.identifier == LATTICE_MANAGED_MODEL_IDENTIFIER
                && observation.model_key == model_key =>
        {
            ModelOperationResult {
                ownership: ModelLoadOwnership::Owned {
                    identifier: observation.identifier.clone(),
                    model_key: observation.model_key.clone(),
                    loaded_since_unix_seconds: unix_timestamp_now(),
                },
                loaded,
                outcome: ModelOperationOutcome::Loaded,
            }
        }
        _ => ModelOperationResult {
            loaded,
            ownership,
            outcome: ModelOperationOutcome::Failed,
        },
    }
}

/// Unloads the Lattice-owned model. Refuses without spawning anything when
/// nothing is loaded (`AlreadyUnloaded`) or the loaded model is not the
/// persisted `Owned` record (`Refused`, covering `Attached`/`Unknown` and a
/// stale/mismatched owned record alike) — the model-layer equivalent of
/// "no kill-by-name or attached-resource stop."
pub fn unload_model(input: &ModelOperationInput<'_>) -> ModelOperationResult {
    let started_at = Instant::now();
    let (loaded, ownership) = observe_loaded_slot(input.executable_path, input.persisted_ownership);

    let Some(loaded) = loaded else {
        return ModelOperationResult {
            loaded: None,
            ownership: ModelLoadOwnership::Unknown,
            outcome: ModelOperationOutcome::AlreadyUnloaded,
        };
    };

    let ModelLoadOwnership::Owned { identifier, .. } = &ownership else {
        return ModelOperationResult {
            loaded: Some(loaded),
            ownership,
            outcome: ModelOperationOutcome::Refused,
        };
    };
    let identifier = identifier.clone();

    let remaining = remaining_budget(started_at, input.deadline);
    if remaining.is_zero() {
        return terminal_result(input, ModelOperationOutcome::TimedOut);
    }

    match run_bounded_command(
        input.executable_path,
        &["unload", &identifier],
        remaining,
        Some(input.cancel),
    ) {
        Ok(_) => {}
        Err(CommandProbeError::Cancelled) => {
            return terminal_result(input, ModelOperationOutcome::Cancelled)
        }
        Err(CommandProbeError::Timeout) => {
            return terminal_result(input, ModelOperationOutcome::TimedOut)
        }
        Err(CommandProbeError::Spawn) | Err(CommandProbeError::Io) => {
            return terminal_result(input, ModelOperationOutcome::Failed)
        }
    }

    let (loaded, ownership) = observe_loaded_slot(input.executable_path, input.persisted_ownership);
    let outcome = if loaded.is_none() {
        ModelOperationOutcome::Unloaded
    } else {
        ModelOperationOutcome::Failed
    };

    ModelOperationResult {
        loaded,
        ownership,
        outcome,
    }
}

/// Re-probes and reports a fixed outcome. Never issues a corrective load or
/// unload for an operation that may have partially succeeded server-side:
/// the probe is authoritative, and a model it shows loaded anyway is
/// recorded `Attached`, not `Owned`, since the caller who asked for this
/// operation already received a terminal `Cancelled`/`TimedOut`/`Failed`
/// outcome and does not get to claim ownership retroactively.
fn terminal_result(
    input: &ModelOperationInput<'_>,
    outcome: ModelOperationOutcome,
) -> ModelOperationResult {
    let (loaded, ownership) = observe_loaded_slot(input.executable_path, input.persisted_ownership);
    ModelOperationResult {
        loaded,
        ownership,
        outcome,
    }
}

fn ownership_matches(persisted: &ModelLoadOwnership, observation: &LoadedModelObservation) -> bool {
    matches!(
        persisted,
        ModelLoadOwnership::Owned { identifier, model_key, .. }
            if *identifier == observation.identifier && *model_key == observation.model_key
    )
}

fn remaining_budget(started_at: Instant, deadline: Duration) -> Duration {
    deadline.saturating_sub(started_at.elapsed())
}

/// Raw shape read from `lms ls --json`. Undocumented beyond the CLI's
/// human-readable example columns (name, parameters, architecture, size);
/// aliases hedge the most likely alternate field names until a real sample
/// pins the exact shape (see design.md "Real CLI behavior" and tasks.md).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LmsModelEntry {
    #[serde(alias = "key", alias = "path")]
    model_key: String,
    #[serde(default, alias = "name")]
    display_name: Option<String>,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, alias = "size")]
    size_bytes: Option<u64>,
}

impl From<LmsModelEntry> for ModelDescriptor {
    fn from(entry: LmsModelEntry) -> Self {
        let is_llm = entry.kind.as_deref() != Some("embedding");
        Self {
            display_name: entry
                .display_name
                .unwrap_or_else(|| entry.model_key.clone()),
            model_key: entry.model_key,
            architecture: entry.architecture,
            is_llm,
            size_bytes: entry.size_bytes,
        }
    }
}

/// Raw shape read from `lms ps --json`. Same undocumented-schema caveat as
/// `LmsModelEntry` above; `identifier` is the one field the public docs
/// name explicitly for this command, so it carries no alias.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LmsLoadedEntry {
    identifier: String,
    #[serde(alias = "key", alias = "path")]
    model_key: String,
    #[serde(default)]
    architecture: Option<String>,
    #[serde(default, alias = "size")]
    size_bytes: Option<u64>,
}

impl From<LmsLoadedEntry> for LoadedModelObservation {
    fn from(entry: LmsLoadedEntry) -> Self {
        Self {
            identifier: entry.identifier,
            model_key: entry.model_key,
            architecture: entry.architecture,
            size_bytes: entry.size_bytes,
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{
        list_installed_models, list_loaded_models, load_model, observe_loaded_slot, unload_model,
        ModelLoadOwnership, ModelOperationInput, ModelOperationOutcome,
    };
    use std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
        sync::atomic::AtomicBool,
        time::Duration,
    };
    use tempfile::{tempdir, TempDir};

    #[test]
    fn lists_installed_models_from_ls_json() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ls --json") echo '[{"modelKey":"qwen/qwen2.5-0.5b-instruct","displayName":"Qwen2.5 0.5B Instruct","architecture":"qwen2","type":"llm","sizeBytes":400000000}]';;
  *) exit 2;;
esac
"#,
        )?;

        let installed = list_installed_models(&fixture.path);

        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].model_key, "qwen/qwen2.5-0.5b-instruct");
        assert_eq!(installed[0].display_name, "Qwen2.5 0.5B Instruct");
        assert!(installed[0].is_llm);
        Ok(())
    }

    #[test]
    fn malformed_ls_json_reports_empty_inventory_without_panicking() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ls --json") echo 'not-json';;
  *) exit 2;;
esac
"#,
        )?;

        assert_eq!(list_installed_models(&fixture.path), Vec::new());
        Ok(())
    }

    #[test]
    fn malformed_ps_json_reports_empty_loaded_list_without_panicking() -> Result<(), Box<dyn Error>>
    {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ps --json") echo 'not-json';;
  *) exit 2;;
esac
"#,
        )?;

        assert_eq!(list_loaded_models(&fixture.path), Vec::new());
        Ok(())
    }

    #[test]
    fn loads_model_into_empty_slot_and_confirms_via_ps_probe() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1" in
  load) touch "$state_dir/loaded"; echo "loaded";;
  ps)
    if [ -f "$state_dir/loaded" ]; then
      echo '[{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct","architecture":"qwen2","sizeBytes":400000000}]'
    else
      echo '[]'
    fi
    ;;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &ModelLoadOwnership::Unknown,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::Loaded);
        assert!(matches!(
            result.ownership,
            ModelLoadOwnership::Owned { ref identifier, ref model_key, .. }
                if identifier == "lattice-managed" && model_key == "qwen/qwen2.5-0.5b-instruct"
        ));
        Ok(())
    }

    #[test]
    fn loading_a_different_model_is_refused_while_slot_is_occupied() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ps --json") echo '[{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct"}]';;
  "load "*) echo "UNEXPECTED SPAWN" >&2; exit 1;;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let owned = ModelLoadOwnership::Owned {
            identifier: "lattice-managed".to_string(),
            model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
            loaded_since_unix_seconds: 0,
        };
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &owned,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "google/gemma-2-2b-it",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::Refused);
        Ok(())
    }

    #[test]
    fn loading_the_same_already_owned_model_is_idempotent() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ps --json") echo '[{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct"}]';;
  "load "*) echo "UNEXPECTED SPAWN" >&2; exit 1;;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let owned = ModelLoadOwnership::Owned {
            identifier: "lattice-managed".to_string(),
            model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
            loaded_since_unix_seconds: 0,
        };
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &owned,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::AlreadyLoaded);
        Ok(())
    }

    #[test]
    fn externally_loaded_model_is_recorded_attached_and_load_is_refused(
    ) -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ps --json") echo '[{"identifier":"someone-elses-session","modelKey":"other/model"}]';;
  "load "*) echo "UNEXPECTED SPAWN" >&2; exit 1;;
  *) exit 2;;
esac
"#,
        )?;

        let (loaded, ownership) = observe_loaded_slot(&fixture.path, &ModelLoadOwnership::Unknown);
        assert!(loaded.is_some());
        assert!(matches!(
            ownership,
            ModelLoadOwnership::Attached { ref identifier, .. } if identifier == "someone-elses-session"
        ));

        let cancel = AtomicBool::new(false);
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &ModelLoadOwnership::Unknown,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );
        assert_eq!(result.outcome, ModelOperationOutcome::Refused);
        Ok(())
    }

    #[test]
    fn insufficient_memory_load_failure_leaves_no_owned_record() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1 $2" in
  "ps --json") echo '[]';;
  *)
    case "$1" in
      load) echo "Error: insufficient memory" >&2; exit 1;;
      *) exit 2;;
    esac
    ;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &ModelLoadOwnership::Unknown,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::Failed);
        assert_eq!(result.ownership, ModelLoadOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn unload_releases_owned_model_state() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1" in
  unload)
    rm -f "$state_dir/loaded"; echo "unloaded";;
  ps)
    if [ -f "$state_dir/loaded" ]; then
      echo '[{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct"}]'
    else
      echo '[]'
    fi
    ;;
  *) exit 2;;
esac
"#,
        )?;
        let state_dir = fixture
            .path
            .parent()
            .ok_or("fixture has no parent")?
            .join("state");
        fs::create_dir_all(&state_dir)?;
        fs::write(state_dir.join("loaded"), b"")?;

        let cancel = AtomicBool::new(false);
        let owned = ModelLoadOwnership::Owned {
            identifier: "lattice-managed".to_string(),
            model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
            loaded_since_unix_seconds: 0,
        };
        let result = unload_model(&ModelOperationInput {
            executable_path: &fixture.path,
            persisted_ownership: &owned,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, ModelOperationOutcome::Unloaded);
        assert_eq!(result.ownership, ModelLoadOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn unload_refuses_attached_model() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1" in
  ps) echo '[{"identifier":"someone-elses-session","modelKey":"other/model"}]';;
  unload) echo "UNEXPECTED STOP" >&2; exit 1;;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = unload_model(&ModelOperationInput {
            executable_path: &fixture.path,
            persisted_ownership: &ModelLoadOwnership::Unknown,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, ModelOperationOutcome::Refused);
        Ok(())
    }

    #[test]
    fn unload_on_empty_slot_is_idempotent() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1" in
  ps) echo '[]';;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = unload_model(&ModelOperationInput {
            executable_path: &fixture.path,
            persisted_ownership: &ModelLoadOwnership::Unknown,
            cancel: &cancel,
            deadline: Duration::from_secs(5),
        });

        assert_eq!(result.outcome, ModelOperationOutcome::AlreadyUnloaded);
        Ok(())
    }

    #[test]
    fn cancelling_an_in_flight_load_never_claims_ownership() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1" in
  ps) echo '[]';;
  load) sleep 5; echo "loaded";;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(true);
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &ModelLoadOwnership::Unknown,
                cancel: &cancel,
                deadline: Duration::from_secs(5),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::Cancelled);
        assert_eq!(result.ownership, ModelLoadOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn load_times_out_without_assuming_success() -> Result<(), Box<dyn Error>> {
        let fixture = models_fixture(
            r#"case "$1" in
  ps) echo '[]';;
  load) sleep 5; echo "loaded";;
  *) exit 2;;
esac
"#,
        )?;

        let cancel = AtomicBool::new(false);
        let result = load_model(
            &ModelOperationInput {
                executable_path: &fixture.path,
                persisted_ownership: &ModelLoadOwnership::Unknown,
                cancel: &cancel,
                deadline: Duration::from_millis(200),
            },
            "qwen/qwen2.5-0.5b-instruct",
        );

        assert_eq!(result.outcome, ModelOperationOutcome::TimedOut);
        assert_eq!(result.ownership, ModelLoadOwnership::Unknown);
        Ok(())
    }

    struct ModelsFixture {
        _directory: TempDir,
        path: PathBuf,
    }

    fn models_fixture(script: &str) -> Result<ModelsFixture, Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lms-fixture");
        fs::write(&path, format!("#!/bin/sh\n{script}"))?;
        make_executable(&path)?;
        Ok(ModelsFixture {
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
