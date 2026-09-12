import type {
  AppError,
  AppInfo,
  AppSettings,
  ChatFinishReason,
  ChatRunHandle,
  ChatStreamEvent,
  ModelDescriptor,
  ModelRuntimeStatus,
  ModelSlotStatus,
  NativeAppInfo
} from '@lattice/types';

const SAFE_APP_INFO_ERROR = 'Lattice could not read application information.';
const SAFE_APP_SETTINGS_ERROR = 'Lattice could not read application settings.';
const SAFE_MODEL_RUNTIME_ERROR = 'Lattice could not read model runtime status.';
const SAFE_MODEL_SLOT_ERROR = 'Lattice could not read model slot status.';
const SAFE_CHAT_ERROR = 'Lattice could not start the chat response.';
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

export function decodeModelSlotStatus(value: unknown): ModelSlotStatus {
  if (isModelSlotStatus(value)) {
    return value;
  }

  throw createBridgeError(SAFE_MODEL_SLOT_ERROR);
}

export function decodeChatRunHandle(value: unknown): ChatRunHandle {
  if (isChatRunHandle(value)) {
    return value;
  }

  throw createBridgeError(SAFE_CHAT_ERROR);
}

/**
 * Unlike the other `decode*` functions, this never throws: a chat stream
 * channel delivers many events over one run's lifetime, and one malformed
 * event must not stop delivery of the ones after it (application-api spec,
 * "Stream Events Are Decoded Defensively"). Callers treat `null` as one
 * skippable malformed event, not a terminal failure of the run.
 */
export function decodeChatStreamEvent(value: unknown): ChatStreamEvent | null {
  return isChatStreamEvent(value) ? value : null;
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

export function normalizeModelSlotError(error: unknown): AppError {
  return normalizeAppError(error, SAFE_MODEL_SLOT_ERROR);
}

export function normalizeChatError(error: unknown): AppError {
  return normalizeAppError(error, SAFE_CHAT_ERROR);
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

function isModelSlotStatus(value: unknown): value is ModelSlotStatus {
  if (!isRecord(value)) {
    return false;
  }

  return (
    isPositiveInteger(value['revision']) &&
    Array.isArray(value['installed']) &&
    value['installed'].every(isModelDescriptor) &&
    optionalLoadedModelObservation(value['loaded']) &&
    isModelLoadOwnership(value['ownership']) &&
    optionalModelOperationOutcome(value['lastOperation']) &&
    optionalPositiveInteger(value['lastCheckedUnixSeconds']) &&
    typeof value['message'] === 'string'
  );
}

function isModelDescriptor(value: unknown): value is ModelDescriptor {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['modelKey'] === 'string' &&
    typeof value['displayName'] === 'string' &&
    optionalString(value['architecture']) &&
    typeof value['isLlm'] === 'boolean' &&
    optionalPositiveInteger(value['sizeBytes'])
  );
}

function optionalLoadedModelObservation(value: unknown): boolean {
  if (value === undefined) {
    return true;
  }

  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['identifier'] === 'string' &&
    typeof value['modelKey'] === 'string' &&
    optionalString(value['architecture']) &&
    optionalPositiveInteger(value['sizeBytes'])
  );
}

function isModelLoadOwnership(value: unknown): value is ModelSlotStatus['ownership'] {
  if (!isRecord(value)) {
    return false;
  }

  if (value['state'] === 'unknown') {
    return true;
  }

  if (typeof value['identifier'] !== 'string' || typeof value['modelKey'] !== 'string') {
    return false;
  }

  if (value['state'] === 'attached') {
    return true;
  }

  return value['state'] === 'owned' && isPositiveInteger(value['loadedSinceUnixSeconds']);
}

function optionalModelOperationOutcome(value: unknown): boolean {
  if (value === undefined) {
    return true;
  }

  return (
    value === 'loaded' ||
    value === 'alreadyLoaded' ||
    value === 'unloaded' ||
    value === 'alreadyUnloaded' ||
    value === 'refused' ||
    value === 'cancelled' ||
    value === 'timedOut' ||
    value === 'failed'
  );
}

function isChatRunHandle(value: unknown): value is ChatRunHandle {
  if (!isRecord(value)) {
    return false;
  }

  return typeof value['runId'] === 'string';
}

function isChatStreamEvent(value: unknown): value is ChatStreamEvent {
  if (!isRecord(value)) {
    return false;
  }

  const runId = value['runId'];
  if (typeof runId !== 'string') {
    return false;
  }

  switch (value['kind']) {
    case 'started':
      return typeof value['modelKey'] === 'string';
    case 'delta':
      return isNonNegativeInteger(value['sequence']) && typeof value['text'] === 'string';
    case 'completed':
      return isNonNegativeInteger(value['sequence']) && isChatFinishReason(value['finishReason']);
    case 'cancelled':
      return isNonNegativeInteger(value['sequence']);
    case 'failed':
      return isNonNegativeInteger(value['sequence']) && isAppError(value['error']);
    default:
      return false;
  }
}

function isChatFinishReason(value: unknown): value is ChatFinishReason {
  return value === 'stop' || value === 'maxOutputTokens';
}

function isNonNegativeInteger(value: unknown): boolean {
  return Number.isInteger(value) && Number(value) >= 0;
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
