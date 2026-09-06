import type { AppError, AppInfo, NativeAppInfo } from '@lattice/types';

const SAFE_APP_INFO_ERROR = 'Lattice could not read application information.';
const MAX_SAFE_MESSAGE_LENGTH = 240;

export function decodeAppInfo(value: unknown): AppInfo {
  if (isNativeAppInfo(value)) {
    return value;
  }

  throw createBridgeError(SAFE_APP_INFO_ERROR);
}

export function normalizeAppError(error: unknown): AppError {
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

  return createBridgeError(SAFE_APP_INFO_ERROR);
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
