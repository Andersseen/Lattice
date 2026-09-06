import type { AppInfo } from '@lattice/types';

export function createWebFallbackInfo(): AppInfo {
  return {
    name: 'Lattice',
    version: '0.1.0',
    buildProfile: 'debug',
    runtime: 'web',
    target: 'browser'
  };
}
