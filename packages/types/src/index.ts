export {
  APP_COMMANDS,
  type AppCommand,
  type AppError,
  type AppearancePreference,
  type AppSettings,
  type BuildProfile,
  type CancelChatStreamRequest,
  type CancelModelOperationRequest,
  type CancelModelRuntimeOperationRequest,
  type ChatFinishReason,
  type ChatMessage,
  type ChatRequest,
  type ChatRole,
  type ChatRunHandle,
  type ChatStreamEvent,
  type ChatStreamEventCancelled,
  type ChatStreamEventCompleted,
  type ChatStreamEventDelta,
  type ChatStreamEventFailed,
  type ChatStreamEventStarted,
  type ConfigureModelRuntimeRequest,
  type Conversation,
  type ConversationCursor,
  type ConversationDetail,
  type ConversationSummary,
  type CreateCredentialRequest,
  type CredentialAvailability,
  type CredentialRef,
  type DeleteConversationRequest,
  type DeleteCredentialRequest,
  type GenerationStatus,
  type GetConversationRequest,
  type GetModelSlotStatusRequest,
  type ListConversationsRequest,
  type ListConversationsResponse,
  type LoadedModelObservation,
  type LoadModelRequest,
  type Message,
  type ModelDescriptor,
  type ModelLoadOwnership,
  type ModelLoadOwnershipAttached,
  type ModelLoadOwnershipOwned,
  type ModelLoadOwnershipUnknown,
  type ModelOperationOutcome,
  type ModelRuntimeAvailability,
  type ModelRuntimeStatus,
  type ModelSlotStatus,
  type NativeAppInfo,
  type NativeAppRuntime,
  type ProbeModelRuntimeRequest,
  type ReplaceCredentialRequest,
  type ResetAppSettingsRequest,
  type RuntimeDaemonObservation,
  type RuntimeDaemonStatus,
  type RuntimeOperationOutcome,
  type RuntimeOwnership,
  type RuntimeOwnershipAttached,
  type RuntimeOwnershipOwned,
  type RuntimeOwnershipUnknown,
  type RuntimeProbeApproval,
  type RuntimeServerObservation,
  type RuntimeServerStatus,
  type StartChatStreamRequest,
  type StartModelRuntimeRequest,
  type StopModelRuntimeRequest,
  type UnloadModelRequest,
  type UpdateAppSettingsRequest
} from './generated';

import type { NativeAppInfo, NativeAppRuntime } from './generated';

export type WebAppRuntime = 'web';

export type AppRuntime = NativeAppRuntime | WebAppRuntime;

export type AppInfo = Omit<NativeAppInfo, 'runtime'> & {
  readonly runtime: AppRuntime;
};

export interface BridgeStatus {
  readonly runtime: AppRuntime;
  readonly ipc: 'available' | 'unavailable';
  readonly message: string;
}
