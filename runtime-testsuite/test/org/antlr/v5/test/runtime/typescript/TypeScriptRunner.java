/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.test.runtime.typescript;

import org.antlr.v5.Tool;
import org.antlr.v5.codegen.target.TypeScriptTarget;
import org.antlr.v5.parse.ANTLRParser;
import org.antlr.v5.test.runtime.GeneratedFile;
import org.antlr.v5.test.runtime.Processor;
import org.antlr.v5.test.runtime.RunOptions;
import org.antlr.v5.test.runtime.RuntimeRunner;
import org.antlr.v5.test.runtime.RuntimeTestUtils;
import org.antlr.v5.test.runtime.states.CompiledState;
import org.antlr.v5.test.runtime.states.GeneratedState;
import org.antlr.v5.tool.ANTLRMessage;
import org.antlr.v5.tool.ANTLRToolListener;
import org.antlr.v5.tool.ast.GrammarRootAST;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

/** Runs the generated TypeScript with Bun against the ultra-parser package of this repository. */
public class TypeScriptRunner extends RuntimeRunner {
	private static final Path rootPath = Paths.get(RuntimeTestUtils.runtimeTestsuitePath.toString(), "..").normalize().toAbsolutePath();
	private static final Path packagePath = rootPath.resolve(Paths.get("packages", "ultra-parser"));

	public TypeScriptRunner() {
		super();
	}

	public TypeScriptRunner(Path tempDir, boolean saveTestDir) {
		super(tempDir, saveTestDir);
	}

	@Override
	public String getLanguage() {
		return "TypeScript";
	}

	@Override
	protected String getRuntimeToolName() {
		return "bun";
	}

	@Override
	protected String grammarParseRuleToRecognizerName(String startRuleName) {
		return startRuleName != null ? TypeScriptTarget.ruleMethodName(startRuleName) : null;
	}

	@Override
	protected void initRuntime(RunOptions runOptions) throws Exception {
		if (!Files.exists(packagePath.resolve("ultra_parser.wasm"))) {
			throw new IllegalStateException("Build the runtime first: bun run build");
		}
	}

	/**
	 * Bun runs TypeScript without checking types, so the TypeScript compiler checks the generated
	 * code first, like the Java target's tests compiled it. Grammars without code are checked
	 * strictly and for unused declarations; the code in test grammars is written for Bun and not
	 * null-safe, so grammars with code are checked without these options.
	 */
	@Override
	protected CompiledState compile(RunOptions runOptions, GeneratedState generatedState) {
		try {
			Path nodeModules = Paths.get(getTempDirPath(), "node_modules");
			Files.createDirectories(nodeModules);
			Files.createSymbolicLink(nodeModules.resolve("ultra-parser"), packagePath);
			List<String> command = new ArrayList<>(List.of(
				rootPath.resolve(Paths.get("node_modules", ".bin", "tsc")).toString(),
				"--noEmit", "--skipLibCheck", "--target", "es2022", "--lib", "es2022,dom",
				"--module", "nodenext", "--moduleResolution", "nodenext",
				"--typeRoots", rootPath.resolve(Paths.get("node_modules", "@types")).toString(), "--types", "bun"));
			if (!hasGrammarCode(Paths.get(getTempDirPath()))) {
				command.add("--strict");
				command.add("--noUnusedLocals");
				command.add("--noUnusedParameters");
			}
			for (GeneratedFile file : generatedState.generatedFiles) {
				command.add(file.name);
			}
			Processor.run(command.toArray(new String[0]), getTempDirPath());
			return new CompiledState(generatedState, null);
		}
		catch (Exception e) {
			return new CompiledState(generatedState, e);
		}
	}

	/**
	 * Whether the grammars in `directory`, including imported ones, hold code: actions, predicates,
	 * or arguments.
	 */
	private static boolean hasGrammarCode(Path directory) throws IOException {
		List<String> grammars = new ArrayList<>();
		try (Stream<Path> files = Files.list(directory)) {
			for (Path file : files.filter(f -> f.toString().endsWith(".g4")).toList()) {
				grammars.add(Files.readString(file));
			}
		}
		Tool tool = new Tool();
		tool.addListener(new ANTLRToolListener() {
			@Override public void info(String msg) { }
			@Override public void error(ANTLRMessage msg) { }
			@Override public void warning(ANTLRMessage msg) { }
		});
		for (String grammar : grammars) {
			GrammarRootAST ast = tool.parseGrammarFromString(grammar);
			if (ast == null) {
				return true;
			}
			for (int type : new int[] {ANTLRParser.ACTION, ANTLRParser.SEMPRED, ANTLRParser.ARG_ACTION}) {
				if (!ast.getNodesWithType(type).isEmpty()) {
					return true;
				}
			}
		}
		return false;
	}
}
