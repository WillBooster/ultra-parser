/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.model;

import org.antlr.v5.codegen.OutputModelFactory;
import org.antlr.v5.codegen.model.chunk.ActionChunk;
import org.antlr.v5.codegen.model.chunk.ActionText;
import org.antlr.v5.parse.ANTLRParser;
import org.antlr.v5.tool.Grammar;
import org.antlr.v5.tool.Rule;
import org.antlr.v5.tool.ast.GrammarAST;

import java.util.Map;
import java.util.regex.Pattern;

/** */
public class ParserFile extends OutputFile {
	public String genPackage; // from -package cmd-line
	public String exportMacro; // from -DexportMacro cmd-line
	public boolean genListener; // from -listener cmd-line
	public boolean genVisitor; // from -visitor cmd-line
	@ModelElement public Parser parser;
	@ModelElement public Map<String, Action> namedActions;
	@ModelElement public ActionChunk contextSuperClass;
	public String grammarName;
	/** Whether the grammar's code may use the `int` type, so that the file declares it. */
	public boolean declaresInt;
	/** Whether parser rules reference tokens, whose context getters return terminal nodes. */
	public boolean hasTokenRefs;

	private static final Pattern INT = Pattern.compile("\\bint\\b");

	public ParserFile(OutputModelFactory factory, String fileName) {
		super(factory, fileName);
		Grammar g = factory.getGrammar();
		namedActions = buildNamedActions(factory.getGrammar());
		genPackage = g.tool.genPackage;
		exportMacro = factory.getGrammar().getOptionString("exportMacro");
		// need the below members in the ST for Python, C++
		genListener = g.tool.gen_listener;
		genVisitor = g.tool.gen_visitor;
		grammarName = g.name;
		for (int type : new int[] {ANTLRParser.ACTION, ANTLRParser.SEMPRED, ANTLRParser.ARG_ACTION}) {
			for (GrammarAST code : g.ast.getNodesWithType(type)) {
				declaresInt |= INT.matcher(code.getText()).find();
			}
		}
		for (Rule r : g.rules.values()) {
			hasTokenRefs |= !Grammar.isTokenName(r.name) && !r.ast.getNodesWithType(ANTLRParser.TOKEN_REF).isEmpty();
		}

		if (g.getOptionString("contextSuperClass") != null) {
			contextSuperClass = new ActionText(null, g.getOptionString("contextSuperClass"));
		}
	}
}
