use crate::{
    model_runtime::{
        availability_from_storage, daemon_status_from_storage, list_installed_models,
        load_model as run_load_model, observe_loaded_slot, probe_model_runtime,
        server_status_from_storage, start_model_runtime as run_start_model_runtime,
        stop_model_runtime as run_stop_model_runtime, unload_model as run_unload_model,
        validate_runtime_executable_path, GetModelSlotStatusRequest, LoadModelRequest,
        ModelLoadOwnership, ModelOperationInput, ModelOperationOutcome, ModelOperationResult,
        ModelRuntimeAvailability, ModelRuntimeStatus, ModelSlotStatus, RuntimeDaemonObservation,
        RuntimeLifecycleInput, RuntimeOwnership, RuntimeProbeApproval, RuntimeServerObservation,
        StartModelRuntimeRequest, StopModelRuntimeRequest, UnloadModelRequest,
        DEFAULT_RUNTIME_MESSAGE, LOAD_MODEL_DEADLINE, SHUTDOWN_STOP_DEADLINE,
        START_MODEL_RUNTIME_DEADLINE, STOP_MODEL_RUNTIME_DEADLINE, UNLOAD_MODEL_DEADLINE,
    },
    AppError,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

pub const GET_APP_SETTINGS_COMMAND: &str = "get_app_settings";
pub const UPDATE_APP_SETTINGS_COMMAND: &str = "update_app_settings";
pub const RESET_APP_SETTINGS_COMMAND: &str = "reset_app_settings";

const CURRENT_SCHEMA_VERSION: u32 = 4;
const SETTINGS_ROW_ID: i64 = 1;
const RUNTIME_ROW_ID: i64 = 1;
const MODEL_LOAD_ROW_ID: i64 = 1;
const DEFAULT_IDLE_UNLOAD_MINUTES: u16 = 5;
const MIN_IDLE_UNLOAD_MINUTES: i64 = 1;
const MAX_IDLE_UNLOAD_MINUTES: i64 = 120;
const MODEL_RUNTIME_NOT_RUNNING_MESSAGE: &str = "Start the runtime before managing models.";
const MODEL_SLOT_READY_MESSAGE: &str = "Model inventory refreshed.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppearancePreference {
    System,
    Light,
    Dark,
}

