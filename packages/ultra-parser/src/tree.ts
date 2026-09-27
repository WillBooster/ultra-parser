import type { RecognitionException } from './errors.js';
import { Token } from './Token.js';

/** ANTLR's `ATN.INVALID_ALT_NUMBER`. */
export const INVALID_ALT_NUMBER = 0;

export type ParseTree = ParserRuleContext | TerminalNode;

/** Something that knows the rule names, such as a parser. */
export interface HasRuleNames {
  readonly ruleNames: readonly string[];
}

/** The context of a rule invocation, which is also a node of the parse tree. */
export class ParserRuleContext {
  parent: ParserRuleContext | null;
  /** The state that invoked the rule, or -1 for the root. */
  invokingState: number;
  children: ParseTree[] | null = null;
  start: Token | null = null;
  stop: Token | null = null;
  /** The error that made the rule return early, if any. */
  exception: RecognitionException | null = null;

  constructor(parent: ParserRuleContext | null = null, invokingState = -1) {
    this.parent = parent;
    this.invokingState = invokingState;
  }

  get ruleIndex(): number {
    return -1;
  }

  /** The outermost alternative the rule matched; only contexts that store it return it. */
  getAltNumber(): number {
    return INVALID_ALT_NUMBER;
  }

  setAltNumber(_altNumber: number): void {}

  /** Copies another context of the same rule into this one, which replaces it in the tree. */
  copyFrom(ctx: ParserRuleContext): void {
    this.parent = ctx.parent;
    this.invokingState = ctx.invokingState;
    this.start = ctx.start;
    this.stop = ctx.stop;
    if (ctx.children) {
      this.children = [];
      for (const child of ctx.children) {
        child.parent = this;
        this.children.push(child);
      }
    }
  }

  addChild<T extends ParseTree>(child: T): T {
    (this.children ??= []).push(child);
    return child;
  }

  addTokenNode(token: Token): TerminalNode {
    const node = new TerminalNode(token);
    node.parent = this;
    return this.addChild(node);
  }

  addErrorNode(token: Token): ErrorNode {
    const node = new ErrorNode(token);
    node.parent = this;
    return this.addChild(node);
  }

  removeLastChild(): void {
    this.children?.pop();
  }

  getChild(i: number): ParseTree | null {
    return this.children?.[i] ?? null;
  }

  getChildCount(): number {
    return this.children?.length ?? 0;
  }

  getToken(tokenType: number, i: number): TerminalNode | null {
    let j = -1;
    for (const child of this.children ?? []) {
      if (child instanceof TerminalNode && child.symbol.type === tokenType && ++j === i) return child;
    }
    return null;
  }

  getTokens(tokenType: number): TerminalNode[] {
    const tokens: TerminalNode[] = [];
    for (const child of this.children ?? []) {
      if (child instanceof TerminalNode && child.symbol.type === tokenType) tokens.push(child);
    }
    return tokens;
  }

  getRuleContext<T extends ParserRuleContext>(ctxType: abstract new (...args: never[]) => T, i: number): T | null {
    let j = -1;
    for (const child of this.children ?? []) {
      if (child instanceof ctxType && ++j === i) return child;
    }
    return null;
  }

  getRuleContexts<T extends ParserRuleContext>(ctxType: abstract new (...args: never[]) => T): T[] {
    const contexts: T[] = [];
    for (const child of this.children ?? []) {
      if (child instanceof ctxType) contexts.push(child);
    }
    return contexts;
  }

  depth(): number {
    let n = 0;
    for (let p: ParserRuleContext | null = this; p; p = p.parent) n++;
    return n;
  }

  isEmpty(): boolean {
    return this.invokingState === -1;
  }

  /** The text of the tokens in the subtree, including error nodes and without hidden tokens. */
  getText(): string {
    let text = '';
    const append = (node: TerminalNode): void => {
      text += node.getText();
    };
    walk(this, { visitTerminal: append, visitErrorNode: append });
    return text;
  }

  enterRule(_listener: ParseTreeListener): void {}

  exitRule(_listener: ParseTreeListener): void {}

  accept<T>(visitor: ParseTreeVisitor<T>): T {
    return visitor.visitChildren(this);
  }

  /** Formats the subtree like ANTLR's `Trees.toStringTree`, e.g., `(e (e 1) + (e 2))`. */
  toStringTree(ruleNames?: readonly string[] | HasRuleNames | null): string {
    const names = Array.isArray(ruleNames) ? ruleNames : (ruleNames as HasRuleNames | null | undefined)?.ruleNames;
    return toStringTree(this, names);
  }

