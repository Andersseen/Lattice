import type { ChatStreamEvent, ChatTarget } from '@lattice/types';
import {
  cancelWebChatStream,
  resetWebChatStreamForTest,
  startWebChatStream
} from './chat-fallback';
import { getWebConversation, resetWebConversationsForTest } from './conversations-fallback';
import { createWebCredential, resetWebCredentialsForTest } from './credentials-fallback';
import {
  configureWebModelRuntime,
  resetWebModelRuntimeStatusForTest,
  startWebModelRuntime
} from './model-runtime-fallback';
import { loadWebModel, resetWebModelSlotStatusForTest } from './model-slot-fallback';
import {
  createWebProviderProfile,
  grantWebProviderConsent,
  resetWebProviderProfilesForTest
} from './provider-profiles-fallback';

const MODEL_KEY = 'qwen/qwen2.5-0.5b-instruct';
const REMOTE_MODEL_KEY = 'gpt-test';
const LOCAL: ChatTarget = { kind: 'local' };

describe('chat streaming browser fallback', () => {
  beforeEach(() => {
    resetWebModelRuntimeStatusForTest();
    resetWebModelSlotStatusForTest();
    resetWebChatStreamForTest();
    resetWebConversationsForTest();
    resetWebCredentialsForTest();
    resetWebProviderProfilesForTest();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function loadModel(): void {
    const configured = configureWebModelRuntime({
      expectedRevision: 1,
      executablePath: '/usr/local/bin/lms'
    });
    startWebModelRuntime({ expectedRevision: configured.revision });
    loadWebModel({ expectedRevision: 1, modelKey: MODEL_KEY });
  }

  it('refuses a request naming a model that is not owned and loaded', () => {
    expect(() =>
      startWebChatStream(
        null,
        { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
        LOCAL,
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'chat.invalid' }));
  });

  it('streams started, delta, and completed events for the owned model', async () => {
    loadModel();
    const events: ChatStreamEvent[] = [];

    const handle = startWebChatStream(
      null,
      { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
      LOCAL,
      (event) => events.push(event)
    );

    await vi.runAllTimersAsync();

    expect(events[0]).toEqual({ kind: 'started', runId: handle.runId, modelKey: MODEL_KEY });
    expect(events.length).toBeGreaterThan(2);
    expect(events.every((event) => event.runId === handle.runId)).toBe(true);
    expect(events.at(-1)).toEqual(
      expect.objectContaining({ kind: 'completed', finishReason: 'stop' })
    );
  });

  it('refuses a second run while one is already streaming', async () => {
    loadModel();
    startWebChatStream(
      null,
      { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
      LOCAL,
      () => {}
    );

    expect(() =>
      startWebChatStream(
        null,
        { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
        LOCAL,
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'chat.conflict' }));

    await vi.runAllTimersAsync();
  });

  it('cancelling mid-stream ends the run with exactly one cancelled event', async () => {
    loadModel();
    const events: ChatStreamEvent[] = [];

    const handle = startWebChatStream(
      null,
      { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
      LOCAL,
      (event) => events.push(event)
    );

    await vi.advanceTimersByTimeAsync(60);
    cancelWebChatStream(handle.runId);
    await vi.runAllTimersAsync();

    const terminalEvents = events.filter(
      (event) => event.kind === 'completed' || event.kind === 'cancelled' || event.kind === 'failed'
    );
    expect(terminalEvents).toEqual([{ kind: 'cancelled', runId: handle.runId, sequence: 1 }]);
  });

  it('cancelling an unknown run id is a no-op', () => {
    expect(() => cancelWebChatStream('not-a-real-run')).not.toThrow();
  });

  function remoteTarget(consented: boolean): ChatTarget {
    const credential = createWebCredential({ label: 'Key', providerKey: 'openai' });
    const profile = createWebProviderProfile({
      label: 'Example',
      endpoint: 'https://api.example.com/v1',
      modelKey: REMOTE_MODEL_KEY,
      credentialId: credential.id
    });
    if (consented) {
      grantWebProviderConsent({
        id: profile.id,
        expectedRevision: profile.revision,
        endpoint: profile.endpoint
      });
    }
    return { kind: 'remote', profileId: profile.id };
  }

  it('streams through a consented remote profile without a loaded local model', async () => {
    const target = remoteTarget(true);
    const events: ChatStreamEvent[] = [];

    const handle = startWebChatStream(
      null,
      { modelKey: REMOTE_MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
      target,
      (event) => events.push(event)
    );
    await vi.runAllTimersAsync();

    expect(events.at(-1)).toEqual(expect.objectContaining({ kind: 'completed' }));
    const detail = getWebConversation({ conversationId: handle.conversationId });
    expect(detail.messages.at(-1)).toEqual(
      expect.objectContaining({
        providerKey: 'remote-openai-compatible',
        modelKey: REMOTE_MODEL_KEY,
        status: 'complete'
      })
    );
  });

  it('refuses a remote request without consent and persists nothing', () => {
    const target = remoteTarget(false);

    expect(() =>
      startWebChatStream(
        null,
        { modelKey: REMOTE_MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
        target,
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'provider.consent_required' }));
    expect(() => getWebConversation({ conversationId: 'web-conversation-1' })).toThrow(
      expect.objectContaining({ code: 'conversation.not_found' })
    );
  });

  it('refuses a remote request naming a different model than the profile', () => {
    const target = remoteTarget(true);

    expect(() =>
      startWebChatStream(
        null,
        { modelKey: 'other-model', messages: [{ role: 'user', text: 'hi' }] },
        target,
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'provider.invalid' }));
  });

  it('keeps provenance when one conversation switches local, remote, then local', async () => {
    loadModel();
    const remote = remoteTarget(true);
    const transcript: { role: 'user' | 'assistant'; text: string }[] = [];
    let conversationId: string | null = null;

    for (const [target, modelKey, text] of [
      [LOCAL, MODEL_KEY, 'first'],
      [remote, REMOTE_MODEL_KEY, 'second'],
      [LOCAL, MODEL_KEY, 'third']
    ] as const) {
      transcript.push({ role: 'user', text });
      const handle = startWebChatStream(
        conversationId,
        { modelKey, messages: [...transcript] },
        target,
        () => {}
      );
      conversationId = handle.conversationId;
      await vi.runAllTimersAsync();
      const detail = getWebConversation({ conversationId });
      transcript.push({ role: 'assistant', text: detail.messages.at(-1)?.text ?? '' });
    }

    const detail = getWebConversation({ conversationId: conversationId ?? '' });
    expect(
      detail.messages
        .filter((message) => message.role === 'assistant')
        .map((message) => [message.providerKey, message.modelKey])
    ).toEqual([
      ['local-openai-compatible', MODEL_KEY],
      ['remote-openai-compatible', REMOTE_MODEL_KEY],
      ['local-openai-compatible', MODEL_KEY]
    ]);
    expect(detail.messages).toHaveLength(6);
  });
});