impl AppearancePreference {
    fn as_storage_value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub revision: u64,
    pub appearance: AppearancePreference,
    pub idle_unload_minutes: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            revision: 1,
            appearance: AppearancePreference::System,
            idle_unload_minutes: DEFAULT_IDLE_UNLOAD_MINUTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppSettingsRequest {
    pub expected_revision: u64,
    #[serde(default)]
    pub appearance: Option<String>,
    #[serde(default)]
    pub idle_unload_minutes: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetAppSettingsRequest {
    pub expected_revision: u64,
}

pub struct SettingsStore {
    conn: Connection,
}

impl SettingsStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                AppError::storage_unavailable("Lattice could not prepare local settings storage.")
            })?;
        }

        let mut conn = Connection::open(path).map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local settings storage.")
        })?;
        migrate(&mut conn, Some(path))?;

        Ok(Self { conn })
    }

    pub fn read(&self) -> Result<AppSettings, AppError> {
        read_settings(&self.conn)
    }

    pub fn update(&mut self, request: UpdateAppSettingsRequest) -> Result<AppSettings, AppError> {
        let next_values = validate_update(&request)?;
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update local settings.")
        })?;
        let current = read_settings(&tx)?;

        if current.revision != request.expected_revision {
            return Err(AppError::settings_conflict(
                "Settings changed before this update could be saved.",
            ));
        }

        let next = AppSettings {
            schema_version: CURRENT_SCHEMA_VERSION,
            revision: current.revision + 1,
            appearance: next_values.appearance.unwrap_or(current.appearance),
            idle_unload_minutes: next_values
                .idle_unload_minutes
                .unwrap_or(current.idle_unload_minutes),
        };

        write_settings(&tx, &next)?;
        tx.commit()
            .map_err(|_| AppError::storage_unavailable("Lattice could not save local settings."))?;
        Ok(next)
    }

    pub fn reset(&mut self, request: ResetAppSettingsRequest) -> Result<AppSettings, AppError> {
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not reset local settings.")
        })?;
        let current = read_settings(&tx)?;

        if current.revision != request.expected_revision {
            return Err(AppError::settings_conflict(
                "Settings changed before reset could be saved.",
            ));
        }

        let next = AppSettings {
            revision: current.revision + 1,
            ..AppSettings::default()
        };

        write_settings(&tx, &next)?;
        tx.commit()
            .map_err(|_| AppError::storage_unavailable("Lattice could not save reset settings."))?;
        Ok(next)
    }

    pub fn read_model_runtime_status(&self) -> Result<ModelRuntimeStatus, AppError> {
        Ok(read_model_runtime_status(&self.conn)?.with_current_file_state())
    }

    pub fn configure_model_runtime(
        &mut self,
        request: crate::model_runtime::ConfigureModelRuntimeRequest,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let executable_path = validate_runtime_executable_path(&request.executable_path)?;
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update model runtime settings.")
        })?;
        let current = read_model_runtime_status(&tx)?;

        if current.revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Runtime settings changed before this update could be saved.",
            ));
        }

        let next =
            ModelRuntimeStatus::configured(current.revision + 1, path_to_string(&executable_path)?);
        write_model_runtime_status(&tx, &next)?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime settings.")
        })?;
        Ok(next.with_current_file_state())
    }

    pub fn probe_model_runtime(
        &mut self,
        request: crate::model_runtime::ProbeModelRuntimeRequest,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let current = read_model_runtime_status(&self.conn)?;
        if current.revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Runtime settings changed before the probe could run.",
            ));
        }

        let Some(executable_path) = current.executable_path.as_deref() else {
            return Err(AppError::invalid_runtime(
                "Choose a runtime executable before probing.",
            ));
        };
        let executable_path = validate_runtime_executable_path(executable_path)?;
        let probe = probe_model_runtime(&executable_path);
        let next = ModelRuntimeStatus {
            revision: current.revision + 1,
            executable_path: current.executable_path,
            availability: probe.availability,
            cli_version: probe.cli_version,
            approved: probe.approved,
            daemon: probe.daemon,
            server: probe.server,
            ownership: current.ownership,
            last_operation: None,
            last_checked_unix_seconds: Some(probe.last_checked_unix_seconds),
            message: probe.message,
        };
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update model runtime status.")
        })?;
        write_model_runtime_status(&tx, &next)?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;
        Ok(next.with_current_file_state())
    }

    pub fn start_model_runtime(
        &mut self,
        request: StartModelRuntimeRequest,
        cancel: &AtomicBool,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let current = read_model_runtime_status(&self.conn)?;
        if current.revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Runtime settings changed before this operation could run.",
            ));
        }

        let Some(executable_path) = current.executable_path.as_deref() else {
            return Err(AppError::invalid_runtime(
                "Choose a runtime executable before starting it.",
            ));
        };
        let executable_path = validate_runtime_executable_path(executable_path)?;

        let result = run_start_model_runtime(&RuntimeLifecycleInput {
            executable_path: &executable_path,
            persisted_ownership: &current.ownership,
            cancel,
            deadline: START_MODEL_RUNTIME_DEADLINE,
        });

        let next = ModelRuntimeStatus {
            revision: current.revision + 1,
            executable_path: current.executable_path,
            availability: result.probe.availability,
            cli_version: result.probe.cli_version,
            approved: result.probe.approved,
            daemon: result.probe.daemon,
            server: result.probe.server,
            ownership: result.ownership,
            last_operation: Some(result.outcome),
            last_checked_unix_seconds: Some(result.probe.last_checked_unix_seconds),
            message: result.probe.message,
        };
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update model runtime status.")
        })?;
        write_model_runtime_status(&tx, &next)?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;
        Ok(next.with_current_file_state())
    }

    pub fn stop_model_runtime(
        &mut self,
        request: StopModelRuntimeRequest,
        cancel: &AtomicBool,
    ) -> Result<ModelRuntimeStatus, AppError> {
        let current = read_model_runtime_status(&self.conn)?;
        if current.revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Runtime settings changed before this operation could run.",
            ));
        }

        let Some(executable_path) = current.executable_path.as_deref() else {
            return Err(AppError::invalid_runtime(
                "Choose a runtime executable before stopping it.",
            ));
        };
        let executable_path = validate_runtime_executable_path(executable_path)?;

        let result = run_stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &executable_path,
            persisted_ownership: &current.ownership,
            cancel,
            deadline: STOP_MODEL_RUNTIME_DEADLINE,
        });

        let next = ModelRuntimeStatus {
            revision: current.revision + 1,
            executable_path: current.executable_path,
            availability: result.probe.availability,
            cli_version: result.probe.cli_version,
            approved: result.probe.approved,
            daemon: result.probe.daemon,
            server: result.probe.server,
            ownership: result.ownership,
            last_operation: Some(result.outcome),
            last_checked_unix_seconds: Some(result.probe.last_checked_unix_seconds),
            message: result.probe.message,
        };
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update model runtime status.")
        })?;
        write_model_runtime_status(&tx, &next)?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;
        Ok(next.with_current_file_state())
    }

    pub fn get_model_slot_status(
        &self,
        _request: GetModelSlotStatusRequest,
    ) -> Result<ModelSlotStatus, AppError> {
        let (revision, ownership) = read_model_load_state(&self.conn)?;
        let runtime = self.read_model_runtime_status()?;
        Ok(compose_model_slot_status(
            revision, &ownership, &runtime, None,
        ))
    }

    pub fn load_model(
        &mut self,
        request: LoadModelRequest,
        cancel: &AtomicBool,
    ) -> Result<ModelSlotStatus, AppError> {
        let (current_revision, persisted_ownership) = read_model_load_state(&self.conn)?;
        if current_revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Model settings changed before this operation could run.",
            ));
        }

        let executable_path = self.running_runtime_executable_path()?;
        let result = run_load_model(
            &ModelOperationInput {
                executable_path: &executable_path,
                persisted_ownership: &persisted_ownership,
                cancel,
                deadline: LOAD_MODEL_DEADLINE,
            },
            &request.model_key,
        );

        self.persist_model_operation_result(current_revision + 1, &executable_path, result)
    }

    pub fn unload_model(
        &mut self,
        request: UnloadModelRequest,
        cancel: &AtomicBool,
    ) -> Result<ModelSlotStatus, AppError> {
        let (current_revision, persisted_ownership) = read_model_load_state(&self.conn)?;
        if current_revision != request.expected_revision {
            return Err(AppError::runtime_conflict(
                "Model settings changed before this operation could run.",
            ));
        }

        let executable_path = self.running_runtime_executable_path()?;
        let result = run_unload_model(&ModelOperationInput {
            executable_path: &executable_path,
            persisted_ownership: &persisted_ownership,
            cancel,
            deadline: UNLOAD_MODEL_DEADLINE,
        });

        self.persist_model_operation_result(current_revision + 1, &executable_path, result)
    }

    /// Shared precondition for `load_model`/`unload_model`: a fast,
    /// non-mutating read of the current runtime status. No `lms` subprocess
    /// is spawned for model management while the runtime is not `running`.
    fn running_runtime_executable_path(&self) -> Result<PathBuf, AppError> {
        let runtime = self.read_model_runtime_status()?;
        if runtime.availability != ModelRuntimeAvailability::Running {
            return Err(AppError::invalid_runtime(MODEL_RUNTIME_NOT_RUNNING_MESSAGE));
        }
        let Some(executable_path) = runtime.executable_path.as_deref() else {
            return Err(AppError::invalid_runtime(MODEL_RUNTIME_NOT_RUNNING_MESSAGE));
        };
        validate_runtime_executable_path(executable_path)
    }

    fn persist_model_operation_result(
        &mut self,
        next_revision: u64,
        executable_path: &Path,
        result: ModelOperationResult,
    ) -> Result<ModelSlotStatus, AppError> {
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not update model load status.")
        })?;
        write_model_load_state(&tx, next_revision, &result.ownership)?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model load status.")
        })?;

        let installed = list_installed_models(executable_path);
        Ok(ModelSlotStatus {
            revision: next_revision,
            installed,
            loaded: result.loaded,
            ownership: result.ownership,
            last_operation: Some(result.outcome),
            last_checked_unix_seconds: Some(now_unix_seconds()),
            message: MODEL_SLOT_READY_MESSAGE.to_string(),
        })
    }

    /// Best-effort stop of a currently-owned runtime, attempted from the
    /// application-exit hook. Never returns an error: an attached/unknown
    /// resource, missing executable, or storage failure all simply mean
    /// nothing is stopped, since shutdown must not be blocked by this.
    pub fn stop_owned_model_runtime_for_shutdown(&mut self) {
        let Ok(current) = read_model_runtime_status(&self.conn) else {
            return;
        };
        if !matches!(current.ownership, RuntimeOwnership::Owned { .. }) {
            return;
        }
        let Some(executable_path) = current.executable_path.as_deref() else {
            return;
        };
        let Ok(executable_path) = validate_runtime_executable_path(executable_path) else {
            return;
        };

        let cancel = AtomicBool::new(false);
        let result = run_stop_model_runtime(&RuntimeLifecycleInput {
            executable_path: &executable_path,
            persisted_ownership: &current.ownership,
            cancel: &cancel,
            deadline: SHUTDOWN_STOP_DEADLINE,
        });

        let next = ModelRuntimeStatus {
            revision: current.revision + 1,
            executable_path: current.executable_path,
            availability: result.probe.availability,
            cli_version: result.probe.cli_version,
            approved: result.probe.approved,
            daemon: result.probe.daemon,
            server: result.probe.server,
            ownership: result.ownership,
            last_operation: Some(result.outcome),
            last_checked_unix_seconds: Some(result.probe.last_checked_unix_seconds),
            message: result.probe.message,
        };
        if let Ok(tx) = self.conn.transaction() {
            let _ = write_model_runtime_status(&tx, &next);
            let _ = tx.commit();
        }
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, AppError> {
        let mut conn = Connection::open_in_memory().map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local settings storage.")
        })?;
        migrate(&mut conn, None)?;
        Ok(Self { conn })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidSettingsPatch {
    appearance: Option<AppearancePreference>,
    idle_unload_minutes: Option<u16>,
}

