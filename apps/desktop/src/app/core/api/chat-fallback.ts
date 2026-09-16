import type {
  AppError,
  ChatRequest,
  ChatRunHandle,
  ChatStreamEvent,
  ChatTarget
} from '@lattice/types';
import { PROVIDER_KEYS } from '@lattice/types';

import {
  beginOrContinueWebConversation,
  checkpointWebAssistantMessage,
  finalizeWebAssistantMessage,
  startWebAssistantMessage
} from './conversations-fallback';
import { getWebModelSlotStatus } from './model-slot-fallback';
import { getWebProviderProfile } from './provider-profiles-fallback';

const NOT_LOADED_MESSAGE = 'Load the requested model before starting a chat.';
const ALREADY_STREAMING_MESSAGE = 'A response is already streaming.';
const SIMULATED_RESPONSE =
  'This is a simulated response because Lattice is running as a browser preview without the desktop shell.';
const MAX_PROMPT_CHARS = 32_000;
const WORD_INTERVAL_MS = 60;

interface WebChatRun {
  readonly runId: string;
  readonly messageId: string;
  accumulatedText: string;
  cancelled: boolean;
}

let activeWebRun: WebChatRun | null = null;
let nextRunId = 1;

export function resetWebChatStreamForTest(): void {
  activeWebRun = null;
  nextRunId = 1;
}

interface WebTarget {
  readonly providerKey: string;
  readonly response: string;
}

/**
 * Simulates the same single-run and per-target preconditions
 * `start_chat_stream` enforces natively — the local model lease, or a
 * remote profile's configured model and destination-bound consent —
 * persists the new message(s) and a streaming assistant reply with the
 * target's provenance through the shared conversations fallback, then
 * streams a canned response word-by-word through `onEvent` so the
 * Chat/History pages are exercisable from a plain browser preview. No
 * network request is ever made, for either target.
 */
export function startWebChatStream(
  conversationId: string | null,
  request: ChatRequest,
  target: ChatTarget,
  onEvent: (event: ChatStreamEvent) => void
): ChatRunHandle {
  if (activeWebRun !== null) {
    throw { code: 'chat.conflict', message: ALREADY_STREAMING_MESSAGE, recoverable: true };
  }

  const resolved = resolveWebTarget(request, target);
  const resolvedConversationId = beginOrContinueWebConversation(conversationId, request.messages);
  const messageId = startWebAssistantMessage(
    resolvedConversationId,
    resolved.providerKey,
    request.modelKey
  );

  const run: WebChatRun = {
    runId: `web-${nextRunId++}`,
    messageId,
    accumulatedText: '',
    cancelled: false
  };
  activeWebRun = run;

  onEvent({ kind: 'started', runId: run.runId, modelKey: request.modelKey });
  scheduleNextWord(run, onEvent, resolved.response.split(' '), 0, 0);

  return { runId: run.runId, conversationId: resolvedConversationId };
}

function resolveWebTarget(request: ChatRequest, target: ChatTarget): WebTarget {
  const promptChars = request.messages.reduce((total, message) => total + message.text.length, 0);
  if (promptChars > MAX_PROMPT_CHARS) {
    throw chatError('chat.invalid', 'The conversation is too long for this model.');
  }

  if (target.kind === 'local') {
    const slotStatus = getWebModelSlotStatus();
    const ownedModelKey =
      slotStatus.ownership.state === 'owned' ? slotStatus.ownership.modelKey : null;
    if (ownedModelKey === null || ownedModelKey !== request.modelKey) {
      throw chatError('chat.invalid', NOT_LOADED_MESSAGE);
    }
    return { providerKey: PROVIDER_KEYS.local, response: SIMULATED_RESPONSE };
  }

  const profile = getWebProviderProfile(target.profileId);
  if (profile.modelKey !== request.modelKey) {
    throw chatError(
      'provider.invalid',
      "The request does not use this provider's configured model."
    );
  }
  if (profile.consent?.endpoint !== profile.endpoint) {
    throw chatError(
      'provider.consent_required',
      'Approve sending this conversation to the provider first.'
    );
  }
  return {
    providerKey: PROVIDER_KEYS.remote,
    response: `This is a simulated remote response from ${profile.label} because Lattice is running as a browser preview without the desktop shell.`
  };
}

function chatError(code: string, message: string): AppError {
  return { code, message, recoverable: true };
}

export function cancelWebChatStream(runId: string): void {
  if (activeWebRun !== null && activeWebRun.runId === runId) {
    activeWebRun.cancelled = true;
  }
}

function scheduleNextWord(
  run: WebChatRun,
  onEvent: (event: ChatStreamEvent) => void,
  words: readonly string[],
  index: number,
  sequence: number
): void {
  setTimeout(() => {
    if (run.cancelled) {
      finalizeWebAssistantMessage(run.messageId, run.accumulatedText, 'cancelled');
      onEvent({ kind: 'cancelled', runId: run.runId, sequence });
      clearActiveRun(run);
      return;
    }

    const word = words[index];
    if (word === undefined) {
      finalizeWebAssistantMessage(run.messageId, run.accumulatedText, 'complete');
      onEvent({ kind: 'completed', runId: run.runId, sequence, finishReason: 'stop' });
      clearActiveRun(run);
      return;
    }

    const text = index === 0 ? word : ` ${word}`;
    run.accumulatedText += text;
    checkpointWebAssistantMessage(run.messageId, run.accumulatedText);
    onEvent({ kind: 'delta', runId: run.runId, sequence, text });
    scheduleNextWord(run, onEvent, words, index + 1, sequence + 1);
  }, WORD_INTERVAL_MS);
}

function clearActiveRun(run: WebChatRun): void {
  if (activeWebRun === run) {
    activeWebRun = null;
  }
}
