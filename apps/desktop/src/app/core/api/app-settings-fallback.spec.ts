import { getWebSettings, resetWebSettings, updateWebSettings } from './app-settings-fallback';

describe('app settings browser fallback', () => {
  beforeEach(() => {
    const current = getWebSettings();
    resetWebSettings({ expectedRevision: current.revision });
  });

  it('updates settings in browser smoke mode', () => {
    const current = getWebSettings();
    const updated = updateWebSettings({
      expectedRevision: current.revision,
      appearance: 'dark',
      idleUnloadMinutes: 20
    });

    expect(updated).toEqual({
      schemaVersion: 2,
      revision: current.revision + 1,
      appearance: 'dark',
      idleUnloadMinutes: 20
    });
  });

  it('rejects stale browser fallback revisions', () => {
    expect(() =>
      updateWebSettings({
        expectedRevision: getWebSettings().revision + 1,
        appearance: 'light'
      })
    ).toThrow(
      expect.objectContaining({
        code: 'settings.conflict'
      })
    );
  });

  it('rejects invalid idle unload values', () => {
    const current = getWebSettings();

    expect(() =>
      updateWebSettings({
        expectedRevision: current.revision,
        idleUnloadMinutes: 0
      })
    ).toThrow(
      expect.objectContaining({
        code: 'settings.invalid'
      })
    );
    expect(getWebSettings()).toEqual(current);
  });
});
