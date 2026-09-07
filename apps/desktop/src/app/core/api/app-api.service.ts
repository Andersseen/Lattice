import { Injectable } from '@angular/core';
import {
  APP_COMMANDS,
  type AppError,
  type AppInfo,
  type AppSettings,
  type ConfigureModelRuntimeRequest,
  type ModelRuntimeStatus,
  type ProbeModelRuntimeRequest,
  type ResetAppSettingsRequest,
  type UpdateAppSettingsRequest
} from '@lattice/types';

import {
  decodeAppInfo,
  decodeAppSettings,
  decodeModelRuntimeStatus,
  normalizeAppError,
  normalizeModelRuntimeError,
  normalizeSettingsError
} from './app-wire';
import { getWebSettings, resetWebSettings, updateWebSettings } from './app-settings-fallback';
import { createWebFallbackInfo } from './app-info-fallback';
import {
  configureWebModelRuntime,
  getWebModelRuntimeStatus,
  probeWebModelRuntime
} from './model-runtime-fallback';
import { isTauriRuntime } from './tauri-runtime';

@Injectable({ providedIn: 'root' })
export class AppApiService {
  async getAppInfo(): Promise<AppInfo> {
    if (!isTauriRuntime()) {
      return createWebFallbackInfo();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeAppInfo(await invoke<unknown>(APP_COMMANDS.getAppInfo));
    } catch (error: unknown) {
      throw normalizeBridgeError(error);
    }
  }

  async getAppSettings(): Promise<AppSettings> {
    if (!isTauriRuntime()) {
      return getWebSettings();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeAppSettings(await invoke<unknown>(APP_COMMANDS.getAppSettings));
    } catch (error: unknown) {
      throw normalizeSettingsError(error);
    }
  }

  async updateAppSettings(request: UpdateAppSettingsRequest): Promise<AppSettings> {
    if (!isTauriRuntime()) {
      return updateWebSettings(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeAppSettings(
        await invoke<unknown>(APP_COMMANDS.updateAppSettings, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeSettingsError(error);
    }
  }

  async resetAppSettings(request: ResetAppSettingsRequest): Promise<AppSettings> {
    if (!isTauriRuntime()) {
      return resetWebSettings(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeAppSettings(
        await invoke<unknown>(APP_COMMANDS.resetAppSettings, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeSettingsError(error);
    }
  }

  async getModelRuntimeStatus(): Promise<ModelRuntimeStatus> {
    if (!isTauriRuntime()) {
      return getWebModelRuntimeStatus();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelRuntimeStatus(await invoke<unknown>(APP_COMMANDS.getModelRuntimeStatus));
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }

  async configureModelRuntime(request: ConfigureModelRuntimeRequest): Promise<ModelRuntimeStatus> {
    if (!isTauriRuntime()) {
      return configureWebModelRuntime(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelRuntimeStatus(
        await invoke<unknown>(APP_COMMANDS.configureModelRuntime, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }

  async probeModelRuntime(request: ProbeModelRuntimeRequest): Promise<ModelRuntimeStatus> {
    if (!isTauriRuntime()) {
      return probeWebModelRuntime(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelRuntimeStatus(
        await invoke<unknown>(APP_COMMANDS.probeModelRuntime, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }
}

function normalizeBridgeError(error: unknown): AppError {
  return normalizeAppError(error);
}