fn migrate(conn: &mut Connection, db_path: Option<&Path>) -> Result<(), AppError> {
    let schema_version = schema_version(conn)?;

    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(AppError::unsupported_schema(
            "Local settings were created by a newer Lattice version.",
        ));
    }

    if schema_version == CURRENT_SCHEMA_VERSION {
        read_settings(conn)?;
        read_model_runtime_status(conn)?;
        read_model_load_state(conn)?;
        return Ok(());
    }

    create_pre_migration_backup(db_path)?;
    let tx = conn
        .transaction()
        .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    match schema_version {
        0 => {
            create_v1_schema(&tx)?;
            create_v2_schema(&tx)?;
            create_v3_schema(&tx)?;
            create_v4_schema(&tx)?;
            write_model_runtime_status(&tx, &ModelRuntimeStatus::default())?;
        }
        1 => {
            create_v2_schema(&tx)?;
            create_v3_schema(&tx)?;
            create_v4_schema(&tx)?;
            write_model_runtime_status(&tx, &ModelRuntimeStatus::default())?;
        }
        2 => {
            create_v3_schema(&tx)?;
            create_v4_schema(&tx)?;
        }
        3 => create_v4_schema(&tx)?,
        _ => {
            return Err(AppError::unsupported_schema(
                "Local settings schema is not supported by this Lattice version.",
            ));
        }
    }

    tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
        .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;
    tx.commit()
        .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;
    Ok(())
}

fn create_v1_schema(tx: &Transaction<'_>) -> Result<(), AppError> {
    tx.execute(
        "CREATE TABLE app_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            revision INTEGER NOT NULL CHECK (revision >= 1),
            appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
            idle_unload_minutes INTEGER NOT NULL CHECK (
                idle_unload_minutes >= 1 AND idle_unload_minutes <= 120
            )
        )",
        [],
    )
    .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    write_settings(tx, &AppSettings::default())?;
    Ok(())
}

fn create_v2_schema(tx: &Transaction<'_>) -> Result<(), AppError> {
    tx.execute(
        "CREATE TABLE model_runtime_discovery (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            revision INTEGER NOT NULL CHECK (revision >= 1),
            executable_path TEXT,
            availability TEXT NOT NULL CHECK (
                availability IN ('missing', 'unsupported', 'stopped', 'running', 'unreachable', 'unknown')
            ),
            cli_version TEXT,
            approved_executable_fingerprint TEXT,
            approved_cli_version TEXT,
            approved_checked_at_unix_seconds INTEGER,
            daemon_status TEXT NOT NULL CHECK (daemon_status IN ('running', 'notRunning', 'unknown')),
            daemon_pid INTEGER,
            daemon_is_daemon INTEGER,
            daemon_version TEXT,
            server_status TEXT NOT NULL CHECK (server_status IN ('running', 'stopped', 'unreachable', 'unknown')),
            server_port INTEGER,
            server_endpoint TEXT,
            last_checked_unix_seconds INTEGER,
            message TEXT NOT NULL
        )",
        [],
    )
    .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    Ok(())
}

fn create_v3_schema(tx: &Transaction<'_>) -> Result<(), AppError> {
    tx.execute_batch(
        "ALTER TABLE model_runtime_discovery ADD COLUMN ownership_state TEXT NOT NULL
            DEFAULT 'unknown' CHECK (ownership_state IN ('owned', 'attached', 'unknown'));
         ALTER TABLE model_runtime_discovery ADD COLUMN owned_daemon_pid INTEGER;
         ALTER TABLE model_runtime_discovery ADD COLUMN owned_since_unix_seconds INTEGER;",
    )
    .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    Ok(())
}

fn create_v4_schema(tx: &Transaction<'_>) -> Result<(), AppError> {
    tx.execute(
        "CREATE TABLE model_load_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            revision INTEGER NOT NULL CHECK (revision >= 1),
            ownership_state TEXT NOT NULL CHECK (ownership_state IN ('owned', 'attached', 'unknown')),
            owned_identifier TEXT,
            owned_model_key TEXT,
            owned_since_unix_seconds INTEGER
        )",
        [],
    )
    .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    tx.execute(
        "INSERT INTO model_load_state (id, revision, ownership_state) VALUES (?1, 1, 'unknown')",
        params![MODEL_LOAD_ROW_ID],
    )
    .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    Ok(())
}

fn create_pre_migration_backup(db_path: Option<&Path>) -> Result<(), AppError> {
    let Some(path) = db_path else {
        return Ok(());
    };

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(()),
    };

    if !metadata.is_file() || metadata.len() == 0 {
        return Ok(());
    }

    let backup_path = backup_path_for(path)?;
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            AppError::migration_failed("Lattice could not back up local settings before migration.")
        })?;
    }

    fs::copy(path, backup_path).map_err(|_| {
        AppError::migration_failed("Lattice could not back up local settings before migration.")
    })?;
    Ok(())
}

fn backup_path_for(path: &Path) -> Result<PathBuf, AppError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("lattice.sqlite3");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    Ok(parent
        .join("backups")
        .join(format!("{file_name}.backup.{timestamp}.sqlite3")))
}

fn schema_version(conn: &Connection) -> Result<u32, AppError> {
    let raw = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))?;

    u32::try_from(raw)
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))
}

fn read_settings(conn: &Connection) -> Result<AppSettings, AppError> {
    let stored = conn
        .query_row(
            "SELECT revision, appearance, idle_unload_minutes
             FROM app_settings
             WHERE id = ?1",
            params![SETTINGS_ROW_ID],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))?;

    let Some((revision, appearance, idle_unload_minutes)) = stored else {
        return Err(AppError::storage_unavailable(
            "Lattice could not read local settings.",
        ));
    };

    Ok(AppSettings {
        schema_version: CURRENT_SCHEMA_VERSION,
        revision: validate_revision(revision)?,
        appearance: validate_appearance(&appearance)?,
        idle_unload_minutes: validate_idle_unload_minutes(idle_unload_minutes)?,
    })
}

fn write_settings(tx: &Transaction<'_>, settings: &AppSettings) -> Result<(), AppError> {
    let revision = i64::try_from(settings.revision)
        .map_err(|_| AppError::storage_unavailable("Lattice could not save local settings."))?;

    tx.execute(
        "INSERT INTO app_settings (id, revision, appearance, idle_unload_minutes)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            revision = excluded.revision,
            appearance = excluded.appearance,
            idle_unload_minutes = excluded.idle_unload_minutes",
        params![
            SETTINGS_ROW_ID,
            revision,
            settings.appearance.as_storage_value(),
            i64::from(settings.idle_unload_minutes)
        ],
    )
    .map_err(|_| AppError::storage_unavailable("Lattice could not save local settings."))?;

    Ok(())
}

