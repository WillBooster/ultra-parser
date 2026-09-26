import { Token, type TokenSource } from './Token.js';

/** A lexer error that waits until the parser fetches the token it precedes. */
interface DeferredLexerError {
  tokenIndex: number;
  report: () => void;
}

/**
 * The tokens of a lexer, of which a parser sees those on one channel, like ANTLR's
 * `CommonTokenStream`. The parser reads all tokens before it parses.
 */
export class CommonTokenStream {
  readonly tokens: Token[] = [];
  private p = -1;
  private fetchedEOF = false;
  private deferredLexerErrors: DeferredLexerError[] = [];

  constructor(
    readonly tokenSource: TokenSource,
    readonly channel: number = Token.DEFAULT_CHANNEL
  ) {}

  /**
   * Reads all tokens from the token source. With `deferLexerErrors`, the errors of a lexer are
   * reported by {@link releaseLexerErrors} instead: ANTLR lexes as the parser needs tokens, so it
   * reports each lexer error when the parser gets to the token after the error.
   */
  fill(deferLexerErrors = false): void {
    const source = this.tokenSource as TokenSource & { errorSink?: ((report: () => void) => void) | null };
    if (deferLexerErrors && !this.fetchedEOF && 'errorSink' in source) {
      source.errorSink = (report) => this.deferredLexerErrors.push({ tokenIndex: this.tokens.length, report });
      try {
        while (!this.fetchedEOF) this.fetch();
      } finally {
        source.errorSink = null;
      }
    } else {
      while (!this.fetchedEOF) this.fetch();
      this.releaseLexerErrors(this.tokens.length);
    }
    if (this.p < 0) this.p = this.nextTokenOnChannel(0);
  }

  /** Reports the deferred errors of the lexer up to the token `tokenIndex`. */
  releaseLexerErrors(tokenIndex: number): void {
    while (this.deferredLexerErrors.length > 0 && (this.deferredLexerErrors[0]?.tokenIndex ?? 0) <= tokenIndex) {
      this.deferredLexerErrors.shift()?.report();
    }
  }

  private fetch(): void {
    const token = this.tokenSource.nextToken();
    token.tokenIndex = this.tokens.length;
    this.tokens.push(token);
    if (token.type === Token.EOF) this.fetchedEOF = true;
  }

  private lazyInit(): void {
    if (this.p < 0) this.fill();
  }

  get size(): number {
    return this.tokens.length;
  }

  get index(): number {
    this.lazyInit();
    return this.p;
  }

  get(i: number): Token {
    this.lazyInit();
    const token = this.tokens[i];
    if (!token) throw new RangeError(`token index ${i} out of range 0..${this.tokens.length - 1}`);
    return token;
  }

  getTokens(start = 0, stop = this.tokens.length - 1, types?: ReadonlySet<number>): Token[] {
    this.lazyInit();
    return this.tokens.slice(start, stop + 1).filter((t) => !types || types.has(t.type));
  }

  seek(index: number): void {
    this.lazyInit();
    this.p = this.nextTokenOnChannel(index);
  }

  consume(): void {
    this.lazyInit();
    if (this.LA(1) === Token.EOF) throw new Error('cannot consume EOF');
    this.p = this.nextTokenOnChannel(this.p + 1);
  }

  LA(k: number): number {
    return this.LT(k)?.type ?? Token.INVALID_TYPE;
  }

  /** The token `k` tokens ahead on the channel (1 is the current one) or behind (-1 is the previous one). */
  LT(k: number): Token | null {
    this.lazyInit();
    if (k === 0) return null;
    if (k < 0) {
      let i = this.p;
      for (let n = 0; n < -k; n++) {
        i = this.previousTokenOnChannel(i - 1);
        if (i < 0) return null;
      }
      return this.tokens[i] ?? null;
    }
    let i = this.p;
    for (let n = 1; n < k; n++) {
      if (i + 1 < this.tokens.length) i = this.nextTokenOnChannel(i + 1);
    }
    return this.tokens[i] ?? null;
  }

  private nextTokenOnChannel(i: number): number {
    if (i >= this.tokens.length) return this.tokens.length - 1;
    let token = this.tokens[i];
    while (token && token.channel !== this.channel) {
      if (token.type === Token.EOF) return i;
      token = this.tokens[++i];
    }
    return i;
  }

  private previousTokenOnChannel(i: number): number {
    while (i >= 0) {
      const token = this.tokens[i];
      if (!token || token.type === Token.EOF || token.channel === this.channel) return i;
      i--;
    }
    return i;
  }

  /** The text of the tokens from `start` to `stop` (inclusive, on all channels), or of all tokens. */
  getText(start?: Token | number | null, stop?: Token | number | null): string {
    this.lazyInit();
    const from = start === undefined ? 0 : typeof start === 'number' ? start : start?.tokenIndex;
    const to = stop === undefined ? this.tokens.length - 1 : typeof stop === 'number' ? stop : stop?.tokenIndex;
    if (from === undefined || to === undefined || from < 0 || to < 0) return '';
    let text = '';
    for (let i = from; i <= Math.min(to, this.tokens.length - 1); i++) {
      const token = this.tokens[i] as Token;
      if (token.type === Token.EOF) break;
      text += token.text;
    }
    return text;
  }

  /** The hidden tokens right of token `index`, up to the next token on the channel. */
  getHiddenTokensToRight(index: number, channel = -1): Token[] | null {
    this.lazyInit();
    const hidden: Token[] = [];
    for (let i = index + 1; i < this.tokens.length; i++) {
      const token = this.tokens[i] as Token;
      if (token.channel === this.channel || token.type === Token.EOF) break;
      if (channel === -1 || token.channel === channel) hidden.push(token);
    }
    return hidden.length > 0 ? hidden : null;
  }

  /** The hidden tokens left of token `index`, up to the previous token on the channel. */
  getHiddenTokensToLeft(index: number, channel = -1): Token[] | null {
    this.lazyInit();
    const hidden: Token[] = [];
    for (let i = index - 1; i >= 0; i--) {
      const token = this.tokens[i] as Token;
      if (token.channel === this.channel) break;
      if (channel === -1 || token.channel === channel) hidden.unshift(token);
    }
    return hidden.length > 0 ? hidden : null;
  }
}

export type TokenStream = CommonTokenStream;
