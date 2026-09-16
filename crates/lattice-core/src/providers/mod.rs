mod completion;
mod openai_compatible;
mod profiles;

pub use completion::{
    authorize_chat_request, new_chat_run_id, run_chat_stream, CancelChatStreamRequest,
    ChatFinishReason, ChatMessage, ChatRequest, ChatRole, ChatRunHandle, ChatStreamEvent,
    ChatTarget, CompletionTarget, StartChatStreamRequest, CANCEL_CHAT_STREAM_COMMAND,
    MAX_OUTPUT_TOKENS, MAX_PROMPT_CHARS, ORCHESTRATOR_POLL_INTERVAL, START_CHAT_STREAM_COMMAND,
    STREAM_DEADLINE,
};
pub use profiles::{
    new_provider_profile_id, prepare_remote_target, BindProviderCredentialRequest,
    CreateProviderProfileRequest, DeleteProviderProfileRequest, GrantProviderConsentRequest,
    ProviderConsent, ProviderProfile, RevokeProviderConsentRequest, UpdateProviderProfileRequest,
    BIND_PROVIDER_CREDENTIAL_COMMAND, CREATE_PROVIDER_PROFILE_COMMAND,
    DELETE_PROVIDER_PROFILE_COMMAND, GRANT_PROVIDER_CONSENT_COMMAND,
    LIST_PROVIDER_PROFILES_COMMAND, REMOTE_PROVIDER_KEY, REVOKE_PROVIDER_CONSENT_COMMAND,
    UPDATE_PROVIDER_PROFILE_COMMAND,
};
pub(crate) use profiles::{
    validate_credential_id, validate_endpoint, validate_model_key, validate_profile_label,
    MAX_PROVIDER_PROFILES,
};
