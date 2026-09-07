import { DOCUMENT } from '@angular/common';
import { effect, inject, Injectable, signal } from '@angular/core';
import type { AppError, AppSettings, AppearancePreference } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeSettingsError } from '../api/app-wire';

export interface AppSettingsDraft {
  readonly appearance: AppearancePreference;
  readonly idleUnloadMinutes: number;
}

@Injectable({ providedIn: 'root' })
export class SettingsStore {
  private readonly appApi = inject(AppApiService);
  private readonly document = inject(DOCUMENT);
  private readonly settingsState = signal<AppSettings | null>(null);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly savingState = signal(false);

  readonly settings = this.settingsState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly isSaving = this.savingState.asReadonly();

  constructor() {
    effect(() => {
      this.applyAppearance(this.settingsState()?.appearance ?? 'system');
    });
    void this.load();
  }

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.settingsState.set(await this.appApi.getAppSettings());
    } catch (error: unknown) {
      this.errorState.set(normalizeSettingsError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async save(draft: AppSettingsDraft): Promise<void> {
    const current = this.settingsState();
    if (current === null) {
      return;
    }

    this.savingState.set(true);
    this.errorState.set(null);

    try {
      this.settingsState.set(
        await this.appApi.updateAppSettings({
          expectedRevision: current.revision,
          appearance: draft.appearance,
          idleUnloadMinutes: draft.idleUnloadMinutes
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeSettingsError(error));
    } finally {
      this.savingState.set(false);
    }
  }

  async reset(): Promise<void> {
    const current = this.settingsState();
    if (current === null) {
      return;
    }

    this.savingState.set(true);
    this.errorState.set(null);

    try {
      this.settingsState.set(
        await this.appApi.resetAppSettings({
          expectedRevision: current.revision
        })
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeSettingsError(error));
    } finally {
      this.savingState.set(false);
    }
  }

  private applyAppearance(appearance: AppearancePreference): void {
    const root = this.document.documentElement;
    if (appearance === 'system') {
      root.removeAttribute('data-appearance');
      return;
    }

    root.setAttribute('data-appearance', appearance);
  }
}
