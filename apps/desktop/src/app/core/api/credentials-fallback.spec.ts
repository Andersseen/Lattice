import {
  createWebCredential,
  deleteWebCredential,
  listWebCredentials,
  replaceWebCredential,
  resetWebCredentialsForTest
} from './credentials-fallback';

describe('credentials browser fallback', () => {
  beforeEach(() => {
    resetWebCredentialsForTest();
  });

  it('creates a credential and reports it as available, never exposing a secret field', () => {
    const created = createWebCredential({ label: 'OpenAI', providerKey: 'openai' });

    expect(created.label).toBe('OpenAI');
    expect(created.providerKey).toBe('openai');
    expect(created.availability).toBe('available');
    expect(Object.keys(created)).not.toContain('secret');
  });

  it('lists credentials most recently updated first', () => {
    const first = createWebCredential({ label: 'First', providerKey: 'a' });
    const second = createWebCredential({ label: 'Second', providerKey: 'b' });

    const listed = listWebCredentials();
    expect(listed.map((credential) => credential.id)).toEqual([second.id, first.id]);
  });

  it('replaces a credential, keeping its id and label', () => {
    const created = createWebCredential({ label: 'OpenAI', providerKey: 'openai' });
    const replaced = replaceWebCredential({ id: created.id });

    expect(replaced.id).toBe(created.id);
    expect(replaced.label).toBe('OpenAI');
    expect(replaced.updatedAtUnixSeconds).toBeGreaterThanOrEqual(created.updatedAtUnixSeconds);
  });

  it('refuses to replace an unknown credential', () => {
    expect(() => replaceWebCredential({ id: 'missing' })).toThrow(
      expect.objectContaining({ code: 'credential.not_found' })
    );
  });

  it('deletes a credential without affecting others', () => {
    const keep = createWebCredential({ label: 'Keep', providerKey: 'keep' });
    const removeMe = createWebCredential({ label: 'Remove', providerKey: 'remove' });

    deleteWebCredential({ id: removeMe.id });

    const listed = listWebCredentials();
    expect(listed.map((credential) => credential.id)).toEqual([keep.id]);
  });

  it('refuses to delete an unknown credential', () => {
    expect(() => deleteWebCredential({ id: 'missing' })).toThrow(
      expect.objectContaining({ code: 'credential.not_found' })
    );
  });
});
