import type { ElementRef, TemplateRef } from '@angular/core';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  input,
  signal,
  ViewContainerRef,
  viewChild
} from '@angular/core';
import { Router, RouterLink } from '@angular/router';
import type { ConversationSummary } from '@lattice/types';
import { PROVIDER_KEYS } from '@lattice/types';
import {
  VoltBadge,
  VoltButton,
  VoltCard,
  VoltCardContent,
  VoltCardDescription,
  VoltCardFooter,
  VoltCardHeader,
  VoltCardTitle,
  VoltFormField,
  VoltLabel,
  VoltNativeSelect,
  VoltTextarea
} from '@voltui/components';
import { MoveAnimateDirective } from 'angular-movement';
import { LmnPaperAirplaneIcon } from 'lumen-icons/paper-airplane';
import { LmnStopIcon } from 'lumen-icons/stop';
import { LmnTrashIcon } from 'lumen-icons/trash';
import type { DialogRef } from 'quartz-headless';
import { DialogService } from 'quartz-headless';

import type { ChatTranscriptEntry } from '../../core/state/chat.store';
import { ChatStore } from '../../core/state/chat.store';
import { ConversationsStore } from '../../core/state/conversations.store';
import { CredentialsStore } from '../../core/state/credentials.store';
import { ModelRuntimeStore } from '../../core/state/model-runtime.store';
import { ModelSlotStore } from '../../core/state/model-slot.store';
import { ProviderProfilesStore } from '../../core/state/provider-profiles.store';

const LOCAL_TARGET_VALUE = 'local';
const REMOTE_TARGET_PREFIX = 'remote:';

