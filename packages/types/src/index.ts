export {
  APP_COMMANDS,
  type AppCommand,
  type AppError,
  type BuildProfile,
  type NativeAppInfo,
  type NativeAppRuntime
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
