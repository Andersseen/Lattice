import type {
  ConfigureModelRuntimeRequest,
  ModelRuntimeStatus,
  ProbeModelRuntimeRequest
} from '@lattice/types';

let webRuntimeStatus: ModelRuntimeStatus = createMissingRuntimeStatus(1);

export function resetWebModelRuntimeStatusForTest(): void {
  webRuntimeStatus = createMissingRuntimeStatus(1);
}

export function getWebModelRuntimeStatus(): ModelRuntimeStatus {
  return webRuntimeStatus;
}

export function configureWebModelRuntime(
  request: ConfigureModelRuntimeRequest
): ModelRuntimeStatus {
  ensureExpectedRevision(request.expectedRevision);
  if (!request.executablePath.startsWith('/')) {
    throw {
      code: 'runtime.invalid',
      message: 'Runtime executable path must be an absolute path.',
      recoverable: true
    };
  }

  webRuntimeStatus = {
    ...createMissingRuntimeStatus(webRuntimeStatus.revision + 1),
    executablePath: request.executablePath,
    availability: 'unknown',
    message: 'Approve a runtime probe to check this executable.'
  };
  return webRuntimeStatus;
}

export function probeWebModelRuntime(request: ProbeModelRuntimeRequest): ModelRuntimeStatus {
  ensureExpectedRevision(request.expectedRevision);
  if (webRuntimeStatus.executablePath === undefined) {
    throw {
      code: 'runtime.invalid',
      message: 'Choose a runtime executable before probing.',
      recoverable: true
    };
  }

  const checkedAtUnixSeconds = Math.floor(Date.now() / 1000);
  webRuntimeStatus = {
    ...webRuntimeStatus,
    revision: webRuntimeStatus.revision + 1,
    availability: 'stopped',
    cliVersion: '0.0.47',
    approved: {
      executableFingerprint: `web:${webRuntimeStatus.executablePath}`,
      cliVersion: '0.0.47',
      checkedAtUnixSeconds
    },
    daemon: {
      status: 'notRunning'
    },
    server: {
      status: 'stopped'
    },
    lastCheckedUnixSeconds: checkedAtUnixSeconds,
    message: 'Runtime daemon or server is not running.'
  };
  return webRuntimeStatus;
}

function createMissingRuntimeStatus(revision: number): ModelRuntimeStatus {
  return {
    revision,
    availability: 'missing',
    daemon: {
      status: 'unknown'
    },
    server: {
      status: 'unknown'
    },
    message: 'No runtime executable configured.'
  };
}

function ensureExpectedRevision(expectedRevision: number): void {
  if (expectedRevision !== webRuntimeStatus.revision) {
    throw {
      code: 'runtime.conflict',
      message: 'Runtime settings changed before this update could be saved.',
      recoverable: true
    };
  }
}
