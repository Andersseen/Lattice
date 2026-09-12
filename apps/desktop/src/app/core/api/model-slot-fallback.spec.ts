import {
  configureWebModelRuntime,
  resetWebModelRuntimeStatusForTest,
  startWebModelRuntime
} from './model-runtime-fallback';
import {
  cancelWebModelOperation,
  getWebModelSlotStatus,
  loadWebModel,
  resetWebModelSlotStatusForTest,
  unloadWebModel
} from './model-slot-fallback';

describe('model slot browser fallback', () => {
  beforeEach(() => {
    resetWebModelRuntimeStatusForTest();
    resetWebModelSlotStatusForTest();
  });

  function startWebRuntime(): void {
    const configured = configureWebModelRuntime({
      expectedRevision: 1,
      executablePath: '/usr/local/bin/lms'
    });
    startWebModelRuntime({ expectedRevision: configured.revision });
  }

  it('reports an unavailable slot without a running runtime', () => {
    const status = getWebModelSlotStatus();

    expect(status.installed).toEqual([]);
    expect(status.ownership).toEqual({ state: 'unknown' });
    expect(status.message).toContain('Start the runtime');
  });

  it('lists candidate installed models once the runtime is running', () => {
    startWebRuntime();

    const status = getWebModelSlotStatus();

    expect(status.installed.length).toBeGreaterThan(0);
    expect(status.loaded).toBeUndefined();
  });

  it('loads an installed model into the empty slot', () => {
    startWebRuntime();
    const current = getWebModelSlotStatus();

    const loaded = loadWebModel({
      expectedRevision: current.revision,
      modelKey: 'qwen/qwen2.5-0.5b-instruct'
    });

    expect(loaded.loaded?.modelKey).toBe('qwen/qwen2.5-0.5b-instruct');
    expect(loaded.ownership).toEqual(
      expect.objectContaining({ state: 'owned', modelKey: 'qwen/qwen2.5-0.5b-instruct' })
    );
    expect(loaded.lastOperation).toBe('loaded');
  });

  it('refuses loading a different model while the slot is occupied', () => {
    startWebRuntime();
    const first = loadWebModel({
      expectedRevision: getWebModelSlotStatus().revision,
      modelKey: 'qwen/qwen2.5-0.5b-instruct'
    });

    const second = loadWebModel({
      expectedRevision: first.revision,
      modelKey: 'google/gemma-2-2b-it'
    });

    expect(second.lastOperation).toBe('refused');
    expect(second.loaded?.modelKey).toBe('qwen/qwen2.5-0.5b-instruct');
  });

  it('unloads an owned model and returns ownership to unknown', () => {
    startWebRuntime();
    const loaded = loadWebModel({
      expectedRevision: getWebModelSlotStatus().revision,
      modelKey: 'qwen/qwen2.5-0.5b-instruct'
    });

    const unloaded = unloadWebModel({ expectedRevision: loaded.revision });

    expect(unloaded.loaded).toBeUndefined();
    expect(unloaded.ownership).toEqual({ state: 'unknown' });
    expect(unloaded.lastOperation).toBe('unloaded');
  });

  it('reports unloading an empty slot as already unloaded', () => {
    startWebRuntime();

    const unloaded = unloadWebModel({ expectedRevision: getWebModelSlotStatus().revision });

    expect(unloaded.lastOperation).toBe('alreadyUnloaded');
  });

  it('rejects stale slot revisions', () => {
    startWebRuntime();
    const current = getWebModelSlotStatus();

    expect(() =>
      loadWebModel({
        expectedRevision: current.revision + 1,
        modelKey: 'qwen/qwen2.5-0.5b-instruct'
      })
    ).toThrow(expect.objectContaining({ code: 'runtime.conflict' }));
  });

  it('rejects load and unload while the runtime is not running', () => {
    expect(() =>
      loadWebModel({ expectedRevision: 1, modelKey: 'qwen/qwen2.5-0.5b-instruct' })
    ).toThrow(expect.objectContaining({ code: 'runtime.invalid' }));
    expect(() => unloadWebModel({ expectedRevision: 1 })).toThrow(
      expect.objectContaining({ code: 'runtime.invalid' })
    );
  });

  it('cancelling a model operation never throws', () => {
    expect(() => cancelWebModelOperation()).not.toThrow();
  });
});
