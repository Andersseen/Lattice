import { computed, inject, Injectable, signal } from '@angular/core';
import type { AppError, ModelSlotStatus } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeModelSlotError } from '../api/app-wire';

@Injectable({ providedIn: 'root' })
export class ModelSlotStore {
  private readonly appApi = inject(AppApiService);
  private readonly statusState = signal<ModelSlotStatus | null>(null);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly loadingModelState = signal(false);
  private readonly unloadingState = signal(false);

  readonly status = this.statusState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly isLoadingModel = this.loadingModelState.asReadonly();
  readonly isUnloading = this.unloadingState.asReadonly();
  readonly isOperationPending = computed(() => this.loadingModelState() || this.unloadingState());
  readonly canLoad = computed(() => this.statusState()?.loaded === undefined);
  readonly canUnload = computed(() => this.statusState()?.ownership.state === 'owned');

  constructor() {
    void this.load();
  }

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(await this.appApi.getModelSlotStatus());
    } catch (error: unknown) {
      this.errorState.set(normalizeModelSlotError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async loadModel(modelKey: string): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.loadingModelState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.loadModel({
          expectedRevision: current.revision,
          modelKey
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelSlotError(error));
    } finally {
      this.loadingModelState.set(false);
    }
  }

  async unloadModel(): Promise<void> {
    const current = this.statusState();
    if (current === null) {
      return;
    }

    this.unloadingState.set(true);
    this.errorState.set(null);

    try {
      this.statusState.set(
        await this.appApi.unloadModel({
          expectedRevision: current.revision
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeModelSlotError(error));
    } finally {
      this.unloadingState.set(false);
    }
  }

  async cancelOperation(): Promise<void> {
    if (!this.isOperationPending()) {
      return;
    }

    try {
      await this.appApi.cancelModelOperation({});
    } catch (error: unknown) {
      this.errorState.set(normalizeModelSlotError(error));
    }
  }
}
