import { computed, inject, Injectable, signal } from '@angular/core';
import type { AppError, AppInfo } from '@lattice/types';

import { normalizeAppError } from '../api/app-wire';
import { AppApiService } from '../api/app-api.service';
import { RefreshGate } from './refresh-gate';

@Injectable({ providedIn: 'root' })
export class AppInfoStore {
  private readonly appApi = inject(AppApiService);
  private readonly appInfoState = signal<AppInfo | null>(null);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly refreshGate = new RefreshGate();

  readonly appInfo = this.appInfoState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly hasAppInfo = computed(() => this.appInfoState() !== null);

  constructor() {
    void this.refresh();
  }

  async refresh(): Promise<void> {
    const sequence = this.refreshGate.begin();

    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      const appInfo = await this.appApi.getAppInfo();
      if (!this.refreshGate.isLatest(sequence)) {
        return;
      }

      this.appInfoState.set(appInfo);
    } catch (error: unknown) {
      if (!this.refreshGate.isLatest(sequence)) {
        return;
      }

      this.appInfoState.set(null);
      this.errorState.set(normalizeAppError(error));
    } finally {
      if (this.refreshGate.isLatest(sequence)) {
        this.loadingState.set(false);
      }
    }
  }
}
