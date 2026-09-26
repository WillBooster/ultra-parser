import type { CharStream } from './CharStream.js';
import { BailErrorStrategy, ParseCancellationException, RecognitionException } from './errors.js';
import { IntervalSet } from './IntervalSet.js';
import { Recognizer } from './Recognizer.js';
import { CommonToken, Token } from './Token.js';
import type { CommonTokenStream } from './TokenStream.js';
import { type ParseTreeListener, ParserRuleContext } from './tree.js';
import { type ParserHost, setPredictionMode, WasmTokens } from './wasm.js';

/** How the parser predicts alternatives, like ANTLR's `PredictionMode`. */
export const PredictionMode = {
  /** SLL prediction only: faster, but it may pick the wrong alternative where full context matters. */
  SLL: 0,
  /** SLL prediction, then full-context prediction where SLL prediction finds a conflict. */
  LL: 1,
  /** Like `LL`, but full-context prediction goes on until it knows the exact ambiguity, for diagnostics. */
  LL_EXACT_AMBIG_DETECTION: 2,
} as const;
export type PredictionMode = (typeof PredictionMode)[keyof typeof PredictionMode];

/** The default error strategy: ANTLR's `DefaultErrorStrategy`, which the runtime implements. */
export class DefaultErrorStrategy {}

type Hook = (this: any, ...args: any[]) => any;

// Context classes take their own parser class and the arguments of their rule.
type ContextClass = new (
  parser: any,
  parent: ParserRuleContext | null,
  invokingState: number,
  ...args: any[]
) => ParserRuleContext;

interface AltHook {
  /** Creates the context of a labeled alternative from the context of the rule. */
  create?: Hook;
  /** Assigns the context of the previous iteration of a left-recursive rule to a label. */
  previous?: Hook;
  altNumber?: number;
}

interface InvokeHook {
  /** Creates the context of the invoked rule, e.g., with arguments. */
  create?: Hook;
  /** Run when the invoked rule returns, e.g., to assign labels. */
  exit: Hook[];
}

/**
 * The grammar code of a generated parser that the runtime reaches through ATN states: actions,
 * labels, rule arguments, contexts of labeled alternatives, and so on. Generated parsers register
 * their hooks once per class; hooks run with the parser as `this`.
 */
export class ParserHooks {
  readonly contexts: (ContextClass | undefined)[] = [];
  readonly init = new Map<number, Hook>();
  readonly after = new Map<number, Hook>();
  readonly finally = new Map<number, Hook>();
  readonly tokens = new Map<number, Hook[]>();
  readonly invokes = new Map<number, InvokeHook>();
  readonly actions = new Map<number, Hook>();
  readonly predicates = new Map<number, { text: string; message?: Hook }>();
  readonly alts = new Map<number, AltHook[]>();
  #last: Hook[] | undefined;

  /** The context class of a rule. */
  context(ruleIndex: number, contextClass: ContextClass): this {
    this.contexts[ruleIndex] = contextClass;
    return this;
  }

  /** Runs `hook` when the rule is entered (`@init`). */
  onInit(ruleIndex: number, hook: Hook): this {
    this.init.set(ruleIndex, hook);
    return this;
  }

  /** Runs `hook` when the rule completes without an error (`@after`). */
  onAfter(ruleIndex: number, hook: Hook): this {
    this.after.set(ruleIndex, hook);
    return this;
  }

  /** Runs `hook` when the rule is left (`@finally`). */
  onFinally(ruleIndex: number, hook: Hook): this {
    this.finally.set(ruleIndex, hook);
    return this;
  }

  /** Runs `hook` with the token that `state` matched. */
  token(state: number, hook: Hook): this {
    let hooks = this.tokens.get(state);
    if (!hooks) this.tokens.set(state, (hooks = []));
    hooks.push(hook);
    this.#last = hooks;
    return this;
  }

  /** Customizes how the rule invoked at `state` starts (`create`) and what happens when it returns (`exit`). */
  invoke(state: number, create?: Hook, exit?: Hook): this {
    let hook = this.invokes.get(state);
    if (!hook) this.invokes.set(state, (hook = { exit: [] }));
    if (create) hook.create = create;
    if (exit) hook.exit.push(exit);
    this.#last = hook.exit;
    return this;
  }

  /** Runs `hook` after the last token or invocation hook, e.g., to add a label to a list. */
  then(hook: Hook): this {
    this.#last?.push(hook);
    return this;
  }

