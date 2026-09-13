import type {
  ChatMessage,
  Conversation,
  ConversationDetail,
  ConversationSummary,
  DeleteConversationRequest,
  GenerationStatus,
  GetConversationRequest,
  ListConversationsRequest,
  ListConversationsResponse,
  Message
} from '@lattice/types';

/**
 * Browser-preview simulation of the 0.9 conversations domain, following
 * `chat-fallback.ts`/`model-slot-fallback.ts`'s precedent of fully
 * simulating a capability rather than stubbing it "unavailable" — the web
 * preview build should exercise the same History/Chat states as native.
 * Shared module-level state, mirroring `ConversationStore`'s single-file
 * shape: `chat-fallback.ts` drives it while streaming a simulated reply,
 * `AppApiService`'s conversation methods drive it directly otherwise.
 */

const CONVERSATION_NOT_FOUND = {
  code: 'conversation.not_found',
  message: 'That conversation no longer exists.',
  recoverable: true
} as const;

const CONVERSATION_CONFLICT = {
  code: 'conversation.conflict',
  message: 'The conversation changed; reload it before sending.',
  recoverable: true
} as const;

const DEFAULT_CONVERSATIONS_PAGE_SIZE = 20;
const MAX_CONVERSATIONS_PAGE_SIZE = 50;
const DEFAULT_MESSAGES_PAGE_SIZE = 50;
const MAX_MESSAGES_PAGE_SIZE = 100;
const MAX_TITLE_CHARS = 80;
const LOCAL_PROVIDER_KEY = 'local-openai-compatible';

/** A mutable working copy of `Message`: the fallback owns and mutates these
 *  records in place (checkpoint/finalize), unlike the real IPC boundary
 *  where every `Message` is a fresh, immutable decoded value. */
type MutableMessage = { -readonly [Key in keyof Message]: Message[Key] };

interface WebConversation {
  readonly id: string;
  title: string;
  readonly createdAtUnixSeconds: number;
  updatedAtUnixSeconds: number;
  readonly messages: MutableMessage[];
}

const conversations = new Map<string, WebConversation>();
let nextConversationId = 1;
let nextMessageId = 1;
let clock = 1;

export function resetWebConversationsForTest(): void {
  conversations.clear();
  nextConversationId = 1;
  nextMessageId = 1;
  clock = 1;
}

/** Monotonic surrogate for `updated_at_unix_seconds`, deterministic for tests. */
function tick(): number {
  return clock++;
}

/**
 * Mirrors `ConversationStore::begin_or_continue`: creates a conversation
 * when `conversationId` is `null`, persisting every message; otherwise
 * persists only the trailing messages beyond what is already stored.
 */
export function beginOrContinueWebConversation(
  conversationId: string | null,
  messages: readonly ChatMessage[]
): string {
  if (conversationId === null) {
    const id = `web-conversation-${nextConversationId++}`;
    const now = tick();
    const lastUserMessage = [...messages].reverse().find((message) => message.role === 'user');
    const conversation: WebConversation = {
      id,
      title: deriveWebConversationTitle(lastUserMessage?.text ?? ''),
      createdAtUnixSeconds: now,
      updatedAtUnixSeconds: now,
      messages: []
    };
    conversations.set(id, conversation);
    appendCompleteMessages(conversation, messages, 0);
    return id;
  }

  const conversation = conversations.get(conversationId);
  if (conversation === undefined) {
    throw CONVERSATION_NOT_FOUND;
  }

  if (messages.length < conversation.messages.length) {
    throw CONVERSATION_CONFLICT;
  }

  const newMessages = messages.slice(conversation.messages.length);
  if (newMessages.length > 0) {
    appendCompleteMessages(conversation, newMessages, conversation.messages.length);
    conversation.updatedAtUnixSeconds = tick();
  }
  return conversationId;
}

export function startWebAssistantMessage(conversationId: string, modelKey: string): string {
  const conversation = conversations.get(conversationId);
  if (conversation === undefined) {
    throw CONVERSATION_NOT_FOUND;
  }

  const id = `web-message-${nextMessageId++}`;
  const now = tick();
  conversation.messages.push({
    id,
    conversationId,
    sequence: conversation.messages.length,
    role: 'assistant',
    text: '',
    status: 'streaming',
    providerKey: LOCAL_PROVIDER_KEY,
    modelKey,
    createdAtUnixSeconds: now,
    updatedAtUnixSeconds: now
  });
  return id;
}

export function checkpointWebAssistantMessage(messageId: string, text: string): void {
  const message = findMessage(messageId);
  if (message === null || message.status !== 'streaming') {
    return;
  }
  message.text = text;
  message.updatedAtUnixSeconds = tick();
}

