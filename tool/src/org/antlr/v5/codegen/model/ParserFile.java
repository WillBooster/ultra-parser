/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.model;

import org.antlr.v5.codegen.OutputModelFactory;
import org.antlr.v5.codegen.model.chunk.ActionChunk;
import org.antlr.v5.codegen.model.chunk.ActionText;
import org.antlr.v5.tool.Attribute;
import org.antlr.v5.tool.AttributeDict;
import org.antlr.v5.tool.Grammar;
import org.antlr.v5.tool.Rule;

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
	/** Whether rule arguments, return values, or locals have the `int` type, which the file declares. */
	public boolean declaresInt;

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
		for (Rule r : g.rules.values()) {
			for (AttributeDict dict : new AttributeDict[] {r.args, r.retvals, r.locals}) {
				if (dict == null) continue;
				for (Attribute a : dict.attributes.values()) {
					declaresInt |= a.type != null && INT.matcher(a.type).find();
				}
			}
		}

		if (g.getOptionString("contextSuperClass") != null) {
			contextSuperClass = new ActionText(null, g.getOptionString("contextSuperClass"));
		}
	}
}
