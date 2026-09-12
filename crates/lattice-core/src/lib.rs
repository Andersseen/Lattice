mod app_info;
mod error;
mod model_runtime;
mod providers;
mod storage;
mod wire;

pub use app_info::{app_info, AppInfo, AppRuntime, BuildProfile, GET_APP_INFO_COMMAND};
pub use error::AppError;
pub use model_runtime::{
    list_loaded_models, CancelModelOperationRequest, CancelModelRuntimeOperationRequest,
    ConfigureModelRuntimeRequest, GetModelSlotStatusRequest, LoadModelRequest,
    LoadedModelObservation, ModelDescriptor, ModelLoadOwnership, ModelOperationOutcome,
    ModelRuntimeAvailability, ModelRuntimeStatus, ModelSlotStatus, ProbeModelRuntimeRequest,
    RuntimeDaemonObservation, RuntimeDaemonStatus, RuntimeOperationOutcome, RuntimeOwnership,
    RuntimeProbeApproval, RuntimeServerObservation, RuntimeServerStatus, StartModelRuntimeRequest,
    StopModelRuntimeRequest, UnloadModelRequest, CANCEL_MODEL_OPERATION_COMMAND,
    CANCEL_MODEL_RUNTIME_OPERATION_COMMAND, CONFIGURE_MODEL_RUNTIME_COMMAND,
    GET_MODEL_RUNTIME_STATUS_COMMAND, GET_MODEL_SLOT_STATUS_COMMAND, LOAD_MODEL_COMMAND,
    PROBE_MODEL_RUNTIME_COMMAND, SHUTDOWN_STOP_DEADLINE, START_MODEL_RUNTIME_COMMAND,
    STOP_MODEL_RUNTIME_COMMAND, UNLOAD_MODEL_COMMAND,
};
pub use providers::{
    authorize_chat_request, new_chat_run_id, run_chat_stream, CancelChatStreamRequest,
    ChatFinishReason, ChatMessage, ChatRequest, ChatRole, ChatRunHandle, ChatStreamEvent,
    CANCEL_CHAT_STREAM_COMMAND, MAX_OUTPUT_TOKENS, MAX_PROMPT_CHARS, ORCHESTRATOR_POLL_INTERVAL,
    START_CHAT_STREAM_COMMAND, STREAM_DEADLINE,
};
pub use storage::{
    AppSettings, AppearancePreference, ResetAppSettingsRequest, SettingsStore,
    UpdateAppSettingsRequest, GET_APP_SETTINGS_COMMAND, RESET_APP_SETTINGS_COMMAND,
    UPDATE_APP_SETTINGS_COMMAND,
};
pub use wire::typescript_bindings;
