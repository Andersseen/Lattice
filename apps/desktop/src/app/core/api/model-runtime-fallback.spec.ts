import {
  configureWebModelRuntime,
  getWebModelRuntimeStatus,
  probeWebModelRuntime,
  resetWebModelRuntimeStatusForTest,
  startWebModelRuntime,
  stopWebModelRuntime
} from './model-runtime-fallback';

describe('model runtime browser fallback', () => {
  beforeEach(() => {
    resetWebModelRuntimeStatusForTest();
  });

  it('configures and probes runtime status in browser smoke mode', () => {
    const current = getWebModelRuntimeStatus();
    const configured = configureWebModelRuntime({
      expectedRevision: current.revision,
      executablePath: '/usr/local/bin/lms'
    });

    expect(configured).toEqual(
      expect.objectContaining({
        revision: current.revision + 1,
        executablePath: '/usr/local/bin/lms',
        availability: 'unknown'
      })
    );

    const probed = probeWebModelRuntime({
      expectedRevision: configured.revision
    });

    expect(probed).toEqual(
      expect.objectContaining({
        revision: configured.revision + 1,
        availability: 'stopped',
        cliVersion: '0.0.47',
        daemon: {
          status: 'notRunning'
        },
        server: {
          status: 'stopped'
        }
      })
    );
  });

  it('rejects relative runtime paths', () => {
    const current = getWebModelRuntimeStatus();

    expect(() =>
      configureWebModelRuntime({
        expectedRevision: current.revision,
        executablePath: 'bin/lms'
      })
    ).toThrow(
      expect.objectContaining({
        code: 'runtime.invalid'
      })
    );
  });

  it('rejects stale runtime revisions', () => {
    const current = getWebModelRuntimeStatus();

    expect(() =>
      probeWebModelRuntime({
        expectedRevision: current.revision + 1
      })
    ).toThrow(
      expect.objectContaining({
        code: 'runtime.conflict'
      })
    );
  });

  it('starting a configured runtime reports owned ownership', () => {
    const configured = configureWebModelRuntime({
      expectedRevision: getWebModelRuntimeStatus().revision,
      executablePath: '/usr/local/bin/lms'
    });

    const started = startWebModelRuntime({ expectedRevision: configured.revision });

    expect(started.availability).toBe('running');
    expect(started.ownership).toEqual(expect.objectContaining({ state: 'owned', daemonPid: 1 }));
    expect(started.lastOperation).toBe('started');
  });

  it('stopping an owned runtime returns ownership to unknown', () => {
    const configured = configureWebModelRuntime({
      expectedRevision: getWebModelRuntimeStatus().revision,
      executablePath: '/usr/local/bin/lms'
    });
    const started = startWebModelRuntime({ expectedRevision: configured.revision });

    const stopped = stopWebModelRuntime({ expectedRevision: started.revision });

    expect(stopped.availability).toBe('stopped');
    expect(stopped.ownership).toEqual({ state: 'unknown' });
    expect(stopped.lastOperation).toBe('stopped');
  });

  it('stopping a runtime that is not owned reports refused', () => {
    const configured = configureWebModelRuntime({
      expectedRevision: getWebModelRuntimeStatus().revision,
      executablePath: '/usr/local/bin/lms'
    });

    const stopped = stopWebModelRuntime({ expectedRevision: configured.revision });

    expect(stopped.lastOperation).toBe('refused');
  });
});
