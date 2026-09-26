/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.test.tool;

import org.antlr.v5.Tool;
import org.antlr.v5.codegen.target.RustTarget;
import org.antlr.v5.tool.BuildDependencyGenerator;
import org.antlr.v5.tool.Grammar;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class TestRustTarget {
	@Test
	public void keepsConstantNamesDistinct() {
		Map<String, String> names = new RustTarget(null).getConstantNames(List.of("FooBar", "FOO_BAR", "T__0", "multiplyExpr", "WS"));
		assertEquals(
			Map.of("FooBar", "FOO_BAR_", "FOO_BAR", "FOO_BAR", "T__0", "T__0", "multiplyExpr", "MULTIPLY_EXPR", "WS", "WS"),
			names);
	}

	@Test
	public void rejectsGrammarsThatGenerateTheSameFile(@TempDir Path dir) throws IOException {
		Path fooBar = Files.writeString(dir.resolve("FooBar.g4"), "grammar FooBar; s : 'a' ;");
		Files.writeString(dir.resolve("Foo_Bar.g4"), "grammar Foo_Bar; s : 'b' ;");
		// An alias of the same directory must not hide the collision.
		Path foo_bar = dir.resolve("out/../Foo_Bar.g4");
		Files.createDirectories(dir.resolve("out"));
		Tool tool = new Tool(new String[] {"-Dlanguage=Rust", fooBar.toString(), foo_bar.toString()});
		tool.processGrammarsOnCommandLine();
		assertEquals(2, tool.getNumErrors());
		assertTrue(Files.readString(dir.resolve("foo_bar_parser.rs")).startsWith("// Generated from FooBar.g4"));
	}

	@Test
	public void listsTheGeneratedFilesAsDependencies(@TempDir Path dir) throws IOException {
		Path grammar = Files.writeString(dir.resolve("Expr.g4"), "grammar Expr; s : ID ; ID : [a-z]+ ;");
		Tool tool = new Tool(new String[] {"-Dlanguage=Rust", "-listener", "-visitor", "-o", dir.toString()});
		Grammar g = tool.loadGrammar(grammar.toString());
		List<String> names = new BuildDependencyGenerator(tool, g).getGeneratedFileList().stream().map(File::getName).toList();
		assertEquals(List.of("expr_parser.rs", "Expr.tokens", "expr_lexer.rs", "ExprLexer.tokens"), names);
	}
}
