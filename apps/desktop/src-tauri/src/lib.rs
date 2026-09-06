use lattice_core::{app_info, AppError, AppInfo};

#[tauri::command]
fn get_app_info() -> Result<AppInfo, AppError> {
    Ok(app_info())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            eprintln!("failed to run Lattice desktop shell: {error}");
        });
}

#[cfg(test)]
mod tests {
    use super::get_app_info;
    use lattice_core::AppRuntime;

    #[test]
    fn ipc_command_returns_core_app_info() {
        let info = get_app_info().ok();

        assert_eq!(info.as_ref().map(|value| value.name), Some("Lattice"));
        assert_eq!(
            info.as_ref().map(|value| value.runtime),
            Some(AppRuntime::Tauri)
        );
    }
}
