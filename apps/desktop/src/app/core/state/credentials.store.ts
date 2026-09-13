import { Injectable, inject, signal } from '@angular/core';
import type { AppError, CredentialRef } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeCredentialError } from '../api/app-wire';

@Injectable({ providedIn: 'root' })
export class CredentialsStore {
  private readonly appApi = inject(AppApiService);
  private readonly credentialsState = signal<readonly CredentialRef[]>([]);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly savingState = signal(false);
  private readonly deletingIdState = signal<string | null>(null);

  readonly credentials = this.credentialsState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  readonly isSaving = this.savingState.asReadonly();
  readonly deletingId = this.deletingIdState.asReadonly();

  constructor() {
    void this.load();
  }

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.credentialsState.set(await this.appApi.listCredentials());
    } catch (error: unknown) {
      this.errorState.set(normalizeCredentialError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async create(label: string, providerKey: string): Promise<void> {
    this.savingState.set(true);
    this.errorState.set(null);

    try {
      const created = await this.appApi.createCredential({ label, providerKey });
      this.credentialsState.update((existing) => [created, ...existing]);
    } catch (error: unknown) {
      this.errorState.set(normalizeCredentialError(error));
    } finally {
      this.savingState.set(false);
    }
  }

  async replace(id: string): Promise<void> {
    this.savingState.set(true);
    this.errorState.set(null);

    try {
      const replaced = await this.appApi.replaceCredential({ id });
      this.credentialsState.update((existing) =>
        existing.map((credential) => (credential.id === id ? replaced : credential))
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeCredentialError(error));
    } finally {
      this.savingState.set(false);
    }
  }

  async delete(id: string): Promise<void> {
    this.deletingIdState.set(id);
    this.errorState.set(null);

    try {
      await this.appApi.deleteCredential({ id });
      this.credentialsState.update((existing) =>
        existing.filter((credential) => credential.id !== id)
      );
    } catch (error: unknown) {
      this.errorState.set(normalizeCredentialError(error));
    } finally {
      this.deletingIdState.set(null);
    }
  }
}
