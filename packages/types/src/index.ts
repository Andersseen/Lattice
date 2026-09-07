export {
  APP_COMMANDS,
  type AppearancePreference,
  type AppCommand,
  type AppError,
  type AppSettings,
  type BuildProfile,
  type ConfigureModelRuntimeRequest,
  type NativeAppInfo,
  type NativeAppRuntime,
  type ModelRuntimeAvailability,
  type ModelRuntimeStatus,
  type ProbeModelRuntimeRequest,
  type ResetAppSettingsRequest,
  type RuntimeDaemonObservation,
  type RuntimeDaemonStatus,
  type RuntimeProbeApproval,
  type RuntimeServerObservation,
  type RuntimeServerStatus,
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
