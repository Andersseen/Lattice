import type { TemplateRef } from '@angular/core';
import {
  ChangeDetectionStrategy,
  Component,
  inject,
  signal,
  ViewContainerRef,
  viewChild
} from '@angular/core';
import { Router } from '@angular/router';
import {
  VoltBadge,
  VoltButton,
  VoltCard,
  VoltCardContent,
  VoltCardDescription,
  VoltCardFooter,
  VoltCardHeader,
  VoltCardTitle
} from '@voltui/components';
import { LmnTrashIcon } from 'lumen-icons/trash';
import { DialogService } from 'quartz-headless';

import { ConversationsStore } from '../../core/state/conversations.store';

@Component({
  selector: 'lat-history-page',
  imports: [
    LmnTrashIcon,
    VoltBadge,
    VoltButton,
    VoltCard,
    VoltCardContent,
    VoltCardDescription,
    VoltCardFooter,
    VoltCardHeader,
    VoltCardTitle
  ],
  templateUrl: './history.page.html',
  styleUrl: './history.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export default class HistoryPage {
  private readonly conversationsStore = inject(ConversationsStore);
  private readonly router = inject(Router);
  private readonly dialog = inject(DialogService);
  private readonly viewContainerRef = inject(ViewContainerRef);
  private readonly deleteConversationDialog = viewChild.required<TemplateRef<unknown>>(
    'deleteConversationDialog'
  );

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
    this.dialog.open(this.deleteConversationDialog(), this.viewContainerRef, {
      ariaLabelledBy: 'history-delete-title',
      ariaDescribedBy: 'history-delete-description',
      panelClass: 'confirmation-dialog-panel',
      backdropClass: 'confirmation-dialog-backdrop'
    });
  }

  protected clearPendingDelete(): void {
    this.pendingDeleteId.set(null);
  }

  protected deletePendingConversation(): void {
    const conversationId = this.pendingDeleteId();
    if (conversationId === null) {
      return;
    }
    this.pendingDeleteId.set(null);
    void this.conversationsStore.delete(conversationId);
  }

  protected refresh(): void {
    void this.conversationsStore.load();
  }
}
