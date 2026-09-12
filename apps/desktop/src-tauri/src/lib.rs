use lattice_core::{
    app_info, authorize_chat_request, new_chat_run_id, run_chat_stream, AppError, AppInfo,
    AppSettings, CancelChatStreamRequest, CancelModelOperationRequest,
    CancelModelRuntimeOperationRequest, ChatRequest, ChatRunHandle, ChatStreamEvent,
    ConfigureModelRuntimeRequest, GetModelSlotStatusRequest, LoadModelRequest, ModelRuntimeStatus,
    ModelSlotStatus, ProbeModelRuntimeRequest, ResetAppSettingsRequest, SettingsStore,
    StartModelRuntimeRequest, StopModelRuntimeRequest, UnloadModelRequest,
    UpdateAppSettingsRequest,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tauri::Manager;

const STARTUP_FAILURE_EXIT_CODE: i32 = 1;
const SETTINGS_DATABASE_FILE: &str = "lattice.sqlite3";

struct DesktopState {
    settings: Mutex<SettingsStore>,
    runtime_operation_cancelled: Arc<AtomicBool>,
    model_operation_cancelled: Arc<AtomicBool>,
    active_chat_run: Mutex<Option<ActiveChatRun>>,
}

/// One globally active chat run's identity and shared cancellation flag,
/// mirroring `runtime_operation_cancelled`/`model_operation_cancelled`'s
/// existing pattern but keyed by run so a stale/unknown `run_id` passed to
/// `cancel_chat_stream` is a no-op rather than cancelling the wrong run.
struct ActiveChatRun {
    run_id: String,
    cancel: Arc<AtomicBool>,
}

#[tauri::command]
fn get_app_info() -> Result<AppInfo, AppError> {
    Ok(app_info())
}

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, DesktopState>) -> Result<AppSettings, AppError> {
    with_settings_store(&state, |store| store.read())
}

#[tauri::command]
fn update_app_settings(
    state: tauri::State<'_, DesktopState>,
    request: UpdateAppSettingsRequest,
) -> Result<AppSettings, AppError> {
    with_settings_store(&state, |store| store.update(request))
}

#[tauri::command]
fn reset_app_settings(
    state: tauri::State<'_, DesktopState>,
    request: ResetAppSettingsRequest,
) -> Result<AppSettings, AppError> {
    with_settings_store(&state, |store| store.reset(request))
}

#[tauri::command]
fn get_model_runtime_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<ModelRuntimeStatus, AppError> {
    with_settings_store(&state, |store| store.read_model_runtime_status())
}

#[tauri::command]
fn configure_model_runtime(
    state: tauri::State<'_, DesktopState>,
    request: ConfigureModelRuntimeRequest,
) -> Result<ModelRuntimeStatus, AppError> {
    with_settings_store(&state, |store| store.configure_model_runtime(request))
}

#[tauri::command]
fn probe_model_runtime(
    state: tauri::State<'_, DesktopState>,
    request: ProbeModelRuntimeRequest,
) -> Result<ModelRuntimeStatus, AppError> {
    with_settings_store(&state, |store| store.probe_model_runtime(request))
}

#[tauri::command]
fn start_model_runtime(
    state: tauri::State<'_, DesktopState>,
    request: StartModelRuntimeRequest,
) -> Result<ModelRuntimeStatus, AppError> {
    state
        .runtime_operation_cancelled
        .store(false, Ordering::SeqCst);
    let cancel = state.runtime_operation_cancelled.clone();
    with_settings_store(&state, |store| store.start_model_runtime(request, &cancel))
}

#[tauri::command]
fn stop_model_runtime(
    state: tauri::State<'_, DesktopState>,
    request: StopModelRuntimeRequest,
) -> Result<ModelRuntimeStatus, AppError> {
    state
        .runtime_operation_cancelled
        .store(false, Ordering::SeqCst);
    let cancel = state.runtime_operation_cancelled.clone();
    with_settings_store(&state, |store| store.stop_model_runtime(request, &cancel))
}

