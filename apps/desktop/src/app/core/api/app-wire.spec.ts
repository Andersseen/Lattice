import type { AppError } from '@lattice/types';

import {
  decodeAppInfo,
  decodeAppSettings,
  decodeModelRuntimeStatus,
  normalizeAppError,
  normalizeModelRuntimeError,
  normalizeSettingsError
} from './app-wire';

describe('app wire boundary', () => {
  it('decodes native app info from untrusted IPC payloads', () => {
    expect(
      decodeAppInfo({
        name: 'Lattice',
        version: '0.1.0',
        buildProfile: 'debug',
        runtime: 'tauri',
        target: 'macos'
      })
    ).toEqual({
      name: 'Lattice',
      version: '0.1.0',
      buildProfile: 'debug',
      runtime: 'tauri',
      target: 'macos'
    });
  });

  it('turns malformed app info into one safe bridge error', () => {
    expect(() => decodeAppInfo({ name: 'Lattice', runtime: 'web' })).toThrow(
      expect.objectContaining<AppError>({
        code: 'bridge.unknown',
        message: 'Lattice could not read application information.',
        recoverable: true
      })
    );
  });

  it('decodes native app settings from untrusted IPC payloads', () => {
    expect(
      decodeAppSettings({
        schemaVersion: 1,
        revision: 3,
        appearance: 'dark',
        idleUnloadMinutes: 10
      })
    ).toEqual({
      schemaVersion: 1,
      revision: 3,
      appearance: 'dark',
      idleUnloadMinutes: 10
    });
  });

  it('turns malformed app settings into one safe bridge error', () => {
    expect(() =>
      decodeAppSettings({
        schemaVersion: 1,
        revision: 1,
        appearance: 'neon',
        idleUnloadMinutes: 5
      })
    ).toThrow(
      expect.objectContaining<AppError>({
        code: 'bridge.unknown',
        message: 'Lattice could not read application settings.',
        recoverable: true
      })
    );
  });

  it('decodes model runtime status from untrusted IPC payloads', () => {
    expect(
      decodeModelRuntimeStatus({
        revision: 4,
        executablePath: '/usr/local/bin/lms',
        availability: 'running',
        cliVersion: '0.0.47',
        approved: {
          executableFingerprint: '/usr/local/bin/lms:42:1',
          cliVersion: '0.0.47',
          checkedAtUnixSeconds: 123
        },
        daemon: {
          status: 'running',
          pid: 12345,
          isDaemon: true,
          version: '0.4.4+1'
        },
        server: {
          status: 'running',
          port: 1234,
          endpoint: 'http://127.0.0.1:1234'
        },
        lastCheckedUnixSeconds: 123,
        message: 'Runtime discovery completed.'
      })
    ).toEqual({
      revision: 4,
      executablePath: '/usr/local/bin/lms',
      availability: 'running',
      cliVersion: '0.0.47',
      approved: {
        executableFingerprint: '/usr/local/bin/lms:42:1',
        cliVersion: '0.0.47',
        checkedAtUnixSeconds: 123
      },
      daemon: {
        status: 'running',
        pid: 12345,
        isDaemon: true,
        version: '0.4.4+1'
      },
      server: {
        status: 'running',
        port: 1234,
        endpoint: 'http://127.0.0.1:1234'
      },
      lastCheckedUnixSeconds: 123,
      message: 'Runtime discovery completed.'
    });
  });

  it('rejects malformed model runtime status payloads', () => {
    expect(() =>
      decodeModelRuntimeStatus({
        revision: 1,
        availability: 'started',
        daemon: {
          status: 'unknown'
        },
        server: {
          status: 'unknown'
        },
        message: 'bad status'
      })
    ).toThrow(
      expect.objectContaining<AppError>({
        code: 'bridge.unknown',
        message: 'Lattice could not read model runtime status.',
        recoverable: true
      })
    );
  });

  it('normalizes unknown errors without exposing raw internals', () => {
    expect(normalizeAppError(new Error('secret path /tmp/lattice'))).toEqual({
      code: 'bridge.unknown',
      message: 'Lattice could not read application information.',
      recoverable: true
    });
  });

  it('keeps checked app errors and bounds the safe message', () => {
    expect(
      normalizeAppError({
        code: 'core.unavailable',
        message: 'x'.repeat(300),
        recoverable: true,
        correlationId: 'trace-1'
      })
    ).toEqual({
      code: 'core.unavailable',
      message: `${'x'.repeat(240)}...`,
      recoverable: true,
      correlationId: 'trace-1'
    });
  });

  it('normalizes unknown settings errors with the settings fallback message', () => {
    expect(normalizeSettingsError(new Error('raw sqlite failure'))).toEqual({
      code: 'bridge.unknown',
      message: 'Lattice could not read application settings.',
      recoverable: true
    });
  });

  it('normalizes unknown model runtime errors with the runtime fallback message', () => {
    expect(normalizeModelRuntimeError(new Error('raw process failure'))).toEqual({
      code: 'bridge.unknown',
      message: 'Lattice could not read model runtime status.',
      recoverable: true
    });
  });
});
