/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.model;

import org.antlr.v5.codegen.OutputModelFactory;
import org.antlr.v5.tool.ast.ActionAST;

/** An action in an alternative, which runs when the parser passes its ATN state. */
public class InlineAction extends Action {
	public InlineAction(OutputModelFactory factory, ActionAST ast) {
		super(factory, ast);
	}
}