  /**
   * Formats the invocation stack like ANTLR's `RuleContext.toString`: rule names, or invoking
   * states without them, from this context up to `stop`, e.g., `[8 0]`.
   */
  toString(ruleNames?: readonly string[] | HasRuleNames | null, stop: ParserRuleContext | null = null): string {
    const names = Array.isArray(ruleNames) ? ruleNames : (ruleNames as HasRuleNames | null | undefined)?.ruleNames;
    const parts: string[] = [];
    for (let p: ParserRuleContext | null = this; p && p !== stop; p = p.parent) {
      if (names) parts.push(names[p.ruleIndex] ?? String(p.ruleIndex));
      else if (!p.isEmpty()) parts.push(String(p.invokingState));
    }
    return `[${parts.join(' ')}]`;
  }
}

export class TerminalNode {
  parent: ParserRuleContext | null = null;

  constructor(readonly symbol: Token) {}

  getChild(_i: number): ParseTree | null {
    return null;
  }

  getChildCount(): number {
    return 0;
  }

  getText(): string {
    return this.symbol.text;
  }

  accept<T>(visitor: ParseTreeVisitor<T>): T {
    return visitor.visitTerminal(this);
  }

  toStringTree(): string {
    return this.toString();
  }

  toString(): string {
    return this.symbol.type === Token.EOF ? '<EOF>' : this.symbol.text;
  }
}

/** A token that error recovery skipped or made up. */
export class ErrorNode extends TerminalNode {
  override accept<T>(visitor: ParseTreeVisitor<T>): T {
    return visitor.visitErrorNode(this);
  }
}

export interface ParseTreeListener {
  visitTerminal?(node: TerminalNode): void;
  visitErrorNode?(node: ErrorNode): void;
  enterEveryRule?(ctx: ParserRuleContext): void;
  exitEveryRule?(ctx: ParserRuleContext): void;
}

/** Visits parse trees, like ANTLR's `AbstractParseTreeVisitor`. */
export class ParseTreeVisitor<T> {
  visit(tree: ParseTree): T {
    return tree.accept(this);
  }

  visitChildren(node: ParserRuleContext): T {
    let result = this.defaultResult();
    for (const child of node.children ?? []) {
      if (!this.shouldVisitNextChild(node, result)) break;
      result = this.aggregateResult(result, child.accept(this));
    }
    return result;
  }

  visitTerminal(_node: TerminalNode): T {
    return this.defaultResult();
  }

  visitErrorNode(_node: ErrorNode): T {
    return this.defaultResult();
  }

  protected defaultResult(): T {
    return null as T;
  }

  protected aggregateResult(_aggregate: T, nextResult: T): T {
    return nextResult;
  }

  protected shouldVisitNextChild(_node: ParserRuleContext, _currentResult: T): boolean {
    return true;
  }
}

/** Walks a tree depth-first without recursion, so that deep trees do not overflow the stack. */
function walk(tree: ParseTree, listener: ParseTreeListener): void {
  const stack: [ParseTree, number][] = [[tree, 0]];
  while (stack.length > 0) {
    const top = stack[stack.length - 1] as [ParseTree, number];
    const [node, next] = top;
    if (node instanceof TerminalNode) {
      stack.pop();
      if (node instanceof ErrorNode) listener.visitErrorNode?.(node);
      else listener.visitTerminal?.(node);
      continue;
    }
    if (next === 0) {
      listener.enterEveryRule?.(node);
      node.enterRule(listener);
    }
    const child = node.children?.[next];
    if (child) {
      top[1] = next + 1;
      stack.push([child, 0]);
    } else {
      stack.pop();
      node.exitRule(listener);
      listener.exitEveryRule?.(node);
    }
  }
}

export class ParseTreeWalker {
  static readonly DEFAULT = new ParseTreeWalker();

  walk(listener: ParseTreeListener, tree: ParseTree): void {
    walk(tree, listener);
  }
}

function escapeWhitespace(s: string): string {
  return s.replace(/\t/g, '\\t').replace(/\n/g, '\\n').replace(/\r/g, '\\r');
}

function nodeText(node: ParseTree, ruleNames: readonly string[] | undefined): string {
  if (node instanceof TerminalNode) return node.toString();
  const name = ruleNames?.[node.ruleIndex] ?? node.constructor.name;
  const alt = node.getAltNumber();
  return alt === INVALID_ALT_NUMBER ? name : `${name}:${alt}`;
}

/** Formats a tree like ANTLR's `Trees.toStringTree`, without recursion. */
export function toStringTree(tree: ParseTree, ruleNames?: readonly string[]): string {
  let text = '';
  const stack: [ParseTree, number][] = [[tree, 0]];
  while (stack.length > 0) {
    const top = stack[stack.length - 1] as [ParseTree, number];
    const [node, next] = top;
    const count = node.getChildCount();
    if (next === 0) {
      if (count === 0) {
        text += escapeWhitespace(nodeText(node, ruleNames));
        stack.pop();
        continue;
      }
      text += `(${escapeWhitespace(nodeText(node, ruleNames))}`;
    }
    const child = node.getChild(next);
    if (child) {
      top[1] = next + 1;
      text += ' ';
      stack.push([child, 0]);
    } else {
      text += ')';
      stack.pop();
    }
  }
  return text;
}
