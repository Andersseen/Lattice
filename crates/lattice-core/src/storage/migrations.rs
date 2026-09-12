//! Schema evolution: what each versioned schema looks like and how an
//! older database is carried forward to [`super::CURRENT_SCHEMA_VERSION`].
//! Connection mechanics live in `database`; what a current-schema
//! connection is used for lives in `settings`/`runtime`.

use super::database::schema_version;
use super::runtime::{
    read_model_load_state, read_model_runtime_status, write_model_runtime_status,
};
use super::settings::read_settings;
use super::{CURRENT_SCHEMA_VERSION, MODEL_LOAD_ROW_ID};
use crate::{model_runtime::ModelRuntimeStatus, AppError};
use rusqlite::{params, Connection, Transaction};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn migrate(conn: &mut Connection, db_path: Option<&Path>) -> Result<(), AppError> {
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

    super::settings::write_settings(tx, &super::settings::AppSettings::default())?;
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

#[cfg(test)]
mod tests {
    use super::super::test_support::backup_count;
    use super::super::CURRENT_SCHEMA_VERSION;
    use crate::{
        model_runtime::{GetModelSlotStatusRequest, ModelLoadOwnership, ModelRuntimeAvailability},
        AppSettings, AppearancePreference, SettingsStore,
    };
    use rusqlite::Connection;
    use std::error::Error;
    use tempfile::tempdir;

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
}
