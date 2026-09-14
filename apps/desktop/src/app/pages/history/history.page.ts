import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { Router } from '@angular/router';
import { VoltButton } from '@voltui/components';

import { ConversationsStore } from '../../core/state/conversations.store';

@Component({
  selector: 'lat-history-page',
  imports: [VoltButton],
  templateUrl: './history.page.html',
  styleUrl: './history.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class HistoryPage {
  private readonly conversationsStore = inject(ConversationsStore);
  private readonly router = inject(Router);

  protected readonly conversations = this.conversationsStore.conversations;
  protected readonly error = this.conversationsStore.error;
  protected readonly isLoading = this.conversationsStore.isLoading;
  protected readonly hasMore = this.conversationsStore.hasMore;
  protected readonly deletingId = this.conversationsStore.deletingId;
  protected readonly pendingDeleteId = signal<string | null>(null);

  constructor() {
    void this.conversationsStore.load();
  }

  protected open(conversationId: string): void {
    void this.router.navigate(['/chat'], { queryParams: { conversationId } });
  }

  protected loadMore(): void {
    void this.conversationsStore.loadMore();
  }

  protected requestDelete(conversationId: string, event: Event): void {
    event.stopPropagation();
    this.pendingDeleteId.set(conversationId);
  }

  protected cancelDelete(event: Event): void {
    event.stopPropagation();
    this.pendingDeleteId.set(null);
  }

  protected delete(conversationId: string, event: Event): void {
    event.stopPropagation();
    this.pendingDeleteId.set(null);
    void this.conversationsStore.delete(conversationId);
  }

  protected refresh(): void {
    void this.conversationsStore.load();
  }
}