  action(state: number, hook: Hook): this {
    this.actions.set(state, hook);
    return this;
  }

  /** Describes predicate `predIndex` for error messages; `message` computes a custom message. */
  predicate(predIndex: number, text: string, message?: Hook): this {
    this.predicates.set(predIndex, { text, message });
    return this;
  }

  /** Customizes alternative `alt` of the block at `state`, one of a rule's outermost alternatives. */
  alt(state: number, alt: number, hook: AltHook): this {
    let hooks = this.alts.get(state);
    if (!hooks) this.alts.set(state, (hooks = []));
    hooks[alt] = { ...hooks[alt], ...hook };
    return this;
  }
}

const hooksCache = new WeakMap<object, ParserHooks>();

/**
 * A parser, like ANTLR's `Parser`. The runtime parses the tokens; this class builds the parse tree
 * and runs the grammar code as the runtime reports its progress.
 */
export abstract class Parser extends Recognizer {
  _input: CommonTokenStream;
  /** The context of the rule being parsed. */
  _ctx: ParserRuleContext | null = null;
  buildParseTrees = true;
  #mode: PredictionMode = PredictionMode.LL;
  #parsing = false;
  _errHandler: DefaultErrorStrategy | BailErrorStrategy = new DefaultErrorStrategy();
  #parseListeners: ParseTreeListener[] = [];
  #syntaxErrors = 0;
  // The state of the current parse
  #contexts: ParserRuleContext[] = [];
  #previousContexts: (ParserRuleContext | undefined)[] = [];
  #createRoot: ((parent: ParserRuleContext | null, invokingState: number) => ParserRuleContext) | undefined;

  constructor(input: CommonTokenStream) {
    super();
    this._input = input;
  }

  /** Generated parsers register their hooks here. */
  protected static registerHooks(_hooks: ParserHooks): void {}

