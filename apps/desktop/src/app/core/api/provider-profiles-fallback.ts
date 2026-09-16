import type {
  AppError,
  BindProviderCredentialRequest,
  CreateProviderProfileRequest,
  DeleteProviderProfileRequest,
  GrantProviderConsentRequest,
  ProviderProfile,
  RevokeProviderConsentRequest,
  UpdateProviderProfileRequest
} from '@lattice/types';

import { hasWebCredential } from './credentials-fallback';

/**
 * Browser-preview simulation of the 0.11 remote provider profiles domain,
 * following the other fallbacks' precedent of simulating a capability
 * rather than stubbing it. It mirrors `ProviderProfileStore`'s observable
 * rules — HTTPS-only normalized endpoints, revision checks, an endpoint
 * change clearing the credential binding and consent, consent granted only
 * for the endpoint shown, and a deleted credential reading as unbound — and
 * it never contacts any endpoint.
 */

const MAX_PROVIDER_PROFILES = 50;

const NOT_FOUND: AppError = {
  code: 'provider.not_found',
  message: 'That provider no longer exists.',
  recoverable: true
};

const STALE_REVISION: AppError = {
  code: 'provider.conflict',
  message: 'This provider changed; reload it and try again.',
  recoverable: true
};

const ENDPOINT_CHANGED: AppError = {
  code: 'provider.conflict',
  message: "The provider's endpoint changed; review it before approving.",
  recoverable: true
};

interface WebProviderProfile {
  readonly id: string;
  revision: number;
  label: string;
  endpoint: string;
  modelKey: string;
  credentialId: string | null;
  consentEndpoint: string | null;
  consentGrantedAtUnixSeconds: number | null;
  readonly createdAtUnixSeconds: number;
  updatedAtUnixSeconds: number;
}

const profiles = new Map<string, WebProviderProfile>();
let nextId = 1;
let clock = 1;

export function resetWebProviderProfilesForTest(): void {
  profiles.clear();
  nextId = 1;
  clock = 1;
}

function tick(): number {
  return clock++;
}

function invalid(message: string): AppError {
  return { code: 'provider.invalid', message, recoverable: true };
}

export function listWebProviderProfiles(): readonly ProviderProfile[] {
  return [...profiles.values()]
    .sort(
      (a, b) =>
        a.label.toLowerCase().localeCompare(b.label.toLowerCase()) || a.id.localeCompare(b.id)
    )
    .map(toProviderProfile);
}

export function getWebProviderProfile(id: string): ProviderProfile {
  return toProviderProfile(requireProfile(id));
}

export function createWebProviderProfile(request: CreateProviderProfileRequest): ProviderProfile {
  const label = validateLabel(request.label);
  const endpoint = normalizeWebEndpoint(request.endpoint);
  const modelKey = validateModelKey(request.modelKey);
  const credentialId = validateCredential(request.credentialId);
  if (profiles.size >= MAX_PROVIDER_PROFILES) {
    throw invalid('Lattice supports up to 50 remote providers.');
  }

  const now = tick();
  const profile: WebProviderProfile = {
    id: `web-provider-${nextId++}`,
    revision: 1,
    label,
    endpoint,
    modelKey,
    credentialId,
    consentEndpoint: null,
    consentGrantedAtUnixSeconds: null,
    createdAtUnixSeconds: now,
    updatedAtUnixSeconds: now
  };
  profiles.set(profile.id, profile);
  return toProviderProfile(profile);
}

export function updateWebProviderProfile(request: UpdateProviderProfileRequest): ProviderProfile {
  const label = validateLabel(request.label);
  const endpoint = normalizeWebEndpoint(request.endpoint);
  const modelKey = validateModelKey(request.modelKey);
  const profile = requireRevision(request.id, request.expectedRevision);

  if (endpoint !== profile.endpoint) {
    profile.credentialId = null;
    profile.consentEndpoint = null;
    profile.consentGrantedAtUnixSeconds = null;
  }
  profile.label = label;
  profile.endpoint = endpoint;
  profile.modelKey = modelKey;
  return touch(profile);
}

export function deleteWebProviderProfile(request: DeleteProviderProfileRequest): void {
  if (!profiles.delete(request.id)) {
    throw NOT_FOUND;
  }
}

export function bindWebProviderCredential(request: BindProviderCredentialRequest): ProviderProfile {
  const credentialId = validateCredential(request.credentialId);
  const profile = requireRevision(request.id, request.expectedRevision);
  profile.credentialId = credentialId;
  return touch(profile);
}

