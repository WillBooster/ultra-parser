import { Token } from './Token.js';
import type { Vocabulary } from './Vocabulary.js';

/** A set of integers as sorted, disjoint, inclusive ranges, like ANTLR's `IntervalSet`. */
export class IntervalSet {
  constructor(readonly intervals: readonly (readonly [number, number])[] = []) {}

  contains(value: number): boolean {
    return this.intervals.some(([a, b]) => a <= value && value <= b);
  }

  get isEmpty(): boolean {
    return this.intervals.length === 0;
  }

  get minElement(): number | undefined {
    return this.intervals[0]?.[0];
  }

  toArray(): number[] {
    const values: number[] = [];
    for (const [a, b] of this.intervals) for (let v = a; v <= b; v++) values.push(v);
    return values;
  }

  /** Formats the set like ANTLR, e.g., `{'+', ID}`, or `{1..3, 5}` without a vocabulary. */
  toString(vocabulary?: Vocabulary): string {
    if (this.isEmpty) return '{}';
    const names = vocabulary
      ? this.toArray().map((t) => (t === Token.EOF ? '<EOF>' : t === Token.EPSILON ? '<EPSILON>' : vocabulary.getDisplayName(t)))
      : this.intervals.map(([a, b]) => (a === b ? String(a) : `${a}..${b}`));
    return names.length === 1 && (vocabulary || this.intervals[0]?.[0] === this.intervals[0]?.[1])
      ? (names[0] ?? '')
      : `{${names.join(', ')}}`;
  }
}
