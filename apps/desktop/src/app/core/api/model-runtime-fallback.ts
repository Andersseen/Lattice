import type {
  ConfigureModelRuntimeRequest,
  ModelRuntimeStatus,
  ProbeModelRuntimeRequest,
  StartModelRuntimeRequest,
  StopModelRuntimeRequest
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

export function startWebModelRuntime(request: StartModelRuntimeRequest): ModelRuntimeStatus {
  ensureExpectedRevision(request.expectedRevision);
  if (webRuntimeStatus.executablePath === undefined) {
    throw {
      code: 'runtime.invalid',
      message: 'Choose a runtime executable before starting it.',
      recoverable: true
    };
  }

  webRuntimeStatus = {
    ...webRuntimeStatus,
    revision: webRuntimeStatus.revision + 1,
    availability: 'running',
    daemon: { status: 'running', pid: 1, isDaemon: true },
    server: { status: 'running', endpoint: 'http://127.0.0.1:0' },
    ownership: {
      state: 'owned',
      daemonPid: 1,
      executableFingerprint: `web:${webRuntimeStatus.executablePath}`,
      ownedSinceUnixSeconds: Math.floor(Date.now() / 1000)
    },
    lastOperation: 'started',
    message: 'Runtime discovery completed.'
  };
  return webRuntimeStatus;
}

export function stopWebModelRuntime(request: StopModelRuntimeRequest): ModelRuntimeStatus {
  ensureExpectedRevision(request.expectedRevision);

  const wasOwned = webRuntimeStatus.ownership.state === 'owned';
  webRuntimeStatus = {
    ...webRuntimeStatus,
    revision: webRuntimeStatus.revision + 1,
    availability: 'stopped',
    daemon: { status: 'notRunning' },
    server: { status: 'stopped' },
    ownership: { state: 'unknown' },
    lastOperation: wasOwned ? 'stopped' : 'refused',
    message: 'Runtime daemon or server is not running.'
  };
  return webRuntimeStatus;
}

export function cancelWebModelRuntimeOperation(): void {
  // The web fallback never runs a real long-lived operation to cancel.
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
    ownership: { state: 'unknown' },
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
