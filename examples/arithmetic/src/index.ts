// Parses and evaluates arithmetic expressions with the parser that the ultra-parser tool generates
// from grammar/Arithmetic.g4.
import { CharStream, CommonTokenStream, type ErrorListener, type ParseTree, TerminalNode } from 'ultra-parser';

import { ArithmeticLexer } from './generated/ArithmeticLexer.js';
import { ArithmeticParser, type ExprContext, type ProgramContext } from './generated/ArithmeticParser.js';

export interface ParseResult {
  tree: ProgramContext;
  /** The parse tree as an S-expression, e.g., `(program (expr 1) <EOF>)`. */
  treeText: string;
  /** Syntax errors formatted like `line 1:4 missing ')' at '<EOF>'`. */
  errors: string[];
}

/** Parses `source`; a tree is produced even when there are syntax errors. */
export function parse(source: string): ParseResult {
  const errors: string[] = [];
  const listener: ErrorListener = {
    syntaxError: (_recognizer, _symbol, line, column, message) => errors.push(`line ${line}:${column} ${message}`),
  };
  const lexer = new ArithmeticLexer(CharStream.fromString(source));
  lexer.removeErrorListeners();
  lexer.addErrorListener(listener);
  const parser = new ArithmeticParser(new CommonTokenStream(lexer));
  parser.removeErrorListeners();
  parser.addErrorListener(listener);
  const tree = parser.program();
  return { tree, treeText: tree.toStringTree(parser), errors };
}

/** Evaluates `source`; throws the syntax errors when there are any. */
export function evaluate(source: string): number {
  const { tree, errors } = parse(source);
  if (errors.length > 0) throw new Error(errors.join('\n'));
  return evaluateExpr(tree.expr());
}

/**
 * Evaluates an expression bottom-up with an explicit stack, since left-recursive rules nest one
 * context per operator and recursion would overflow the stack for long expressions.
 */
function evaluateExpr(root: ExprContext): number {
  const values: number[] = [];
  // Each entry is a context and whether its operands are already on `values`.
  const stack: [ExprContext, boolean][] = [[root, false]];
  for (let entry = stack.pop(); entry; entry = stack.pop()) {
    const [ctx, operandsEvaluated] = entry;
    const children = ctx.children ?? [];
    if (!operandsEvaluated) {
      stack.push([ctx, true]);
      const operands = ctx.expr_list();
      for (let index = operands.length - 1; index >= 0; index--) stack.push([operands[index]!, false]);
      continue;
    }
    const pop = (): number => values.pop() as number;
    const [first, op] = children;
    if (children.length === 1) {
      values.push(Number(text(first)));
    } else if (children.length === 2) {
      values.push(-pop());
    } else if (text(first) === '(') {
      values.push(pop());
    } else {
      const right = pop();
      const left = pop();
      switch (text(op)) {
        case '^': {
          values.push(left ** right);
          break;
        }
        case '*': {
          values.push(left * right);
          break;
        }
        case '/': {
          values.push(left / right);
          break;
        }
        case '+': {
          values.push(left + right);
          break;
        }
        case '-': {
          values.push(left - right);
          break;
        }
        default: {
          throw new Error(`unknown operator ${text(op)}`);
        }
      }
    }
  }
  return values.pop() as number;
}

function text(child: ParseTree | undefined): string {
  return child instanceof TerminalNode ? child.getText() : '';
}
