import { computed, Injectable, inject, signal } from '@angular/core';
import type {
  AppError,
  ChatMessage,
  ChatRole,
  ChatStreamEvent,
  ChatTarget,
  Message,
  ProviderProfile
} from '@lattice/types';
import { PROVIDER_KEYS } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeChatError, normalizeConversationError } from '../api/app-wire';
import { ConversationsStore } from './conversations.store';
import { ModelSlotStore } from './model-slot.store';
import { ProviderProfilesStore } from './provider-profiles.store';

export type ChatTranscriptEntryStatus =
  | 'complete'
  | 'streaming'
  | 'cancelled'
  | 'failed'
  | 'interrupted';

export interface ChatTranscriptEntry {
  readonly role: ChatRole;
  readonly text: string;
  readonly status: ChatTranscriptEntryStatus;
  /** Provenance of an assistant reply; absent for user turns. */
  readonly providerKey?: string;
  readonly modelKey?: string;
}

/** A message waiting for the user to approve sending context to a remote endpoint. */
export interface PendingConsent {
  readonly text: string;
  readonly profile: ProviderProfile;
}

@Injectable({ providedIn: 'root' })
export class ChatStore {
  private readonly appApi = inject(AppApiService);
  private readonly modelSlotStore = inject(ModelSlotStore);
  private readonly conversationsStore = inject(ConversationsStore);
  private readonly providerProfilesStore = inject(ProviderProfilesStore);

  private readonly transcriptState = signal<readonly ChatTranscriptEntry[]>([]);
  private readonly errorState = signal<AppError | null>(null);
  private readonly streamingState = signal(false);
  private readonly conversationIdState = signal<string | null>(null);
  private readonly loadingState = signal(false);
  private readonly targetState = signal<ChatTarget>({ kind: 'local' });
  private readonly pendingConsentState = signal<PendingConsent | null>(null);
  private activeRunId: string | null = null;

  readonly transcript = this.transcriptState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isStreaming = this.streamingState.asReadonly();
  readonly conversationId = this.conversationIdState.asReadonly();
  readonly isLoadingConversation = this.loadingState.asReadonly();
  /** Selection for the next request only; never persisted or dispatched on its own. */
  readonly target = this.targetState.asReadonly();
  readonly pendingConsent = this.pendingConsentState.asReadonly();
  readonly loadedModelKey = computed(() => {
    const ownership = this.modelSlotStore.status()?.ownership;
    return ownership?.state === 'owned' ? ownership.modelKey : null;
  });
  readonly selectedProfile = computed(() => {
    const target = this.targetState();
    return target.kind === 'remote'
      ? (this.providerProfilesStore.profiles().find((profile) => profile.id === target.profileId) ??
          null)
      : null;
  });
  /** The model the next request names: the loaded local model or the profile's model. */
  readonly activeModelKey = computed(() =>
    this.targetState().kind === 'local'
      ? this.loadedModelKey()
      : (this.selectedProfile()?.modelKey ?? null)
  );
  readonly canSend = computed(() => !this.streamingState() && this.activeModelKey() !== null);

  selectTarget(target: ChatTarget): void {
    if (this.streamingState()) {
      return;
    }
    this.targetState.set(target);
    this.pendingConsentState.set(null);
  }

  /** Clears in-memory state so the next `send` starts a brand-new conversation. */
  startNewConversation(): void {
    this.conversationIdState.set(null);
    this.transcriptState.set([]);
    this.errorState.set(null);
    this.streamingState.set(false);
    this.activeRunId = null;
  }