export function finalizeWebAssistantMessage(
  messageId: string,
  text: string,
  status: GenerationStatus,
  errorMessage?: string
): void {
  const message = findMessage(messageId);
  if (message === null) {
    return;
  }
  message.text = text;
  message.status = status;
  if (errorMessage !== undefined) {
    message.errorMessage = errorMessage;
  }
  message.updatedAtUnixSeconds = tick();

  const conversation = conversations.get(message.conversationId);
  if (conversation !== undefined) {
    conversation.updatedAtUnixSeconds = tick();
  }
}

export function listWebConversations(request: ListConversationsRequest): ListConversationsResponse {
  const limit = boundedPageSize(
    request.limit,
    DEFAULT_CONVERSATIONS_PAGE_SIZE,
    MAX_CONVERSATIONS_PAGE_SIZE
  );
  const sorted = [...conversations.values()].sort(
    (a, b) => b.updatedAtUnixSeconds - a.updatedAtUnixSeconds || (a.id < b.id ? 1 : -1)
  );

  const before = request.before;
  const startIndex =
    before === undefined
      ? 0
      : sorted.findIndex(
          (conversation) =>
            conversation.updatedAtUnixSeconds < before.updatedAtUnixSeconds ||
            (conversation.updatedAtUnixSeconds === before.updatedAtUnixSeconds &&
              conversation.id < before.id)
        );
  const from = startIndex === -1 ? sorted.length : startIndex;
  const page = sorted.slice(from, from + limit);
  const hasMore = from + limit < sorted.length;
  const lastInPage = page.at(-1);

  return {
    conversations: page.map(toSummary),
    ...(hasMore && lastInPage
      ? {
          nextBefore: {
            updatedAtUnixSeconds: lastInPage.updatedAtUnixSeconds,
            id: lastInPage.id
          }
        }
      : {})
  };
}

export function getWebConversation(request: GetConversationRequest): ConversationDetail {
  const conversation = conversations.get(request.conversationId);
  if (conversation === undefined) {
    throw CONVERSATION_NOT_FOUND;
  }

  const limit = boundedPageSize(request.limit, DEFAULT_MESSAGES_PAGE_SIZE, MAX_MESSAGES_PAGE_SIZE);
  const beforeSequence = request.beforeSequence;
  const eligible =
    beforeSequence === undefined
      ? conversation.messages
      : conversation.messages.filter((message) => message.sequence < beforeSequence);
  const startIndex = Math.max(0, eligible.length - limit);
  const page = eligible.slice(startIndex);

  return {
    conversation: toConversation(conversation),
    messages: page,
    hasMoreBefore: startIndex > 0
  };
}

export function deleteWebConversation(request: DeleteConversationRequest): void {
  if (!conversations.delete(request.conversationId)) {
    throw CONVERSATION_NOT_FOUND;
  }
}

function appendCompleteMessages(
  conversation: WebConversation,
  messages: readonly ChatMessage[],
  startSequence: number
): void {
  const now = tick();
  messages.forEach((message, offset) => {
    conversation.messages.push({
      id: `web-message-${nextMessageId++}`,
      conversationId: conversation.id,
      sequence: startSequence + offset,
      role: message.role,
      text: message.text,
      status: 'complete',
      createdAtUnixSeconds: now,
      updatedAtUnixSeconds: now
    });
  });
}

function findMessage(messageId: string): MutableMessage | null {
  for (const conversation of conversations.values()) {
    const message = conversation.messages.find((candidate) => candidate.id === messageId);
    if (message !== undefined) {
      return message;
    }
  }
  return null;
}

function toSummary(conversation: WebConversation): ConversationSummary {
  const lastMessage = conversation.messages[conversation.messages.length - 1];
  return {
    id: conversation.id,
    title: conversation.title,
    updatedAtUnixSeconds: conversation.updatedAtUnixSeconds,
    messageCount: conversation.messages.length,
    ...(lastMessage ? { lastStatus: lastMessage.status } : {})
  };
}

function toConversation(conversation: WebConversation): Conversation {
  return {
    id: conversation.id,
    title: conversation.title,
    createdAtUnixSeconds: conversation.createdAtUnixSeconds,
    updatedAtUnixSeconds: conversation.updatedAtUnixSeconds
  };
}

function boundedPageSize(requested: number | undefined, fallback: number, max: number): number {
  if (requested === undefined || requested <= 0) {
    return fallback;
  }
  return Math.min(requested, max);
}

function deriveWebConversationTitle(text: string): string {
  const collapsed = text.split(/\s+/).filter(Boolean).join(' ').trim();
  if (collapsed.length === 0) {
    return 'New conversation';
  }
  return collapsed.length > MAX_TITLE_CHARS ? `${collapsed.slice(0, MAX_TITLE_CHARS)}…` : collapsed;
}
