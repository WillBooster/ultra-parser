import type { CharStream } from './CharStream.js';

/** A token, like ANTLR's `Token`. Offsets count code points. */
export interface Token {
  type: number;
  channel: number;
  /** Offset of the first code point, or -1 for a token that error recovery made up. */
  start: number;
  /** Offset of the last code point. */
  stop: number;
  /** 1-based line. */
  line: number;
  /** 0-based column in code points. */
  column: number;
  /** Index in the token stream, or -1. */
  tokenIndex: number;
  text: string;
  readonly inputStream: CharStream | null;
  readonly tokenSource: TokenSource | null;
  /** Whether the lexer set the text instead of using the matched input. */
  readonly hasText: boolean;
  /** The text, like `text`; grammar actions written for ANTLR call `$t.getText()`. */
  getText(): string;
}

export interface TokenSource {
  nextToken(): Token;
  readonly line: number;
  readonly column: number;
  readonly inputStream: CharStream;
  readonly sourceName: string;
}

export const Token = {
  INVALID_TYPE: 0,
  EPSILON: -2,
  MIN_USER_TOKEN_TYPE: 1,
  EOF: -1,
  DEFAULT_CHANNEL: 0,
  HIDDEN_CHANNEL: 1,
} as const;

let version = 0;

/** A number that changes whenever a `CommonToken` changes, so that parsers copy changed tokens again. */
export function tokenVersion(): number {
  return version;
}

export class CommonToken implements Token {
  tokenIndex = -1;
  #type: number;
  #channel: number;
  #start: number;
  #stop: number;
  #line: number;
  #column: number;
  #explicitText: string | undefined;

  constructor(
    type: number,
    channel: number,
    start: number,
    stop: number,
    line: number,
    column: number,
    readonly tokenSource: TokenSource | null,
    readonly inputStream: CharStream | null,
    text?: string
  ) {
    this.#type = type;
    this.#channel = channel;
    this.#start = start;
    this.#stop = stop;
    this.#line = line;
    this.#column = column;
    this.#explicitText = text;
  }

  get type(): number {
    return this.#type;
  }

  set type(type: number) {
    this.#type = type;
    version++;
  }

  get channel(): number {
    return this.#channel;
  }

  set channel(channel: number) {
    this.#channel = channel;
    version++;
  }

  get start(): number {
    return this.#start;
  }

  set start(start: number) {
    this.#start = start;
    version++;
  }

  get stop(): number {
    return this.#stop;
  }

  set stop(stop: number) {
    this.#stop = stop;
    version++;
  }

  get line(): number {
    return this.#line;
  }

  set line(line: number) {
    this.#line = line;
    version++;
  }

  get column(): number {
    return this.#column;
  }

  set column(column: number) {
    this.#column = column;
    version++;
  }

  get text(): string {
    if (this.#explicitText !== undefined) return this.#explicitText;
    const input = this.inputStream;
    if (!input) return '';
    const n = input.size;
    if (this.start < n && this.stop < n) return input.getText(this.start, this.stop);
    return '<EOF>';
  }

  set text(text: string) {
    this.#explicitText = text;
    version++;
  }

  get hasText(): boolean {
    return this.#explicitText !== undefined;
  }

  getText(): string {
    return this.text;
  }

  /** Formats the token like ANTLR, e.g., `[@0,0:1='ab',<1>,1:0]`. */
  toString(): string {
    const channel = this.channel > 0 ? `,channel=${this.channel}` : '';
    const text = this.text.replace(/\n/g, '\\n').replace(/\r/g, '\\r').replace(/\t/g, '\\t');
    return `[@${this.tokenIndex},${this.start}:${this.stop}='${text}',<${this.type}>${channel},${this.line}:${this.column}]`;
  }
}
