export type AppRuntime = 'tauri' | 'web';

export interface AppInfo {
  readonly name: string;
  readonly version: string;
  readonly buildProfile: 'debug' | 'release';
  readonly runtime: AppRuntime;
  readonly target: string;
}

export interface AppError {
  readonly code: string;
  readonly message: string;
  readonly recoverable: boolean;
}

export interface BridgeStatus {
  readonly runtime: AppRuntime;
  readonly ipc: 'available' | 'unavailable';
  readonly message: string;
}
