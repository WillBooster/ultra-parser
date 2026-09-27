// Loads the WebAssembly runtime and wraps its C ABI (see crates/ultra-parser-wasm).

interface WasmExports {
  memory: WebAssembly.Memory;
  alloc_bytes(len: number): number;
  free_bytes(ptr: number, len: number): void;
  last_error_ptr(): number;
  last_error_len(): number;
  grammar_new(atn: number, atnLen: number, names: number, namesLen: number, nRules: number, nLiterals: number): number;
  grammar_lexer_actions(grammar: number): number;
  grammar_expected_tokens(grammar: number, state: number, invokingStates: number, n: number): number;
  tokens_new(input: number, len: number): number;
  tokens_free(tokens: number): void;
  tokens_set(tokens: number, data: number, n: number): void;
  tokens_set_text(tokens: number, index: number, text: number, len: number): void;
  lexer_match(grammar: number, tokens: number, mode: number, index: number, line: number, column: number): number;
  parse(grammar: number, tokens: number, startToken: number, startRule: number, mode: number): number;
  set_prediction_mode(mode: number): void;
}

/** Runs the predicates of a lexer grammar while the runtime matches a token. */
export interface LexerHost {
  lexerSempred(ruleIndex: number, predIndex: number, index: number, line: number, column: number): boolean;
}

/** Receives what the runtime parses and runs the code of a parser grammar. */
export interface ParserHost {
  enterRule(
    ctx: number,
    parent: number,
    invokingState: number,
    ruleIndex: number,
    state: number,
    startToken: number,
    recursive: boolean
  ): void;
  pushRecursion(ctx: number, previous: number, state: number, previousStop: number): void;
  outerAlt(ctx: number, state: number, alt: number): void;
  exitRule(ctx: number, stopToken: number, error: boolean): void;
  unroll(ctx: number, parent: number, stopToken: number, error: boolean): void;
  token(ctx: number, state: number, tokenIndex: number, error: boolean): void;
  conjure(ctx: number, state: number, tokenType: number, line: number, column: number, addToTree: boolean): void;
  action(ctx: number, state: number, inputIndex: number): void;
  sempred(ctx: number, ruleIndex: number, predIndex: number, inputIndex: number): boolean;
  syntaxError(tokenIndex: number, line: number, column: number, message: string): void;
  failedPredicate(ctx: number, ruleIndex: number, predIndex: number, tokenIndex: number, message: string): void;
  fetched(tokenIndex: number): void;
  diagnostic(
    kind: number,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    alts: number[]
  ): void;
}

/** A call into the runtime, which hosts serve; an exception in grammar code aborts it. */
interface Call<H> {
  host: H;
  error?: { value: unknown };
}

const ABORT = 1;
const PREDICATE_ABORT = 2;

let wasm: WasmExports | undefined;
let lexerCall: Call<LexerHost> | undefined;
let parserCall: Call<ParserHost> | undefined;

function guard<H>(call: Call<H> | undefined, fn: (host: H) => void): number {
  if (!call || call.error) return ABORT;
  try {
    fn(call.host);
    return 0;
  } catch (error) {
    call.error = { value: error };
    return ABORT;
  }
}

function guardPredicate<H>(call: Call<H> | undefined, fn: (host: H) => boolean): number {
  if (!call || call.error) return PREDICATE_ABORT;
  try {
    return fn(call.host) ? 1 : 0;
  } catch (error) {
    call.error = { value: error };
    return PREDICATE_ABORT;
  }
}

const imports = {
  host: {
    lexer_sempred: (ruleIndex: number, predIndex: number, index: number, line: number, column: number): number =>
      guardPredicate(lexerCall, (h) => h.lexerSempred(ruleIndex, predIndex, index, line, column)),
    enter_rule: (
      ctx: number,
      parent: number,
      invokingState: number,
      ruleIndex: number,
      state: number,
      startToken: number,
      recursive: number
    ): number =>
      guard(parserCall, (h) => h.enterRule(ctx, parent, invokingState, ruleIndex, state, startToken, recursive !== 0)),
    fetched: (tokenIndex: number): number => guard(parserCall, (h) => h.fetched(tokenIndex)),
    push_recursion: (ctx: number, previous: number, state: number, previousStop: number): number =>
      guard(parserCall, (h) => h.pushRecursion(ctx, previous, state, previousStop)),
    outer_alt: (ctx: number, state: number, alt: number): number => guard(parserCall, (h) => h.outerAlt(ctx, state, alt)),
    exit_rule: (ctx: number, stopToken: number, error: number): number =>
      guard(parserCall, (h) => h.exitRule(ctx, stopToken, error !== 0)),
    unroll: (ctx: number, parent: number, stopToken: number, error: number): number =>
      guard(parserCall, (h) => h.unroll(ctx, parent, stopToken, error !== 0)),
    token: (ctx: number, state: number, tokenIndex: number, error: number): number =>
      guard(parserCall, (h) => h.token(ctx, state, tokenIndex, error !== 0)),
    conjure: (ctx: number, state: number, tokenType: number, line: number, column: number, addToTree: number): number =>
      guard(parserCall, (h) => h.conjure(ctx, state, tokenType, line, column, addToTree !== 0)),
    action: (ctx: number, state: number, inputIndex: number): number =>
      guard(parserCall, (h) => h.action(ctx, state, inputIndex)),
    sempred: (ctx: number, ruleIndex: number, predIndex: number, inputIndex: number): number =>
      guardPredicate(parserCall, (h) => h.sempred(ctx, ruleIndex, predIndex, inputIndex)),
    syntax_error: (tokenIndex: number, line: number, column: number, message: number, len: number): number => {
      const text = readString(message, len);
      return guard(parserCall, (h) => h.syntaxError(tokenIndex, line, column, text));
    },
    failed_predicate: (
      ctx: number,
      ruleIndex: number,
      predIndex: number,
      tokenIndex: number,
      message: number,
      len: number
    ): number => {
      const text = readString(message, len);
      return guard(parserCall, (h) => h.failedPredicate(ctx, ruleIndex, predIndex, tokenIndex, text));
    },
    diagnostic: (
      kind: number,
      decision: number,
      ruleIndex: number,
      startIndex: number,
      stopIndex: number,
      alts: number,
      nAlts: number
    ): number => {
      const altList = [...new Uint32Array(runtime().memory.buffer, alts, nAlts)];
      return guard(parserCall, (h) => h.diagnostic(kind, decision, ruleIndex, startIndex, stopIndex, altList));
    },
  },
};