export function grantWebProviderConsent(request: GrantProviderConsentRequest): ProviderProfile {
  const profile = requireRevision(request.id, request.expectedRevision);
  let shownEndpoint: string;
  try {
    shownEndpoint = normalizeWebEndpoint(request.endpoint);
  } catch {
    throw ENDPOINT_CHANGED;
  }
  if (shownEndpoint !== profile.endpoint) {
    throw ENDPOINT_CHANGED;
  }

  const now = tick();
  profile.consentEndpoint = shownEndpoint;
  profile.consentGrantedAtUnixSeconds = now;
  return touch(profile);
}

export function revokeWebProviderConsent(request: RevokeProviderConsentRequest): ProviderProfile {
  const profile = requireRevision(request.id, request.expectedRevision);
  profile.consentEndpoint = null;
  profile.consentGrantedAtUnixSeconds = null;
  return touch(profile);
}

/**
 * Mirrors `profiles::validate_endpoint`: an absolute `https` URL with a
 * host, without user information, query, fragment or whitespace, returned
 * with a lowercase scheme/authority and no trailing slash.
 */
export function normalizeWebEndpoint(endpoint: string): string {
  const trimmed = endpoint.trim();
  if (trimmed.length === 0) {
    throw invalid("Enter the provider's HTTPS endpoint.");
  }
  if (!/^[\x21-\x7e]+$/.test(trimmed)) {
    throw invalid('The endpoint must be a plain URL without spaces.');
  }

  const separator = trimmed.indexOf('://');
  if (separator === -1 || trimmed.slice(0, separator).toLowerCase() !== 'https') {
    throw invalid('Remote endpoints must start with https://.');
  }
  const rest = trimmed.slice(separator + 3);
  if (/[?#@\\]/.test(rest)) {
    throw invalid('The endpoint cannot contain credentials, a query, or a fragment.');
  }

  const slash = rest.indexOf('/');
  const authority = (slash === -1 ? rest : rest.slice(0, slash)).toLowerCase();
  const path = slash === -1 ? '' : rest.slice(slash).replace(/\/+$/, '');
  if (!/^([a-z0-9.-]+|\[[0-9a-f:.]+\])(:([1-9][0-9]{0,4}))?$/.test(authority)) {
    throw invalid('The endpoint needs a host name.');
  }
  const port = authority.match(/:(\d+)$/)?.[1];
  if (port !== undefined && Number(port) > 65_535) {
    throw invalid('The endpoint port is not valid.');
  }

  return `https://${authority}${path}`;
}

function validateLabel(label: string): string {
  const trimmed = label.trim();
  if (trimmed.length === 0) {
    throw invalid('Give the provider a label.');
  }
  if ([...trimmed].length > 120) {
    throw invalid('The provider label is too long.');
  }
  return trimmed;
}

function validateModelKey(modelKey: string): string {
  const trimmed = modelKey.trim();
  if (trimmed.length === 0) {
    throw invalid("Enter the provider's model name.");
  }
  if ([...trimmed].length > 200) {
    throw invalid('The model name is too long.');
  }
  return trimmed;
}

function validateCredential(credentialId: string | null | undefined): string | null {
  if (credentialId === null || credentialId === undefined) {
    return null;
  }
  if (!hasWebCredential(credentialId)) {
    throw invalid('That credential no longer exists.');
  }
  return credentialId;
}

function requireProfile(id: string): WebProviderProfile {
  const profile = profiles.get(id);
  if (profile === undefined) {
    throw NOT_FOUND;
  }
  return profile;
}

function requireRevision(id: string, expectedRevision: number): WebProviderProfile {
  const profile = requireProfile(id);
  if (profile.revision !== expectedRevision) {
    throw STALE_REVISION;
  }
  return profile;
}

function touch(profile: WebProviderProfile): ProviderProfile {
  profile.revision += 1;
  profile.updatedAtUnixSeconds = tick();
  return toProviderProfile(profile);
}

function toProviderProfile(profile: WebProviderProfile): ProviderProfile {
  // Emulates the `ON DELETE SET NULL` foreign key: a credential deleted in
  // the credentials fallback reads as unbound here.
  if (profile.credentialId !== null && !hasWebCredential(profile.credentialId)) {
    profile.credentialId = null;
  }

  const consentValid =
    profile.consentEndpoint !== null &&
    profile.consentEndpoint === profile.endpoint &&
    profile.consentGrantedAtUnixSeconds !== null;

  return {
    id: profile.id,
    revision: profile.revision,
    label: profile.label,
    endpoint: profile.endpoint,
    modelKey: profile.modelKey,
    ...(profile.credentialId !== null ? { credentialId: profile.credentialId } : {}),
    ...(consentValid
      ? {
          consent: {
            endpoint: profile.endpoint,
            grantedAtUnixSeconds: profile.consentGrantedAtUnixSeconds ?? 0
          }
        }
      : {}),
    createdAtUnixSeconds: profile.createdAtUnixSeconds,
    updatedAtUnixSeconds: profile.updatedAtUnixSeconds
  };
}