  get #hooks(): ParserHooks {
    let hooks = hooksCache.get(this.constructor);
    if (!hooks) {
      hooks = new ParserHooks();
      (this.constructor as typeof Parser).registerHooks(hooks);
      hooksCache.set(this.constructor, hooks);
    }
    return hooks;
  }

  get predictionMode(): PredictionMode {
    return this.#mode;
  }

  /** The prediction mode, which grammar code may also change while parsing. */
  set predictionMode(mode: PredictionMode) {
    this.#mode = mode;
    if (this.#parsing) setPredictionMode(mode);
  }

  get tokenStream(): CommonTokenStream {
    return this._input;
  }

  set tokenStream(input: CommonTokenStream) {
    this._input = input;
    this.reset();
  }

  get inputStream(): CommonTokenStream {
    return this._input;
  }

  get numberOfSyntaxErrors(): number {
    return this.#syntaxErrors;
  }

  get context(): ParserRuleContext | null {
    return this._ctx;
  }

  reset(): void {
    this._input.seek(0);
    this._ctx = null;
    this.#syntaxErrors = 0;
    this.state = -1;
  }

  getCurrentToken(): Token {
    return this._input.LT(1) as Token;
  }

  addParseListener(listener: ParseTreeListener): void {
    this.#parseListeners.push(listener);
  }

  removeParseListener(listener: ParseTreeListener): void {
    this.#parseListeners = this.#parseListeners.filter((l) => l !== listener);
  }

  removeParseListeners(): void {
    this.#parseListeners = [];
  }

  /**
   * Parses the token stream from its current token with rule `ruleIndex`, whose root context
   * `createRoot` creates; the stream then stands after the last token the rule consumed.
   */
  protected enterStartRule<T extends ParserRuleContext>(
    ruleIndex: number,
    createRoot: (parent: ParserRuleContext | null, invokingState: number) => T
  ): T {
    this._input.fill(true);
    const startToken = this._input.index;
    const tokens = this.#loadTokens();
    this.#contexts = [];
    this.#previousContexts = [];
    this.#createRoot = createRoot;
    const parsing = this.#parsing;
    this.#parsing = true;
    try {
      const root = this.grammar.parse(tokens, startToken, ruleIndex, this.#mode, this.#createHost());
      return this.#contexts[root] as T;
    } finally {
      this.#parsing = parsing;
      this.#contexts = [];
      this.#previousContexts = [];
      this.#createRoot = undefined;
    }
  }

  /** Copies the tokens into the runtime's memory, sharing the input they come from. */
  #loadTokens(): WasmTokens {
    const tokens = this._input.tokens;
    const input = tokens[0]?.inputStream ?? null;
    const shared = input && tokens.every((t) => t.inputStream === input);
    const wasmTokens = shared ? (input as CharStream).wasm : new WasmTokens(new Uint32Array());
    const data = new Int32Array(tokens.length * 6);
    let j = 0;
    for (const t of tokens) {
      data[j++] = t.type;
      data[j++] = t.channel;
      data[j++] = t.start;
      data[j++] = t.stop;
      data[j++] = t.line;
      data[j++] = t.column;
    }
    wasmTokens.setTokens(data);
    for (let i = 0; i < tokens.length; i++) {
      const t = tokens[i] as Token;
      if (!shared || t.hasText) wasmTokens.setText(i, t.text);
    }
    return wasmTokens;
  }

  #createHost(): ParserHost {
    return {
      enterRule: (ctx, parent, invokingState, ruleIndex, state, startToken, recursive) =>
        this.#onEnterRule(ctx, parent, invokingState, ruleIndex, state, startToken, recursive),
      fetched: (tokenIndex) => this._input.releaseLexerErrors(tokenIndex),
      pushRecursion: (ctx, previous, state, previousStop) => this.#onPushRecursion(ctx, previous, state, previousStop),
      outerAlt: (ctx, state, alt) => this.#onOuterAlt(ctx, state, alt),
      exitRule: (ctx, stopToken, error) => this.#onExitRule(ctx, stopToken, error),
      unroll: (ctx, parent, stopToken, error) => this.#onUnroll(ctx, parent, stopToken, error),
      token: (ctx, state, tokenIndex, error) => this.#onToken(ctx, state, tokenIndex, error),
      conjure: (ctx, state, tokenType, line, column, addToTree) =>
        this.#onConjure(ctx, state, tokenType, line, column, addToTree),
      action: (ctx, state, inputIndex) => this.#onAction(ctx, state, inputIndex),
      sempred: (ctx, ruleIndex, predIndex, inputIndex) => {
        this._input.seek(inputIndex);
        return this.sempred(ctx >= 0 ? this.#ctx(ctx) : null, ruleIndex, predIndex);
      },
      syntaxError: (tokenIndex, line, column, message) => this.#onSyntaxError(tokenIndex, line, column, message),
      failedPredicate: (ctx, ruleIndex, predIndex, tokenIndex, message) =>
        this.#onFailedPredicate(ctx, predIndex, tokenIndex, message),
      diagnostic: (kind, decision, ruleIndex, startIndex, stopIndex, alts) =>
        this.#onDiagnostic(kind, decision, ruleIndex, startIndex, stopIndex, alts),
    };
  }

  #ctx(id: number): ParserRuleContext {
    return this.#contexts[id] as ParserRuleContext;
  }

  #newContext(ruleIndex: number, parent: ParserRuleContext | null, invokingState: number): ParserRuleContext {
    const contextClass = this.#hooks.contexts[ruleIndex];
    return contextClass ? new contextClass(this, parent, invokingState) : new ParserRuleContext(parent, invokingState);
  }

  #onEnterRule(
    id: number,
    parentId: number,
    invokingState: number,
    ruleIndex: number,
    state: number,
    startToken: number,
    recursive: boolean
  ): void {
    const parent = parentId >= 0 ? this.#ctx(parentId) : null;
    this.state = state;
    let ctx: ParserRuleContext;
    if (!parent && this.#createRoot) {
      ctx = this.#createRoot(null, -1);
    } else {
      const create = this.#hooks.invokes.get(invokingState)?.create;
      ctx = create ? create.call(this, parent, invokingState) : this.#newContext(ruleIndex, parent, invokingState);
    }
    ctx.start = this._input.get(startToken);
    this.#contexts[id] = ctx;
    this._ctx = ctx;
    if (!recursive && this.buildParseTrees && parent) parent.addChild(ctx);
    this.#triggerEnterRuleEvent(ctx);
    this.#hooks.init.get(ruleIndex)?.call(this, ctx);
  }

  #onPushRecursion(id: number, previousId: number, state: number, previousStop: number): void {
    const previous = this.#ctx(previousId);
    if (this.#parseListeners.length > 0) this.#triggerExitRuleEvent(previous);
    const ctx = this.#newContext(previous.ruleIndex, previous.parent, previous.invokingState);
    previous.parent = ctx;
    previous.invokingState = state;
    previous.stop = previousStop >= 0 ? this._input.get(previousStop) : null;
    ctx.start = previous.start;
    this.#contexts[id] = ctx;
    this.#previousContexts[id] = previous;
    this._ctx = ctx;
    if (this.buildParseTrees) ctx.addChild(previous);
    this.#triggerEnterRuleEvent(ctx);
  }

  #onOuterAlt(id: number, state: number, alt: number): void {
    const hook = this.#hooks.alts.get(state)?.[alt];
    if (!hook) return;
    let ctx = this.#ctx(id);
    if (hook.create) {
      const labeled: ParserRuleContext = hook.create.call(this, ctx);
      const siblings = ctx.parent?.children;
      const index = siblings?.lastIndexOf(ctx) ?? -1;
      if (siblings && index >= 0) siblings[index] = labeled;
      this.#contexts[id] = labeled;
      if (this._ctx === ctx) this._ctx = labeled;
      ctx = labeled;
    }
    if (hook.altNumber !== undefined) ctx.setAltNumber(hook.altNumber);
    if (hook.previous) hook.previous.call(this, ctx, this.#previousContexts[id]);
  }

  #exitHooks(ctx: ParserRuleContext, stopToken: number, error: boolean): void {
    ctx.stop = stopToken >= 0 ? this._input.get(stopToken) : null;
    if (error) {
      ctx.exception ??= new RecognitionException('the rule ended early because of a syntax error', ctx.stop);
    } else {
      this.#hooks.after.get(ctx.ruleIndex)?.call(this, ctx);
    }
    this.#hooks.finally.get(ctx.ruleIndex)?.call(this, ctx);
    this.#triggerExitRuleEvent(ctx);
  }

  #returned(ctx: ParserRuleContext): void {
    this.state = ctx.invokingState;
    this._ctx = ctx.parent;
    if (!ctx.parent) return;
    for (const hook of this.#hooks.invokes.get(ctx.invokingState)?.exit ?? []) hook.call(this, ctx.parent, ctx);
  }

  #onExitRule(id: number, stopToken: number, error: boolean): void {
    const ctx = this.#ctx(id);
    this._ctx = ctx;
    this.#exitHooks(ctx, stopToken, error);
    this.#returned(ctx);
  }

  #onUnroll(id: number, parentId: number, stopToken: number, error: boolean): void {
    const ctx = this.#ctx(id);
    const parent = parentId >= 0 ? this.#ctx(parentId) : null;
    this._ctx = ctx;
    this.#exitHooks(ctx, stopToken, error);
    ctx.parent = parent;
    if (this.buildParseTrees && parent) parent.addChild(ctx);
    this.#returned(ctx);
  }

  #onToken(id: number, state: number, tokenIndex: number, error: boolean): void {
    const ctx = this.#ctx(id);
    const token = this._input.get(tokenIndex);
    this._input.seek(tokenIndex + 1);
    if (this.buildParseTrees || this.#parseListeners.length > 0) {
      if (error) {
        const node = ctx.addErrorNode(token);
        for (const listener of this.#parseListeners) listener.visitErrorNode?.(node);
      } else {
        const node = ctx.addTokenNode(token);
        for (const listener of this.#parseListeners) listener.visitTerminal?.(node);
      }
    }
    if (error || state < 0) return;
    this.state = state;
    for (const hook of this.#hooks.tokens.get(state) ?? []) hook.call(this, ctx, token);
  }

  #onConjure(id: number, state: number, type: number, line: number, column: number, addToTree: boolean): void {
    const ctx = this.#ctx(id);
    const text = type === Token.EOF ? '<missing EOF>' : `<missing ${this.vocabulary.getDisplayName(type)}>`;
    const current = this._input.LT(1);
    const token = new CommonToken(
      type,
      Token.DEFAULT_CHANNEL,
      -1,
      -1,
      line,
      column,
      current?.tokenSource ?? null,
      current?.inputStream ?? null,
      text
    );
    if (addToTree && this.buildParseTrees) ctx.addErrorNode(token);
    this.state = state;
    for (const hook of this.#hooks.tokens.get(state) ?? []) hook.call(this, ctx, token);
  }

  #onAction(id: number, state: number, inputIndex: number): void {
    const ctx = this.#ctx(id);
    this._input.seek(inputIndex);
    this.state = state;
    this._ctx = ctx;
    this.#hooks.actions.get(state)?.call(this, ctx);
  }

  #onSyntaxError(tokenIndex: number, line: number, column: number, message: string): void {
    const token = tokenIndex >= 0 ? this._input.get(tokenIndex) : null;
    this.#reportError(token, line, column, message);
  }

  #onFailedPredicate(id: number, predIndex: number, tokenIndex: number, fallback: string): void {
    const ctx = this.#ctx(id);
    const token = this._input.get(tokenIndex);
    const predicate = this.#hooks.predicates.get(predIndex);
    let message = fallback;
    if (predicate) {
      const custom: string | undefined = predicate.message?.call(this, ctx);
      const detail = custom ?? `failed predicate: {${predicate.text}}?`;
      message = `rule ${this.ruleNames[ctx.ruleIndex]} ${detail}`;
    }
    this.#reportError(token, token.line, token.column, message);
  }

  #reportError(token: Token | null, line: number, column: number, message: string): void {
    this.#syntaxErrors++;
    const e = new RecognitionException(message, token);
    for (const listener of this.errorListeners) listener.syntaxError(this, token, line, column, message, e);
    if (this._errHandler instanceof BailErrorStrategy) throw new ParseCancellationException(e);
  }

  #onDiagnostic(
    kind: number,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    alts: number[]
  ): void {
    // Listeners report at the token where prediction stopped; hooks must still see the parser's position.
    const index = this._input.index;
    this._input.seek(stopIndex);
    try {
      this.#dispatchDiagnostic(kind, decision, ruleIndex, startIndex, stopIndex, alts);
    } finally {
      this._input.seek(index);
    }
  }

  #dispatchDiagnostic(
    kind: number,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    alts: number[]
  ): void {
    for (const listener of this.errorListeners) {
      switch (kind) {
        case 0:
          listener.reportAttemptingFullContext?.(this, decision, ruleIndex, startIndex, stopIndex, alts);
          break;
        case 1:
          listener.reportContextSensitivity?.(this, decision, ruleIndex, startIndex, stopIndex);
          break;
        default:
          listener.reportAmbiguity?.(this, decision, ruleIndex, startIndex, stopIndex, kind === 3, alts);
      }
    }
  }

  #triggerEnterRuleEvent(ctx: ParserRuleContext): void {
    for (const listener of this.#parseListeners) {
      listener.enterEveryRule?.(ctx);
      ctx.enterRule(listener);
    }
  }

  #triggerExitRuleEvent(ctx: ParserRuleContext): void {
    for (let i = this.#parseListeners.length - 1; i >= 0; i--) {
      const listener = this.#parseListeners[i] as ParseTreeListener;
      ctx.exitRule(listener);
      listener.exitEveryRule?.(ctx);
    }
  }

  /** Reports a syntax error at the current token. */
  notifyErrorListeners(message: string, offendingToken: Token | null = null, e: RecognitionException | null = null): void {
    const token = offendingToken ?? this.getCurrentToken();
    this.#syntaxErrors++;
    for (const listener of this.errorListeners) {
      listener.syntaxError(this, token, token.line, token.column, message, e);
    }
  }

  override getTokenText(start: number, stop: number): string {
    return this._input.getText(start, stop);
  }

  /** The tokens that can follow the current state in the current context. */
  getExpectedTokens(): IntervalSet {
    const invokingStates: number[] = [];
    for (let ctx = this._ctx; ctx && ctx.invokingState >= 0; ctx = ctx.parent) invokingStates.push(ctx.invokingState);
    return new IntervalSet(this.grammar.expectedTokens(this.state, invokingStates));
  }

  isExpectedToken(symbol: number): boolean {
    return this.getExpectedTokens().contains(symbol);
  }

  /** The innermost context of rule `ruleIndex` being parsed, for references to attributes of other rules. */
  getInvokingContext(ruleIndex: number): ParserRuleContext | null {
    for (let p = this._ctx; p; p = p.parent) {
      if (p.ruleIndex === ruleIndex) return p;
    }
    return null;
  }

  /** The names of the rules being parsed, innermost first. */
  getRuleInvocationStack(ctx: ParserRuleContext | null = this._ctx): string[] {
    const stack: string[] = [];
    for (let p = ctx; p; p = p.parent) stack.push(this.ruleNames[p.ruleIndex] ?? 'n/a');
    return stack;
  }

  /** Precedence predicates of left-recursive rules run in the runtime; generated code never needs this. */
  precpred(_localctx: ParserRuleContext | null, _precedence: number): boolean {
    return true;
  }
}
