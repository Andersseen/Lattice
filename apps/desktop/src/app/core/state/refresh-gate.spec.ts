import { RefreshGate } from './refresh-gate';

describe('RefreshGate', () => {
  it('keeps only the latest refresh sequence current', () => {
    const gate = new RefreshGate();

    const earlier = gate.begin();
    const later = gate.begin();

    expect(gate.isLatest(later)).toBe(true);
    expect(gate.isLatest(earlier)).toBe(false);
  });
});
