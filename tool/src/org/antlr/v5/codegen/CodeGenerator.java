/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen;

import org.antlr.v5.Tool;
import org.antlr.v5.codegen.model.OutputModelObject;
import org.antlr.v5.runtime.core.Token;
import org.antlr.v5.runtime.core.atn.ATNSerializer;
import org.antlr.v5.tool.ErrorType;
import org.antlr.v5.tool.Grammar;
import org.antlr.v5.tool.LexerGrammar;
import org.antlr.v5.tool.Rule;
import org.stringtemplate.v4.AutoIndentWriter;
import org.stringtemplate.v4.ST;
import org.stringtemplate.v4.STGroup;
import org.stringtemplate.v4.STWriter;

import java.io.IOException;
import java.io.StringWriter;
import java.io.Writer;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** General controller for code gen.  Can instantiate sub generator(s).
 */
public class CodeGenerator {
	public static final String TEMPLATE_ROOT = "org/antlr/v5/tool/templates/codegen";
	public static final String VOCAB_FILE_EXTENSION = ".tokens";
	public static final String vocabFilePattern =
		"<tokens.keys:{t | <t>=<tokens.(t)>\n}>" +
		"<literals.keys:{t | <t>=<literals.(t)>\n}>";

	public final Grammar g;

	public final Tool tool;

	public final String language;

	private Target target;

	public int lineWidth = 72;

	public static CodeGenerator create(Grammar g) {
		return create(g.tool, g, g.getLanguage());
	}

	public static CodeGenerator create(Tool tool, Grammar g, String language) {
		String targetName = "org.antlr.v5.codegen.target."+language+"Target";
		try {
			Class<? extends Target> c = Class.forName(targetName).asSubclass(Target.class);
			Constructor<? extends Target> ctor = c.getConstructor(CodeGenerator.class);
			CodeGenerator codeGenerator = new CodeGenerator(tool, g, language);
			codeGenerator.target = ctor.newInstance(codeGenerator);
			return codeGenerator;
		}
		catch (Exception e) {
			g.tool.errMgr.toolError(ErrorType.CANNOT_CREATE_TARGET_GENERATOR, e, language);
			return null;
		}
	}

	private CodeGenerator(Tool tool, Grammar g, String language) {
		this.g = g;
		this.tool = tool;
		this.language = language;
	}

	public Target getTarget() {
		return target;
	}

	public STGroup getTemplates() {
		return target.getTemplates();
	}

	// CREATE TEMPLATES BY WALKING MODEL

	private OutputModelController createController() {
		OutputModelFactory factory = new ParserFactory(this);
		OutputModelController controller = new OutputModelController(factory);
		factory.setController(controller);
		return controller;
	}

	private ST walk(OutputModelObject outputModel, boolean header) {
		OutputModelWalker walker = new OutputModelWalker(tool, getTemplates());
		return walker.walk(outputModel, header);
	}

	public ST generateLexer() { return generateLexer(false); }
	public ST generateLexer(boolean header) { return walk(createController().buildLexerOutputModel(header), header); }

	public ST generateParser() { return generateParser(false); }
	public ST generateParser(boolean header) { return walk(createController().buildParserOutputModel(header), header); }

	public ST generateListener() { return generateListener(false); }
	public ST generateListener(boolean header) { return walk(createController().buildListenerOutputModel(header), header); }

	public ST generateBaseListener() { return generateBaseListener(false); }
	public ST generateBaseListener(boolean header) { return walk(createController().buildBaseListenerOutputModel(header), header); }

	public ST generateVisitor() { return generateVisitor(false); }
	public ST generateVisitor(boolean header) { return walk(createController().buildVisitorOutputModel(header), header); }

	public ST generateBaseVisitor() { return generateBaseVisitor(false); }
	public ST generateBaseVisitor(boolean header) { return walk(createController().buildBaseVisitorOutputModel(header), header); }

	/** Generate a token vocab file with all the token names/types.  For example:
	 *  ID=7
	 *  FOR=8
	 *  'for'=8
	 *
	 *  This is independent of the target language; used by antlr internally
	 */
	ST getTokenVocabOutput() {
		ST vocabFileST = new ST(vocabFilePattern);
		Map<String,Integer> tokens = new LinkedHashMap<String,Integer>();
		// make constants for the token names
		for (String t : g.tokenNameToTypeMap.keySet()) {
			int tokenType = g.tokenNameToTypeMap.get(t);
			if ( tokenType>=Token.MIN_USER_TOKEN_TYPE) {
				tokens.put(t, tokenType);
			}
		}
		vocabFileST.add("tokens", tokens);

		// now dump the strings
		Map<String,Integer> literals = new LinkedHashMap<String,Integer>();
		for (String literal : g.stringLiteralToTypeMap.keySet()) {
			int tokenType = g.stringLiteralToTypeMap.get(literal);
			if ( tokenType>=Token.MIN_USER_TOKEN_TYPE) {
				literals.put(literal, tokenType);
			}
		}
		vocabFileST.add("literals", literals);

		return vocabFileST;
	}

	public void writeRecognizer(ST outputFileST, boolean header) {
		target.genFile(g, outputFileST, getRecognizerFileName(header));
	}

	public void writeListener(ST outputFileST, boolean header) {
		target.genFile(g, outputFileST, getListenerFileName(header));
	}

