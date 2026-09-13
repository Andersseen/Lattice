import { Injectable, inject, signal } from '@angular/core';
import type { AppError, ConversationCursor, ConversationSummary } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeConversationError } from '../api/app-wire';

const CONVERSATIONS_PAGE_SIZE = 20;

@Injectable({ providedIn: 'root' })
export class ConversationsStore {
  private readonly appApi = inject(AppApiService);
  private readonly conversationsState = signal<readonly ConversationSummary[]>([]);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly deletingIdState = signal<string | null>(null);
  private readonly hasMoreState = signal(false);
  private nextBefore: ConversationCursor | null = null;

  readonly conversations = this.conversationsState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly deletingId = this.deletingIdState.asReadonly();
  readonly hasMore = this.hasMoreState.asReadonly();

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      const response = await this.appApi.listConversations({ limit: CONVERSATIONS_PAGE_SIZE });
      this.conversationsState.set(response.conversations);
      this.nextBefore = response.nextBefore ?? null;
      this.hasMoreState.set(response.nextBefore !== undefined);
    } catch (error: unknown) {
      this.errorState.set(normalizeConversationError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async loadMore(): Promise<void> {
    if (!this.hasMoreState() || this.loadingState()) {
      return;
    }

    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      const response = await this.appApi.listConversations({
        limit: CONVERSATIONS_PAGE_SIZE,
        ...(this.nextBefore ? { before: this.nextBefore } : {})
      });
      this.conversationsState.update((existing) => [...existing, ...response.conversations]);
      this.nextBefore = response.nextBefore ?? null;
      this.hasMoreState.set(response.nextBefore !== undefined);
    } catch (error: unknown) {
      this.errorState.set(normalizeConversationError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async delete(conversationId: string): Promise<void> {
    this.deletingIdState.set(conversationId);
    this.errorState.set(null);

    try {
      await this.appApi.deleteConversation({ conversationId });
      this.conversationsState.update((existing) =>
        existing.filter((conversation) => conversation.id !== conversationId)
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeConversationError(error));
    } finally {
      this.deletingIdState.set(null);
    }
  }
}
