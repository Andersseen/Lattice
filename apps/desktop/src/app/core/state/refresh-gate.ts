export class RefreshGate {
  private latestSequence = 0;

  begin(): number {
    this.latestSequence += 1;
    return this.latestSequence;
  }

  isLatest(sequence: number): boolean {
    return sequence === this.latestSequence;
  }
}