#[tauri::command]
fn cancel_model_runtime_operation(
    state: tauri::State<'_, DesktopState>,
    _request: CancelModelRuntimeOperationRequest,
) -> Result<(), AppError> {
    state
        .runtime_operation_cancelled
        .store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn get_model_slot_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<ModelSlotStatus, AppError> {
    with_settings_store(&state, |store| {
        store.get_model_slot_status(GetModelSlotStatusRequest {})
    })
}

#[tauri::command]
fn load_model(
    state: tauri::State<'_, DesktopState>,
    request: LoadModelRequest,
) -> Result<ModelSlotStatus, AppError> {
    state
        .model_operation_cancelled
        .store(false, Ordering::SeqCst);
    let cancel = state.model_operation_cancelled.clone();
    with_settings_store(&state, |store| store.load_model(request, &cancel))
}

#[tauri::command]
fn unload_model(
    state: tauri::State<'_, DesktopState>,
    request: UnloadModelRequest,
) -> Result<ModelSlotStatus, AppError> {
    let chat_run_active = state
        .active_chat_run
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false);
    if chat_run_active {
        return Err(AppError::runtime_conflict(
            "Cancel the active chat before unloading its model.",
        ));
    }

    state
        .model_operation_cancelled
        .store(false, Ordering::SeqCst);
    let cancel = state.model_operation_cancelled.clone();
    with_settings_store(&state, |store| store.unload_model(request, &cancel))
}

#[tauri::command]
fn cancel_model_operation(
    state: tauri::State<'_, DesktopState>,
    _request: CancelModelOperationRequest,
) -> Result<(), AppError> {
    state
        .model_operation_cancelled
        .store(true, Ordering::SeqCst);
    Ok(())
}

/// Reserves the single global chat-run slot, checks the model lease and
/// runtime endpoint, then spawns one orchestrator thread that runs
/// `run_chat_stream` and forwards every event through `channel`. Returns
/// as soon as the slot is reserved and preconditions pass — before the
/// spawned thread produces its first event — so the frontend's channel is
/// always bound before any event can arrive. See design.md's "Model lease
/// and run ownership" and "Stream lifecycle, limits and cancellation".
#[tauri::command]
fn start_chat_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: ChatRequest,
    channel: tauri::ipc::Channel<ChatStreamEvent>,
) -> Result<ChatRunHandle, AppError> {
    let run_id = new_chat_run_id();
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let mut active_chat_run = state.active_chat_run.lock().map_err(|_| {
            AppError::storage_unavailable("Lattice could not access chat run state.")
        })?;
        if active_chat_run.is_some() {
            return Err(AppError::chat_conflict("A response is already streaming."));
        }
        *active_chat_run = Some(ActiveChatRun {
            run_id: run_id.clone(),
            cancel: cancel.clone(),
        });
    }

    match begin_chat_stream(&state, &request) {
        Ok(endpoint) => {
            let handle = ChatRunHandle {
                run_id: run_id.clone(),
            };
            let guard_run_id = run_id.clone();
            thread::spawn(move || {
                // Cleared on every exit path of this closure, including an
                // unexpected panic unwinding through it: `Drop` runs during
                // unwinding, so the active-run slot is never left stuck even
                // if `run_chat_stream`/`channel.send` panics unexpectedly.
                let _guard = ActiveChatRunGuard {
                    app,
                    run_id: guard_run_id,
                };
                run_chat_stream(&endpoint, run_id, request, &cancel, |event| {
                    let _ = channel.send(event);
                });
            });
            Ok(handle)
        }
        Err(error) => {
            clear_active_chat_run(&state, &run_id);
            Err(error)
        }
    }
}