  /** Hydrates the transcript from a previously persisted conversation ("reopen"). */
  async loadConversation(conversationId: string): Promise<void> {
    this.startNewConversation();
    this.conversationIdState.set(conversationId);
    this.loadingState.set(true);

    try {
      const detail = await this.appApi.getConversation({ conversationId });
      this.transcriptState.set(detail.messages.map(toTranscriptEntry));
    } catch (error: unknown) {
      this.errorState.set(normalizeConversationError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  /**
   * Sends through the selected target. A remote profile without consent for
   * its current endpoint is not contacted: the message waits in
   * `pendingConsent` until the user approves or dismisses the disclosure.
   * Rust enforces the same rule independently.
   */
  async send(text: string): Promise<void> {
    const trimmed = text.trim();
    const target = this.targetState();
    const modelKey = this.activeModelKey();
    if (trimmed.length === 0 || this.streamingState() || modelKey === null) {
      return;
    }

    const profile = this.selectedProfile();
    if (profile !== null && profile.consent?.endpoint !== profile.endpoint) {
      this.pendingConsentState.set({ text: trimmed, profile });
      return;
    }

    this.errorState.set(null);
    const priorMessages = this.transcriptState().map(toChatMessage);
    this.transcriptState.update((entries) => [
      ...entries,
      { role: 'user', text: trimmed, status: 'complete' },
      {
        role: 'assistant',
        text: '',
        status: 'streaming',
        providerKey: target.kind === 'local' ? PROVIDER_KEYS.local : PROVIDER_KEYS.remote,
        modelKey
      }
    ]);
    this.streamingState.set(true);

    try {
      const handle = await this.appApi.startChatStream(
        this.conversationIdState(),
        { modelKey, messages: [...priorMessages, { role: 'user', text: trimmed }] },
        target,
        (event) => this.applyEvent(event)
      );
      this.activeRunId = handle.runId;
      this.conversationIdState.set(handle.conversationId);
      void this.conversationsStore.load();
    } catch (error: unknown) {
      this.streamingState.set(false);
      this.errorState.set(normalizeChatError(error));
      this.updateLastEntry((entry) => ({ ...entry, status: 'failed' }));
    }
  }

  /**
   * Approves the pending disclosure for exactly the endpoint it showed, then
   * sends the waiting message. If the profile changed meanwhile, Rust
   * refuses the approval and nothing is sent.
   */
  async approvePendingConsent(): Promise<void> {
    const pending = this.pendingConsentState();
    if (pending === null) {
      return;
    }
    this.pendingConsentState.set(null);

    const granted = await this.providerProfilesStore.grantConsent(pending.profile);
    if (!granted) {
      this.errorState.set(this.providerProfilesStore.error());
      return;
    }
    await this.send(pending.text);
  }

  dismissPendingConsent(): void {
    this.pendingConsentState.set(null);
  }

  async cancel(): Promise<void> {
    if (this.activeRunId === null) {
      return;
    }

    try {
      await this.appApi.cancelChatStream({ runId: this.activeRunId });
    } catch (error: unknown) {
      this.errorState.set(normalizeChatError(error));
    }
  }

  private applyEvent(event: ChatStreamEvent): void {
    if (this.activeRunId === null) {
      if (event.kind === 'started') {
        this.activeRunId = event.runId;
      }
      return;
    }

    if (event.runId !== this.activeRunId) {
      return;
    }

    switch (event.kind) {
      case 'started':
        return;
      case 'delta':
        this.updateLastEntry((entry) => ({ ...entry, text: entry.text + event.text }));
        return;
      case 'completed':
        this.updateLastEntry((entry) => ({ ...entry, status: 'complete' }));
        this.finishRun();
        return;
      case 'cancelled':
        this.updateLastEntry((entry) => ({ ...entry, status: 'cancelled' }));
        this.finishRun();
        return;
      case 'failed':
        this.updateLastEntry((entry) => ({ ...entry, status: 'failed' }));
        this.errorState.set(event.error);
        this.finishRun();
        return;
    }
  }

  private finishRun(): void {
    this.activeRunId = null;
    this.streamingState.set(false);
    void this.conversationsStore.load();
  }

  private updateLastEntry(update: (entry: ChatTranscriptEntry) => ChatTranscriptEntry): void {
    this.transcriptState.update((entries) => {
      if (entries.length === 0) {
        return entries;
      }
      const lastIndex = entries.length - 1;
      return entries.map((entry, index) => (index === lastIndex ? update(entry) : entry));
    });
  }
}

function toChatMessage(entry: ChatTranscriptEntry): ChatMessage {
  return { role: entry.role, text: entry.text };
}

function toTranscriptEntry(message: Message): ChatTranscriptEntry {
  return {
    role: message.role,
    text: message.text,
    status: message.status === 'streaming' ? 'interrupted' : message.status,
    ...(message.providerKey !== undefined ? { providerKey: message.providerKey } : {}),
    ...(message.modelKey !== undefined ? { modelKey: message.modelKey } : {})
  };
}
