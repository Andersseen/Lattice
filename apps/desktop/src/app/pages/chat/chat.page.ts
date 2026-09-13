import { ChangeDetectionStrategy, Component, effect, inject, input, signal } from '@angular/core';
import { Router } from '@angular/router';
import { VoltButton } from '@voltui/components';

import { ChatStore } from '../../core/state/chat.store';

@Component({
  selector: 'lat-chat-page',
  imports: [VoltButton],
  templateUrl: './chat.page.html',
  styleUrl: './chat.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ChatPage {
  private readonly chatStore = inject(ChatStore);
  private readonly router = inject(Router);

  /** Bound from the `?conversationId=` query param (see `withComponentInputBinding`). */
  readonly conversationId = input<string>();

  protected readonly transcript = this.chatStore.transcript;
  protected readonly error = this.chatStore.error;
  protected readonly isStreaming = this.chatStore.isStreaming;
  protected readonly isLoadingConversation = this.chatStore.isLoadingConversation;
  protected readonly loadedModelKey = this.chatStore.loadedModelKey;
  protected readonly canSend = this.chatStore.canSend;
  protected readonly draft = signal('');

  constructor() {
    effect(() => {
      const id = this.conversationId();
      if (id !== undefined && id !== this.chatStore.conversationId()) {
        void this.chatStore.loadConversation(id);
      }
    });
  }

  protected newChat(): void {
    this.chatStore.startNewConversation();
    this.draft.set('');
    // Clears `?conversationId=` too, so the reopen effect below does not
    // immediately reload the conversation this action just left.
    void this.router.navigate(['/chat']);
  }

  protected setDraft(event: Event): void {
    const target = event.target as HTMLTextAreaElement;
    this.draft.set(target.value);
  }

  protected send(): void {
    if (!this.canSend() || this.draft().trim().length === 0) {
      return;
    }

    const text = this.draft();
    this.draft.set('');
    void this.chatStore.send(text);
  }

  protected cancel(): void {
    void this.chatStore.cancel();
  }
}
