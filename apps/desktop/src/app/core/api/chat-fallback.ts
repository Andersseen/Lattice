import type { ChatRequest, ChatRunHandle, ChatStreamEvent } from '@lattice/types';

import {
  beginOrContinueWebConversation,
  checkpointWebAssistantMessage,
  finalizeWebAssistantMessage,
  startWebAssistantMessage
} from './conversations-fallback';
import { getWebModelSlotStatus } from './model-slot-fallback';

const NOT_LOADED_MESSAGE = 'Load the requested model before starting a chat.';
const ALREADY_STREAMING_MESSAGE = 'A response is already streaming.';
const SIMULATED_RESPONSE =
  'This is a simulated response because Lattice is running as a browser preview without the desktop shell.';
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

/**
 * Simulates the same lease/single-run preconditions `start_chat_stream`
 * enforces natively, persists the new message(s) and a streaming assistant
 * reply through the shared conversations fallback, then streams a canned
 * response word-by-word through `onEvent` so the Chat/History pages are
 * exercisable from a plain browser preview.
 */
export function startWebChatStream(
  conversationId: string | null,
  request: ChatRequest,
  onEvent: (event: ChatStreamEvent) => void
): ChatRunHandle {
  if (activeWebRun !== null) {
    throw { code: 'chat.conflict', message: ALREADY_STREAMING_MESSAGE, recoverable: true };
  }

  const slotStatus = getWebModelSlotStatus();
  const ownedModelKey =
    slotStatus.ownership.state === 'owned' ? slotStatus.ownership.modelKey : null;
  if (ownedModelKey === null || ownedModelKey !== request.modelKey) {
    throw { code: 'chat.invalid', message: NOT_LOADED_MESSAGE, recoverable: true };
  }

  const resolvedConversationId = beginOrContinueWebConversation(conversationId, request.messages);
  const messageId = startWebAssistantMessage(resolvedConversationId, request.modelKey);

  const run: WebChatRun = {
    runId: `web-${nextRunId++}`,
    messageId,
    accumulatedText: '',
    cancelled: false
  };
  activeWebRun = run;

  onEvent({ kind: 'started', runId: run.runId, modelKey: request.modelKey });
  scheduleNextWord(run, onEvent, SIMULATED_RESPONSE.split(' '), 0, 0);

  return { runId: run.runId, conversationId: resolvedConversationId };
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
