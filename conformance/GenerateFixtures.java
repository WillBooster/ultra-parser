/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

import org.antlr.v5.Tool;
import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.runtime.core.CommonTokenStream;
import org.antlr.v5.runtime.core.LexerInterpreter;
import org.antlr.v5.runtime.core.Parser;
import org.antlr.v5.runtime.core.ParserInterpreter;
import org.antlr.v5.runtime.core.Recognizer;
import org.antlr.v5.runtime.core.Token;
import org.antlr.v5.runtime.core.atn.ATN;
import org.antlr.v5.runtime.core.atn.ATNConfigSet;
import org.antlr.v5.runtime.core.atn.ATNSerializer;
import org.antlr.v5.runtime.core.dfa.DFA;
import org.antlr.v5.runtime.core.error.ANTLRErrorListener;
import org.antlr.v5.runtime.core.error.RecognitionException;
import org.antlr.v5.runtime.core.tree.ParseTree;
import org.antlr.v5.runtime.java.CharStreams;
import org.antlr.v5.tool.Grammar;
import org.antlr.v5.tool.LexerGrammar;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.Collection;
import java.util.List;
import java.util.stream.Stream;

/**
 * Records how ANTLR's own interpreters lex and parse the inputs of each grammar in the given
 * directories, so that the Rust runtime can be checked against them. Usage:
 * {@code GenerateFixtures <fixtures directory> <grammar directory>...}
 *
 * <p>Each {@code T.inputs} file goes with a combined grammar {@code T.g4} or with the grammars
 * {@code TLexer.g4} and {@code TParser.g4}. Its first line is
 * {@code start=<rule>}, and every following line is one input where {@code \n}, {@code \r},
 * {@code \t}, and {@code \\} are escapes; {@code <empty>} stands for the empty input. The output is
 * {@code T.json} in the fixtures directory.
 */
public class GenerateFixtures {
	public static void main(String[] args) throws IOException {
		Path fixturesDir = Path.of(args[0]);
		Files.createDirectories(fixturesDir);
		List<Path> inputFiles = new ArrayList<>();
		for (String grammarsDirArg : Arrays.asList(args).subList(1, args.length)) {
			try (Stream<Path> files = Files.list(Path.of(grammarsDirArg))) {
				inputFiles.addAll(files.filter(p -> p.toString().endsWith(".inputs")).sorted().toList());
			}
		}
		for (Path inputFile : inputFiles) {
			Path grammarsDir = inputFile.getParent();
			String name = inputFile.getFileName().toString().replaceFirst("\\.inputs$", "");
			List<String> lines = Files.readAllLines(inputFile, StandardCharsets.UTF_8);
			String startRule = lines.get(0).replaceFirst("^start=", "");
			Grammar g;
			LexerGrammar lg;
			String grammarFiles;
			Path combined = grammarsDir.resolve(name + ".g4");
			if (Files.exists(combined)) {
				g = Grammar.load(combined.toString());
				lg = g.getImplicitLexer();
				grammarFiles = name + ".g4";
			}
			else {
				Path lexerPath = grammarsDir.resolve(name + "Lexer.g4");
				Path parserPath = grammarsDir.resolve(name + "Parser.g4");
				// The parser grammar imports the lexer's vocabulary from its .tokens file.
				Path libDir = Files.createTempDirectory("fixtures");
				try {
					Tool tool = new Tool(new String[] {"-o", libDir.toString(), "-lib", libDir.toString(), "-Xexact-output-dir"});
					lg = (LexerGrammar) tool.loadGrammar(lexerPath.toString());
					CodeGenerator.create(lg).writeVocabFile();
					g = tool.loadGrammar(parserPath.toString());
				}
				finally {
					try (Stream<Path> files = Files.list(libDir)) {
						for (Path file : files.toList()) {
							Files.delete(file);
						}
					}
					Files.delete(libDir);
				}
				grammarFiles = name + "Lexer.g4, " + name + "Parser.g4";
			}
			if (g.tool.getNumErrors() > 0 || lg.tool.getNumErrors() > 0) {
				throw new IllegalStateException("cannot load the grammars for " + inputFile);
			}

			StringBuilder json = new StringBuilder();
			json.append("{\n");
			json.append("  \"grammar\": ").append(str(grammarFiles)).append(",\n");
			json.append("  \"startRule\": ").append(str(startRule)).append(",\n");
			List<String> channelNames = new ArrayList<>(Arrays.asList("DEFAULT_TOKEN_CHANNEL", "HIDDEN"));
			channelNames.addAll(lg.channelNameToValueMap.keySet());
			json.append("  \"lexer\": ").append(grammarData(lg, lg.atn, channelNames, lg.modes.keySet())).append(",\n");
			json.append("  \"parser\": ").append(grammarData(g, g.atn, List.of(), List.of())).append(",\n");
			json.append("  \"cases\": [");
			boolean first = true;
			for (String line : lines.subList(1, lines.size())) {
				if (line.isEmpty() || line.startsWith("#")) {
					continue;
				}
				String input = line.equals("<empty>") ? "" : unescape(line);
				json.append(first ? "\n" : ",\n").append(runCase(g, lg, startRule, input));
				first = false;
			}
			json.append("\n  ]\n}\n");
			Files.writeString(fixturesDir.resolve(name + ".json"), json.toString(), StandardCharsets.UTF_8);
		}
	}

