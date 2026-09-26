/*
 * Copyright 2016-present The ANTLR Project. All rights reserved.
 * Licensed under the BSD-3-Clause license. See LICENSE file in the project root for license information.
 */
package org.antlr.v5.codegen.target;

import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.codegen.Target;

import java.util.Set;

/**
 * Generates TypeScript for the ultra-parser runtime: recognizers that hold the serialized ATN,
 * which the WebAssembly runtime interprets, and the grammar's code as hooks.
 */
public class TypeScriptTarget extends Target {
	/** Identifiers that variables and the members of rule contexts cannot have. */
	protected static final Set<String> reservedWords = Set.of(
		// source: https://github.com/microsoft/TypeScript/blob/fad889283e710ee947e8412e173d2c050107a3c1/src/compiler/scanner.ts
		"any", "as", "boolean", "break", "case", "catch", "class", "continue", "const", "constructor",
		"debugger", "declare", "default", "delete", "do", "else", "enum", "export", "extends", "false",
		"finally", "for", "from", "function", "get", "if", "implements", "import", "in", "instanceof",
		"interface", "let", "module", "new", "null", "number", "package", "private", "protected",
		"public", "require", "return", "set", "static", "string", "super", "switch", "symbol", "this",
		"throw", "true", "try", "type", "typeof", "var", "void", "while", "with", "yield", "of",
		// Members of the runtime's rule contexts, which getters, arguments, and return values would override.
		"parser", "parent", "invokingState", "children", "start", "stop", "exception", "ruleIndex",
		"getAltNumber", "setAltNumber", "copyFrom", "addChild", "addTokenNode", "addErrorNode",
		"removeLastChild", "getChild", "getChildCount", "getToken", "getTokens", "getRuleContext",
		"getRuleContexts", "depth", "isEmpty", "getText", "enterRule", "exitRule", "accept",
		"toStringTree", "toString"
	);

	/** Members of the runtime's parser, which the methods of rules would override. */
	private static final Set<String> parserMembers = Set.of(
		"constructor", "state", "grammarFileName", "ruleNames", "literalNames", "symbolicNames",
		"serializedATN", "vocabulary", "grammar", "errorListeners", "addErrorListener",
		"removeErrorListener", "removeErrorListeners", "notifyErrorListeners", "getTokenText",
		"sempred", "action", "buildParseTrees", "predictionMode", "tokenStream", "inputStream",
		"numberOfSyntaxErrors", "context", "reset", "getCurrentToken", "addParseListener",
		"removeParseListener", "removeParseListeners", "enterStartRule", "getExpectedTokens",
		"isExpectedToken", "getInvokingContext", "getRuleInvocationStack", "precpred", "toString"
	);

	/** The name of the parser's method for a rule; methods may have the names of keywords. */
	public static String ruleMethodName(String ruleName) {
		return parserMembers.contains(ruleName) ? ruleName + "_" : ruleName;
	}

	public TypeScriptTarget(CodeGenerator gen) {
		super(gen);
	}

	@Override
	protected Set<String> getReservedWords() {
		return reservedWords;
	}

	@Override
	public String getRuleMethodName(String ruleName) {
		return ruleMethodName(ruleName);
	}

	@Override
	public int getInlineTestSetWordSize() {
		return 32;
	}

	@Override
	public boolean wantsBaseListener() {
		return false;
	}

	@Override
	public boolean wantsBaseVisitor() {
		return false;
	}

	@Override
	public boolean supportsOverloadedMethods() {
		return true;
	}

	@Override
	public boolean isATNSerializedAsInts() {
		return true;
	}
}
