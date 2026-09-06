import { computed, inject, Injectable, signal } from '@angular/core';
import type { AppError, AppInfo } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';

@Injectable({ providedIn: 'root' })
export class AppInfoStore {
  private readonly appApi = inject(AppApiService);
  private readonly appInfoState = signal<AppInfo | null>(null);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);

  readonly appInfo = this.appInfoState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly hasAppInfo = computed(() => this.appInfoState() !== null);

  constructor() {
    void this.refresh();
  }

  async refresh(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.appInfoState.set(await this.appApi.getAppInfo());
    } catch (error: unknown) {
      this.appInfoState.set(null);
      this.errorState.set(toAppError(error));
    } finally {
      this.loadingState.set(false);
    }
  }
}

function toAppError(error: unknown): AppError {
  if (typeof error === 'object' && error !== null) {
    const candidate = error as Partial<AppError>;
    if (
      typeof candidate.code === 'string' &&
      typeof candidate.message === 'string' &&
      typeof candidate.recoverable === 'boolean'
    ) {
      return candidate as AppError;
    }
  }

  return {
    code: 'app.unknown',
    message: 'An unexpected application error occurred.',
    recoverable: true
  };
}
