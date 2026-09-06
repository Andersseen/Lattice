import { isTauriRuntime } from './tauri-runtime';

describe('isTauriRuntime', () => {
  it('reports false in the web test runtime', () => {
    expect(isTauriRuntime()).toBe(false);
  });
});
