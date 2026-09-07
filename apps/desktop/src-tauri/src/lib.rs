use lattice_core::{app_info, AppError, AppInfo};

const STARTUP_FAILURE_EXIT_CODE: i32 = 1;

#[tauri::command]
fn get_app_info() -> Result<AppInfo, AppError> {
    Ok(app_info())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = run_desktop_shell() {
        exit_after_startup_failure(&error);
    }
}

fn run_desktop_shell() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
}

fn exit_after_startup_failure(error: &tauri::Error) -> ! {
    eprintln!("{}", startup_failure_message(error));
    std::process::exit(STARTUP_FAILURE_EXIT_CODE);
}

fn startup_failure_message(error: &tauri::Error) -> String {
    format!("failed to run Lattice desktop shell: {error}")
}

#[cfg(test)]
mod tests {
    use super::{get_app_info, startup_failure_message, STARTUP_FAILURE_EXIT_CODE};
    use lattice_core::{AppRuntime, GET_APP_INFO_COMMAND};
    use serde_json::Value;
    use std::{error::Error, fs, io, path::PathBuf};

    #[test]
    fn ipc_command_returns_core_app_info() {
        let info = get_app_info().ok();

        assert_eq!(info.as_ref().map(|value| value.name), Some("Lattice"));
        assert_eq!(
            info.as_ref().map(|value| value.runtime),
            Some(AppRuntime::Tauri)
        );
    }

    #[test]
    fn application_command_inventory_matches_tauri_handler() {
        assert_eq!([GET_APP_INFO_COMMAND], ["get_app_info"]);
    }

    #[test]
    fn tauri_config_uses_restrictive_production_csp() -> Result<(), Box<dyn Error>> {
        let config = read_tauri_json("tauri.conf.json")?;
        let csp = required_path(&config, &["app", "security", "csp"])?;

        if !csp.is_object() {
            return Err(io::Error::other("production CSP must be configured as an object").into());
        }

        let default_src = directive_tokens(csp, "default-src")?;
        assert!(default_src.contains(&"'self'".to_string()));
        assert!(default_src.contains(&"customprotocol:".to_string()));
        assert!(default_src.contains(&"asset:".to_string()));

        let script_src = directive_tokens(csp, "script-src")?;
        assert!(script_src.contains(&"'self'".to_string()));
        assert!(!script_src.contains(&"'unsafe-eval'".to_string()));
        assert!(!script_src.contains(&"*".to_string()));
        assert!(!script_src.iter().any(|token| token.starts_with("http:")));
        assert!(!script_src.iter().any(|token| token.starts_with("https:")));

        let style_src = directive_tokens(csp, "style-src")?;
        assert!(style_src.contains(&"'self'".to_string()));
        assert!(style_src.contains(&"'unsafe-inline'".to_string()));
        assert!(!style_src.contains(&"'unsafe-eval'".to_string()));

        let connect_src = directive_tokens(csp, "connect-src")?;
        assert!(connect_src.contains(&"ipc:".to_string()));
        assert!(connect_src.contains(&"http://ipc.localhost".to_string()));
        assert!(!connect_src.contains(&"*".to_string()));
        assert!(!connect_src
            .iter()
            .any(|token| token.contains("127.0.0.1:1420")));
        assert!(!connect_src
            .iter()
            .any(|token| token.contains("localhost:1420")));

        assert!(directive_tokens(csp, "object-src")?.contains(&"'none'".to_string()));
        assert!(directive_tokens(csp, "base-uri")?.contains(&"'none'".to_string()));
        assert!(directive_tokens(csp, "frame-src")?.contains(&"'none'".to_string()));

        Ok(())
    }

    #[test]
    fn tauri_main_window_has_minimal_foundation_capability() -> Result<(), Box<dyn Error>> {
        let config = read_tauri_json("tauri.conf.json")?;
        let windows = required_path(&config, &["app", "windows"])?
            .as_array()
            .ok_or_else(|| io::Error::other("app windows must be an array"))?;
        let Some(main_window) = windows.first() else {
            return Err(io::Error::other("app must define a main window").into());
        };

        assert_eq!(
            main_window.get("label").and_then(Value::as_str),
            Some("main")
        );

        let capability = read_tauri_json("capabilities/default.json")?;
        assert_eq!(
            string_array(&capability, "windows")?,
            vec!["main".to_string()]
        );
        assert_eq!(
            string_array(&capability, "permissions")?,
            vec!["allow-get-app-info".to_string()]
        );
        assert!(capability.get("remote").is_none());
        assert!(capability
            .get("local")
            .and_then(Value::as_bool)
            .unwrap_or(true));

        Ok(())
    }

    #[test]
    fn startup_failure_uses_unsuccessful_exit_policy() {
        let error = tauri::Error::AssetNotFound("missing shell asset".into());
        let message = startup_failure_message(&error);

        assert_eq!(STARTUP_FAILURE_EXIT_CODE, 1);
        assert!(message.starts_with("failed to run Lattice desktop shell:"));
        assert!(message.contains("missing shell asset"));
    }

    fn read_tauri_json(relative_path: &str) -> Result<Value, Box<dyn Error>> {
        let path = tauri_manifest_dir().join(relative_path);
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    fn tauri_manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn required_path<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, Box<dyn Error>> {
        let mut current = value;
        for segment in path {
            current = current
                .get(segment)
                .ok_or_else(|| io::Error::other(format!("missing JSON path segment {segment}")))?;
        }
        Ok(current)
    }

    fn directive_tokens(csp: &Value, directive: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let value = csp
            .get(directive)
            .ok_or_else(|| io::Error::other(format!("missing CSP directive {directive}")))?;

        if let Some(raw) = value.as_str() {
            return Ok(raw.split_whitespace().map(String::from).collect());
        }

        if let Some(values) = value.as_array() {
            let mut tokens = Vec::new();
            for entry in values {
                let Some(raw) = entry.as_str() else {
                    return Err(io::Error::other(format!(
                        "CSP directive {directive} must be strings"
                    ))
                    .into());
                };
                tokens.extend(raw.split_whitespace().map(String::from));
            }
            return Ok(tokens);
        }

        Err(io::Error::other(format!("CSP directive {directive} must be string or array")).into())
    }

    fn string_array(value: &Value, key: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let array = value
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| io::Error::other(format!("{key} must be an array")))?;
        let mut strings = Vec::new();

        for entry in array {
            let Some(text) = entry.as_str() else {
                return Err(io::Error::other(format!("{key} entries must be strings")).into());
            };
            strings.push(text.to_string());
        }

        Ok(strings)
    }
}
