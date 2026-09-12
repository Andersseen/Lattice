import type { ChatStreamEvent } from '@lattice/types';

import {
  configureWebModelRuntime,
  resetWebModelRuntimeStatusForTest,
  startWebModelRuntime
} from './model-runtime-fallback';
import { loadWebModel, resetWebModelSlotStatusForTest } from './model-slot-fallback';
import {
  cancelWebChatStream,
  resetWebChatStreamForTest,
  startWebChatStream
} from './chat-fallback';

const MODEL_KEY = 'qwen/qwen2.5-0.5b-instruct';

describe('chat streaming browser fallback', () => {
  beforeEach(() => {
    resetWebModelRuntimeStatusForTest();
    resetWebModelSlotStatusForTest();
    resetWebChatStreamForTest();
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
        { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'chat.invalid' }));
  });

  it('streams started, delta, and completed events for the owned model', async () => {
    loadModel();
    const events: ChatStreamEvent[] = [];

    const handle = startWebChatStream(
      { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
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
    startWebChatStream({ modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] }, () => {});

    expect(() =>
      startWebChatStream(
        { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
        () => {}
      )
    ).toThrow(expect.objectContaining({ code: 'chat.conflict' }));

    await vi.runAllTimersAsync();
  });

  it('cancelling mid-stream ends the run with exactly one cancelled event', async () => {
    loadModel();
    const events: ChatStreamEvent[] = [];

    const handle = startWebChatStream(
      { modelKey: MODEL_KEY, messages: [{ role: 'user', text: 'hi' }] },
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
});
