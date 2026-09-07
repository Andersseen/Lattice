import type {
  AppSettings,
  ResetAppSettingsRequest,
  UpdateAppSettingsRequest
} from '@lattice/types';

let webSettings: AppSettings = createDefaultWebSettings();

export function getWebSettings(): AppSettings {
  return webSettings;
}

export function updateWebSettings(request: UpdateAppSettingsRequest): AppSettings {
  ensureExpectedRevision(request.expectedRevision);

  if (request.idleUnloadMinutes !== undefined && !isIdleUnloadMinutes(request.idleUnloadMinutes)) {
    throw invalidSettingsError();
  }

  webSettings = {
    ...webSettings,
    revision: webSettings.revision + 1,
    ...(request.appearance !== undefined ? { appearance: request.appearance } : {}),
    ...(request.idleUnloadMinutes !== undefined
      ? { idleUnloadMinutes: request.idleUnloadMinutes }
      : {})
  };

  return webSettings;
}

export function resetWebSettings(request: ResetAppSettingsRequest): AppSettings {
  ensureExpectedRevision(request.expectedRevision);
  webSettings = {
    ...createDefaultWebSettings(),
    revision: webSettings.revision + 1
  };

  return webSettings;
}

function createDefaultWebSettings(): AppSettings {
  return {
    schemaVersion: 2,
    revision: 1,
    appearance: 'system',
    idleUnloadMinutes: 5
  };
}

function ensureExpectedRevision(expectedRevision: number): void {
  if (expectedRevision !== webSettings.revision) {
    throw {
      code: 'settings.conflict',
      message: 'Settings changed before this update could be saved.',
      recoverable: true
    };
  }
}

function invalidSettingsError(): never {
  throw {
    code: 'settings.invalid',
    message: 'Idle unload must be between 1 and 120 minutes.',
    recoverable: true
  };
}

function isIdleUnloadMinutes(value: number): boolean {
  return Number.isInteger(value) && value >= 1 && value <= 120;
}
