/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */
package org.antlr.v5.test.tool;

import org.antlr.runtime.RecognitionException;
import org.antlr.v5.automata.LexerATNFactory;
import org.antlr.v5.automata.ParserATNFactory;
import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.semantics.SemanticPipeline;
import org.antlr.v5.tool.Grammar;
import org.antlr.v5.tool.LexerGrammar;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class TestCodeGeneration {
	@Test public void testArgDecl() throws Exception {
		String g =
				"grammar T;\n" +
				"a[int xyz] : 'a' ;\n";
		String parser = generateParser(g);
		assertTrue(parser.contains("a(xyz: int): AContext"), parser);
		assertTrue(parser.contains("xyz!: int;"), parser);
	}

	@Test public void AssignTokenNamesToStringLiteralsInGeneratedParserRuleContexts() throws Exception {
		String g =
			"grammar T;\n" +
			"root: 't1';\n" +
			"Token: 't1';";
		assertTrue(generateParser(g).contains("Token(): TerminalNode {"));
	}

	@Test public void AssignTokenNamesToStringLiteralArraysInGeneratedParserRuleContexts() throws Exception {
		String g =
			"grammar T;\n" +
				"root: 't1' 't1';\n" +
				"Token: 't1';";
		assertTrue(generateParser(g).contains("Token_list(): TerminalNode[] {"));
	}

	/** A recognizer extending its superclass does not import the one of the runtime, which it would not use. */
	@Test public void testSuperClassReplacesRuntimeImport() throws Exception {
		String parser = generateParser(
			"grammar T;\n" +
			"options { superClass = MyParser; }\n" +
			"a : 'a' ;\n");
		assertTrue(parser.contains("MyParser"), parser);
		assertFalse(parser.contains("\tParser,\n"), parser);
		String lexer = generateLexer(
			"lexer grammar L;\n" +
			"options { superClass = MyLexer; }\n" +
			"A : 'a' ;\n");
		assertTrue(lexer.contains("MyLexer"), lexer);
		assertFalse(lexer.contains(" Lexer, "), lexer);
	}

	private static String generateLexer(String grammarString) throws RecognitionException {
		LexerGrammar g = new LexerGrammar(grammarString);
		SemanticPipeline sem = new SemanticPipeline(g);
		sem.process();
		g.atn = new LexerATNFactory(g).createATN();
		return CodeGenerator.create(g).generateLexer().render();
	}

	private static String generateParser(String grammarString) throws RecognitionException {
		Grammar g = new Grammar(grammarString);
		SemanticPipeline sem = new SemanticPipeline(g);
		sem.process();
		g.atn = new ParserATNFactory(g).createATN();
		return CodeGenerator.create(g).generateParser().render();
	}
}