fn read_model_runtime_status(conn: &Connection) -> Result<ModelRuntimeStatus, AppError> {
    let row = conn
        .query_row(
            "SELECT revision, executable_path, availability, cli_version,
                    approved_executable_fingerprint, approved_cli_version,
                    approved_checked_at_unix_seconds, daemon_status, daemon_pid,
                    daemon_is_daemon, daemon_version, server_status, server_port,
                    server_endpoint, ownership_state, owned_daemon_pid,
                    owned_since_unix_seconds, last_checked_unix_seconds, message
             FROM model_runtime_discovery
             WHERE id = ?1",
            params![RUNTIME_ROW_ID],
            |row| {
                Ok(RuntimeStorageRow {
                    revision: row.get(0)?,
                    executable_path: row.get(1)?,
                    availability: row.get(2)?,
                    cli_version: row.get(3)?,
                    approved_executable_fingerprint: row.get(4)?,
                    approved_cli_version: row.get(5)?,
                    approved_checked_at_unix_seconds: row.get(6)?,
                    daemon_status: row.get(7)?,
                    daemon_pid: row.get(8)?,
                    daemon_is_daemon: row.get(9)?,
                    daemon_version: row.get(10)?,
                    server_status: row.get(11)?,
                    server_port: row.get(12)?,
                    server_endpoint: row.get(13)?,
                    ownership_state: row.get(14)?,
                    owned_daemon_pid: row.get(15)?,
                    owned_since_unix_seconds: row.get(16)?,
                    last_checked_unix_seconds: row.get(17)?,
                    message: row.get(18)?,
                })
            },
        )
        .optional()
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not read model runtime status.")
        })?;

    let Some(row) = row else {
        return Err(AppError::storage_unavailable(
            "Lattice could not read model runtime status.",
        ));
    };

    model_runtime_status_from_row(row)
}

fn write_model_runtime_status(
    tx: &Transaction<'_>,
    status: &ModelRuntimeStatus,
) -> Result<(), AppError> {
    let revision = i64::try_from(status.revision).map_err(|_| {
        AppError::storage_unavailable("Lattice could not save model runtime status.")
    })?;
    let approved_checked_at = status
        .approved
        .as_ref()
        .map(|approval| i64::try_from(approval.checked_at_unix_seconds))
        .transpose()
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;
    let last_checked = status
        .last_checked_unix_seconds
        .map(i64::try_from)
        .transpose()
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;
    let owned_since = status
        .ownership
        .owned_since_unix_seconds()
        .map(i64::try_from)
        .transpose()
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save model runtime status.")
        })?;

    tx.execute(
        "INSERT INTO model_runtime_discovery (
            id, revision, executable_path, availability, cli_version,
            approved_executable_fingerprint, approved_cli_version,
            approved_checked_at_unix_seconds, daemon_status, daemon_pid,
            daemon_is_daemon, daemon_version, server_status, server_port,
            server_endpoint, ownership_state, owned_daemon_pid, owned_since_unix_seconds,
            last_checked_unix_seconds, message
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(id) DO UPDATE SET
            revision = excluded.revision,
            executable_path = excluded.executable_path,
            availability = excluded.availability,
            cli_version = excluded.cli_version,
            approved_executable_fingerprint = excluded.approved_executable_fingerprint,
            approved_cli_version = excluded.approved_cli_version,
            approved_checked_at_unix_seconds = excluded.approved_checked_at_unix_seconds,
            daemon_status = excluded.daemon_status,
            daemon_pid = excluded.daemon_pid,
            daemon_is_daemon = excluded.daemon_is_daemon,
            daemon_version = excluded.daemon_version,
            server_status = excluded.server_status,
            server_port = excluded.server_port,
            server_endpoint = excluded.server_endpoint,
            ownership_state = excluded.ownership_state,
            owned_daemon_pid = excluded.owned_daemon_pid,
            owned_since_unix_seconds = excluded.owned_since_unix_seconds,
            last_checked_unix_seconds = excluded.last_checked_unix_seconds,
            message = excluded.message",
        params![
            RUNTIME_ROW_ID,
            revision,
            status.executable_path.as_deref(),
            status.availability.as_storage_value(),
            status.cli_version.as_deref(),
            status
                .approved
                .as_ref()
                .map(|approval| approval.executable_fingerprint.as_str()),
            status
                .approved
                .as_ref()
                .map(|approval| approval.cli_version.as_str()),
            approved_checked_at,
            status.daemon.status.as_storage_value(),
            status.daemon.pid.map(i64::from),
            status.daemon.is_daemon.map(i64::from),
            status.daemon.version.as_deref(),
            status.server.status.as_storage_value(),
            status.server.port.map(i64::from),
            status.server.endpoint.as_deref(),
            status.ownership.as_storage_value(),
            status.ownership.owned_daemon_pid().map(i64::from),
            owned_since,
            last_checked,
            status.message.as_str()
        ],
    )
    .map_err(|_| AppError::storage_unavailable("Lattice could not save model runtime status."))?;

    Ok(())
}

#[derive(Debug)]
struct RuntimeStorageRow {
    revision: i64,
    executable_path: Option<String>,
    availability: String,
    cli_version: Option<String>,
    approved_executable_fingerprint: Option<String>,
    approved_cli_version: Option<String>,
    approved_checked_at_unix_seconds: Option<i64>,
    daemon_status: String,
    daemon_pid: Option<i64>,
    daemon_is_daemon: Option<i64>,
    daemon_version: Option<String>,
    server_status: String,
    server_port: Option<i64>,
    server_endpoint: Option<String>,
    ownership_state: String,
    owned_daemon_pid: Option<i64>,
    owned_since_unix_seconds: Option<i64>,
    last_checked_unix_seconds: Option<i64>,
    message: String,
}

fn model_runtime_status_from_row(row: RuntimeStorageRow) -> Result<ModelRuntimeStatus, AppError> {
    let approved = runtime_approval_from_row(&row)?;
    let ownership = runtime_ownership_from_row(&row)?;

    Ok(ModelRuntimeStatus {
        revision: validate_revision(row.revision)?,
        executable_path: row.executable_path,
        availability: availability_from_storage(&row.availability)?,
        cli_version: row.cli_version,
        approved,
        daemon: RuntimeDaemonObservation {
            status: daemon_status_from_storage(&row.daemon_status)?,
            pid: row.daemon_pid.map(u32::try_from).transpose().map_err(|_| {
                AppError::storage_unavailable("Lattice could not read model runtime status.")
            })?,
            is_daemon: row.daemon_is_daemon.map(|value| value != 0),
            version: row.daemon_version,
        },
        server: RuntimeServerObservation {
            status: server_status_from_storage(&row.server_status)?,
            port: row
                .server_port
                .map(u16::try_from)
                .transpose()
                .map_err(|_| {
                    AppError::storage_unavailable("Lattice could not read model runtime status.")
                })?,
            endpoint: row.server_endpoint,
        },
        ownership,
        last_operation: None,
        last_checked_unix_seconds: row
            .last_checked_unix_seconds
            .map(validate_revision)
            .transpose()?,
        message: if row.message.is_empty() {
            DEFAULT_RUNTIME_MESSAGE.to_string()
        } else {
            row.message
        },
    })
}

