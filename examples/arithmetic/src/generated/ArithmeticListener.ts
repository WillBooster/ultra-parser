// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

import type { ErrorNode, ParseTreeListener, ParserRuleContext, TerminalNode } from 'ultra-parser';
import type { ProgramContext, ExprContext } from './ArithmeticParser.js';

/** A listener for parse trees produced by `ArithmeticParser`. */
export class ArithmeticListener implements ParseTreeListener {
  /**
   * Enter a parse tree produced by `ArithmeticParser.program`.
   */
  enterProgram?(ctx: ProgramContext): void;
  /**
   * Exit a parse tree produced by `ArithmeticParser.program`.
   */
  exitProgram?(ctx: ProgramContext): void;
  /**
   * Enter a parse tree produced by `ArithmeticParser.expr`.
   */
  enterExpr?(ctx: ExprContext): void;
  /**
   * Exit a parse tree produced by `ArithmeticParser.expr`.
   */
  exitExpr?(ctx: ExprContext): void;
  visitTerminal?(node: TerminalNode): void;
  visitErrorNode?(node: ErrorNode): void;
  enterEveryRule?(ctx: ParserRuleContext): void;
  exitEveryRule?(ctx: ParserRuleContext): void;
}
