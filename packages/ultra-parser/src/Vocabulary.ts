import { Token } from './Token.js';

/** The names of token types, like ANTLR's `Vocabulary`. */
export class Vocabulary {
  constructor(
    readonly literalNames: readonly (string | null)[],
    readonly symbolicNames: readonly (string | null)[]
  ) {}

  get maxTokenType(): number {
    return Math.max(this.literalNames.length, this.symbolicNames.length) - 1;
  }

  getLiteralName(tokenType: number): string | null {
    return this.literalNames[tokenType] ?? null;
  }

  getSymbolicName(tokenType: number): string | null {
    if (tokenType === Token.EOF) return 'EOF';
    return this.symbolicNames[tokenType] ?? null;
  }

  getDisplayName(tokenType: number): string {
    return this.getLiteralName(tokenType) ?? this.getSymbolicName(tokenType) ?? String(tokenType);
  }
}
