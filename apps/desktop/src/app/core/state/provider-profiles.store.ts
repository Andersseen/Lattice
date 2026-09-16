import { Injectable, inject, signal } from '@angular/core';
import type { AppError, ProviderProfile } from '@lattice/types';

import { AppApiService } from '../api/app-api.service';
import { normalizeProviderError } from '../api/app-wire';

export interface ProviderProfileDraft {
  readonly label: string;
  readonly endpoint: string;
  readonly modelKey: string;
}

/**
 * Presentation state for remote provider profiles. Every rule (HTTPS-only
 * endpoints, revision checks, the endpoint change clearing the credential
 * binding and consent) is enforced in Rust; this store only keeps the
 * latest returned profiles and the last safe error.
 */
@Injectable({ providedIn: 'root' })
export class ProviderProfilesStore {
  private readonly appApi = inject(AppApiService);
  private readonly profilesState = signal<readonly ProviderProfile[]>([]);
  private readonly errorState = signal<AppError | null>(null);
  private readonly loadingState = signal(false);
  private readonly busyIdState = signal<string | null>(null);

  readonly profiles = this.profilesState.asReadonly();
  readonly error = this.errorState.asReadonly();
  readonly isLoading = this.loadingState.asReadonly();
  /** The profile id (or `'new'`) with a request in flight. */
  readonly busyId = this.busyIdState.asReadonly();

  constructor() {
    void this.load();
  }

  async load(): Promise<void> {
    this.loadingState.set(true);
    this.errorState.set(null);

    try {
      this.profilesState.set(await this.appApi.listProviderProfiles());
    } catch (error: unknown) {
      this.errorState.set(normalizeProviderError(error));
    } finally {
      this.loadingState.set(false);
    }
  }

  async create(draft: ProviderProfileDraft, credentialId: string | null): Promise<boolean> {
    return this.run('new', async () => {
      const created = await this.appApi.createProviderProfile({ ...draft, credentialId });
      this.profilesState.update((existing) =>
        [...existing, created].sort((a, b) => a.label.localeCompare(b.label))
      );
    });
  }

  async update(profile: ProviderProfile, draft: ProviderProfileDraft): Promise<boolean> {
    return this.run(profile.id, async () => {
      this.replace(
        await this.appApi.updateProviderProfile({
          id: profile.id,
          expectedRevision: profile.revision,
          ...draft
        })
      );
    });
  }

  async bindCredential(profile: ProviderProfile, credentialId: string | null): Promise<boolean> {
    return this.run(profile.id, async () => {
      this.replace(
        await this.appApi.bindProviderCredential({
          id: profile.id,
          expectedRevision: profile.revision,
          credentialId
        })
      );
    });
  }

  /** Grants consent for exactly the endpoint the disclosure displayed. */
  async grantConsent(profile: ProviderProfile): Promise<boolean> {
    return this.run(profile.id, async () => {
      this.replace(
        await this.appApi.grantProviderConsent({
          id: profile.id,
          expectedRevision: profile.revision,
          endpoint: profile.endpoint
        })
      );
    });
  }

  async revokeConsent(profile: ProviderProfile): Promise<boolean> {
    return this.run(profile.id, async () => {
      this.replace(
        await this.appApi.revokeProviderConsent({
          id: profile.id,
          expectedRevision: profile.revision
        })
      );
    });
  }

  async delete(id: string): Promise<boolean> {
    return this.run(id, async () => {
      await this.appApi.deleteProviderProfile({ id });
      this.profilesState.update((existing) => existing.filter((profile) => profile.id !== id));
    });
  }

  private replace(updated: ProviderProfile): void {
    this.profilesState.update((existing) =>
      existing.map((profile) => (profile.id === updated.id ? updated : profile))
    );
  }

  private async run(busyId: string, operation: () => Promise<void>): Promise<boolean> {
    this.busyIdState.set(busyId);
    this.errorState.set(null);

    try {
      await operation();
      return true;
    } catch (error: unknown) {
      const normalized = normalizeProviderError(error);
      if (normalized.code === 'provider.conflict' || normalized.code === 'provider.not_found') {
        // The local copy is stale; show what Rust holds now, keeping the error visible.
        await this.load();
      }
      this.errorState.set(normalized);
      return false;
    } finally {
      this.busyIdState.set(null);
    }
  }
}
