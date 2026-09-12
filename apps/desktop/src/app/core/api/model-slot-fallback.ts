import type {
  LoadModelRequest,
  LoadedModelObservation,
  ModelDescriptor,
  ModelOperationOutcome,
  ModelSlotStatus,
  UnloadModelRequest
} from '@lattice/types';

import { getWebModelRuntimeStatus } from './model-runtime-fallback';

const LATTICE_MANAGED_IDENTIFIER = 'lattice-managed';
const NOT_RUNNING_MESSAGE = 'Start the runtime before managing models.';
const READY_MESSAGE = 'Model inventory refreshed.';

const CANDIDATE_MODELS: readonly ModelDescriptor[] = [
  {
    modelKey: 'qwen/qwen2.5-0.5b-instruct',
    displayName: 'Qwen2.5 0.5B Instruct',
    architecture: 'qwen2',
    isLlm: true,
    sizeBytes: 397_000_000
  },
  {
    modelKey: 'google/gemma-2-2b-it',
    displayName: 'Gemma 2 2B IT',
    architecture: 'gemma2',
    isLlm: true,
    sizeBytes: 1_500_000_000
  }
];

interface InternalSlotState {
  readonly revision: number;
  readonly loaded: LoadedModelObservation | null;
  readonly ownership: ModelSlotStatus['ownership'];
  readonly lastOperation: ModelOperationOutcome | null;
}

let webSlotState: InternalSlotState = createUnknownSlotState(1);

export function resetWebModelSlotStatusForTest(): void {
  webSlotState = createUnknownSlotState(1);
}

export function getWebModelSlotStatus(): ModelSlotStatus {
  return composeWebModelSlotStatus();
}

export function loadWebModel(request: LoadModelRequest): ModelSlotStatus {
  ensureExpectedRevision(request.expectedRevision);
  ensureRuntimeRunning();

  if (webSlotState.loaded !== null) {
    const alreadyLoaded =
      webSlotState.ownership.state === 'owned' &&
      webSlotState.ownership.modelKey === request.modelKey;
    webSlotState = {
      ...webSlotState,
      revision: webSlotState.revision + 1,
      lastOperation: alreadyLoaded ? 'alreadyLoaded' : 'refused'
    };
    return composeWebModelSlotStatus();
  }

  const candidate = CANDIDATE_MODELS.find((model) => model.modelKey === request.modelKey);
  webSlotState = {
    revision: webSlotState.revision + 1,
    loaded: {
      identifier: LATTICE_MANAGED_IDENTIFIER,
      modelKey: request.modelKey,
      ...(candidate?.architecture !== undefined ? { architecture: candidate.architecture } : {}),
      ...(candidate?.sizeBytes !== undefined ? { sizeBytes: candidate.sizeBytes } : {})
    },
    ownership: {
      state: 'owned',
      identifier: LATTICE_MANAGED_IDENTIFIER,
      modelKey: request.modelKey,
      loadedSinceUnixSeconds: Math.floor(Date.now() / 1000)
    },
    lastOperation: 'loaded'
  };
  return composeWebModelSlotStatus();
}

export function unloadWebModel(request: UnloadModelRequest): ModelSlotStatus {
  ensureExpectedRevision(request.expectedRevision);
  ensureRuntimeRunning();

  if (webSlotState.loaded === null) {
    webSlotState = {
      ...webSlotState,
      revision: webSlotState.revision + 1,
      lastOperation: 'alreadyUnloaded'
    };
    return composeWebModelSlotStatus();
  }

  if (webSlotState.ownership.state !== 'owned') {
    webSlotState = {
      ...webSlotState,
      revision: webSlotState.revision + 1,
      lastOperation: 'refused'
    };
    return composeWebModelSlotStatus();
  }

  webSlotState = {
    revision: webSlotState.revision + 1,
    loaded: null,
    ownership: { state: 'unknown' },
    lastOperation: 'unloaded'
  };
  return composeWebModelSlotStatus();
}

export function cancelWebModelOperation(): void {
  // The web fallback never runs a real long-lived operation to cancel.
}

function composeWebModelSlotStatus(): ModelSlotStatus {
  const base = {
    revision: webSlotState.revision,
    ownership: webSlotState.ownership,
    ...(webSlotState.lastOperation !== null ? { lastOperation: webSlotState.lastOperation } : {})
  };

  if (getWebModelRuntimeStatus().availability !== 'running') {
    return {
      ...base,
      installed: [],
      ownership: { state: 'unknown' },
      message: NOT_RUNNING_MESSAGE
    };
  }

  return {
    ...base,
    installed: CANDIDATE_MODELS,
    ...(webSlotState.loaded !== null ? { loaded: webSlotState.loaded } : {}),
    lastCheckedUnixSeconds: Math.floor(Date.now() / 1000),
    message: READY_MESSAGE
  };
}

function createUnknownSlotState(revision: number): InternalSlotState {
  return {
    revision,
    loaded: null,
    ownership: { state: 'unknown' },
    lastOperation: null
  };
}

function ensureRuntimeRunning(): void {
  if (getWebModelRuntimeStatus().availability !== 'running') {
    throw {
      code: 'runtime.invalid',
      message: NOT_RUNNING_MESSAGE,
      recoverable: true
    };
  }
}

function ensureExpectedRevision(expectedRevision: number): void {
  if (expectedRevision !== webSlotState.revision) {
    throw {
      code: 'runtime.conflict',
      message: 'Model settings changed before this operation could run.',
      recoverable: true
    };
  }
}
