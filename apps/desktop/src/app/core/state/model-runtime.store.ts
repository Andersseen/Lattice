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

  readonly status = this.statusState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly isSaving = this.savingState.asReadonly();
  readonly isProbing = this.probingState.asReadonly();
  readonly canProbe = computed(() => this.statusState()?.executablePath !== undefined);

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
}
