import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  inject,
  signal
} from '@angular/core';
import { VoltButton } from '@voltui/components';
import type { AppearancePreference } from '@lattice/types';

import { CredentialsStore } from '../../core/state/credentials.store';
import { SettingsStore } from '../../core/state/settings.store';

const APPEARANCE_OPTIONS: ReadonlyArray<{
  readonly value: AppearancePreference;
  readonly label: string;
}> = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' }
];

@Component({
  selector: 'lat-settings-page',
  imports: [VoltButton],
  templateUrl: './settings.page.html',
  styleUrl: './settings.page.css',
  changeDetection: ChangeDetectionStrategy.OnPush
})
export class SettingsPage {
  private readonly settingsStore = inject(SettingsStore);
  private readonly credentialsStore = inject(CredentialsStore);

  protected readonly appearanceOptions = APPEARANCE_OPTIONS;
  protected readonly credentials = this.credentialsStore.credentials;
  protected readonly credentialsError = this.credentialsStore.error;
  protected readonly isLoadingCredentials = this.credentialsStore.isLoading;
  protected readonly isSavingCredential = this.credentialsStore.isSaving;
  protected readonly deletingCredentialId = this.credentialsStore.deletingId;
  protected readonly draftCredentialLabel = signal('');
  protected readonly draftCredentialProviderKey = signal('');
  protected readonly canAddCredential = computed(
    () =>
      this.draftCredentialLabel().trim().length > 0 &&
      this.draftCredentialProviderKey().trim().length > 0 &&
      !this.isSavingCredential()
  );
  protected readonly settings = this.settingsStore.settings;
  protected readonly error = this.settingsStore.error;
  protected readonly isLoading = this.settingsStore.isLoading;
  protected readonly isSaving = this.settingsStore.isSaving;
  protected readonly draftAppearance = signal<AppearancePreference>('system');
  protected readonly draftIdleUnloadMinutes = signal(5);
  protected readonly idleUnloadIsValid = computed(() => {
    const value = this.draftIdleUnloadMinutes();
    return Number.isInteger(value) && value >= 1 && value <= 120;
  });
  protected readonly hasChanges = computed(() => {
    const settings = this.settings();
    return (
      settings !== null &&
      (settings.appearance !== this.draftAppearance() ||
        settings.idleUnloadMinutes !== this.draftIdleUnloadMinutes())
    );
  });
  protected readonly canSave = computed(
    () => this.hasChanges() && this.idleUnloadIsValid() && !this.isSaving()
  );

  constructor() {
    effect(() => {
      const settings = this.settings();
      if (settings === null) {
        return;
      }

      this.draftAppearance.set(settings.appearance);
      this.draftIdleUnloadMinutes.set(settings.idleUnloadMinutes);
    });
  }

  protected setAppearance(appearance: AppearancePreference): void {
    this.draftAppearance.set(appearance);
  }

  protected setIdleUnloadMinutes(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.draftIdleUnloadMinutes.set(Number(input.value));
  }

  protected save(): void {
    if (!this.canSave()) {
      return;
    }

    void this.settingsStore.save({
      appearance: this.draftAppearance(),
      idleUnloadMinutes: this.draftIdleUnloadMinutes()
    });
  }

  protected reset(): void {
    void this.settingsStore.reset();
  }

  protected retry(): void {
    void this.settingsStore.load();
  }

  protected setCredentialLabel(event: Event): void {
    const target = event.target as HTMLInputElement;
    this.draftCredentialLabel.set(target.value);
  }

  protected setCredentialProviderKey(event: Event): void {
    const target = event.target as HTMLInputElement;
    this.draftCredentialProviderKey.set(target.value);
  }

  protected addCredential(): void {
    if (!this.canAddCredential()) {
      return;
    }

    const label = this.draftCredentialLabel().trim();
    const providerKey = this.draftCredentialProviderKey().trim();
    this.draftCredentialLabel.set('');
    this.draftCredentialProviderKey.set('');
    void this.credentialsStore.create(label, providerKey);
  }

  protected replaceCredential(id: string): void {
    void this.credentialsStore.replace(id);
  }

  protected deleteCredential(id: string, label: string, event: Event): void {
    event.stopPropagation();
    if (!globalThis.confirm(`Delete "${label}"? This cannot be undone.`)) {
      return;
    }
    void this.credentialsStore.delete(id);
  }

  protected retryCredentials(): void {
    void this.credentialsStore.load();
  }
}
