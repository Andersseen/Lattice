import { computed, inject, Injectable, signal } from '@angular/core';
import type { AppError, ChatMessage, ChatRole, ChatStreamEvent } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeChatError } from '../api/app-wire';
import { ModelSlotStore } from './model-slot.store';

export type ChatTranscriptEntryStatus = 'complete' | 'streaming' | 'cancelled' | 'failed';

export interface ChatTranscriptEntry {
  readonly role: ChatRole;
  readonly text: string;
  readonly status: ChatTranscriptEntryStatus;
}

@Injectable({ providedIn: 'root' })
export class ChatStore {
  private readonly appApi = inject(AppApiService);
  private readonly modelSlotStore = inject(ModelSlotStore);

  private readonly transcriptState = signal<readonly ChatTranscriptEntry[]>([]);
  private readonly errorState = signal<AppError | null>(null);
  private readonly streamingState = signal(false);
  private activeRunId: string | null = null;

  readonly transcript = this.transcriptState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isStreaming = this.streamingState.asReadonly();
  readonly loadedModelKey = computed(() => {
    const ownership = this.modelSlotStore.status()?.ownership;
    return ownership?.state === 'owned' ? ownership.modelKey : null;
  });
  readonly canSend = computed(() => !this.streamingState() && this.loadedModelKey() !== null);

  async send(text: string): Promise<void> {
    const trimmed = text.trim();
    const modelKey = this.loadedModelKey();
    if (trimmed.length === 0 || this.streamingState() || modelKey === null) {
      return;
    }

    this.errorState.set(null);
    const priorMessages = this.transcriptState().map(toChatMessage);
    this.transcriptState.update((entries) => [
      ...entries,
      { role: 'user', text: trimmed, status: 'complete' },
      { role: 'assistant', text: '', status: 'streaming' }
    ]);
    this.streamingState.set(true);

    try {
      const handle = await this.appApi.startChatStream(
        { modelKey, messages: [...priorMessages, { role: 'user', text: trimmed }] },
        (event) => this.applyEvent(event)
      );
      this.activeRunId = handle.runId;
    } catch (error: unknown) {
      this.streamingState.set(false);
      this.errorState.set(normalizeChatError(error));
      this.updateLastEntry((entry) => ({ ...entry, status: 'failed' }));
    }
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
