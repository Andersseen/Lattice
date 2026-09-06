import { Injectable } from '@angular/core';
import type { AppError, AppInfo } from '@lattice/types';

import { createWebFallbackInfo } from './app-info-fallback';
import { isTauriRuntime } from './tauri-runtime';

@Injectable({ providedIn: 'root' })
export class AppApiService {
  async getAppInfo(): Promise<AppInfo> {
    if (!isTauriRuntime()) {
      return createWebFallbackInfo();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<AppInfo>('get_app_info');
    } catch (error: unknown) {
      throw normalizeBridgeError(error);
    }
  }
}

function normalizeBridgeError(error: unknown): AppError {
  if (isAppError(error)) {
    return error;
  }

  return {
    code: 'bridge.unknown',
    message: 'Lattice could not read application information.',
    recoverable: true
  };
}

function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<AppError>;
  return (
    typeof candidate.code === 'string' &&
    typeof candidate.message === 'string' &&
    typeof candidate.recoverable === 'boolean'
  );
}
