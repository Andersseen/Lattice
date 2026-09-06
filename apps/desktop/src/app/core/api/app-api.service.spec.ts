import { createWebFallbackInfo } from './app-info-fallback';

describe('createWebFallbackInfo', () => {
  it('returns typed app info for browser-only verification', () => {
    expect(createWebFallbackInfo()).toEqual({
      name: 'Lattice',
      version: '0.1.0',
      buildProfile: 'debug',
      runtime: 'web',
      target: 'browser'
    });
  });
});
