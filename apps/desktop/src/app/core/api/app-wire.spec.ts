import type { AppError } from '@lattice/types';

import {
  decodeAppInfo,
  decodeAppSettings,
  normalizeAppError,
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
});
