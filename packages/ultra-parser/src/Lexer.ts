import type { CharStream } from './CharStream.js';
import { RecognitionException } from './errors.js';
import { Recognizer } from './Recognizer.js';
import { CommonToken, Token, type TokenSource } from './Token.js';
import type { LexerHost } from './wasm.js';

const enum LexerActionKind {
  Channel = 0,
  Custom = 1,
  Mode = 2,
  More = 3,
  PopMode = 4,
  PushMode = 5,
  Skip = 6,
  Type = 7,
}

/**
 * A lexer, like ANTLR's `Lexer`. The runtime matches tokens; this class runs the actions of the
 * grammar and emits tokens.
 */
export abstract class Lexer extends Recognizer implements TokenSource, LexerHost {
  static readonly DEFAULT_MODE = 0;
  static readonly MORE = -2;
  static readonly SKIP = -3;
  static readonly DEFAULT_TOKEN_CHANNEL = Token.DEFAULT_CHANNEL;
  static readonly HIDDEN = Token.HIDDEN_CHANNEL;
  static readonly MIN_CHAR_VALUE = 0x0000;
  static readonly MAX_CHAR_VALUE = 0x10ffff;

  _input: CharStream;
  /** The token to return from `nextToken`, if an action emitted one. */
  _token: Token | null = null;
  _tokenStartCharIndex = -1;
  _tokenStartLine = -1;
  _tokenStartColumn = -1;
  _hitEOF = false;
  _channel: number = Token.DEFAULT_CHANNEL;
  _type: number = Token.INVALID_TYPE;
  _modeStack: number[] = [];
  _mode = Lexer.DEFAULT_MODE;
  /** Text an action set for the current token. */
  _text: string | null = null;
  /** The line of the lexer's position in the input. */
  line = 1;
  /** The column of the lexer's position in the input. */
  column = 0;
  /** Receives the reports of errors instead of the error listeners, e.g., to defer them. */
  errorSink: ((report: () => void) => void) | null = null;

  constructor(input: CharStream) {
    super();
    this._input = input;
  }

  abstract get channelNames(): readonly string[];
  abstract get modeNames(): readonly string[];

  get inputStream(): CharStream {
    return this._input;
  }

  set inputStream(input: CharStream) {
    this._input = input;
    this.reset();
  }

  get sourceName(): string {
    return this._input.sourceName;
  }

  reset(): void {
    this._input.seek(0);
    this._token = null;
    this._type = Token.INVALID_TYPE;
    this._channel = Token.DEFAULT_CHANNEL;
    this._tokenStartCharIndex = -1;
    this._tokenStartColumn = -1;
    this._tokenStartLine = -1;
    this._text = null;
    this._hitEOF = false;
    this._mode = Lexer.DEFAULT_MODE;
    this._modeStack = [];
    this.line = 1;
    this.column = 0;
  }

  /** Returns the next token, skipping tokens that the grammar skips. */
  nextToken(): Token {
    outer: while (true) {
      if (this._hitEOF) {
        this.emitEOF();
        return this._token as Token;
      }
      this._token = null;
      this._channel = Token.DEFAULT_CHANNEL;
      this._tokenStartCharIndex = this._input.index;
      this._tokenStartColumn = this.column;
      this._tokenStartLine = this.line;
      this._text = null;
      do {
        this._type = Token.INVALID_TYPE;
        const type = this.#matchToken();
        if (this._input.LA(1) === Token.EOF) this._hitEOF = true;
        if (this._type === Token.INVALID_TYPE) this._type = type;
        if (this._type === Lexer.SKIP) continue outer;
      } while (this._type === Lexer.MORE);
      if (this._token === null) this.emit();
      return this._token as unknown as Token;
    }
  }

