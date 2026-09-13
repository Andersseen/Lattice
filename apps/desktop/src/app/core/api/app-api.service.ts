import { Injectable } from '@angular/core';
import {
  APP_COMMANDS,
  type AppError,
  type AppInfo,
  type AppSettings,
  type CancelChatStreamRequest,
  type CancelModelOperationRequest,
  type CancelModelRuntimeOperationRequest,
  type ChatRequest,
  type ChatRunHandle,
  type ChatStreamEvent,
  type ConfigureModelRuntimeRequest,
  type ConversationDetail,
  type DeleteConversationRequest,
  type GetConversationRequest,
  type ListConversationsRequest,
  type ListConversationsResponse,
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
  decodeChatRunHandle,
  decodeChatStreamEvent,
  decodeConversationDetail,
  decodeListConversationsResponse,
  decodeModelRuntimeStatus,
  decodeModelSlotStatus,
  normalizeAppError,
  normalizeChatError,
  normalizeConversationError,
  normalizeModelRuntimeError,
  normalizeModelSlotError,
  normalizeSettingsError
} from './app-wire';
import { getWebSettings, resetWebSettings, updateWebSettings } from './app-settings-fallback';
import { createWebFallbackInfo } from './app-info-fallback';
import { cancelWebChatStream, startWebChatStream } from './chat-fallback';
import {
  deleteWebConversation,
  getWebConversation,
  listWebConversations
} from './conversations-fallback';
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

  /**
   * `conversationId` is `null` to start a new conversation, or an existing
   * conversation's ID to continue it; the response's `conversationId` names
   * the resolved conversation either way. `onEvent` is called for every
   * event of the run, including its one terminal event; a malformed single
   * channel payload is skipped rather than thrown (see
   * `decodeChatStreamEvent`), so later valid events for the same run keep
   * arriving.
   */
  async startChatStream(
    conversationId: string | null,
    chat: ChatRequest,
    onEvent: (event: ChatStreamEvent) => void
  ): Promise<ChatRunHandle> {
    if (!isTauriRuntime()) {
      return startWebChatStream(conversationId, chat, onEvent);
    }

    try {
      const { invoke, Channel } = await import('@tauri-apps/api/core');
      const channel = new Channel<unknown>();
      channel.onmessage = (raw: unknown) => {
        const event = decodeChatStreamEvent(raw);
        if (event !== null) {
          onEvent(event);
        }
      };

      return decodeChatRunHandle(
        await invoke<unknown>(APP_COMMANDS.startChatStream, {
          request: { conversationId, chat },
          channel
        })
      );
    } catch (error: unknown) {
      throw normalizeChatError(error);
    }
  }

  async cancelChatStream(request: CancelChatStreamRequest): Promise<void> {
    if (!isTauriRuntime()) {
      cancelWebChatStream(request.runId);
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.cancelChatStream, {
        request
      });
    } catch (error: unknown) {
      throw normalizeChatError(error);
    }
  }

  async listConversations(request: ListConversationsRequest): Promise<ListConversationsResponse> {
    if (!isTauriRuntime()) {
      return listWebConversations(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeListConversationsResponse(
        await invoke<unknown>(APP_COMMANDS.listConversations, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeConversationError(error);
    }
  }

  async getConversation(request: GetConversationRequest): Promise<ConversationDetail> {
    if (!isTauriRuntime()) {
      return getWebConversation(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeConversationDetail(
        await invoke<unknown>(APP_COMMANDS.getConversation, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeConversationError(error);
    }
  }

  async deleteConversation(request: DeleteConversationRequest): Promise<void> {
    if (!isTauriRuntime()) {
      deleteWebConversation(request);
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.deleteConversation, {
        request
      });
    } catch (error: unknown) {
      throw normalizeConversationError(error);
    }
  }
}

function normalizeBridgeError(error: unknown): AppError {
  return normalizeAppError(error);
}
