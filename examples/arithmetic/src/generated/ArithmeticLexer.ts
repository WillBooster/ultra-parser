// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

import { type CharStream, Lexer, Token } from 'ultra-parser';

export class ArithmeticLexer extends Lexer {
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

	static readonly channelNames: string[] = ["DEFAULT_TOKEN_CHANNEL", "HIDDEN"];
	static readonly literalNames: (string | null)[] = [
		null, "'^'", "'-'", "'*'", "'/'", "'+'", "'('", "')'"
	];
	static readonly symbolicNames: (string | null)[] = [
		null, null, null, null, null, null, null, null, "NUMBER", "WS"
	];
	static readonly modeNames: string[] = [
		"DEFAULT_MODE"
	];
	static readonly ruleNames: string[] = [
		"T__0", "T__1", "T__2", "T__3", "T__4", "T__5", "T__6", "NUMBER", "WS"
	];
	static readonly _serializedATN: readonly number[] = [
		4,0,9,34,6,-1,2,0,7,0,2,1,7,1,2,2,7,2,2,3,7,3,2,4,7,4,2,5,7,5,2,6,7,6,
		2,7,7,7,2,8,7,8,4,7,20,8,7,11,7,12,7,21,1,7,4,7,25,8,7,11,7,12,7,26,3,
		7,29,8,7,4,8,31,8,8,11,8,12,8,32,0,0,9,1,1,3,2,5,3,7,4,9,5,11,6,13,7,15,
		8,17,9,1,0,2,1,0,48,57,3,0,9,10,13,13,32,32,37,0,1,1,0,0,0,0,3,1,0,0,0,
		0,5,1,0,0,0,0,7,1,0,0,0,0,9,1,0,0,0,0,11,1,0,0,0,0,13,1,0,0,0,0,15,1,0,
		0,0,0,17,1,0,0,0,1,2,5,94,0,0,3,4,5,45,0,0,5,6,5,42,0,0,7,8,5,47,0,0,9,
		10,5,43,0,0,11,12,5,40,0,0,13,14,5,41,0,0,15,19,1,0,0,0,17,30,1,0,0,0,
		19,20,7,0,0,0,20,21,1,0,0,0,21,19,1,0,0,0,21,22,1,0,0,0,22,28,1,0,0,0,
		23,24,5,46,0,0,24,25,7,0,0,0,25,26,1,0,0,0,26,24,1,0,0,0,26,27,1,0,0,0,
		27,29,1,0,0,0,28,23,1,0,0,0,28,29,1,0,0,0,29,16,1,0,0,0,30,31,7,1,0,0,
		31,32,1,0,0,0,32,30,1,0,0,0,32,33,1,0,0,0,33,18,6,8,0,0,5,0,21,26,28,32,
		1,6,0,0
	];

	constructor(input: CharStream) {
		super(input);
	}

	get grammarFileName(): string {
		return "Arithmetic.g4";
	}

	get literalNames(): readonly (string | null)[] {
		return ArithmeticLexer.literalNames;
	}

	get symbolicNames(): readonly (string | null)[] {
		return ArithmeticLexer.symbolicNames;
	}

	get ruleNames(): readonly string[] {
		return ArithmeticLexer.ruleNames;
	}

	get serializedATN(): readonly number[] {
		return ArithmeticLexer._serializedATN;
	}

	get channelNames(): readonly string[] {
		return ArithmeticLexer.channelNames;
	}

	get modeNames(): readonly string[] {
		return ArithmeticLexer.modeNames;
	}
}