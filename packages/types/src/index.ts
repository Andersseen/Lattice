export {
  APP_COMMANDS,
  type AppearancePreference,
  type AppCommand,
  type AppError,
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
  type GetModelSlotStatusRequest,
  type LoadModelRequest,
  type LoadedModelObservation,
  type NativeAppInfo,
  type NativeAppRuntime,
  type ModelDescriptor,
  type ModelLoadOwnership,
  type ModelLoadOwnershipAttached,
  type ModelLoadOwnershipOwned,
  type ModelLoadOwnershipUnknown,
  type ModelOperationOutcome,
  type ModelRuntimeAvailability,
  type ModelRuntimeStatus,
  type ModelSlotStatus,
  type ProbeModelRuntimeRequest,
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
