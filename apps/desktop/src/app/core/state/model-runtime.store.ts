import { computed, inject, Injectable, signal } from '@angular/core';
import type { AppError, ModelRuntimeStatus } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeModelRuntimeError } from '../api/app-wire';

@Injectable({ providedIn: 'root' })
export class ModelRuntimeStore {
  private readonly appApi = inject(AppApiService);
  private readonly statusState = signal<ModelRuntimeStatus | null>(null);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly savingState = signal(false);
  private readonly probingState = signal(false);
  private readonly startingState = signal(false);
  private readonly stoppingState = signal(false);

  readonly status = this.statusState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly isSaving = this.savingState.asReadonly();
  readonly isProbing = this.probingState.asReadonly();
  readonly isStarting = this.startingState.asReadonly();
  readonly isStopping = this.stoppingState.asReadonly();
  readonly canProbe = computed(() => this.statusState()?.executablePath !== undefined);
  readonly canStart = computed(() => {
    const status = this.statusState();
    return (
      status !== null &&
      (status.availability === 'stopped' || status.availability === 'unreachable') &&
      !this.startingState() &&
      !this.stoppingState()
    );
  });
  readonly canStop = computed(
    () =>
      this.statusState()?.ownership.state === 'owned' &&
      !this.startingState() &&
      !this.stoppingState()
  );
  readonly isOperationPending = computed(() => this.startingState() || this.stoppingState());

  constructor() {
    void this.load();
  }

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(await this.appApi.getModelRuntimeStatus());
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async configure(executablePath: string): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.savingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.configureModelRuntime({
          expectedRevision: current.revision,
          executablePath
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    } finally {
      this.savingState.set(false);
    }
  }

  async probe(): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.probingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.probeModelRuntime({
          expectedRevision: current.revision
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    } finally {
      this.probingState.set(false);
    }
  }

  async start(): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.startingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.startModelRuntime({
          expectedRevision: current.revision
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    } finally {
      this.startingState.set(false);
    }
  }

  async stop(): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.stoppingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.stopModelRuntime({
          expectedRevision: current.revision
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    } finally {
      this.stoppingState.set(false);
    }
  }

  async cancelOperation(): Promise<void> {
    if (!this.isOperationPending()) {
      return;
    }

    try {
      await this.appApi.cancelModelRuntimeOperation({});
    } catch (error: unknown) {
      this.errorState.set(normalizeModelRuntimeError(error));
    }
  }
}
