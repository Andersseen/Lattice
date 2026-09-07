import {
  configureWebModelRuntime,
  getWebModelRuntimeStatus,
  probeWebModelRuntime,
  resetWebModelRuntimeStatusForTest
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
});
