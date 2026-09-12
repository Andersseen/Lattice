import { Injectable } from '@angular/core';
import {
  APP_COMMANDS,
  type AppError,
  type AppInfo,
  type AppSettings,
  type CancelModelOperationRequest,
  type CancelModelRuntimeOperationRequest,
  type ConfigureModelRuntimeRequest,
  type LoadModelRequest,
  type ModelRuntimeStatus,
  type ModelSlotStatus,
  type ProbeModelRuntimeRequest,
  type ResetAppSettingsRequest,
  type StartModelRuntimeRequest,
  type StopModelRuntimeRequest,
  type UnloadModelRequest,
  type UpdateAppSettingsRequest
} from '@lattice/types';

import {
  decodeAppInfo,
  decodeAppSettings,
  decodeModelRuntimeStatus,
  decodeModelSlotStatus,
  normalizeAppError,
  normalizeModelRuntimeError,
  normalizeModelSlotError,
  normalizeSettingsError
} from './app-wire';
import { getWebSettings, resetWebSettings, updateWebSettings } from './app-settings-fallback';
import { createWebFallbackInfo } from './app-info-fallback';
import {
  cancelWebModelRuntimeOperation,
  configureWebModelRuntime,
  getWebModelRuntimeStatus,
  probeWebModelRuntime,
  startWebModelRuntime,
  stopWebModelRuntime
} from './model-runtime-fallback';
import {
  cancelWebModelOperation,
  getWebModelSlotStatus,
  loadWebModel,
  unloadWebModel
} from './model-slot-fallback';
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

  async startModelRuntime(request: StartModelRuntimeRequest): Promise<ModelRuntimeStatus> {
    if (!isTauriRuntime()) {
      return startWebModelRuntime(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelRuntimeStatus(
        await invoke<unknown>(APP_COMMANDS.startModelRuntime, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }

  async stopModelRuntime(request: StopModelRuntimeRequest): Promise<ModelRuntimeStatus> {
    if (!isTauriRuntime()) {
      return stopWebModelRuntime(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelRuntimeStatus(
        await invoke<unknown>(APP_COMMANDS.stopModelRuntime, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }

  async cancelModelRuntimeOperation(request: CancelModelRuntimeOperationRequest): Promise<void> {
    if (!isTauriRuntime()) {
      cancelWebModelRuntimeOperation();
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.cancelModelRuntimeOperation, {
        request
      });
    } catch (error: unknown) {
      throw normalizeModelRuntimeError(error);
    }
  }

  async getModelSlotStatus(): Promise<ModelSlotStatus> {
    if (!isTauriRuntime()) {
      return getWebModelSlotStatus();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelSlotStatus(await invoke<unknown>(APP_COMMANDS.getModelSlotStatus));
    } catch (error: unknown) {
      throw normalizeModelSlotError(error);
    }
  }

  async loadModel(request: LoadModelRequest): Promise<ModelSlotStatus> {
    if (!isTauriRuntime()) {
      return loadWebModel(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelSlotStatus(
        await invoke<unknown>(APP_COMMANDS.loadModel, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelSlotError(error);
    }
  }

  async unloadModel(request: UnloadModelRequest): Promise<ModelSlotStatus> {
    if (!isTauriRuntime()) {
      return unloadWebModel(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeModelSlotStatus(
        await invoke<unknown>(APP_COMMANDS.unloadModel, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeModelSlotError(error);
    }
  }

  async cancelModelOperation(request: CancelModelOperationRequest): Promise<void> {
    if (!isTauriRuntime()) {
      cancelWebModelOperation();
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.cancelModelOperation, {
        request
      });
    } catch (error: unknown) {
      throw normalizeModelSlotError(error);
    }
  }
}

function normalizeBridgeError(error: unknown): AppError {
  return normalizeAppError(error);
}