/** Changes the prediction mode of the parse in progress. */
export function setPredictionMode(mode: number): void {
  runtime().set_prediction_mode(mode);
}

/** Where the WebAssembly module of the runtime comes from. */
export type WasmSource = WebAssembly.Module | BufferSource | URL | string | Response;

const defaultWasmUrl = new URL('../ultra_parser.wasm', import.meta.url);

/**
 * Initializes the runtime from a compiled module or the bytes of `ultra_parser.wasm`. Runtimes
 * that can read files (Node.js, Bun, Deno) initialize it on first use; others, such as browsers
 * and Cloudflare Workers, must call this or {@link init} first.
 */
export function initSync(source: WebAssembly.Module | BufferSource): void {
  if (wasm) return;
  const module = source instanceof WebAssembly.Module ? source : new WebAssembly.Module(source);
  wasm = new WebAssembly.Instance(module, imports).exports as unknown as WasmExports;
}

/** Initializes the runtime, fetching `ultra_parser.wasm` next to this package by default. */
export async function init(source: WasmSource = defaultWasmUrl): Promise<void> {
  if (wasm) return;
  if (source instanceof WebAssembly.Module || ArrayBuffer.isView(source) || source instanceof ArrayBuffer) {
    initSync(source);
    return;
  }
  const response = source instanceof Response ? source : await fetch(source);
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), imports);
  wasm ??= instance.exports as unknown as WasmExports;
}

function runtime(): WasmExports {
  if (wasm) return wasm;
  const fs = (
    globalThis as {
      process?: { getBuiltinModule?: (id: string) => { readFileSync?: (path: URL) => Uint8Array } | undefined };
    }
  ).process?.getBuiltinModule?.('node:fs');
  if (!fs?.readFileSync || defaultWasmUrl.protocol !== 'file:') {
    throw new Error('The ultra-parser runtime is not initialized: call init() or initSync() first.');
  }
  initSync(fs.readFileSync(defaultWasmUrl) as Uint8Array<ArrayBuffer>);
  return wasm as unknown as WasmExports;
}

// Memory

function readString(ptr: number, len: number): string {
  return new TextDecoder().decode(new Uint8Array(runtime().memory.buffer, ptr, len));
}

/** Copies `values` into the runtime's memory; the caller frees them with {@link free}. */
function allocInts(values: ArrayLike<number>, unsigned = false): number {
  const bytes = values.length * 4;
  const ptr = runtime().alloc_bytes(bytes);
  const view = unsigned
    ? new Uint32Array(runtime().memory.buffer, ptr, values.length)
    : new Int32Array(runtime().memory.buffer, ptr, values.length);
  view.set(values);
  return ptr;
}