/// RAII guard that clears the active-run slot when dropped — on normal
/// return from the spawning thread's closure, or, just as importantly,
/// while unwinding through it after an unexpected panic. See
/// `start_chat_stream`.
struct ActiveChatRunGuard {
    app: tauri::AppHandle,
    run_id: String,
}

impl Drop for ActiveChatRunGuard {
    fn drop(&mut self) {
        if let Some(state) = self.app.try_state::<DesktopState>() {
            clear_active_chat_run(&state, &self.run_id);
        }
    }
}

/// Fast, non-mutating precondition: the requested model must be the one
/// currently owned and loaded, and the runtime must expose a reachable
/// endpoint. No subprocess/network call beyond the existing settings-store
/// reads `get_model_slot_status`/`read_model_runtime_status` already make.
fn begin_chat_stream(
    state: &tauri::State<'_, DesktopState>,
    request: &ChatRequest,
) -> Result<String, AppError> {
    let slot_status = with_settings_store(state, |store| {
        store.get_model_slot_status(GetModelSlotStatusRequest {})
    })?;
    authorize_chat_request(&slot_status, request)?;

    let runtime_status = with_settings_store(state, |store| store.read_model_runtime_status())?;
    runtime_status.server.endpoint.ok_or(AppError::chat_invalid(
        "The local model runtime has no reachable endpoint.",
    ))
}

/// Clears the active-run slot only if it still names `run_id`, so a stale
/// clear from an abandoned reader thread can never clear a newer run.
fn clear_active_chat_run(state: &DesktopState, run_id: &str) {
    let Ok(mut active_chat_run) = state.active_chat_run.lock() else {
        return;
    };
    if active_chat_run
        .as_ref()
        .map(|active| active.run_id.as_str())
        == Some(run_id)
    {
        *active_chat_run = None;
    }
}

#[tauri::command]
fn cancel_chat_stream(
    state: tauri::State<'_, DesktopState>,
    request: CancelChatStreamRequest,
) -> Result<(), AppError> {
    let active_chat_run = state
        .active_chat_run
        .lock()
        .map_err(|_| AppError::storage_unavailable("Lattice could not access chat run state."))?;
    if let Some(active) = active_chat_run.as_ref() {
        if active.run_id == request.run_id {
            active.cancel.store(true, Ordering::SeqCst);
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = run_desktop_shell() {
        exit_after_startup_failure(&error);
    }
}

fn run_desktop_shell() -> Result<(), tauri::Error> {
    let app = tauri::Builder::default()
        .setup(|app| {
            let settings_path = settings_database_path(app)?;
            let settings = SettingsStore::open(settings_path)?;
            app.manage(DesktopState {
                settings: Mutex::new(settings),
                runtime_operation_cancelled: Arc::new(AtomicBool::new(false)),
                model_operation_cancelled: Arc::new(AtomicBool::new(false)),
                active_chat_run: Mutex::new(None),
            });
            Ok(())
        })
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_app_settings,
            update_app_settings,
            reset_app_settings,
            get_model_runtime_status,
            configure_model_runtime,
            probe_model_runtime,
            start_model_runtime,
            stop_model_runtime,
            cancel_model_runtime_operation,
            get_model_slot_status,
            load_model,
            unload_model,
            cancel_model_operation,
            start_chat_stream,
            cancel_chat_stream
        ])
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            cancel_active_chat_run_before_exit(app_handle);
            stop_owned_runtime_before_exit(app_handle);
        }
    });

    Ok(())
}

/// Best-effort cancel signal for any in-flight chat run, attempted right
/// before the application exits. Never blocks exit: the abandoned reader
/// thread (see design.md's "Stream lifecycle, limits and cancellation") is
/// simply dropped along with the rest of the process; this only spares it
/// from attempting a corrective action it can no longer report anywhere.
fn cancel_active_chat_run_before_exit(app_handle: &tauri::AppHandle) {
    let Some(state) = app_handle.try_state::<DesktopState>() else {
        return;
    };
    let Ok(active_chat_run) = state.active_chat_run.lock() else {
        return;
    };
    if let Some(active) = active_chat_run.as_ref() {
        active.cancel.store(true, Ordering::SeqCst);
    }
}

