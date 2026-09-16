import { Injectable } from '@angular/core';
import type {
  AppError,
  AppInfo,
  AppSettings,
  BindProviderCredentialRequest,
  CancelChatStreamRequest,
  CancelModelOperationRequest,
  CancelModelRuntimeOperationRequest,
  ChatRequest,
  ChatRunHandle,
  ChatStreamEvent,
  ChatTarget,
  ConfigureModelRuntimeRequest,
  ConversationDetail,
  CreateCredentialRequest,
  CreateProviderProfileRequest,
  CredentialRef,
  DeleteConversationRequest,
  DeleteCredentialRequest,
  DeleteProviderProfileRequest,
  GetConversationRequest,
  GrantProviderConsentRequest,
  ListConversationsRequest,
  ListConversationsResponse,
  LoadModelRequest,
  ModelRuntimeStatus,
  ModelSlotStatus,
  ProbeModelRuntimeRequest,
  ProviderProfile,
  ReplaceCredentialRequest,
  ResetAppSettingsRequest,
  RevokeProviderConsentRequest,
  StartModelRuntimeRequest,
  StopModelRuntimeRequest,
  UnloadModelRequest,
  UpdateAppSettingsRequest,
  UpdateProviderProfileRequest
} from '@lattice/types';
import { APP_COMMANDS } from '@lattice/types';
import { createWebFallbackInfo } from './app-info-fallback';
import { getWebSettings, resetWebSettings, updateWebSettings } from './app-settings-fallback';
import {
  decodeAppInfo,
  decodeAppSettings,
  decodeChatRunHandle,
  decodeChatStreamEvent,
  decodeConversationDetail,
  decodeCredentialRef,
  decodeCredentialRefs,
  decodeListConversationsResponse,
  decodeModelRuntimeStatus,
  decodeModelSlotStatus,
  decodeProviderProfile,
  decodeProviderProfiles,
  normalizeAppError,
  normalizeChatError,
  normalizeConversationError,
  normalizeCredentialError,
  normalizeModelRuntimeError,
  normalizeModelSlotError,
  normalizeProviderError,
  normalizeSettingsError
} from './app-wire';
import { cancelWebChatStream, startWebChatStream } from './chat-fallback';
import {
  deleteWebConversation,
  getWebConversation,
  listWebConversations
} from './conversations-fallback';
import {
  createWebCredential,
  deleteWebCredential,
  listWebCredentials,
  replaceWebCredential
} from './credentials-fallback';
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
import {
  bindWebProviderCredential,
  createWebProviderProfile,
  deleteWebProviderProfile,
  grantWebProviderConsent,
  listWebProviderProfiles,
  revokeWebProviderConsent,
  updateWebProviderProfile
} from './provider-profiles-fallback';
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
   * the resolved conversation either way. `target` selects the local model
   * or a remote profile for this request only; Rust resolves and authorizes
   * it (model, consent, credential) before anything is sent or persisted.
   * `onEvent` is called for every
   * event of the run, including its one terminal event; a malformed single
   * channel payload is skipped rather than thrown (see
   * `decodeChatStreamEvent`), so later valid events for the same run keep
   * arriving.
   */
  async startChatStream(
    conversationId: string | null,
    chat: ChatRequest,
    target: ChatTarget,
    onEvent: (event: ChatStreamEvent) => void
  ): Promise<ChatRunHandle> {
    if (!isTauriRuntime()) {
      return startWebChatStream(conversationId, chat, target, onEvent);
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
          request: { conversationId, chat, target },
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

  async listCredentials(): Promise<readonly CredentialRef[]> {
    if (!isTauriRuntime()) {
      return listWebCredentials();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeCredentialRefs(await invoke<unknown>(APP_COMMANDS.listCredentials));
    } catch (error: unknown) {
      throw normalizeCredentialError(error);
    }
  }

  /**
   * Triggers native secure entry (blocking, bounded by its own ~125s
   * timeout on the native side); never resolves to or accepts a secret
   * value here — Angular only ever sees the resulting `CredentialRef`.
   */
  async createCredential(request: CreateCredentialRequest): Promise<CredentialRef> {
    if (!isTauriRuntime()) {
      return createWebCredential(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeCredentialRef(
        await invoke<unknown>(APP_COMMANDS.createCredential, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeCredentialError(error);
    }
  }

  async replaceCredential(request: ReplaceCredentialRequest): Promise<CredentialRef> {
    if (!isTauriRuntime()) {
      return replaceWebCredential(request);
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeCredentialRef(
        await invoke<unknown>(APP_COMMANDS.replaceCredential, {
          request
        })
      );
    } catch (error: unknown) {
      throw normalizeCredentialError(error);
    }
  }

  async deleteCredential(request: DeleteCredentialRequest): Promise<void> {
    if (!isTauriRuntime()) {
      deleteWebCredential(request);
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.deleteCredential, {
        request
      });
    } catch (error: unknown) {
      throw normalizeCredentialError(error);
    }
  }

  async listProviderProfiles(): Promise<readonly ProviderProfile[]> {
    if (!isTauriRuntime()) {
      return listWebProviderProfiles();
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeProviderProfiles(await invoke<unknown>(APP_COMMANDS.listProviderProfiles));
    } catch (error: unknown) {
      throw normalizeProviderError(error);
    }
  }

  async createProviderProfile(request: CreateProviderProfileRequest): Promise<ProviderProfile> {
    if (!isTauriRuntime()) {
      return createWebProviderProfile(request);
    }

    return this.invokeProviderProfile(APP_COMMANDS.createProviderProfile, request);
  }

  /**
   * Carries no credential: when the endpoint changes, Rust clears the
   * credential binding and consent, and the returned profile shows that.
   */
  async updateProviderProfile(request: UpdateProviderProfileRequest): Promise<ProviderProfile> {
    if (!isTauriRuntime()) {
      return updateWebProviderProfile(request);
    }

    return this.invokeProviderProfile(APP_COMMANDS.updateProviderProfile, request);
  }

  async deleteProviderProfile(request: DeleteProviderProfileRequest): Promise<void> {
    if (!isTauriRuntime()) {
      deleteWebProviderProfile(request);
      return;
    }

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<void>(APP_COMMANDS.deleteProviderProfile, {
        request
      });
    } catch (error: unknown) {
      throw normalizeProviderError(error);
    }
  }

  async bindProviderCredential(request: BindProviderCredentialRequest): Promise<ProviderProfile> {
    if (!isTauriRuntime()) {
      return bindWebProviderCredential(request);
    }

    return this.invokeProviderProfile(APP_COMMANDS.bindProviderCredential, request);
  }

  /** `request.endpoint` must be the endpoint the disclosure showed the user. */
  async grantProviderConsent(request: GrantProviderConsentRequest): Promise<ProviderProfile> {
    if (!isTauriRuntime()) {
      return grantWebProviderConsent(request);
    }

    return this.invokeProviderProfile(APP_COMMANDS.grantProviderConsent, request);
  }

  async revokeProviderConsent(request: RevokeProviderConsentRequest): Promise<ProviderProfile> {
    if (!isTauriRuntime()) {
      return revokeWebProviderConsent(request);
    }

    return this.invokeProviderProfile(APP_COMMANDS.revokeProviderConsent, request);
  }

  private async invokeProviderProfile(command: string, request: unknown): Promise<ProviderProfile> {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return decodeProviderProfile(await invoke<unknown>(command, { request }));
    } catch (error: unknown) {
      throw normalizeProviderError(error);
    }
  }
}

function normalizeBridgeError(error: unknown): AppError {
  return normalizeAppError(error);
}