  /** Matches one rule and runs its actions, like `LexerATNSimulator.match`; returns the token type. */
  #matchToken(): number {
    const start = this._input.index;
    const result = this.grammar.matchToken(this._input.wasm, this._mode, start, this.line, this.column, this);
    this._input.seek(result.index);
    this.line = result.line;
    this.column = result.column;
    if (result.status !== 0) {
      this.#notifyListeners();
      this.#recover();
      return Lexer.SKIP;
    }
    this.#runActions(start, result.actions);
    return result.type;
  }

  /** Runs the actions of an accepted token like ANTLR's `LexerActionExecutor.execute`. */
  #runActions(start: number, actions: Int32Array): void {
    const lexerActions = this.grammar.lexerActions;
    const stop = this._input.index;
    let requiresSeek = false;
    try {
      for (let i = 0; i < actions.length; i += 2) {
        const action = lexerActions[actions[i] ?? 0];
        if (!action) continue;
        const offset = actions[i + 1] ?? -1;
        if (offset >= 0) {
          this._input.seek(start + offset);
          requiresSeek = start + offset !== stop;
        } else if (action.kind === LexerActionKind.Custom) {
          this._input.seek(stop);
          requiresSeek = false;
        }
        switch (action.kind) {
          case LexerActionKind.Channel:
            this._channel = action.data1;
            break;
          case LexerActionKind.Custom:
            this.action(null, action.data1, action.data2);
            break;
          case LexerActionKind.Mode:
            this.mode(action.data1);
            break;
          case LexerActionKind.More:
            this.more();
            break;
          case LexerActionKind.PopMode:
            this.popMode();
            break;
          case LexerActionKind.PushMode:
            this.pushMode(action.data1);
            break;
          case LexerActionKind.Skip:
            this.skip();
            break;
          case LexerActionKind.Type:
            this._type = action.data1;
            break;
        }
      }
    } finally {
      if (requiresSeek) this._input.seek(stop);
    }
  }

  /** Evaluates a predicate with the lexer at the position the runtime gives. */
  lexerSempred(ruleIndex: number, predIndex: number, index: number, line: number, column: number): boolean {
    const saved = [this._input.index, this.line, this.column] as const;
    this._input.seek(index);
    this.line = line;
    this.column = column;
    try {
      return this.sempred(null, ruleIndex, predIndex);
    } finally {
      this._input.seek(saved[0]);
      this.line = saved[1];
      this.column = saved[2];
    }
  }

  skip(): void {
    this._type = Lexer.SKIP;
  }

  more(): void {
    this._type = Lexer.MORE;
  }

  mode(m: number): void {
    this._mode = m;
  }

  pushMode(m: number): void {
    this._modeStack.push(this._mode);
    this.mode(m);
  }

  popMode(): number {
    const mode = this._modeStack.pop();
    if (mode === undefined) throw new Error('Empty Stack');
    this.mode(mode);
    return this._mode;
  }

  emitToken(token: Token): void {
    this._token = token;
  }

  /** Emits the current token; override it to emit custom tokens. */
  emit(): Token {
    const token = new CommonToken(
      this._type,
      this._channel,
      this._tokenStartCharIndex,
      this._input.index - 1,
      this._tokenStartLine,
      this._tokenStartColumn,
      this,
      this._input,
      this._text ?? undefined
    );
    this.emitToken(token);
    return token;
  }

  emitEOF(): Token {
    const eof = new CommonToken(
      Token.EOF,
      Token.DEFAULT_CHANNEL,
      this._input.index,
      this._input.index - 1,
      this.line,
      this.column,
      this,
      this._input
    );
    this.emitToken(eof);
    return eof;
  }

  /** All tokens up to, but excluding, EOF. */
  getAllTokens(): Token[] {
    const tokens: Token[] = [];
    for (let t = this.nextToken(); t.type !== Token.EOF; t = this.nextToken()) tokens.push(t);
    return tokens;
  }

  get type(): number {
    return this._type;
  }

  set type(type: number) {
    this._type = type;
  }

  get channel(): number {
    return this._channel;
  }

  set channel(channel: number) {
    this._channel = channel;
  }

  /** The text of the current token, including the matches before `more`, or the text an action set. */
  get text(): string {
    return this._text ?? this._input.getText(this._tokenStartCharIndex, this._input.index - 1);
  }

  set text(text: string) {
    this._text = text;
  }

  /** The index of the current code point in the input. */
  get charIndex(): number {
    return this._input.index;
  }

  notifyErrorListeners(message: string): void {
    for (const listener of this.errorListeners) {
      listener.syntaxError(this, null, this._tokenStartLine, this._tokenStartColumn, message, null);
    }
  }

  #notifyListeners(): void {
    const text = this._input.getText(this._tokenStartCharIndex, this._input.index);
    const message = `token recognition error at: '${this.getErrorDisplay(text)}'`;
    const e = new RecognitionException(message);
    const [line, column] = [this._tokenStartLine, this._tokenStartColumn];
    const report = () => {
      for (const listener of this.errorListeners) listener.syntaxError(this, null, line, column, message, e);
    };
    if (this.errorSink) this.errorSink(report);
    else report();
  }

  getErrorDisplay(text: string): string {
    return text.replace(/\n/g, '\\n').replace(/\t/g, '\\t').replace(/\r/g, '\\r');
  }

  /** Skips the code point where no rule matched. */
  #recover(): void {
    const c = this._input.LA(1);
    if (c === Token.EOF) return;
    if (c === 0x0a) {
      this.line++;
      this.column = 0;
    } else {
      this.column++;
    }
    this._input.consume();
  }
}
