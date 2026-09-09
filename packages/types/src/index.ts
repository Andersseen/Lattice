export {
  APP_COMMANDS,
  type AppearancePreference,
  type AppCommand,
  type AppError,
  type AppSettings,
  type BuildProfile,
  type CancelModelRuntimeOperationRequest,
  type ConfigureModelRuntimeRequest,
  type NativeAppInfo,
  type NativeAppRuntime,
  type ModelRuntimeAvailability,
  type ModelRuntimeStatus,
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