/// Best-effort stop of a runtime this session owns, attempted right before
/// the application exits. Never delays or blocks exit: a missing state, a
/// poisoned lock, or a stop that cannot complete within its short bounded
/// deadline all simply mean nothing is stopped here.
fn stop_owned_runtime_before_exit(app_handle: &tauri::AppHandle) {
    let Some(state) = app_handle.try_state::<DesktopState>() else {
        return;
    };
    let Ok(mut settings) = state.settings.lock() else {
        return;
    };
    settings.stop_owned_model_runtime_for_shutdown();
}

fn settings_database_path(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(app.path().app_data_dir()?.join(SETTINGS_DATABASE_FILE))
}

fn with_settings_store<T>(
    state: &tauri::State<'_, DesktopState>,
    operation: impl FnOnce(&mut SettingsStore) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let mut settings = state.settings.lock().map_err(|_| {
        AppError::storage_unavailable("Lattice could not access local settings storage.")
    })?;

    operation(&mut settings)
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
    use lattice_core::{
        AppRuntime, CANCEL_CHAT_STREAM_COMMAND, CANCEL_MODEL_OPERATION_COMMAND,
        CANCEL_MODEL_RUNTIME_OPERATION_COMMAND, CONFIGURE_MODEL_RUNTIME_COMMAND,
        GET_APP_INFO_COMMAND, GET_APP_SETTINGS_COMMAND, GET_MODEL_RUNTIME_STATUS_COMMAND,
        GET_MODEL_SLOT_STATUS_COMMAND, LOAD_MODEL_COMMAND, PROBE_MODEL_RUNTIME_COMMAND,
        RESET_APP_SETTINGS_COMMAND, START_CHAT_STREAM_COMMAND, START_MODEL_RUNTIME_COMMAND,
        STOP_MODEL_RUNTIME_COMMAND, UNLOAD_MODEL_COMMAND, UPDATE_APP_SETTINGS_COMMAND,
    };
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
        assert_eq!(
            [
                GET_APP_INFO_COMMAND,
                GET_APP_SETTINGS_COMMAND,
                UPDATE_APP_SETTINGS_COMMAND,
                RESET_APP_SETTINGS_COMMAND,
                GET_MODEL_RUNTIME_STATUS_COMMAND,
                CONFIGURE_MODEL_RUNTIME_COMMAND,
                PROBE_MODEL_RUNTIME_COMMAND,
                START_MODEL_RUNTIME_COMMAND,
                STOP_MODEL_RUNTIME_COMMAND,
                CANCEL_MODEL_RUNTIME_OPERATION_COMMAND,
                GET_MODEL_SLOT_STATUS_COMMAND,
                LOAD_MODEL_COMMAND,
                UNLOAD_MODEL_COMMAND,
                CANCEL_MODEL_OPERATION_COMMAND,
                START_CHAT_STREAM_COMMAND,
                CANCEL_CHAT_STREAM_COMMAND
            ],
            [
                "get_app_info",
                "get_app_settings",
                "update_app_settings",
                "reset_app_settings",
                "get_model_runtime_status",
                "configure_model_runtime",
                "probe_model_runtime",
                "start_model_runtime",
                "stop_model_runtime",
                "cancel_model_runtime_operation",
                "get_model_slot_status",
                "load_model",
                "unload_model",
                "cancel_model_operation",
                "start_chat_stream",
                "cancel_chat_stream"
            ]
        );
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
            vec![
                "allow-get-app-info".to_string(),
                "allow-application-settings".to_string(),
                "allow-model-runtime-discovery".to_string(),
                "allow-model-runtime-lifecycle".to_string(),
                "allow-local-models".to_string(),
                "allow-chat-streaming".to_string()
            ]
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
