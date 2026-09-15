import { DOCUMENT } from '@angular/common';
import { DestroyRef, effect, Injectable, inject, signal } from '@angular/core';
import type { AppError, AppearancePreference, AppSettings } from '@lattice/types';
import { applyVoltTheme } from '@voltui/components';

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
  private readonly destroyRef = inject(DestroyRef);
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
    this.syncSystemAppearanceChanges();
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

  previewAppearance(appearance: AppearancePreference): void {
    this.applyAppearance(appearance);
  }

  restoreSavedAppearance(): void {
    this.applyAppearance(this.settingsState()?.appearance ?? 'system');
  }

  private applyAppearance(appearance: AppearancePreference): void {
    const root = this.document.documentElement;
    const prefersDark = this.document.defaultView?.matchMedia(
      '(prefers-color-scheme: dark)'
    ).matches;
    const dark = appearance === 'dark' || (appearance === 'system' && prefersDark === true);

    applyVoltTheme({ color: 'sage', style: 'sharp', dark }, this.document);

    if (appearance === 'system') {
      root.removeAttribute('data-appearance');
      return;
    }

    root.setAttribute('data-appearance', appearance);
  }

  private syncSystemAppearanceChanges(): void {
    const mediaQuery = this.document.defaultView?.matchMedia('(prefers-color-scheme: dark)');

    if (mediaQuery === undefined) {
      return;
    }

    const applySystemAppearance = (): void => {
      if ((this.settingsState()?.appearance ?? 'system') === 'system') {
        this.applyAppearance('system');
      }
    };

    mediaQuery.addEventListener('change', applySystemAppearance);
    this.destroyRef.onDestroy(() => {
      mediaQuery.removeEventListener('change', applySystemAppearance);
    });
  }
}
