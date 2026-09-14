import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
  viewChild
} from '@angular/core';
import type { ElementRef } from '@angular/core';
import { Router, RouterLink } from '@angular/router';
import { VoltButton } from '@voltui/components';
import type { ConversationSummary } from '@lattice/types';

import { ChatStore } from '../../core/state/chat.store';
import { ConversationsStore } from '../../core/state/conversations.store';
import { ModelRuntimeStore } from '../../core/state/model-runtime.store';
import { ModelSlotStore } from '../../core/state/model-slot.store';

@Component({
  selector: 'lat-chat-page',
  imports: [RouterLink, VoltButton],
  templateUrl: './chat.page.html',
  styleUrl: './chat.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class ChatPage {
  private readonly chatStore = inject(ChatStore);
  private readonly conversationsStore = inject(ConversationsStore);
  private readonly runtimeStore = inject(ModelRuntimeStore);
  private readonly slotStore = inject(ModelSlotStore);
  private readonly router = inject(Router);
  private readonly transcriptViewport = viewChild<ElementRef<HTMLElement>>('transcriptViewport');

  /** Bound from the `?conversationId=` query param (see `withComponentInputBinding`). */
  readonly conversationId = input<string>();

  protected readonly transcript = this.chatStore.transcript;
  protected readonly error = this.chatStore.error;
  protected readonly isStreaming = this.chatStore.isStreaming;
  protected readonly isLoadingConversation = this.chatStore.isLoadingConversation;
  protected readonly loadedModelKey = this.chatStore.loadedModelKey;
  protected readonly canSend = this.chatStore.canSend;
  protected readonly activeConversationId = this.chatStore.conversationId;
  protected readonly conversations = this.conversationsStore.conversations;
  protected readonly conversationsError = this.conversationsStore.error;
  protected readonly isLoadingConversations = this.conversationsStore.isLoading;
  protected readonly deletingConversationId = this.conversationsStore.deletingId;
  protected readonly runtimeStatus = this.runtimeStore.status;
  protected readonly canStart = this.runtimeStore.canStart;
  protected readonly isStarting = this.runtimeStore.isStarting;
  protected readonly slotStatus = this.slotStore.status;
  protected readonly draft = signal('');
  protected readonly pendingDeleteId = signal<string | null>(null);
  protected readonly userScrolledUp = signal(false);
  protected readonly composerPlaceholder = computed(() =>
    this.canSend() ? 'Ask Lattice...' : this.disabledReason()
  );
  protected readonly disabledReason = computed(() => {
    const runtime = this.runtimeStatus();
    if (runtime === null) {
      return 'Checking local runtime.';
    }
    if (runtime.executablePath === undefined) {
      return 'Configure a local runtime first.';
    }
    if (runtime.availability !== 'running') {
      return 'Start the local runtime first.';
    }
    if (this.loadedModelKey() === null) {
      return 'Load a local model first.';
    }
    return '';
  });
  protected readonly readiness = computed(() => {
    const runtime = this.runtimeStatus();
    if (runtime === null) {
      return {
        title: 'Checking local AI.',
        message: 'Reading runtime and model status.',
        action: null
      } as const;
    }
    if (runtime.executablePath === undefined) {
      return {
        title: 'Local runtime is not configured.',
        message: 'Configure a runtime to use local models.',
        action: 'configure'
      } as const;
    }
    if (runtime.availability !== 'running') {
      return {
        title: 'Local runtime is stopped.',
        message: runtime.message,
        action: 'start'
      } as const;
    }
    if (this.loadedModelKey() === null) {
      return {
        title: 'Choose a local model to start chatting.',
        message: this.slotStatus()?.message ?? 'No local model is loaded.',
        action: 'model'
      } as const;
    }
    return null;
  });

  constructor() {
    void this.conversationsStore.load();

    effect(() => {
      const id = this.conversationId();
      if (id !== undefined && id !== this.chatStore.conversationId()) {
        void this.chatStore.loadConversation(id);
      }
    });

    effect(() => {
      this.transcript();
      if (!this.userScrolledUp()) {
        queueMicrotask(() => this.scrollTranscriptToEnd());
      }
    });
  }

  protected newChat(): void {
    if (this.isStreaming()) {
      return;
    }
    this.chatStore.startNewConversation();
    this.draft.set('');
    this.pendingDeleteId.set(null);
    // Clears `?conversationId=` too, so the reopen effect below does not
    // immediately reload the conversation this action just left.
    void this.router.navigate(['/chat']);
  }

  protected setDraft(event: Event): void {
    const target = event.target as HTMLTextAreaElement;
    this.draft.set(target.value);
  }

  protected onComposerKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Enter' || event.shiftKey || event.isComposing) {
      return;
    }

    event.preventDefault();
    this.send();
  }

  protected send(): void {
    if (!this.canSend() || this.draft().trim().length === 0) {
      return;
    }

    const text = this.draft();
    this.draft.set('');
    this.userScrolledUp.set(false);
    void this.chatStore.send(text);
  }

  protected cancel(): void {
    void this.chatStore.cancel();
  }

  protected startRuntime(): void {
    void this.runtimeStore.start();
  }

  protected openConversation(conversationId: string): void {
    this.pendingDeleteId.set(null);
    void this.router.navigate(['/chat'], { queryParams: { conversationId } });
  }

  protected requestDelete(conversationId: string, event: Event): void {
    event.stopPropagation();
    this.pendingDeleteId.set(conversationId);
  }

  protected cancelDelete(event: Event): void {
    event.stopPropagation();
    this.pendingDeleteId.set(null);
  }

  protected async deleteConversation(conversationId: string, event: Event): Promise<void> {
    event.stopPropagation();
    this.pendingDeleteId.set(null);
    await this.conversationsStore.delete(conversationId);
    if (this.activeConversationId() === conversationId) {
      this.newChat();
    }
  }

  protected onTranscriptScroll(event: Event): void {
    const element = event.target as HTMLElement;
    const distanceFromBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
    this.userScrolledUp.set(distanceFromBottom > 96);
  }

  protected conversationTime(conversation: ConversationSummary): string {
    return formatConversationTime(conversation.updatedAtUnixSeconds);
  }

  protected conversationGroup(
    index: number,
    conversations: readonly ConversationSummary[]
  ): string {
    const current = conversationGroup(conversations[index]?.updatedAtUnixSeconds);
    const previous = conversationGroup(conversations[index - 1]?.updatedAtUnixSeconds);
    return current === previous ? '' : current;
  }

  private scrollTranscriptToEnd(): void {
    const element = this.transcriptViewport()?.nativeElement;
    if (element === undefined) {
      return;
    }
    element.scrollTop = element.scrollHeight;
  }
}

function formatConversationTime(unixSeconds: number): string {
  if (unixSeconds < 946_684_800) {
    return 'Recent';
  }

  const formatter = new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit'
  });
  return formatter.format(new Date(unixSeconds * 1000));
}

function conversationGroup(unixSeconds: number | undefined): string {
  if (unixSeconds === undefined || unixSeconds < 946_684_800) {
    return 'Recent';
  }

  const date = new Date(unixSeconds * 1000);
  const today = new Date();
  const startOfToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const startOfConversationDay = new Date(
    date.getFullYear(),
    date.getMonth(),
    date.getDate()
  ).getTime();
  const dayDelta = Math.round((startOfToday - startOfConversationDay) / 86_400_000);

  if (dayDelta === 0) {
    return 'Today';
  }
  if (dayDelta === 1) {
    return 'Yesterday';
  }
  return 'Older';
}
