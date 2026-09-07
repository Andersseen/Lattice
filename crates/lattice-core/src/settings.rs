use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const GET_APP_SETTINGS_COMMAND: &str = "get_app_settings";
pub const UPDATE_APP_SETTINGS_COMMAND: &str = "update_app_settings";
pub const RESET_APP_SETTINGS_COMMAND: &str = "reset_app_settings";

const CURRENT_SCHEMA_VERSION: u32 = 1;
const SETTINGS_ROW_ID: i64 = 1;
const DEFAULT_IDLE_UNLOAD_MINUTES: u16 = 5;
const MIN_IDLE_UNLOAD_MINUTES: i64 = 1;
const MAX_IDLE_UNLOAD_MINUTES: i64 = 120;

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
        return Ok(());
    }

    create_pre_migration_backup(db_path)?;
    let tx = conn
        .transaction()
        .map_err(|_| AppError::migration_failed("Lattice could not migrate local settings."))?;

    if schema_version == 0 {
        create_v1_schema(&tx)?;
    } else {
        return Err(AppError::unsupported_schema(
            "Local settings schema is not supported by this Lattice version.",
        ));
    }

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
    tx.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
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
    use rusqlite::Connection;
    use std::{error::Error, fs};
    use tempfile::tempdir;

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
}