fn runtime_ownership_from_row(row: &RuntimeStorageRow) -> Result<RuntimeOwnership, AppError> {
    match row.ownership_state.as_str() {
        "owned" => {
            let (Some(daemon_pid), Some(executable_fingerprint), Some(owned_since)) = (
                row.owned_daemon_pid,
                row.approved_executable_fingerprint.clone(),
                row.owned_since_unix_seconds,
            ) else {
                return Ok(RuntimeOwnership::Unknown);
            };
            let daemon_pid = u32::try_from(daemon_pid).map_err(|_| {
                AppError::storage_unavailable("Lattice could not read model runtime status.")
            })?;
            let owned_since_unix_seconds = validate_revision(owned_since)?;

            Ok(RuntimeOwnership::Owned {
                daemon_pid,
                executable_fingerprint,
                owned_since_unix_seconds,
            })
        }
        "attached" => Ok(RuntimeOwnership::Attached),
        "unknown" => Ok(RuntimeOwnership::Unknown),
        _ => Err(AppError::storage_unavailable(
            "Lattice could not read model runtime status.",
        )),
    }
}

fn runtime_approval_from_row(
    row: &RuntimeStorageRow,
) -> Result<Option<RuntimeProbeApproval>, AppError> {
    let Some(executable_fingerprint) = row.approved_executable_fingerprint.clone() else {
        return Ok(None);
    };
    let Some(cli_version) = row.approved_cli_version.clone() else {
        return Ok(None);
    };
    let Some(checked_at) = row.approved_checked_at_unix_seconds else {
        return Ok(None);
    };

    Ok(Some(RuntimeProbeApproval {
        executable_fingerprint,
        cli_version,
        checked_at_unix_seconds: validate_revision(checked_at)?,
    }))
}

struct ModelLoadStorageRow {
    revision: i64,
    ownership_state: String,
    owned_identifier: Option<String>,
    owned_model_key: Option<String>,
    owned_since_unix_seconds: Option<i64>,
}

