// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

import { ParseTreeVisitor } from 'ultra-parser';
import type {
	ProgramContext,
	ExprContext,
} from './ArithmeticParser.js';

/**
 * A visitor for parse trees produced by `ArithmeticParser`.
 *
 * @param <Result> The return type of the visit operation.
 */
export class ArithmeticVisitor<Result> extends ParseTreeVisitor<Result> {
	/**
	 * Visit a parse tree produced by `ArithmeticParser.program`.
	 */
	visitProgram?(ctx: ProgramContext): Result;
	/**
	 * Visit a parse tree produced by `ArithmeticParser.expr`.
	 */
	visitExpr?(ctx: ExprContext): Result;
}

