import {
  createWebCredential,
  deleteWebCredential,
  resetWebCredentialsForTest
} from './credentials-fallback';
import {
  bindWebProviderCredential,
  createWebProviderProfile,
  deleteWebProviderProfile,
  getWebProviderProfile,
  grantWebProviderConsent,
  listWebProviderProfiles,
  normalizeWebEndpoint,
  resetWebProviderProfilesForTest,
  revokeWebProviderConsent,
  updateWebProviderProfile
} from './provider-profiles-fallback';

describe('remote provider profiles browser fallback', () => {
  beforeEach(() => {
    resetWebCredentialsForTest();
    resetWebProviderProfilesForTest();
  });

  function consentedProfile() {
    const credential = createWebCredential({ label: 'Key', providerKey: 'openai' });
    const created = createWebProviderProfile({
      label: 'Example',
      endpoint: 'https://API.example.com/v1/',
      modelKey: 'gpt-test',
      credentialId: credential.id
    });
    const consented = grantWebProviderConsent({
      id: created.id,
      expectedRevision: created.revision,
      endpoint: created.endpoint
    });
    return { credential, profile: consented };
  }

  it('normalizes HTTPS endpoints and refuses anything else', () => {
    expect(normalizeWebEndpoint(' HTTPS://Api.Example.com:8443/v1/ ')).toBe(
      'https://api.example.com:8443/v1'
    );
    for (const endpoint of [
      'http://api.example.com/v1',
      'https://user:secret@api.example.com/v1',
      'https://api.example.com/v1?key=1',
      'https://api.example.com/v1#top',
      'https:///v1',
      'https://api.example.com:70000/v1'
    ]) {
      expect(() => normalizeWebEndpoint(endpoint)).toThrow(
        expect.objectContaining({ code: 'provider.invalid' })
      );
    }
  });

  it('creates a profile referencing a credential without any secret field', () => {
    const { credential, profile } = consentedProfile();

    expect(profile.endpoint).toBe('https://api.example.com/v1');
    expect(profile.credentialId).toBe(credential.id);
    expect(profile.consent?.endpoint).toBe(profile.endpoint);
    expect(Object.keys(profile).some((key) => /secret|key$/i.test(key) && key !== 'modelKey')).toBe(
      false
    );
    expect(listWebProviderProfiles()).toEqual([profile]);
  });

  it('refuses binding an unknown credential', () => {
    expect(() =>
      createWebProviderProfile({
        label: 'Example',
        endpoint: 'https://api.example.com/v1',
        modelKey: 'gpt-test',
        credentialId: 'missing'
      })
    ).toThrow(expect.objectContaining({ code: 'provider.invalid' }));
    expect(listWebProviderProfiles()).toEqual([]);
  });

  it('clears credential binding and consent when the endpoint changes', () => {
    const { profile } = consentedProfile();

    const moved = updateWebProviderProfile({
      id: profile.id,
      expectedRevision: profile.revision,
      label: profile.label,
      endpoint: 'https://other.example.com/v1',
      modelKey: profile.modelKey
    });

    expect(moved.credentialId).toBeUndefined();
    expect(moved.consent).toBeUndefined();
  });

  it('keeps bindings when only the label or model changes', () => {
    const { credential, profile } = consentedProfile();

    const renamed = updateWebProviderProfile({
      id: profile.id,
      expectedRevision: profile.revision,
      label: 'Renamed',
      endpoint: 'https://api.example.com/v1/',
      modelKey: 'other-model'
    });

    expect(renamed.credentialId).toBe(credential.id);
    expect(renamed.consent?.endpoint).toBe('https://api.example.com/v1');
  });

  it('grants consent only for the endpoint that was shown and can revoke it', () => {
    const created = createWebProviderProfile({
      label: 'Example',
      endpoint: 'https://api.example.com/v1',
      modelKey: 'gpt-test',
      credentialId: null
    });

    expect(() =>
      grantWebProviderConsent({
        id: created.id,
        expectedRevision: created.revision,
        endpoint: 'https://attacker.example.net/v1'
      })
    ).toThrow(expect.objectContaining({ code: 'provider.conflict' }));

    const granted = grantWebProviderConsent({
      id: created.id,
      expectedRevision: created.revision,
      endpoint: created.endpoint
    });
    const revoked = revokeWebProviderConsent({
      id: created.id,
      expectedRevision: granted.revision
    });
    expect(revoked.consent).toBeUndefined();
  });

  it('refuses a stale revision without changing the profile', () => {
    const { profile } = consentedProfile();

    expect(() =>
      bindWebProviderCredential({
        id: profile.id,
        expectedRevision: profile.revision - 1,
        credentialId: null
      })
    ).toThrow(expect.objectContaining({ code: 'provider.conflict' }));
    expect(getWebProviderProfile(profile.id)).toEqual(profile);
  });

  it('reads a deleted credential as unbound', () => {
    const { credential, profile } = consentedProfile();

    deleteWebCredential({ id: credential.id });

    expect(getWebProviderProfile(profile.id).credentialId).toBeUndefined();
  });

  it('deletes profiles and reports unknown ids', () => {
    const { profile } = consentedProfile();

    deleteWebProviderProfile({ id: profile.id });

    expect(listWebProviderProfiles()).toEqual([]);
    expect(() => deleteWebProviderProfile({ id: profile.id })).toThrow(
      expect.objectContaining({ code: 'provider.not_found' })
    );
  });
});