fn read_model_load_state(conn: &Connection) -> Result<(u64, ModelLoadOwnership), AppError> {
    let row = conn
        .query_row(
            "SELECT revision, ownership_state, owned_identifier, owned_model_key, owned_since_unix_seconds
             FROM model_load_state
             WHERE id = ?1",
            params![MODEL_LOAD_ROW_ID],
            |row| {
                Ok(ModelLoadStorageRow {
                    revision: row.get(0)?,
                    ownership_state: row.get(1)?,
                    owned_identifier: row.get(2)?,
                    owned_model_key: row.get(3)?,
                    owned_since_unix_seconds: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|_| AppError::storage_unavailable("Lattice could not read model load status."))?;

    let Some(row) = row else {
        return Err(AppError::storage_unavailable(
            "Lattice could not read model load status.",
        ));
    };

    let revision = validate_revision(row.revision)?;
    let ownership = model_load_ownership_from_row(&row)?;
    Ok((revision, ownership))
}

fn write_model_load_state(
    tx: &Transaction<'_>,
    revision: u64,
    ownership: &ModelLoadOwnership,
) -> Result<(), AppError> {
    let revision = i64::try_from(revision)
        .map_err(|_| AppError::storage_unavailable("Lattice could not save model load status."))?;
    let (identifier, model_key, since) = match ownership {
        ModelLoadOwnership::Owned {
            identifier,
            model_key,
            loaded_since_unix_seconds,
        } => {
            let since = i64::try_from(*loaded_since_unix_seconds).map_err(|_| {
                AppError::storage_unavailable("Lattice could not save model load status.")
            })?;
            (
                Some(identifier.as_str()),
                Some(model_key.as_str()),
                Some(since),
            )
        }
        ModelLoadOwnership::Attached {
            identifier,
            model_key,
        } => (Some(identifier.as_str()), Some(model_key.as_str()), None),
        ModelLoadOwnership::Unknown => (None, None, None),
    };

    tx.execute(
        "INSERT INTO model_load_state (id, revision, ownership_state, owned_identifier, owned_model_key, owned_since_unix_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            revision = excluded.revision,
            ownership_state = excluded.ownership_state,
            owned_identifier = excluded.owned_identifier,
            owned_model_key = excluded.owned_model_key,
            owned_since_unix_seconds = excluded.owned_since_unix_seconds",
        params![
            MODEL_LOAD_ROW_ID,
            revision,
            ownership.as_storage_value(),
            identifier,
            model_key,
            since
        ],
    )
    .map_err(|_| AppError::storage_unavailable("Lattice could not save model load status."))?;

    Ok(())
}

fn model_load_ownership_from_row(
    row: &ModelLoadStorageRow,
) -> Result<ModelLoadOwnership, AppError> {
    match row.ownership_state.as_str() {
        "owned" => {
            let (Some(identifier), Some(model_key), Some(since)) = (
                row.owned_identifier.clone(),
                row.owned_model_key.clone(),
                row.owned_since_unix_seconds,
            ) else {
                return Ok(ModelLoadOwnership::Unknown);
            };
            Ok(ModelLoadOwnership::Owned {
                identifier,
                model_key,
                loaded_since_unix_seconds: validate_revision(since)?,
            })
        }
        "attached" => {
            let (Some(identifier), Some(model_key)) =
                (row.owned_identifier.clone(), row.owned_model_key.clone())
            else {
                return Ok(ModelLoadOwnership::Unknown);
            };
            Ok(ModelLoadOwnership::Attached {
                identifier,
                model_key,
            })
        }
        "unknown" => Ok(ModelLoadOwnership::Unknown),
        _ => Err(AppError::storage_unavailable(
            "Lattice could not read model load status.",
        )),
    }
}

/// Composes the full read-only `ModelSlotStatus` for `get_model_slot_status`.
/// Never mutates storage: like `ModelRuntimeStatus::with_current_file_state`,
/// this recomputes a live view from the current external runtime state every
/// call rather than trusting a previously-persisted reconciliation, so a
/// stale `owned` record that no longer matches reality downgrades to
/// `unknown` on every read without needing an explicit write-back.
fn compose_model_slot_status(
    revision: u64,
    persisted_ownership: &ModelLoadOwnership,
    runtime: &ModelRuntimeStatus,
    last_operation: Option<ModelOperationOutcome>,
) -> ModelSlotStatus {
    if runtime.availability != ModelRuntimeAvailability::Running {
        return ModelSlotStatus {
            revision,
            installed: Vec::new(),
            loaded: None,
            ownership: ModelLoadOwnership::Unknown,
            last_operation,
            last_checked_unix_seconds: None,
            message: MODEL_RUNTIME_NOT_RUNNING_MESSAGE.to_string(),
        };
    }

    let executable_path = runtime
        .executable_path
        .as_deref()
        .and_then(|path| validate_runtime_executable_path(path).ok());
    let Some(executable_path) = executable_path else {
        return ModelSlotStatus {
            revision,
            installed: Vec::new(),
            loaded: None,
            ownership: ModelLoadOwnership::Unknown,
            last_operation,
            last_checked_unix_seconds: None,
            message: MODEL_RUNTIME_NOT_RUNNING_MESSAGE.to_string(),
        };
    };

    let installed = list_installed_models(&executable_path);
    let (loaded, ownership) = observe_loaded_slot(&executable_path, persisted_ownership);

    ModelSlotStatus {
        revision,
        installed,
        loaded,
        ownership,
        last_operation,
        last_checked_unix_seconds: Some(now_unix_seconds()),
        message: MODEL_SLOT_READY_MESSAGE.to_string(),
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn path_to_string(path: &Path) -> Result<String, AppError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::invalid_runtime("Runtime executable path must be UTF-8."))
}

fn validate_update(request: &UpdateAppSettingsRequest) -> Result<ValidSettingsPatch, AppError> {
    Ok(ValidSettingsPatch {
        appearance: request
            .appearance
            .as_deref()
            .map(validate_appearance)
            .transpose()?,
        idle_unload_minutes: request
            .idle_unload_minutes
            .map(validate_idle_unload_minutes)
            .transpose()?,
    })
}

fn validate_appearance(value: &str) -> Result<AppearancePreference, AppError> {
    match value {
        "system" => Ok(AppearancePreference::System),
        "light" => Ok(AppearancePreference::Light),
        "dark" => Ok(AppearancePreference::Dark),
        _ => Err(AppError::invalid_settings(
            "Appearance must be system, light, or dark.",
        )),
    }
}

fn validate_idle_unload_minutes(value: i64) -> Result<u16, AppError> {
    if !(MIN_IDLE_UNLOAD_MINUTES..=MAX_IDLE_UNLOAD_MINUTES).contains(&value) {
        return Err(AppError::invalid_settings(
            "Idle unload must be between 1 and 120 minutes.",
        ));
    }

    u16::try_from(value)
        .map_err(|_| AppError::invalid_settings("Idle unload must be between 1 and 120 minutes."))
}

fn validate_revision(value: i64) -> Result<u64, AppError> {
    if value < 1 {
        return Err(AppError::storage_unavailable(
            "Lattice could not read local settings.",
        ));
    }

    u64::try_from(value)
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))
}

#[cfg(test)]
mod tests {
    use super::{
        AppSettings, AppearancePreference, ResetAppSettingsRequest, SettingsStore,
        UpdateAppSettingsRequest, CURRENT_SCHEMA_VERSION,
    };
    use crate::model_runtime::{
        ConfigureModelRuntimeRequest, GetModelSlotStatusRequest, LoadModelRequest,
        ModelLoadOwnership, ModelRuntimeAvailability, ProbeModelRuntimeRequest, RuntimeOwnership,
        StartModelRuntimeRequest, UnloadModelRequest,
    };
    use rusqlite::Connection;
    use std::{error::Error, fs, net::TcpListener, path::PathBuf, sync::atomic::AtomicBool};
    use tempfile::{tempdir, TempDir};

    #[test]
    fn creates_default_settings_in_empty_database() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let store = SettingsStore::open(&path)?;

        assert_eq!(store.read()?, AppSettings::default());
        assert!(path.exists());
        Ok(())
    }

    #[test]
    fn updates_settings_and_reopens_saved_values() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let mut store = SettingsStore::open(&path)?;

        let saved = store.update(UpdateAppSettingsRequest {
            expected_revision: 1,
            appearance: Some("dark".to_string()),
            idle_unload_minutes: Some(15),
        })?;
        drop(store);

        let reopened = SettingsStore::open(&path)?;
        assert_eq!(
            reopened.read()?,
            AppSettings {
                schema_version: CURRENT_SCHEMA_VERSION,
                revision: 2,
                appearance: AppearancePreference::Dark,
                idle_unload_minutes: 15
            }
        );
        assert_eq!(saved.revision, 2);
        Ok(())
    }

    #[test]
    fn invalid_update_preserves_old_revision() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let original = store.read()?;

        let error = store
            .update(UpdateAppSettingsRequest {
                expected_revision: original.revision,
                appearance: Some("neon".to_string()),
                idle_unload_minutes: None,
            })
            .err()
            .ok_or("expected invalid settings error")?;

        assert_eq!(error.code, "settings.invalid");
        assert_eq!(store.read()?, original);
        Ok(())
    }

    #[test]
    fn revision_conflict_preserves_existing_settings() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let original = store.read()?;

        let error = store
            .reset(ResetAppSettingsRequest {
                expected_revision: original.revision + 1,
            })
            .err()
            .ok_or("expected settings conflict")?;

        assert_eq!(error.code, "settings.conflict");
        assert_eq!(store.read()?, original);
        Ok(())
    }

    #[test]
    fn default_runtime_status_is_missing() -> Result<(), Box<dyn Error>> {
        let store = SettingsStore::open_in_memory()?;

        let status = store.read_model_runtime_status()?;

        assert_eq!(status.revision, 1);
        assert_eq!(status.availability, ModelRuntimeAvailability::Missing);
        assert!(status.executable_path.is_none());
        Ok(())
    }

    #[test]
    fn model_slot_status_reports_unavailable_without_running_runtime() -> Result<(), Box<dyn Error>>
    {
        let store = SettingsStore::open_in_memory()?;

        let status = store.get_model_slot_status(GetModelSlotStatusRequest {})?;

        assert_eq!(status.revision, 1);
        assert!(status.installed.is_empty());
        assert!(status.loaded.is_none());
        assert_eq!(status.ownership, ModelLoadOwnership::Unknown);
        assert!(status.message.contains("Start the runtime"));
        Ok(())
    }

    #[test]
    fn load_model_requires_running_runtime() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let cancel = AtomicBool::new(false);

        let error = store
            .load_model(
                LoadModelRequest {
                    expected_revision: 1,
                    model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
                },
                &cancel,
            )
            .err()
            .ok_or("expected invalid runtime error")?;

        assert_eq!(error.code, "runtime.invalid");
        Ok(())
    }

    #[test]
    fn unload_model_requires_running_runtime() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let cancel = AtomicBool::new(false);

        let error = store
            .unload_model(
                UnloadModelRequest {
                    expected_revision: 1,
                },
                &cancel,
            )
            .err()
            .ok_or("expected invalid runtime error")?;

        assert_eq!(error.code, "runtime.invalid");
        Ok(())
    }

    #[test]
    fn load_model_rejects_stale_revision() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let cancel = AtomicBool::new(false);

        let error = store
            .load_model(
                LoadModelRequest {
                    expected_revision: 99,
                    model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
                },
                &cancel,
            )
            .err()
            .ok_or("expected runtime conflict")?;

        assert_eq!(error.code, "runtime.conflict");
        Ok(())
    }

    #[test]
    fn model_load_ownership_survives_restart_when_verified() -> Result<(), Box<dyn Error>> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let port = listener.local_addr()?.port();
        let directory = tempdir()?;
        let db_path = directory.path().join("lattice.sqlite3");
        let fixture = executable_fixture(&format!(
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
      ls) echo '[{{"modelKey":"qwen/qwen2.5-0.5b-instruct","displayName":"Qwen2.5 0.5B Instruct","architecture":"qwen2","type":"llm","sizeBytes":400000000}}]';;
      ps)
        if [ -f "$state_dir/model-loaded" ]; then
          echo '[{{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct","architecture":"qwen2","sizeBytes":400000000}}]'
        else
          echo '[]'
        fi
        ;;
      load) touch "$state_dir/model-loaded"; echo "loaded";;
      unload) rm -f "$state_dir/model-loaded"; echo "unloaded";;
      *) exit 2;;
    esac
    ;;
