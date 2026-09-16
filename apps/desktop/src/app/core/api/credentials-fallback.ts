import type {
  CreateCredentialRequest,
  CredentialRef,
  DeleteCredentialRequest,
  ReplaceCredentialRequest
} from '@lattice/types';

/**
 * Browser-preview simulation of the 0.10 credentials domain, following
 * `conversations-fallback.ts`/`chat-fallback.ts`'s precedent of fully
 * simulating a capability rather than stubbing it "unavailable". There is
 * no native secure-entry dialog in a browser preview, so this deterministic
 * fallback fabricates a canned secret value on create/replace instead of
 * prompting — it never round-trips a real value, and (like the real native
 * store) never exposes that fabricated value back to any caller either.
 */

const NOT_FOUND = {
  code: 'credential.not_found',
  message: 'That credential no longer exists.',
  recoverable: true
} as const;

interface WebCredential {
  readonly id: string;
  readonly label: string;
  readonly providerKey: string;
  secret: string;
  readonly createdAtUnixSeconds: number;
  updatedAtUnixSeconds: number;
}

const credentials = new Map<string, WebCredential>();
let nextId = 1;
let clock = 1;

export function resetWebCredentialsForTest(): void {
  credentials.clear();
  nextId = 1;
  clock = 1;
}

/** Monotonic surrogate for `updated_at_unix_seconds`, deterministic for tests. */
function tick(): number {
  return clock++;
}

/** Lets sibling fallbacks check a reference the way SQLite's foreign key does. */
export function hasWebCredential(id: string): boolean {
  return credentials.has(id);
}

export function listWebCredentials(): readonly CredentialRef[] {
  return [...credentials.values()]
    .sort((a, b) => b.updatedAtUnixSeconds - a.updatedAtUnixSeconds || (a.id < b.id ? 1 : -1))
    .map(toCredentialRef);
}

export function createWebCredential(request: CreateCredentialRequest): CredentialRef {
  const id = `web-credential-${nextId++}`;
  const now = tick();
  const credential: WebCredential = {
    id,
    label: request.label.trim(),
    providerKey: request.providerKey.trim(),
    secret: `simulated-secret-for-${request.label.trim()}`,
    createdAtUnixSeconds: now,
    updatedAtUnixSeconds: now
  };
  credentials.set(id, credential);
  return toCredentialRef(credential);
}

export function replaceWebCredential(request: ReplaceCredentialRequest): CredentialRef {
  const credential = credentials.get(request.id);
  if (credential === undefined) {
    throw NOT_FOUND;
  }
  credential.secret = `simulated-secret-for-${credential.label}-replaced`;
  credential.updatedAtUnixSeconds = tick();
  return toCredentialRef(credential);
}

export function deleteWebCredential(request: DeleteCredentialRequest): void {
  if (!credentials.delete(request.id)) {
    throw NOT_FOUND;
  }
}

function toCredentialRef(credential: WebCredential): CredentialRef {
  return {
    id: credential.id,
    label: credential.label,
    providerKey: credential.providerKey,
    availability: 'available',
    createdAtUnixSeconds: credential.createdAtUnixSeconds,
    updatedAtUnixSeconds: credential.updatedAtUnixSeconds
  };
}
