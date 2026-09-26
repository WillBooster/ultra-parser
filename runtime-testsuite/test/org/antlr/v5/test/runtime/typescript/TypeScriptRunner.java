/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.test.runtime.typescript;

import org.antlr.v5.codegen.target.TypeScriptTarget;
import org.antlr.v5.test.runtime.GeneratedFile;
import org.antlr.v5.test.runtime.Processor;
import org.antlr.v5.test.runtime.RunOptions;
import org.antlr.v5.test.runtime.RuntimeRunner;
import org.antlr.v5.test.runtime.RuntimeTestUtils;
import org.antlr.v5.test.runtime.Stage;
import org.antlr.v5.test.runtime.states.CompiledState;
import org.antlr.v5.test.runtime.states.GeneratedState;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

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
	 * Bun runs TypeScript directly, so the generated code only needs to resolve 'ultra-parser'.
	 * When compilation is the last stage, the TypeScript compiler checks the generated code.
	 */
	@Override
	protected CompiledState compile(RunOptions runOptions, GeneratedState generatedState) {
		try {
			Path nodeModules = Paths.get(getTempDirPath(), "node_modules");
			Files.createDirectories(nodeModules);
			Files.createSymbolicLink(nodeModules.resolve("ultra-parser"), packagePath);
			if (runOptions.endStage == Stage.Compile) {
				List<String> command = new ArrayList<>(List.of(
					rootPath.resolve(Paths.get("node_modules", ".bin", "tsc")).toString(),
					"--noEmit", "--strict", "--skipLibCheck", "--target", "es2022", "--lib", "es2022,dom",
					"--module", "nodenext", "--moduleResolution", "nodenext"));
				for (GeneratedFile file : generatedState.generatedFiles) {
					command.add(file.name);
				}
				Processor.run(command.toArray(new String[0]), getTempDirPath());
			}
			return new CompiledState(generatedState, null);
		}
		catch (Exception e) {
			return new CompiledState(generatedState, e);
		}
	}
}
