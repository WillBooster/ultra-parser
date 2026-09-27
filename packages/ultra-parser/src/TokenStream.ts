import { CommonToken, Token, type TokenSource } from './Token.js';

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
  readonly #tokens: Token[] = [];
  #onlyCommonTokens = true;
  private p = -1;
  private fetchedEOF = false;
  private deferredLexerErrors: DeferredLexerError[] = [];

  constructor(
    readonly tokenSource: TokenSource,
    readonly channel: number = Token.DEFAULT_CHANNEL
  ) {}

  /**
   * Reads all tokens from the token source. ANTLR lexes as tokens are needed, so it reports each
   * lexer error when a token after the error is first needed; the stream reads all tokens at once
   * but keeps the errors of a lexer until then: until a token is looked at, or until a parser
   * reports it has read the token, unless `deferLexerErrors` is false.
   */
  fill(deferLexerErrors = false): void {
    if (!this.fetchedEOF) {
      const source = this.tokenSource as TokenSource & { errorSink?: ((report: () => void) => void) | null };
      const deferring = 'errorSink' in source;
      if (deferring) {
        source.errorSink = (report) => this.deferredLexerErrors.push({ tokenIndex: this.tokens.length, report });
      }
      try {
        while (!this.fetchedEOF) this.fetch();
      } finally {
        if (deferring) source.errorSink = null;
      }
    }
    if (!deferLexerErrors) this.releaseLexerErrors(this.tokens.length);
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
    this.#tokens.push(token);
    if (Object.getPrototypeOf(token) !== CommonToken.prototype) this.#onlyCommonTokens = false;
    if (token.type === Token.EOF) this.fetchedEOF = true;
  }

  private lazyInit(): void {
    if (this.p < 0) this.fill(true);
  }

  /** The tokens read so far; they change only as the stream reads more. */
  get tokens(): readonly Token[] {
    return this.#tokens;
  }

  /**
   * Whether all tokens are of exactly the `CommonToken` class, whose changes parsers notice without
   * comparing every token; parsers copy other tokens, including of subclasses, for every parse.
   */
  get onlyCommonTokens(): boolean {
    return this.#onlyCommonTokens;
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
    this.releaseLexerErrors(i);
    const token = this.tokens[i];
    if (!token) throw new RangeError(`token index ${i} out of range 0..${this.tokens.length - 1}`);
    return token;
  }

  getTokens(start = 0, stop?: number, types?: ReadonlySet<number>): Token[] {
    this.lazyInit();
    const end = stop ?? this.tokens.length - 1;
    this.releaseLexerErrors(end);
    return this.tokens.slice(start, end + 1).filter((t) => !types || types.has(t.type));
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
    const next = this.#scanOnChannel(i);
    // Like ANTLR's lazy stream, looking at a token reports the lexer errors before it.
    this.releaseLexerErrors(next);
    return next;
  }

  #scanOnChannel(i: number): number {
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
    // Like ANTLR's, the whole text needs every token and the text of a range the tokens up to its end.
    this.releaseLexerErrors(start === undefined && stop === undefined ? this.tokens.length : to);
    let text = '';
    for (let i = from; i <= Math.min(to, this.tokens.length - 1); i++) {
      const token = this.tokens[i] as Token;
      if (token.type === Token.EOF) break;
      text += token.text;
    }
    return text;
  }

  /**
   * The hidden tokens right of token `index`, up to the next token on the default channel (on any
   * channel but the default one, or on `channel`), like ANTLR's.
   */
  getHiddenTokensToRight(index: number, channel = -1): Token[] | null {
    this.lazyInit();
    const hidden: Token[] = [];
    let i = index + 1;
    for (; i < this.tokens.length; i++) {
      const token = this.tokens[i] as Token;
      if (token.channel === Token.DEFAULT_CHANNEL || token.type === Token.EOF) break;
      if (channel === -1 || token.channel === channel) hidden.push(token);
    }
    // Like ANTLR's, the scan reads up to the next token on the default channel.
    this.releaseLexerErrors(i);
    return hidden.length > 0 ? hidden : null;
  }

  /**
   * The hidden tokens left of token `index`, up to the previous token on the default channel (on
   * any channel but the default one, or on `channel`), like ANTLR's.
   */
  getHiddenTokensToLeft(index: number, channel = -1): Token[] | null {
    this.lazyInit();
    const hidden: Token[] = [];
    for (let i = index - 1; i >= 0; i--) {
      const token = this.tokens[i] as Token;
      if (token.channel === Token.DEFAULT_CHANNEL) break;
      if (channel === -1 || token.channel === channel) hidden.unshift(token);
    }
    return hidden.length > 0 ? hidden : null;
  }
}

export type TokenStream = CommonTokenStream;
