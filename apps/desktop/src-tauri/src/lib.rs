use lattice_core::{
    app_info, authorize_chat_request, new_chat_run_id, run_chat_stream, AppError, AppInfo,
    AppSettings, CancelChatStreamRequest, CancelModelOperationRequest,
    CancelModelRuntimeOperationRequest, ChatRequest, ChatRunHandle, ChatStreamEvent,
    ConfigureModelRuntimeRequest, ConversationDetail, ConversationStore, CreateCredentialRequest,
    CredentialRef, CredentialStore, DeleteConversationRequest, DeleteCredentialRequest,
    GenerationStatus, GetConversationRequest, GetModelSlotStatusRequest, ListConversationsRequest,
    ListConversationsResponse, LoadModelRequest, ModelRuntimeStatus, ModelSlotStatus,
    ProbeModelRuntimeRequest, ReplaceCredentialRequest, ResetAppSettingsRequest, SettingsStore,
    StartChatStreamRequest, StartModelRuntimeRequest, StopModelRuntimeRequest, UnloadModelRequest,
    UpdateAppSettingsRequest, CHECKPOINT_DELTA_BATCH, CHECKPOINT_MIN_INTERVAL,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Instant,
};
use tauri::Manager;

const STARTUP_FAILURE_EXIT_CODE: i32 = 1;
const SETTINGS_DATABASE_FILE: &str = "lattice.sqlite3";

struct DesktopState {
    settings: Mutex<SettingsStore>,
    conversations: Mutex<ConversationStore>,
    credentials: Mutex<CredentialStore>,
    runtime_operation_cancelled: Arc<AtomicBool>,
    model_operation_cancelled: Arc<AtomicBool>,
    active_chat_run: Mutex<Option<ActiveChatRun>>,
}

