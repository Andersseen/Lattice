import type {
  AppError,
  AppInfo,
  AppSettings,
  ModelRuntimeStatus,
  NativeAppInfo
} from '@lattice/types';

const SAFE_APP_INFO_ERROR = 'Lattice could not read application information.';
const SAFE_APP_SETTINGS_ERROR = 'Lattice could not read application settings.';
const SAFE_MODEL_RUNTIME_ERROR = 'Lattice could not read model runtime status.';
const MAX_SAFE_MESSAGE_LENGTH = 240;

export function decodeAppInfo(value: unknown): AppInfo {
  if (isNativeAppInfo(value)) {
    return value;
  }

  throw createBridgeError(SAFE_APP_INFO_ERROR);
}

export function decodeAppSettings(value: unknown): AppSettings {
  if (isAppSettings(value)) {
    return value;
  }

  throw createBridgeError(SAFE_APP_SETTINGS_ERROR);
}

export function decodeModelRuntimeStatus(value: unknown): ModelRuntimeStatus {
  if (isModelRuntimeStatus(value)) {
    return value;
  }

  throw createBridgeError(SAFE_MODEL_RUNTIME_ERROR);
}

export function normalizeAppError(error: unknown, fallbackMessage = SAFE_APP_INFO_ERROR): AppError {
  if (isAppError(error)) {
    const safeError: AppError = {
      code: error.code,
      message: safeMessage(error.message),
      recoverable: error.recoverable
    };

    if (error.correlationId !== undefined) {
      return {
        ...safeError,
        correlationId: error.correlationId
      };
    }

    return safeError;
  }

  return createBridgeError(fallbackMessage);
}

export function normalizeSettingsError(error: unknown): AppError {
  return normalizeAppError(error, SAFE_APP_SETTINGS_ERROR);
}

export function normalizeModelRuntimeError(error: unknown): AppError {
  return normalizeAppError(error, SAFE_MODEL_RUNTIME_ERROR);
}

function createBridgeError(message: string): AppError {
  return {
    code: 'bridge.unknown',
    message,
    recoverable: true
  };
}

function isNativeAppInfo(value: unknown): value is NativeAppInfo {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['name'] === 'string' &&
    typeof value['version'] === 'string' &&
    isBuildProfile(value['buildProfile']) &&
    value['runtime'] === 'tauri' &&
    typeof value['target'] === 'string'
  );
}

function isBuildProfile(value: unknown): value is NativeAppInfo['buildProfile'] {
  return value === 'debug' || value === 'release';
}

function isAppSettings(value: unknown): value is AppSettings {
  if (!isRecord(value)) {
    return false;
  }

  return (
    isPositiveInteger(value['schemaVersion']) &&
    isPositiveInteger(value['revision']) &&
    isAppearancePreference(value['appearance']) &&
    isIdleUnloadMinutes(value['idleUnloadMinutes'])
  );
}

function isAppearancePreference(value: unknown): value is AppSettings['appearance'] {
  return value === 'system' || value === 'light' || value === 'dark';
}

function isIdleUnloadMinutes(value: unknown): boolean {
  return Number.isInteger(value) && Number(value) >= 1 && Number(value) <= 120;
}

function isPositiveInteger(value: unknown): boolean {
  return Number.isInteger(value) && Number(value) >= 1;
}

function isModelRuntimeStatus(value: unknown): value is ModelRuntimeStatus {
  if (!isRecord(value)) {
    return false;
  }

  return (
    isPositiveInteger(value['revision']) &&
    optionalString(value['executablePath']) &&
    isModelRuntimeAvailability(value['availability']) &&
    optionalString(value['cliVersion']) &&
    optionalRuntimeProbeApproval(value['approved']) &&
    isRuntimeDaemonObservation(value['daemon']) &&
    isRuntimeServerObservation(value['server']) &&
    isRuntimeOwnership(value['ownership']) &&
    optionalRuntimeOperationOutcome(value['lastOperation']) &&
    optionalPositiveInteger(value['lastCheckedUnixSeconds']) &&
    typeof value['message'] === 'string'
  );
}

function isRuntimeOwnership(value: unknown): value is ModelRuntimeStatus['ownership'] {
  if (!isRecord(value)) {
    return false;
  }

  if (value['state'] === 'attached' || value['state'] === 'unknown') {
    return true;
  }

  return (
    value['state'] === 'owned' &&
    isPositiveInteger(value['daemonPid']) &&
    typeof value['executableFingerprint'] === 'string' &&
    isPositiveInteger(value['ownedSinceUnixSeconds'])
  );
}

function optionalRuntimeOperationOutcome(value: unknown): boolean {
  if (value === undefined) {
    return true;
  }

  return (
    value === 'started' ||
    value === 'alreadyRunning' ||
    value === 'stopped' ||
    value === 'alreadyStopped' ||
    value === 'refused' ||
    value === 'cancelled' ||
    value === 'timedOut' ||
    value === 'failed'
  );
}

function isModelRuntimeAvailability(value: unknown): boolean {
  return (
    value === 'missing' ||
    value === 'unsupported' ||
    value === 'stopped' ||
    value === 'running' ||
    value === 'unreachable' ||
    value === 'unknown'
  );
}

function optionalRuntimeProbeApproval(value: unknown): boolean {
  if (value === undefined) {
    return true;
  }

  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['executableFingerprint'] === 'string' &&
    typeof value['cliVersion'] === 'string' &&
    isPositiveInteger(value['checkedAtUnixSeconds'])
  );
}

function isRuntimeDaemonObservation(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }

  return (
    (value['status'] === 'running' ||
      value['status'] === 'notRunning' ||
      value['status'] === 'unknown') &&
    optionalPositiveInteger(value['pid']) &&
    optionalBoolean(value['isDaemon']) &&
    optionalString(value['version'])
  );
}

function isRuntimeServerObservation(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }

  return (
    (value['status'] === 'running' ||
      value['status'] === 'stopped' ||
      value['status'] === 'unreachable' ||
      value['status'] === 'unknown') &&
    optionalPositiveInteger(value['port']) &&
    optionalString(value['endpoint'])
  );
}

function optionalString(value: unknown): boolean {
  return value === undefined || typeof value === 'string';
}

function optionalBoolean(value: unknown): boolean {
  return value === undefined || typeof value === 'boolean';
}

function optionalPositiveInteger(value: unknown): boolean {
  return value === undefined || isPositiveInteger(value);
}

function isAppError(value: unknown): value is AppError {
  if (!isRecord(value)) {
    return false;
  }

  const correlationId = value['correlationId'];

  return (
    typeof value['code'] === 'string' &&
    typeof value['message'] === 'string' &&
    typeof value['recoverable'] === 'boolean' &&
    (correlationId === undefined || typeof correlationId === 'string')
  );
}

function safeMessage(message: string): string {
  return message.length > MAX_SAFE_MESSAGE_LENGTH
    ? `${message.slice(0, MAX_SAFE_MESSAGE_LENGTH)}...`
    : message;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
