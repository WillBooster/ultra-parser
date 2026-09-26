/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */
package org.antlr.v5.codegen;

import org.antlr.v5.tool.ErrorType;
import org.antlr.v5.tool.Grammar;
import org.stringtemplate.v4.ST;
import org.stringtemplate.v4.gui.STViz;

import java.io.File;
import java.io.IOException;

public class CodeGenPipeline {
	final Grammar g;
	final CodeGenerator gen;

	public CodeGenPipeline(Grammar g, CodeGenerator gen) {
		this.g = g;
		this.gen = gen;
	}

	public void process() {
		// all templates are generated in memory to report the most complete
		// error information possible, but actually writing output files stops
		// after the first error is reported
		int errorCount = g.tool.errMgr.getNumErrors();

		if ( gen.getTarget().isATNInterpreted() ) {
			String fileName = gen.getRecognizerFileName(false);
			File file = new File(g.tool.getOutputDirectory(g.fileName), fileName);
			String path;
			try {
				// Canonical paths resolve `..` and symbolic links, so aliases of a directory collide.
				path = file.getCanonicalPath();
			}
			catch (IOException e) {
				g.tool.errMgr.toolError(ErrorType.CANNOT_WRITE_FILE, e, file.getPath(), e.getMessage());
				return;
			}
			String previous = g.tool.interpretedRecognizerFiles.putIfAbsent(path, g.getRecognizerName());
			if ( previous!=null && !previous.equals(g.getRecognizerName()) ) {
				g.tool.errMgr.toolError(ErrorType.CANNOT_WRITE_FILE, path, "it is also generated for " + previous);
				return;
			}
			ST recognizer = gen.generateInterpretedRecognizer();
			if (g.tool.errMgr.getNumErrors() == errorCount) {
				writeRecognizer(recognizer, gen, false);
			}
			gen.writeVocabFile();
			return;
		}

		if ( g.isLexer() ) {
			if (gen.getTarget().needsHeader()) {
				ST lexer = gen.generateLexer(true); // Header file if needed.
				if (g.tool.errMgr.getNumErrors() == errorCount) {
					writeRecognizer(lexer, gen, true);
				}
			}
			ST lexer = gen.generateLexer(false);
			if (g.tool.errMgr.getNumErrors() == errorCount) {
				writeRecognizer(lexer, gen, false);
			}
		}
		else {
			if (gen.getTarget().needsHeader()) {
				ST parser = gen.generateParser(true);
				if (g.tool.errMgr.getNumErrors() == errorCount) {
					writeRecognizer(parser, gen, true);
				}
			}
			ST parser = gen.generateParser(false);
			if (g.tool.errMgr.getNumErrors() == errorCount) {
				writeRecognizer(parser, gen, false);
			}

			if ( g.tool.gen_listener ) {
				if (gen.getTarget().needsHeader()) {
					ST listener = gen.generateListener(true);
					if (g.tool.errMgr.getNumErrors() == errorCount) {
						gen.writeListener(listener, true);
					}
				}
				ST listener = gen.generateListener(false);
				if (g.tool.errMgr.getNumErrors() == errorCount) {
					gen.writeListener(listener, false);
				}

				if (gen.getTarget().needsHeader()) {
					ST baseListener = gen.generateBaseListener(true);
					if (g.tool.errMgr.getNumErrors() == errorCount) {
						gen.writeBaseListener(baseListener, true);
					}
				}
				if (gen.getTarget().wantsBaseListener()) {
					ST baseListener = gen.generateBaseListener(false);
					if ( g.tool.errMgr.getNumErrors()==errorCount ) {
						gen.writeBaseListener(baseListener, false);
					}
				}
			}
			if ( g.tool.gen_visitor ) {
				if (gen.getTarget().needsHeader()) {
					ST visitor = gen.generateVisitor(true);
					if (g.tool.errMgr.getNumErrors() == errorCount) {
						gen.writeVisitor(visitor, true);
					}
				}
				ST visitor = gen.generateVisitor(false);
				if (g.tool.errMgr.getNumErrors() == errorCount) {
					gen.writeVisitor(visitor, false);
				}

				if (gen.getTarget().needsHeader()) {
					ST baseVisitor = gen.generateBaseVisitor(true);
					if (g.tool.errMgr.getNumErrors() == errorCount) {
						gen.writeBaseVisitor(baseVisitor, true);
					}
				}
				if (gen.getTarget().wantsBaseVisitor()) {
					ST baseVisitor = gen.generateBaseVisitor(false);
					if ( g.tool.errMgr.getNumErrors()==errorCount ) {
						gen.writeBaseVisitor(baseVisitor, false);
					}
				}
			}
		}
		gen.writeVocabFile();
	}

	protected void writeRecognizer(ST template, CodeGenerator gen, boolean header) {
		if ( g.tool.launch_ST_inspector ) {
			STViz viz = template.inspect();
			if (g.tool.ST_inspector_wait_for_close) {
				try {
					viz.waitForClose();
				}
				catch (InterruptedException ex) {
					g.tool.errMgr.toolError(ErrorType.INTERNAL_ERROR, ex);
				}
			}
		}

		gen.writeRecognizer(template, header);
	}
}
