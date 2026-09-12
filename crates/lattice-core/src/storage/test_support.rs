//! Test-only fixtures shared across `migrations`/`runtime`'s own test
//! modules: a disposable executable double for the `lms` CLI and a backup
//! directory counter. Not part of the storage module's real behavior; only
//! included via `#[cfg(test)] mod test_support;` in `mod.rs`.

use std::{error::Error, fs, path::Path};
use tempfile::{tempdir, TempDir};

pub(super) fn backup_count(directory: &Path) -> Result<usize, Box<dyn Error>> {
    let backup_dir = directory.join("backups");
    let entries = fs::read_dir(backup_dir)?;
    Ok(entries.count())
}

#[cfg(unix)]
pub(super) struct ExecutableFixture {
    _directory: TempDir,
    pub(super) path: std::path::PathBuf,
}

#[cfg(unix)]
pub(super) fn executable_fixture(script: &str) -> Result<ExecutableFixture, Box<dyn Error>> {
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