@Component({
  selector: 'lat-chat-page',
  imports: [
    RouterLink,
    MoveAnimateDirective,
    LmnPaperAirplaneIcon,
    LmnStopIcon,
    LmnTrashIcon,
    VoltBadge,
    VoltButton,
    VoltCard,
    VoltCardContent,
    VoltCardDescription,
    VoltCardFooter,
    VoltCardHeader,
    VoltCardTitle,
    VoltFormField,
    VoltLabel,
    VoltNativeSelect,
    VoltTextarea
  ],
  templateUrl: './chat.page.html',
  styleUrl: './chat.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export default class ChatPage {
  private readonly chatStore = inject(ChatStore);
  private readonly conversationsStore = inject(ConversationsStore);
  private readonly runtimeStore = inject(ModelRuntimeStore);
  private readonly slotStore = inject(ModelSlotStore);
  private readonly providerProfilesStore = inject(ProviderProfilesStore);
  private readonly credentialsStore = inject(CredentialsStore);
  private readonly router = inject(Router);
  private readonly dialog = inject(DialogService);
  private readonly viewContainerRef = inject(ViewContainerRef);
  private readonly transcriptViewport = viewChild<ElementRef<HTMLElement>>('transcriptViewport');
  private readonly deleteConversationDialog = viewChild.required<TemplateRef<unknown>>(
    'deleteConversationDialog'
  );
  private readonly consentDialog = viewChild.required<TemplateRef<unknown>>('consentDialog');
  private consentDialogRef: DialogRef | null = null;

  /** Bound from the `?conversationId=` query param (see `withComponentInputBinding`). */
  readonly conversationId = input<string>();

  protected readonly transcript = this.chatStore.transcript;
  protected readonly error = this.chatStore.error;
  protected readonly isStreaming = this.chatStore.isStreaming;
  protected readonly isLoadingConversation = this.chatStore.isLoadingConversation;
  protected readonly loadedModelKey = this.chatStore.loadedModelKey;
  protected readonly activeModelKey = this.chatStore.activeModelKey;
  protected readonly selectedProfile = this.chatStore.selectedProfile;
  protected readonly pendingConsent = this.chatStore.pendingConsent;
  protected readonly providerProfiles = this.providerProfilesStore.profiles;
  protected readonly isRemoteTarget = computed(() => this.chatStore.target().kind === 'remote');
  protected readonly targetValue = computed(() => {
    const target = this.chatStore.target();
    return target.kind === 'local'
      ? LOCAL_TARGET_VALUE
      : `${REMOTE_TARGET_PREFIX}${target.profileId}`;
  });
  protected readonly pendingConsentCredentialLabel = computed(() => {
    const credentialId = this.pendingConsent()?.profile.credentialId;
    if (credentialId === undefined) {
      return 'No credential (unauthenticated request)';
    }
    return (
      this.credentialsStore.credentials().find((credential) => credential.id === credentialId)
        ?.label ?? 'A stored credential'
    );
  });
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
  protected readonly composerPlaceholder = computed(() => {
    if (!this.canSend()) {
      return this.disabledReason();
    }
    const profile = this.selectedProfile();
    return profile === null ? 'Ask Lattice...' : `Ask ${profile.label}...`;
  });
  protected readonly disabledReason = computed(() => {
    if (this.isRemoteTarget()) {
      return this.selectedProfile() === null ? 'Choose a remote provider.' : '';
    }
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
    if (this.isRemoteTarget()) {
      return null;
    }
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

    // A deleted profile cannot stay selected; fall back to local chat.
    effect(() => {
      if (
        this.isRemoteTarget() &&
        this.selectedProfile() === null &&
        !this.providerProfilesStore.isLoading()
      ) {
        this.chatStore.selectTarget({ kind: 'local' });
      }
    });

    effect(() => {
      if (this.pendingConsent() !== null) {
        queueMicrotask(() => this.openConsentDialog());
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
    this.draft.set((event.target as HTMLTextAreaElement).value);
  }

  protected setDraftValue(value: string): void {
    this.draft.set(value);
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

  protected onTargetChange(event: Event): void {
    this.selectTarget((event.target as HTMLSelectElement).value);
  }

  private selectTarget(value: string): void {
    if (value.startsWith(REMOTE_TARGET_PREFIX)) {
      this.chatStore.selectTarget({
        kind: 'remote',
        profileId: value.slice(REMOTE_TARGET_PREFIX.length)
      });
    } else {
      this.chatStore.selectTarget({ kind: 'local' });
    }
  }

  protected approveConsent(): void {
    this.userScrolledUp.set(false);
    void this.chatStore.approvePendingConsent();
  }

  /** Nothing is sent; the waiting message returns to the composer. */
  protected dismissConsent(): void {
    const pending = this.pendingConsent();
    if (pending !== null) {
      this.draft.set(pending.text);
    }
    this.chatStore.dismissPendingConsent();
  }

  protected provenanceLabel(entry: ChatTranscriptEntry): string {
    if (entry.role !== 'assistant' || entry.providerKey === undefined) {
      return '';
    }
    const origin = entry.providerKey === PROVIDER_KEYS.remote ? 'Remote' : 'Local';
    return entry.modelKey === undefined ? origin : `${origin} · ${entry.modelKey}`;
  }

  private openConsentDialog(): void {
    if (this.pendingConsent() === null || this.consentDialogRef !== null) {
      return;
    }
    const dialogRef = this.dialog.open(this.consentDialog(), this.viewContainerRef, {
      ariaLabelledBy: 'consent-title',
      ariaDescribedBy: 'consent-description',
      panelClass: 'confirmation-dialog-panel',
      backdropClass: 'confirmation-dialog-backdrop'
    });
    // Closing by Escape or backdrop dismisses the disclosure: nothing is sent.
    this.consentDialogRef = dialogRef;
    dialogRef.closed$.subscribe(() => {
      this.consentDialogRef = null;
      this.dismissConsent();
    });
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
    this.dialog.open(this.deleteConversationDialog(), this.viewContainerRef, {
      ariaLabelledBy: 'delete-conversation-title',
      ariaDescribedBy: 'delete-conversation-description',
      panelClass: 'confirmation-dialog-panel',
      backdropClass: 'confirmation-dialog-backdrop'
    });
  }

  protected clearPendingDelete(): void {
    this.pendingDeleteId.set(null);
  }

  protected async deletePendingConversation(): Promise<void> {
    const conversationId = this.pendingDeleteId();
    if (conversationId === null) {
      return;
    }
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