esac
"#
        ))?;

        let mut store = SettingsStore::open(&db_path)?;
        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let cancel = AtomicBool::new(false);
        store.start_model_runtime(
            StartModelRuntimeRequest {
                expected_revision: configured.revision,
            },
            &cancel,
        )?;

        let loaded = store.load_model(
            LoadModelRequest {
                expected_revision: 1,
                model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
            },
            &cancel,
        )?;
        assert!(matches!(loaded.ownership, ModelLoadOwnership::Owned { .. }));
        drop(store);

        let reopened = SettingsStore::open(&db_path)?;
        let status = reopened.get_model_slot_status(GetModelSlotStatusRequest {})?;

        assert!(matches!(status.ownership, ModelLoadOwnership::Owned { .. }));
        assert_eq!(
            status.loaded.map(|loaded| loaded.model_key),
            Some("qwen/qwen2.5-0.5b-instruct".to_string())
        );
        drop(listener);
        Ok(())
    }

    #[test]
    fn unverifiable_model_load_ownership_downgrades_to_unknown_after_restart(
    ) -> Result<(), Box<dyn Error>> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let port = listener.local_addr()?.port();
        let directory = tempdir()?;
        let db_path = directory.path().join("lattice.sqlite3");
        let fixture = executable_fixture(&format!(
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
      ls) echo '[{{"modelKey":"qwen/qwen2.5-0.5b-instruct","displayName":"Qwen2.5 0.5B Instruct","architecture":"qwen2","type":"llm","sizeBytes":400000000}}]';;
      ps)
        if [ -f "$state_dir/model-loaded" ]; then
          echo '[{{"identifier":"lattice-managed","modelKey":"qwen/qwen2.5-0.5b-instruct","architecture":"qwen2","sizeBytes":400000000}}]'
        else
          echo '[]'
        fi
        ;;
      load) touch "$state_dir/model-loaded"; echo "loaded";;
      unload) rm -f "$state_dir/model-loaded"; echo "unloaded";;
      *) exit 2;;
    esac
    ;;
esac
"#
        ))?;

        let mut store = SettingsStore::open(&db_path)?;
        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let cancel = AtomicBool::new(false);
        store.start_model_runtime(
            StartModelRuntimeRequest {
                expected_revision: configured.revision,
            },
            &cancel,
        )?;
        store.load_model(
            LoadModelRequest {
                expected_revision: 1,
                model_key: "qwen/qwen2.5-0.5b-instruct".to_string(),
            },
            &cancel,
        )?;
        drop(store);

        // Simulate the loaded model having been released externally before restart.
        fs::remove_file(
            fixture
                .path
                .parent()
                .ok_or("fixture has no parent")?
                .join("state")
                .join("model-loaded"),
        )?;

        let reopened = SettingsStore::open(&db_path)?;
        let status = reopened.get_model_slot_status(GetModelSlotStatusRequest {})?;

        assert_eq!(status.ownership, ModelLoadOwnership::Unknown);
        assert!(status.loaded.is_none());
        drop(listener);
        Ok(())
    }

    #[test]
    fn migrates_v3_settings_schema_to_model_load_schema() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
                idle_unload_minutes INTEGER NOT NULL CHECK (
                    idle_unload_minutes >= 1 AND idle_unload_minutes <= 120
                )
            );
            INSERT INTO app_settings (id, revision, appearance, idle_unload_minutes)
            VALUES (1, 3, 'system', 5);
            CREATE TABLE model_runtime_discovery (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                executable_path TEXT,
                availability TEXT NOT NULL,
                cli_version TEXT,
                approved_executable_fingerprint TEXT,
                approved_cli_version TEXT,
                approved_checked_at_unix_seconds INTEGER,
                daemon_status TEXT NOT NULL,
                daemon_pid INTEGER,
                daemon_is_daemon INTEGER,
                daemon_version TEXT,
                server_status TEXT NOT NULL,
                server_port INTEGER,
                server_endpoint TEXT,
                last_checked_unix_seconds INTEGER,
                message TEXT NOT NULL,
                ownership_state TEXT NOT NULL DEFAULT 'unknown',
                owned_daemon_pid INTEGER,
                owned_since_unix_seconds INTEGER
            );
            INSERT INTO model_runtime_discovery (id, revision, availability, daemon_status, server_status, message)
            VALUES (1, 1, 'missing', 'unknown', 'unknown', 'No runtime executable configured.');
            PRAGMA user_version = 3;",
        )?;
        drop(conn);

        let store = SettingsStore::open(&path)?;

        assert_eq!(store.read()?.appearance, AppearancePreference::System);
        assert_eq!(
            store.read_model_runtime_status()?.availability,
            ModelRuntimeAvailability::Missing
        );
        let status = store.get_model_slot_status(GetModelSlotStatusRequest {})?;
        assert_eq!(status.revision, 1);
        assert_eq!(status.ownership, ModelLoadOwnership::Unknown);
        assert!(backup_count(directory.path())? >= 1);
        Ok(())
    }

    #[test]
    fn invalid_runtime_path_preserves_runtime_revision() -> Result<(), Box<dyn Error>> {
        let mut store = SettingsStore::open_in_memory()?;
        let original = store.read_model_runtime_status()?;

        let error = store
            .configure_model_runtime(ConfigureModelRuntimeRequest {
                expected_revision: original.revision,
                executable_path: "relative/lms".to_string(),
            })
            .err()
            .ok_or("expected invalid runtime path")?;

        assert_eq!(error.code, "runtime.invalid");
        assert_eq!(store.read_model_runtime_status()?, original);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn runtime_probe_metadata_reopens_from_storage() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let fixture = executable_fixture(
            r#"case "$1" in
  --version) echo "lms v0.0.47";;
  daemon) echo '{"status":"not-running"}';;
  server) echo '{"running":false}';;
  *) exit 2;;
