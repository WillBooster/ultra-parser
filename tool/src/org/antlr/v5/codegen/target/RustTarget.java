/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.target;

import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.codegen.Target;

import java.util.Arrays;
import java.util.Collection;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * Generates Rust modules holding the serialized ATN and names of a grammar, which the
 * {@code ultra-parser-runtime} crate interprets.
 */
public class RustTarget extends Target {
	/* source: https://doc.rust-lang.org/reference/keywords.html */
	protected static final HashSet<String> reservedWords = new HashSet<>(Arrays.asList(
		"as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
		"extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
		"move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
		"trait", "true", "type", "unsafe", "use", "where", "while",
		"abstract", "become", "box", "do", "final", "macro", "override", "priv", "try",
		"typeof", "unsized", "virtual", "yield"
	));

	public RustTarget(CodeGenerator gen) {
		super(gen);
	}

	@Override
	protected Set<String> getReservedWords() {
		return reservedWords;
	}

	@Override
	public boolean isATNInterpreted() {
		return true;
	}

	/** Generate {@code t_parser.rs} and {@code t_lexer.rs} from {@code T.g4}. */
	@Override
	public String getRecognizerFileName(String recognizerName) {
		return toSnakeCase(recognizerName) + ".rs";
	}

	@Override
	public String getTargetStringLiteralFromString(String s, boolean quoted) {
		if (s == null) {
			return null;
		}
		StringBuilder buf = new StringBuilder();
		if (quoted) {
			buf.append('"');
		}
		s.codePoints().forEach(c -> {
			switch (c) {
				case '\t': buf.append("\\t"); break;
				case '\n': buf.append("\\n"); break;
				case '\r': buf.append("\\r"); break;
				case '"': buf.append("\\\""); break;
				case '\\': buf.append("\\\\"); break;
				default:
					if (c < 0x20 || c == 0x7F) {
						buf.append(String.format("\\u{%X}", c));
					}
					else {
						buf.appendCodePoint(c);
					}
			}
		});
		if (quoted) {
			buf.append('"');
		}
		return buf.toString();
	}

	/**
	 * Names constants in SCREAMING_SNAKE_CASE. Names already in that case keep it; any other name
	 * whose conversion is taken gets underscores appended until it is distinct, so that names
	 * like {@code FOO_BAR} and {@code FooBar} yield distinct constants.
	 */
	@Override
	public Map<String, String> getConstantNames(Collection<String> names) {
		Map<String, String> constantNames = new LinkedHashMap<>();
		Set<String> used = new HashSet<>();
		for (String name : names) {
			String constantName = toScreamingSnakeCase(name);
			if (constantName.equals(name)) {
				constantNames.put(name, constantName);
				used.add(constantName);
			}
		}
		for (String name : names) {
			if (constantNames.containsKey(name)) {
				continue;
			}
			String constantName = toScreamingSnakeCase(name);
			while (!used.add(constantName)) {
				constantName += "_";
			}
			constantNames.put(name, constantName);
		}
		return constantNames;
	}

	private static String toScreamingSnakeCase(String name) {
		return toSnakeCase(name).toUpperCase(Locale.ROOT);
	}

	/** Converts {@code MyGrammarLexer} to {@code my_grammar_lexer}; keeps existing underscores. */
	static String toSnakeCase(String name) {
		StringBuilder buf = new StringBuilder();
		for (int i = 0; i < name.length(); i++) {
			char c = name.charAt(i);
			if (Character.isUpperCase(c)) {
				boolean afterLower = i > 0 && (Character.isLowerCase(name.charAt(i - 1)) || Character.isDigit(name.charAt(i - 1)));
				boolean beforeLower = i > 0 && i + 1 < name.length() && Character.isUpperCase(name.charAt(i - 1)) && Character.isLowerCase(name.charAt(i + 1));
				if (afterLower || beforeLower) {
					buf.append('_');
				}
				buf.append(Character.toLowerCase(c));
			}
			else {
				buf.append(c);
			}
		}
		return buf.toString();
	}
}