	public void writeBaseListener(ST outputFileST, boolean header) {
		target.genFile(g, outputFileST, getBaseListenerFileName(header));
	}

	public void writeVisitor(ST outputFileST, boolean header) {
		target.genFile(g, outputFileST, getVisitorFileName(header));
	}

	public void writeBaseVisitor(ST outputFileST, boolean header) {
		target.genFile(g, outputFileST, getBaseVisitorFileName(header));
	}

	public void writeVocabFile() {
		// write out the vocab interchange file; used by antlr,
		// does not change per target
		ST tokenVocabSerialization = getTokenVocabOutput();
		String fileName = getVocabFileName();
		if ( fileName!=null ) {
			target.genFile(g, tokenVocabSerialization, fileName);
		}
	}

	public void write(ST code, String fileName) {
		try (Writer w = tool.getOutputFileWriter(g, fileName)) {
			StringWriter buf = new StringWriter();
			STWriter wr = new GrammarCodeWriter(buf);
			wr.setLineWidth(lineWidth);
			code.write(wr);
			w.write(tidy(buf.toString()));
		}
		catch (IOException ioe) {
			tool.errMgr.toolError(ErrorType.CANNOT_WRITE_FILE,
								  ioe,
								  fileName);
		}
	}

	/** Indents templates' output except the grammar's code, whose whitespace may be significant. */
	public static class GrammarCodeWriter extends AutoIndentWriter {
		private boolean inGrammarCode;

		public GrammarCodeWriter(Writer out) {
			super(out);
		}

		@Override
		public int write(String str) throws IOException {
			int n = 0;
			int from = 0;
			for (int i = 0; i < str.length(); i++) {
				char c = str.charAt(i);
				if (c == GRAMMAR_CODE_START || c == GRAMMAR_CODE_END) {
					n += super.write(str.substring(from, i + 1));
					from = i + 1;
					inGrammarCode = c == GRAMMAR_CODE_START;
				}
			}
			return n + super.write(str.substring(from));
		}

		@Override
		public int indent() throws IOException {
			return inGrammarCode ? 0 : super.indent();
		}
	}

	/** Starts the grammar's code in templates' output, which {@link #tidy} leaves as it is. */
	public static final char GRAMMAR_CODE_START = '\u0001';
	/** Ends the grammar's code in templates' output. */
	public static final char GRAMMAR_CODE_END = '\u0002';

	/**
	 * Removes the blank lines that templates leave where parts render nothing: trailing
	 * whitespace, runs of blank lines, and blank lines at the start or end of a block. The
	 * grammar's code, which templates enclose in {@link #GRAMMAR_CODE_START} and
	 * {@link #GRAMMAR_CODE_END}, keeps its whitespace, and the markers are removed.
	 */
	public static String tidy(String code) {
		List<String> grammarCode = new ArrayList<>();
		StringBuilder template = new StringBuilder();
		int i = 0;
		while (i < code.length()) {
			int start = code.indexOf(GRAMMAR_CODE_START, i);
			int end = start < 0 ? -1 : code.indexOf(GRAMMAR_CODE_END, start);
			if (end < 0) {
				template.append(code, i, code.length());
				break;
			}
			template.append(code, i, start).append(GRAMMAR_CODE_START).append(grammarCode.size()).append(GRAMMAR_CODE_END);
			grammarCode.add(code.substring(start + 1, end));
			i = end + 1;
		}
		String tidied = template.toString()
			.replaceAll("(?m)[ \\t]+$", "")
			.replaceAll("\n{3,}", "\n\n")
			.replaceAll("([{(\\[])\n\n", "$1\n")
			.replaceAll("\n\n(\t*[})\\]])", "\n$1");
		StringBuilder result = new StringBuilder();
		i = 0;
		while (i < tidied.length()) {
			int start = tidied.indexOf(GRAMMAR_CODE_START, i);
			if (start < 0) {
				result.append(tidied, i, tidied.length());
				break;
			}
			int end = tidied.indexOf(GRAMMAR_CODE_END, start);
			result.append(tidied, i, start).append(grammarCode.get(Integer.parseInt(tidied.substring(start + 1, end))));
			i = end + 1;
		}
		return result.toString();
	}

	public String getRecognizerFileName() { return getRecognizerFileName(false); }
	public String getListenerFileName() { return getListenerFileName(false); }
	public String getVisitorFileName() { return getVisitorFileName(false); }
	public String getBaseListenerFileName() { return getBaseListenerFileName(false); }
	public String getBaseVisitorFileName() { return getBaseVisitorFileName(false); }

	public String getRecognizerFileName(boolean header) { return target.getRecognizerFileName(header); }
	public String getListenerFileName(boolean header) { return target.getListenerFileName(header); }
	public String getVisitorFileName(boolean header) { return target.getVisitorFileName(header); }
	public String getBaseListenerFileName(boolean header) { return target.getBaseListenerFileName(header); }
	public String getBaseVisitorFileName(boolean header) { return target.getBaseVisitorFileName(header); }

	/** What is the name of the vocab file generated for this grammar?
	 *  Returns null if no .tokens file should be generated.
	 */
	public String getVocabFileName() {
		return g.name+VOCAB_FILE_EXTENSION;
	}

	public String getHeaderFileName() {
		ST extST = getTemplates().getInstanceOf("headerFileExtension");
		if ( extST==null ) return null;
		String recognizerName = g.getRecognizerName();
		return recognizerName+extST.render();
	}

}