	private static String runCase(Grammar g, LexerGrammar lg, String startRule, String input) {
		List<String> errors = new ArrayList<>();
		LexerInterpreter lexer = lg.createLexerInterpreter(CharStreams.fromString(input));
		lexer.removeErrorListeners();
		lexer.addErrorListener(new Collector(errors));
		CommonTokenStream tokens = new CommonTokenStream(lexer);
		tokens.fill();
		List<String> tokenStrings = new ArrayList<>();
		for (Token t : tokens.getTokens()) {
			tokenStrings.add(t.toString());
		}
		ParserInterpreter parser = g.createParserInterpreter(tokens);
		parser.removeErrorListeners();
		parser.addErrorListener(new Collector(errors));
		ParseTree tree = parser.parse(g.getRule(startRule).index);
		return "    {\n" +
			"      \"input\": " + str(input) + ",\n" +
			"      \"tokens\": " + strs(tokenStrings) + ",\n" +
			"      \"tree\": " + str(tree.toStringTree(parser)) + ",\n" +
			"      \"errors\": " + strs(errors) + "\n" +
			"    }";
	}

	private static String grammarData(Grammar g, ATN atn, Collection<String> channelNames, Collection<String> modeNames) {
		int[] serialized = ATNSerializer.Companion.getSerialized(atn).toArray();
		StringBuilder atnJson = new StringBuilder("[");
		for (int i = 0; i < serialized.length; i++) {
			atnJson.append(i == 0 ? "" : ",").append(serialized[i]);
		}
		atnJson.append("]");
		return "{\n" +
			"    \"recognizerName\": " + str(g.getRecognizerName()) + ",\n" +
			"    \"serializedAtn\": " + atnJson + ",\n" +
			"    \"ruleNames\": " + strs(Arrays.asList(g.getRuleNames())) + ",\n" +
			"    \"literalNames\": " + strs(Arrays.asList(g.getTokenLiteralNames())) + ",\n" +
			"    \"symbolicNames\": " + strs(Arrays.asList(g.getTokenSymbolicNames())) + ",\n" +
			"    \"channelNames\": " + strs(channelNames) + ",\n" +
			"    \"modeNames\": " + strs(modeNames) + "\n" +
			"  }";
	}

	private static String unescape(String line) {
		StringBuilder buf = new StringBuilder();
		for (int i = 0; i < line.length(); i++) {
			char c = line.charAt(i);
			if (c == '\\' && i + 1 < line.length()) {
				char next = line.charAt(++i);
				switch (next) {
					case 'n' -> buf.append('\n');
					case 'r' -> buf.append('\r');
					case 't' -> buf.append('\t');
					default -> buf.append(next);
				}
			}
			else {
				buf.append(c);
			}
		}
		return buf.toString();
	}

	private static String strs(Collection<String> values) {
		StringBuilder buf = new StringBuilder("[");
		boolean first = true;
		for (String value : values) {
			buf.append(first ? "" : ", ").append(str(value));
			first = false;
		}
		return buf.append("]").toString();
	}

	private static String str(String s) {
		if (s == null) {
			return "null";
		}
		StringBuilder buf = new StringBuilder("\"");
		for (char c : s.toCharArray()) {
			switch (c) {
				case '"' -> buf.append("\\\"");
				case '\\' -> buf.append("\\\\");
				case '\n' -> buf.append("\\n");
				case '\r' -> buf.append("\\r");
				case '\t' -> buf.append("\\t");
				default -> {
					if (c < 0x20) {
						buf.append(String.format("\\u%04x", (int) c));
					}
					else {
						buf.append(c);
					}
				}
			}
		}
		return buf.append('"').toString();
	}

	/** Formats syntax errors like {@code ConsoleErrorListener}. */
	private record Collector(List<String> errors) implements ANTLRErrorListener {
		@Override
		public void syntaxError(Recognizer<?, ?> recognizer, Object offendingSymbol, int line, int charPositionInLine, String msg, RecognitionException e) {
			errors.add("line " + line + ":" + charPositionInLine + " " + msg);
		}

		@Override
		public void reportAmbiguity(Parser recognizer, DFA dfa, int startIndex, int stopIndex, boolean exact, BitSet ambigAlts, ATNConfigSet configs) {
		}

		@Override
		public void reportAttemptingFullContext(Parser recognizer, DFA dfa, int startIndex, int stopIndex, BitSet conflictingAlts, ATNConfigSet configs) {
		}

		@Override
		public void reportContextSensitivity(Parser recognizer, DFA dfa, int startIndex, int stopIndex, int prediction, ATNConfigSet configs) {
		}
	}
}
