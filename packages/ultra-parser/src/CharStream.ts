import { toCodePoints, WasmTokens } from './wasm.js';

/** The input of a lexer as Unicode code points, like ANTLR's `CodePointCharStream`. */
export class CharStream {
  static readonly EOF = -1;

  /** The code points; lone surrogates are code points of their own. */
  readonly data: Uint32Array;
  /** The offset of each code point in `text`, plus the length of `text`; `undefined` when every code point takes one UTF-16 code unit. */
  private readonly utf16Offsets: Uint32Array | undefined;
  private wasmTokens: WasmTokens | undefined;
  /** The index of the next code point to read. */
  index = 0;

  constructor(
    readonly text: string,
    readonly sourceName = '<unknown>'
  ) {
    this.data = toCodePoints(text);
    const n = this.data.length;
    if (n !== text.length) {
      const offsets = new Uint32Array(n + 1);
      let offset = 0;
      for (let i = 0; i < n; i++) {
        offsets[i] = offset;
        offset += (this.data[i] ?? 0) > 0xFF_FF ? 2 : 1;
      }
      offsets[n] = offset;
      this.utf16Offsets = offsets;
    }
  }

  static fromString(text: string, sourceName?: string): CharStream {
    return new CharStream(text, sourceName);
  }

  get size(): number {
    return this.data.length;
  }

  /** The code point `i` positions ahead (1 is the next one) or behind (-1 is the previous one), or EOF. */
  LA(i: number): number {
    if (i === 0) return 0;
    const p = i < 0 ? this.index + i : this.index + i - 1;
    if (p < 0 || p >= this.data.length) return CharStream.EOF;
    return this.data[p] ?? CharStream.EOF;
  }

  consume(): void {
    if (this.index >= this.data.length) throw new Error('cannot consume EOF');
    this.index++;
  }

  seek(index: number): void {
    this.index = Math.min(index, this.data.length);
  }

  mark(): number {
    return -1;
  }

  release(_marker: number): void {}

  /** The text from code point `start` to `stop`, both inclusive. */
  getText(start: number, stop: number): string {
    const from = Math.max(start, 0);
    const to = Math.min(stop, this.data.length - 1);
    if (to < from) return '';
    if (!this.utf16Offsets) return this.text.slice(from, to + 1);
    return this.text.slice(this.utf16Offsets[from], this.utf16Offsets[to + 1]);
  }

  toString(): string {
    return this.text;
  }

  /** The input in the runtime's memory, where tokens of this input are matched and parsed. */
  get wasm(): WasmTokens {
    this.wasmTokens ??= new WasmTokens(this.data);
    return this.wasmTokens;
  }
}
