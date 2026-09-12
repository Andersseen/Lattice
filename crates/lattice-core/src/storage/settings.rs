//! Non-secret application preferences: appearance and idle-unload minutes.
//! Owns only the `app_settings` table; runtime/model persistence lives in
//! `runtime`, schema evolution in `migrations`.

use super::database::validate_revision;
use super::{SettingsStore, CURRENT_SCHEMA_VERSION};
use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

pub const GET_APP_SETTINGS_COMMAND: &str = "get_app_settings";
pub const UPDATE_APP_SETTINGS_COMMAND: &str = "update_app_settings";
pub const RESET_APP_SETTINGS_COMMAND: &str = "reset_app_settings";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidSettingsPatch {
    appearance: Option<AppearancePreference>,
    idle_unload_minutes: Option<u16>,
}

impl SettingsStore {
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
}

pub(super) fn read_settings(conn: &Connection) -> Result<AppSettings, AppError> {
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

pub(super) fn write_settings(tx: &Transaction<'_>, settings: &AppSettings) -> Result<(), AppError> {
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

#[cfg(test)]
mod tests {
    use super::{
        AppSettings, AppearancePreference, ResetAppSettingsRequest, SettingsStore,
        UpdateAppSettingsRequest, CURRENT_SCHEMA_VERSION,
    };
    use std::error::Error;
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
}