function allocString(text: string): [ptr: number, len: number] {
  const bytes = new TextEncoder().encode(text);
  const ptr = runtime().alloc_bytes(bytes.length);
  new Uint8Array(runtime().memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

function free(ptr: number, bytes: number): void {
  runtime().free_bytes(ptr, bytes);
}

/** Reads `n` values of a result buffer, starting at `offset`. */
function readResult(ptr: number, offset: number, n: number): Int32Array {
  return new Int32Array(runtime().memory.buffer, ptr + offset * 4, n);
}

function lastError(): string {
  return readString(runtime().last_error_ptr(), runtime().last_error_len());
}

// Grammars

/** An action that a lexer runs when it accepts a token, as ANTLR serializes it. */
export interface LexerActionData {
  /** ANTLR's lexer action type: 0 channel, 1 custom, 2 mode, 3 more, 4 popMode, 5 pushMode, 6 skip, 7 type. */
  kind: number;
  data1: number;
  data2: number;
}

/** A grammar loaded into the runtime. Grammars stay loaded as long as the runtime. */
export class WasmGrammar {
  private cachedLexerActions?: LexerActionData[];

  private constructor(readonly ptr: number) {}

  static load(
    serializedATN: readonly number[],
    ruleNames: readonly string[],
    literalNames: readonly (string | null)[],
    symbolicNames: readonly (string | null)[]
  ): WasmGrammar {
    const names = [...ruleNames, ...literalNames, ...symbolicNames].map((name) => `${name ?? '\u0001'}\0`).join('');
    const atn = allocInts(serializedATN);
    const [namesPtr, namesLen] = allocString(names);
    try {
      const ptr = runtime().grammar_new(
        atn,
        serializedATN.length,
        namesPtr,
        namesLen,
        ruleNames.length,
        literalNames.length
      );
      if (ptr === 0) throw new Error(lastError());
      return new WasmGrammar(ptr);
    } finally {
      free(atn, serializedATN.length * 4);
      free(namesPtr, namesLen);
    }
  }

  get lexerActions(): LexerActionData[] {
    if (!this.cachedLexerActions) {
      const ptr = runtime().grammar_lexer_actions(this.ptr);
      const n = readResult(ptr, 0, 1)[0] ?? 0;
      const data = readResult(ptr, 1, n * 3);
      this.cachedLexerActions = [];
      for (let i = 0; i < n; i++) {
        this.cachedLexerActions.push({ kind: data[i * 3] ?? 0, data1: data[i * 3 + 1] ?? 0, data2: data[i * 3 + 2] ?? 0 });
      }
    }
    return this.cachedLexerActions;
  }

  /** The tokens that can follow `state` in a rule invoked from `invokingStates` (innermost first), as inclusive ranges. */
  expectedTokens(state: number, invokingStates: readonly number[]): [number, number][] {
    const states = allocInts(invokingStates, true);
    try {
      const ptr = runtime().grammar_expected_tokens(this.ptr, state, states, invokingStates.length);
      const n = readResult(ptr, 0, 1)[0] ?? 0;
      const data = readResult(ptr, 1, n * 2);
      const intervals: [number, number][] = [];
      for (let i = 0; i < n; i++) intervals.push([data[i * 2] ?? 0, data[i * 2 + 1] ?? 0]);
      return intervals;
    } finally {
      free(states, invokingStates.length * 4);
    }
  }

  /** Matches a token like `LexerATNSimulator.match`; see `lexer_match` in the runtime. */
  matchToken(
    tokens: WasmTokens,
    mode: number,
    index: number,
    line: number,
    column: number,
    host: LexerHost
  ): { status: number; type: number; index: number; line: number; column: number; actions: Int32Array } {
    const previous = lexerCall;
    const call: Call<LexerHost> = { host };
    lexerCall = call;
    let ptr: number;
    try {
      ptr = runtime().lexer_match(this.ptr, tokens.ptr, mode, index, line, column);
    } finally {
      lexerCall = previous;
    }
    if (call.error) throw call.error.value;
    const head = readResult(ptr, 0, 6);
    const [status = 0, type = 0, stopIndex = 0, stopLine = 0, stopColumn = 0, n = 0] = head;
    return {
      status,
      type,
      index: stopIndex,
      line: stopLine,
      column: stopColumn,
      actions: readResult(ptr, 6, n * 2).slice(),
    };
  }

  /** Parses `tokens` from token `startToken` with `startRule`, reporting to `host`; returns the id of the root context. */
  parse(tokens: WasmTokens, startToken: number, startRule: number, mode: number, host: ParserHost): number {
    const previous = parserCall;
    const call: Call<ParserHost> = { host };
    parserCall = call;
    let root: number;
    try {
      root = runtime().parse(this.ptr, tokens.ptr, startToken, startRule, mode);
    } finally {
      parserCall = previous;
    }
    if (call.error) throw call.error.value;
    return root;
  }
}

const tokensRegistry = new FinalizationRegistry<number>((ptr) => wasm?.tokens_free(ptr));

/** An input and its tokens in the runtime's memory. */
export class WasmTokens {
  readonly ptr: number;
  /** The tokens last copied in and how many there were, so that unchanged tokens are not copied again. */
  loaded: { tokens: readonly unknown[]; length: number } | undefined;
  /** How many running parses read the tokens, which must not change until they finish. */
  parses = 0;

  constructor(input: Uint32Array) {
    const data = allocInts(input, true);
    try {
      this.ptr = runtime().tokens_new(data, input.length);
    } finally {
      free(data, input.length * 4);
    }
    tokensRegistry.register(this, this.ptr, this);
  }

  /** Replaces the tokens with `(type, channel, start, stop, line, column)` tuples. */
  setTokens(data: Int32Array): void {
    const ptr = allocInts(data);
    try {
      runtime().tokens_set(this.ptr, ptr, data.length / 6);
    } finally {
      free(ptr, data.length * 4);
    }
  }

  setText(index: number, text: string): void {
    const [ptr, len] = allocString(text);
    try {
      runtime().tokens_set_text(this.ptr, index, ptr, len);
    } finally {
      free(ptr, len);
    }
  }
}
