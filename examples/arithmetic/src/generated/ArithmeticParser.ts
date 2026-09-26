// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

import {
	Parser,
	type ParserHooks,
	ParserRuleContext,
	type TerminalNode,
	Token,
	type TokenStream,
} from 'ultra-parser';
import type { ArithmeticListener } from './ArithmeticListener.js';
import type { ArithmeticVisitor } from './ArithmeticVisitor.js';

// Grammars may declare arguments and return values with Java's int type.
type int = number;

export class ArithmeticParser extends Parser {
	static readonly T__0 = 1;
	static readonly T__1 = 2;
	static readonly T__2 = 3;
	static readonly T__3 = 4;
	static readonly T__4 = 5;
	static readonly T__5 = 6;
	static readonly T__6 = 7;
	static readonly NUMBER = 8;
	static readonly WS = 9;
	static readonly EOF = Token.EOF;
	static readonly RULE_program = 0;
	static readonly RULE_expr = 1;

	static readonly literalNames: (string | null)[] = [
		null, "'^'", "'-'", "'*'", "'/'", "'+'", "'('", "')'"
	];
	static readonly symbolicNames: (string | null)[] = [
		null, null, null, null, null, null, null, null, "NUMBER", "WS"
	];
	static readonly ruleNames: string[] = [
		"program", "expr"
	];
	static readonly _serializedATN: readonly number[] = [
		4,1,9,29,2,0,7,0,2,1,7,1,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,3,1,13,8,1,1,
		1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,5,1,24,8,1,10,1,12,1,27,9,1,1,1,0,1,
		2,2,0,2,0,2,1,0,3,4,2,0,2,2,5,5,31,0,4,3,2,1,0,2,12,1,0,0,0,4,1,5,0,0,
		1,5,6,6,1,-1,0,6,7,5,2,0,0,7,13,3,2,1,5,8,9,5,6,0,0,9,10,3,2,1,0,10,13,
		5,7,0,0,11,13,5,8,0,0,12,5,1,0,0,0,12,8,1,0,0,0,12,11,1,0,0,0,13,25,1,
		0,0,0,14,15,10,6,0,0,15,16,5,1,0,0,16,24,3,2,1,6,17,18,10,4,0,0,18,19,
		7,0,0,0,19,24,3,2,1,5,20,21,10,3,0,0,21,22,7,1,0,0,22,24,3,2,1,4,23,14,
		1,0,0,0,23,17,1,0,0,0,23,20,1,0,0,0,24,27,1,0,0,0,25,23,1,0,0,0,25,26,
		1,0,0,0,26,3,1,0,0,0,27,25,1,0,0,0,3,12,23,25
	];

	get grammarFileName(): string {
		return "Arithmetic.g4";
	}

	get literalNames(): readonly (string | null)[] {
		return ArithmeticParser.literalNames;
	}

	get symbolicNames(): readonly (string | null)[] {
		return ArithmeticParser.symbolicNames;
	}

	get ruleNames(): readonly string[] {
		return ArithmeticParser.ruleNames;
	}

	get serializedATN(): readonly number[] {
		return ArithmeticParser._serializedATN;
	}

	constructor(input: TokenStream) {
		super(input);
	}

	public program(): ProgramContext {
		return this.enterStartRule(ArithmeticParser.RULE_program, (parent, invokingState) =>
			new ProgramContext(this, parent, invokingState)
		);
	}

	public expr(): ExprContext {
		return this.enterStartRule(ArithmeticParser.RULE_expr, (parent, invokingState) =>
			new ExprContext(this, parent, invokingState)
		);
	}

	override sempred(localctx: ParserRuleContext | null, ruleIndex: number, predIndex: number): boolean {
		switch (ruleIndex) {
			case 1:
				return this.expr_sempred(localctx as ExprContext, predIndex);
		}
		return true;
	}

	private expr_sempred(localctx: ExprContext, predIndex: number): boolean {
		switch (predIndex) {
			case 0:
				return (this.precpred(this._ctx, 6));
			case 1:
				return (this.precpred(this._ctx, 4));
			case 2:
				return (this.precpred(this._ctx, 3));
		}
		return true;
	}

	protected static override registerHooks(h: ParserHooks): void {
		h.context(ArithmeticParser.RULE_program, ProgramContext);
		h.alt(0, 1, {
			altNumber: 1,
		});

		h.context(ArithmeticParser.RULE_expr, ExprContext);
		h.alt(2, 1, {
			altNumber: 1,
		});
		h.token(18, function (this: ArithmeticParser, localctx: ExprContext, token: Token) {
			localctx._op = token;
		});

		h.token(21, function (this: ArithmeticParser, localctx: ExprContext, token: Token) {
			localctx._op = token;
		});
	}
}

export class ProgramContext extends ParserRuleContext {
	readonly parser: ArithmeticParser | undefined;

	constructor(parser?: ArithmeticParser, parent: ParserRuleContext | null = null, invokingState = -1) {
		super(parent, invokingState);
		this.parser = parser;
	}

	expr(): ExprContext {
		return this.getRuleContext(ExprContext, 0)!;
	}

	EOF(): TerminalNode {
		return this.getToken(ArithmeticParser.EOF, 0)!;
	}

	override get ruleIndex(): number {
		return ArithmeticParser.RULE_program;
	}
	override enterRule(listener: ArithmeticListener): void {
		listener.enterProgram?.(this);
	}

	override exitRule(listener: ArithmeticListener): void {
		listener.exitProgram?.(this);
	}

	override accept<Result>(visitor: ArithmeticVisitor<Result>): Result {
		return visitor.visitProgram ? visitor.visitProgram(this) : visitor.visitChildren(this);
	}
}

export class ExprContext extends ParserRuleContext {
	readonly parser: ArithmeticParser | undefined;
	_op!: Token;

	constructor(parser?: ArithmeticParser, parent: ParserRuleContext | null = null, invokingState = -1) {
		super(parent, invokingState);
		this.parser = parser;
	}

	expr_list(): ExprContext[] {
		return this.getRuleContexts(ExprContext);
	}

	expr(i: number): ExprContext | null {
		return this.getRuleContext(ExprContext, i);
	}

	NUMBER(): TerminalNode | null {
		return this.getToken(ArithmeticParser.NUMBER, 0);
	}

	override get ruleIndex(): number {
		return ArithmeticParser.RULE_expr;
	}
	override enterRule(listener: ArithmeticListener): void {
		listener.enterExpr?.(this);
	}

	override exitRule(listener: ArithmeticListener): void {
		listener.exitExpr?.(this);
	}

	override accept<Result>(visitor: ArithmeticVisitor<Result>): Result {
		return visitor.visitExpr ? visitor.visitExpr(this) : visitor.visitChildren(this);
	}
}
