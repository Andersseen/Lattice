import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
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

  protected readonly transcript = this.chatStore.transcript;
  protected readonly error = this.chatStore.error;
  protected readonly isStreaming = this.chatStore.isStreaming;
  protected readonly loadedModelKey = this.chatStore.loadedModelKey;
  protected readonly canSend = this.chatStore.canSend;
  protected readonly draft = signal('');

  protected setDraft(event: Event): void {
    const input = event.target as HTMLTextAreaElement;
    this.draft.set(input.value);
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