/// One globally active chat run's identity, the conversation it belongs
/// to, and its shared cancellation flag, mirroring
/// `runtime_operation_cancelled`/`model_operation_cancelled`'s existing
/// pattern but keyed by run so a stale/unknown `run_id` passed to
/// `cancel_chat_stream` is a no-op rather than cancelling the wrong run.
/// `conversation_id` starts empty for the brief window between reserving
/// this slot and `begin_or_continue` resolving the real ID (see
/// `start_chat_stream`); an empty string never matches a real conversation
/// ID, so `delete_conversation`'s conflict check stays correct throughout.
struct ActiveChatRun {
    run_id: String,
    conversation_id: String,
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
/// runtime endpoint, resolves/creates the conversation and persists its new
/// message(s), starts the streaming assistant message row, then spawns one
/// orchestrator thread that runs `run_chat_stream` and forwards every event
/// through `channel` while also checkpointing it into conversation storage.
/// Returns as soon as the slot is reserved and preconditions pass — before
/// the spawned thread produces its first event — so the frontend's channel
/// is always bound before any event can arrive. See design.md's "Model
/// lease and run ownership" and "Stream lifecycle, limits and cancellation"
/// (0.8), and `conversation-persistence`'s design.md "Checkpointing and
/// message persistence" (0.9).
#[tauri::command]
fn start_chat_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopState>,
    request: StartChatStreamRequest,
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
            conversation_id: String::new(),
            cancel: cancel.clone(),
        });
    }

    match begin_or_start_chat_stream(&state, &request) {
        Ok((endpoint, conversation_id, message_id)) => {
            if let Ok(mut active_chat_run) = state.active_chat_run.lock() {
                if let Some(active) = active_chat_run.as_mut() {
                    if active.run_id == run_id {
                        active.conversation_id.clone_from(&conversation_id);
                    }
                }
            }

            let handle = ChatRunHandle {
                run_id: run_id.clone(),
                conversation_id: conversation_id.clone(),
            };
            let guard_run_id = run_id.clone();
            let checkpoint_app = app.clone();
            thread::spawn(move || {
                // Cleared on every exit path of this closure, including an
                // unexpected panic unwinding through it: `Drop` runs during
                // unwinding, so the active-run slot is never left stuck even
                // if `run_chat_stream`/`channel.send` panics unexpectedly.
                let _guard = ActiveChatRunGuard {
                    app,
                    run_id: guard_run_id,
                };
                let mut checkpoint = ChatCheckpointSink::new(checkpoint_app, message_id);
                run_chat_stream(&endpoint, run_id, request.chat, &cancel, move |event| {
                    checkpoint.observe(&event);
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

/// Checks preconditions exactly as 0.8's `begin_chat_stream` did, then (only
/// once they pass) persists the request's new trailing message(s) against
/// `conversation_id` — creating a new conversation when it is `None` — and
/// starts the empty `streaming` assistant message row. No conversation or
/// message is ever written for a request that fails its preconditions.
fn begin_or_start_chat_stream(
    state: &tauri::State<'_, DesktopState>,
    request: &StartChatStreamRequest,
) -> Result<(String, String, String), AppError> {
    let endpoint = begin_chat_stream(state, &request.chat)?;

    let conversation_id = with_conversation_store(state, |store| {
        store.begin_or_continue(request.conversation_id.as_deref(), &request.chat.messages)
    })?;
    let message_id = with_conversation_store(state, |store| {
        store.start_assistant_message(&conversation_id, &request.chat.model_key)
    })?;

    Ok((endpoint, conversation_id, message_id))
}

/// Accumulates a streaming assistant reply and checkpoints it into
/// conversation storage at bounded intervals (never per token), always
/// flushing synchronously on the run's terminal event. Runs on the
/// orchestrator thread `start_chat_stream` spawns, so it reaches managed
/// state through a cloned `AppHandle` rather than a borrowed `tauri::State`
/// — the same pattern `ActiveChatRunGuard`/`stop_owned_runtime_before_exit`
/// already use to touch `DesktopState` from outside a command call.
struct ChatCheckpointSink {
    app: tauri::AppHandle,
    message_id: String,
    accumulated_text: String,
    deltas_since_checkpoint: u32,
    last_checkpoint_at: Instant,
}

impl ChatCheckpointSink {
    fn new(app: tauri::AppHandle, message_id: String) -> Self {
        Self {
            app,
            message_id,
            accumulated_text: String::new(),
            deltas_since_checkpoint: 0,
            last_checkpoint_at: Instant::now(),
        }
    }

    fn observe(&mut self, event: &ChatStreamEvent) {
        match event {
            ChatStreamEvent::Started { .. } => {}
            ChatStreamEvent::Delta { text, .. } => {
                self.accumulated_text.push_str(text);
                self.deltas_since_checkpoint += 1;
                if self.deltas_since_checkpoint >= CHECKPOINT_DELTA_BATCH
                    || self.last_checkpoint_at.elapsed() >= CHECKPOINT_MIN_INTERVAL
                {
                    self.checkpoint();
                }
            }
            ChatStreamEvent::Completed { .. } => self.finalize(GenerationStatus::Complete, None),
            ChatStreamEvent::Cancelled { .. } => self.finalize(GenerationStatus::Cancelled, None),
            ChatStreamEvent::Failed { error, .. } => {
                self.finalize(GenerationStatus::Failed, Some(error.message));
            }
        }
    }

    fn checkpoint(&mut self) {
        self.deltas_since_checkpoint = 0;
        self.last_checkpoint_at = Instant::now();
        let Some(state) = self.app.try_state::<DesktopState>() else {
            return;
        };
        let Ok(store) = state.conversations.lock() else {
            return;
        };
        let _ = store.checkpoint_assistant_message(&self.message_id, &self.accumulated_text);
    }

    /// Always synchronous, regardless of the last checkpoint's timing — the
    /// terminal write is never itself batched or skipped.
    fn finalize(&mut self, status: GenerationStatus, error_message: Option<&'static str>) {
        let Some(state) = self.app.try_state::<DesktopState>() else {
            return;
        };
        let Ok(mut store) = state.conversations.lock() else {
            return;
        };
        let _ = store.finalize_assistant_message(
            &self.message_id,
            &self.accumulated_text,
            status,
            error_message,
        );
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

#[tauri::command]
fn list_conversations(
    state: tauri::State<'_, DesktopState>,
    request: ListConversationsRequest,
) -> Result<ListConversationsResponse, AppError> {
    with_conversation_store(&state, |store| store.list(request))
}

#[tauri::command]
fn get_conversation(
    state: tauri::State<'_, DesktopState>,
    request: GetConversationRequest,
) -> Result<ConversationDetail, AppError> {
    with_conversation_store(&state, |store| store.get(request))
}

/// Refuses the delete without touching storage when `conversation_id`
/// names the conversation currently streaming — the one case a deleted
/// conversation could otherwise vanish out from under an in-flight write.
#[tauri::command]
fn delete_conversation(
    state: tauri::State<'_, DesktopState>,
    request: DeleteConversationRequest,
) -> Result<(), AppError> {
    let active_conversation_id = state
        .active_chat_run
        .lock()
        .map_err(|_| AppError::storage_unavailable("Lattice could not access chat run state."))?
        .as_ref()
        .map(|active| active.conversation_id.clone());

    if active_conversation_id.as_deref() == Some(request.conversation_id.as_str()) {
        return Err(AppError::conversation_conflict(
            "Cancel the active chat before deleting this conversation.",
        ));
    }

    with_conversation_store(&state, |store| store.delete(request))
}

#[tauri::command]
fn list_credentials(state: tauri::State<'_, DesktopState>) -> Result<Vec<CredentialRef>, AppError> {
    with_credential_store(&state, |store| store.list())
}

/// Blocking, bounded by the native prompt's own ~125s timeout — no new
/// threading model, matching how `configure_model_runtime`/`probe_model_runtime`
/// already block synchronously on a bounded subprocess call.
#[tauri::command]
fn create_credential(
    state: tauri::State<'_, DesktopState>,
    request: CreateCredentialRequest,
) -> Result<CredentialRef, AppError> {
    with_credential_store(&state, |store| store.create(request))
}

#[tauri::command]
fn replace_credential(
    state: tauri::State<'_, DesktopState>,
    request: ReplaceCredentialRequest,
) -> Result<CredentialRef, AppError> {
    with_credential_store(&state, |store| store.replace(request))
}

#[tauri::command]
fn delete_credential(
    state: tauri::State<'_, DesktopState>,
    request: DeleteCredentialRequest,
) -> Result<(), AppError> {
    with_credential_store(&state, |store| store.delete(request))
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
            let settings = SettingsStore::open(&settings_path)?;
            // Same file as `settings` above, per `ConversationStore`'s own
            // module doc comment: a second, independent connection, not a
            // second database. Opened after `settings` so the migration
            // cascade (schema 4 -> 5) runs exactly once, from whichever
            // store opens first; this one then only takes the existing
            // "schema already current" read-validation branch.
            let conversations = ConversationStore::open(&settings_path)?;
            // Same file again, third independent connection, same reason —
            // see `CredentialStore`'s own module doc comment. Only
            // reference metadata lives in this file; the secret itself
            // never does (see `credentials`'s module doc comment).
            let credentials = CredentialStore::open(&settings_path)?;
            app.manage(DesktopState {
                settings: Mutex::new(settings),
                conversations: Mutex::new(conversations),
                credentials: Mutex::new(credentials),
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
            cancel_chat_stream,
            list_conversations,
            get_conversation,
            delete_conversation,
            list_credentials,
            create_credential,
            replace_credential,
            delete_credential
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

fn with_conversation_store<T>(
    state: &tauri::State<'_, DesktopState>,
    operation: impl FnOnce(&mut ConversationStore) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let mut conversations = state.conversations.lock().map_err(|_| {
        AppError::storage_unavailable("Lattice could not access local conversation storage.")
    })?;

    operation(&mut conversations)
}

fn with_credential_store<T>(
    state: &tauri::State<'_, DesktopState>,
    operation: impl FnOnce(&mut CredentialStore) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let mut credentials = state.credentials.lock().map_err(|_| {
        AppError::storage_unavailable("Lattice could not access local credential storage.")
    })?;

    operation(&mut credentials)
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
        CREATE_CREDENTIAL_COMMAND, DELETE_CONVERSATION_COMMAND, DELETE_CREDENTIAL_COMMAND,
        GET_APP_INFO_COMMAND, GET_APP_SETTINGS_COMMAND, GET_CONVERSATION_COMMAND,
        GET_MODEL_RUNTIME_STATUS_COMMAND, GET_MODEL_SLOT_STATUS_COMMAND,
        LIST_CONVERSATIONS_COMMAND, LIST_CREDENTIALS_COMMAND, LOAD_MODEL_COMMAND,
        PROBE_MODEL_RUNTIME_COMMAND, REPLACE_CREDENTIAL_COMMAND, RESET_APP_SETTINGS_COMMAND,
        START_CHAT_STREAM_COMMAND, START_MODEL_RUNTIME_COMMAND, STOP_MODEL_RUNTIME_COMMAND,
        UNLOAD_MODEL_COMMAND, UPDATE_APP_SETTINGS_COMMAND,
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
                CANCEL_CHAT_STREAM_COMMAND,
                LIST_CONVERSATIONS_COMMAND,
                GET_CONVERSATION_COMMAND,
                DELETE_CONVERSATION_COMMAND,
                LIST_CREDENTIALS_COMMAND,
                CREATE_CREDENTIAL_COMMAND,
                REPLACE_CREDENTIAL_COMMAND,
                DELETE_CREDENTIAL_COMMAND
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
                "cancel_chat_stream",
                "list_conversations",
                "get_conversation",
                "delete_conversation",
                "list_credentials",
                "create_credential",
                "replace_credential",
                "delete_credential"
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
                "allow-chat-streaming".to_string(),
                "allow-conversations".to_string(),
                "allow-credentials".to_string()
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