esac
"#,
        )?;
        let mut store = SettingsStore::open(&path)?;

        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let probed = store.probe_model_runtime(ProbeModelRuntimeRequest {
            expected_revision: configured.revision,
        })?;
        drop(store);

        let reopened = SettingsStore::open(&path)?;
        let status = reopened.read_model_runtime_status()?;
        assert_eq!(status.revision, probed.revision);
        assert_eq!(status.availability, ModelRuntimeAvailability::Stopped);
        assert_eq!(status.cli_version, Some("0.0.47".to_string()));
        assert!(status.approved.is_some());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn changed_runtime_executable_requires_new_probe() -> Result<(), Box<dyn Error>> {
        let fixture = executable_fixture(
            r#"case "$1" in
  --version) echo "lms v0.0.47";;
  daemon) echo '{"status":"not-running"}';;
  server) echo '{"running":false}';;
  *) exit 2;;
esac
"#,
        )?;
        let mut store = SettingsStore::open_in_memory()?;
        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let probed = store.probe_model_runtime(ProbeModelRuntimeRequest {
            expected_revision: configured.revision,
        })?;
        fs::write(&fixture.path, b"changed runtime binary")?;

        let status = store.read_model_runtime_status()?;

        assert_eq!(probed.availability, ModelRuntimeAvailability::Stopped);
        assert_eq!(status.availability, ModelRuntimeAvailability::Unknown);
        assert!(status.message.contains("changed"));
        Ok(())
    }

    #[test]
    fn ownership_survives_restart_when_fingerprint_unchanged() -> Result<(), Box<dyn Error>> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let port = listener.local_addr()?.port();
        let directory = tempdir()?;
        let db_path = directory.path().join("lattice.sqlite3");
        let fixture = executable_fixture(&format!(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1 $2" in
  "daemon up") touch "$state_dir/daemon"; echo '{{"status":"running","pid":555,"isDaemon":true}}';;
  "daemon status") if [ -f "$state_dir/daemon" ]; then echo '{{"status":"running","pid":555,"isDaemon":true}}'; else echo '{{"status":"not-running"}}'; fi;;
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

        let mut store = SettingsStore::open(&db_path)?;
        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let cancel = AtomicBool::new(false);
        let started = store.start_model_runtime(
            StartModelRuntimeRequest {
                expected_revision: configured.revision,
            },
            &cancel,
        )?;
        assert!(matches!(
            started.ownership,
            RuntimeOwnership::Owned {
                daemon_pid: 555,
                ..
            }
        ));
        drop(store);

        let reopened = SettingsStore::open(&db_path)?;
        let status = reopened.read_model_runtime_status()?;

        assert!(matches!(
            status.ownership,
            RuntimeOwnership::Owned {
                daemon_pid: 555,
                ..
            }
        ));
        drop(listener);
        Ok(())
    }

    #[test]
    fn ownership_downgrades_to_unknown_when_executable_changes_after_owning(
    ) -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let db_path = directory.path().join("lattice.sqlite3");
        let fixture = executable_fixture(
            r#"state_dir="$(dirname "$0")/state"
mkdir -p "$state_dir"
case "$1 $2" in
  "daemon up") touch "$state_dir/daemon"; echo '{"status":"running","pid":777,"isDaemon":true}';;
  "daemon status") if [ -f "$state_dir/daemon" ]; then echo '{"status":"running","pid":777,"isDaemon":true}'; else echo '{"status":"not-running"}'; fi;;
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

        let mut store = SettingsStore::open(&db_path)?;
        let configured = store.configure_model_runtime(ConfigureModelRuntimeRequest {
            expected_revision: 1,
            executable_path: fixture.path.to_string_lossy().to_string(),
        })?;
        let cancel = AtomicBool::new(false);
        let started = store.start_model_runtime(
            StartModelRuntimeRequest {
                expected_revision: configured.revision,
            },
            &cancel,
        )?;
        assert!(matches!(
            started.ownership,
            RuntimeOwnership::Owned {
                daemon_pid: 777,
                ..
            }
        ));
        drop(store);

        fs::write(&fixture.path, b"#!/bin/sh\nexit 2\n")?;
        let reopened = SettingsStore::open(&db_path)?;
        let status = reopened.read_model_runtime_status()?;

        assert_eq!(status.ownership, RuntimeOwnership::Unknown);
        Ok(())
    }

    #[test]
    fn migrates_v1_settings_schema_to_runtime_discovery_schema() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
                idle_unload_minutes INTEGER NOT NULL CHECK (
                    idle_unload_minutes >= 1 AND idle_unload_minutes <= 120
                )
            );
            INSERT INTO app_settings (id, revision, appearance, idle_unload_minutes)
            VALUES (1, 7, 'dark', 30);
            PRAGMA user_version = 1;",
        )?;
        drop(conn);

        let store = SettingsStore::open(&path)?;

        assert_eq!(
            store.read()?,
            AppSettings {
                schema_version: CURRENT_SCHEMA_VERSION,
                revision: 7,
                appearance: AppearancePreference::Dark,
                idle_unload_minutes: 30
            }
        );
        assert_eq!(
            store.read_model_runtime_status()?.availability,
            ModelRuntimeAvailability::Missing
        );
        assert!(backup_count(directory.path())? >= 1);
        Ok(())
    }

    #[test]
    fn failed_migration_preserves_existing_database_and_backup() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE app_settings (marker TEXT NOT NULL);
             INSERT INTO app_settings (marker) VALUES ('old-data');
             PRAGMA user_version = 0;",
        )?;
        drop(conn);

        let error = SettingsStore::open(&path)
            .err()
            .ok_or("expected migration failure")?;

        assert_eq!(error.code, "storage.migration_failed");
        let conn = Connection::open(&path)?;
        let marker: String =
            conn.query_row("SELECT marker FROM app_settings", [], |row| row.get(0))?;
        assert_eq!(marker, "old-data");
        assert!(backup_count(directory.path())? >= 1);
        Ok(())
    }

    #[test]
    fn newer_schema_is_refused_without_overwrite() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA user_version = 99;")?;
        drop(conn);

        let error = SettingsStore::open(&path)
            .err()
            .ok_or("expected unsupported schema error")?;

        assert_eq!(error.code, "storage.unsupported_schema");
        let conn = Connection::open(&path)?;
        let user_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        assert_eq!(user_version, 99);
        Ok(())
    }

    #[test]
    fn storage_path_must_be_preparable() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let not_a_directory = directory.path().join("settings-parent");
        fs::write(&not_a_directory, b"not a directory")?;

        let error = SettingsStore::open(not_a_directory.join("lattice.sqlite3"))
            .err()
            .ok_or("expected storage preparation error")?;

        assert_eq!(error.code, "storage.unavailable");
        Ok(())
    }

    fn backup_count(directory: &std::path::Path) -> Result<usize, Box<dyn Error>> {
        let backup_dir = directory.join("backups");
        let entries = fs::read_dir(backup_dir)?;
        Ok(entries.count())
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
    fn make_executable(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }
}
