/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */
package org.antlr.v5.test.tool;

import org.antlr.runtime.RecognitionException;
import org.antlr.v5.automata.ParserATNFactory;
import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.semantics.SemanticPipeline;
import org.antlr.v5.tool.Grammar;
import org.junit.jupiter.api.Test;

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

	private static String generateParser(String grammarString) throws RecognitionException {
		Grammar g = new Grammar(grammarString);
		SemanticPipeline sem = new SemanticPipeline(g);
		sem.process();
		g.atn = new ParserATNFactory(g).createATN();
		return CodeGenerator.create(g).generateParser().render();
	}
}
