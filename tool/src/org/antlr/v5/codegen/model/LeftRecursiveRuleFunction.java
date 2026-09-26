/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.model;

import org.antlr.v5.analysis.LeftRecursiveRuleAltInfo;
import org.antlr.v5.codegen.CodeGenerator;
import org.antlr.v5.codegen.OutputModelFactory;
import org.antlr.v5.codegen.Target;
import org.antlr.v5.codegen.model.decl.RuleContextDecl;
import org.antlr.v5.codegen.model.decl.RuleContextListDecl;
import org.antlr.v5.codegen.model.decl.StructDecl;
import org.antlr.v5.parse.ANTLRParser;
import org.antlr.v5.runtime.core.state.ATNState;
import org.antlr.v5.runtime.core.state.LoopEndState;
import org.antlr.v5.runtime.core.state.RuleStopState;
import org.antlr.v5.runtime.core.state.StarLoopEntryState;
import kotlin.Pair;
import org.antlr.v5.tool.LeftRecursiveRule;
import org.antlr.v5.tool.Rule;
import org.antlr.v5.tool.ast.GrammarAST;

import java.util.List;

public class LeftRecursiveRuleFunction extends RuleFunction {
	public LeftRecursiveRuleFunction(OutputModelFactory factory, LeftRecursiveRule r) {
		super(factory, r);

		CodeGenerator gen = factory.getGenerator();
		// Since we delete x=lr, we have to manually add decls for all labels
		// on left-recur refs to proper structs
		for (Pair<GrammarAST,String> pair : r.leftRecursiveRuleRefLabels) {
			GrammarAST idAST = pair.getFirst();
			String altLabel = pair.getSecond();
			String label = idAST.getText();
			GrammarAST rrefAST = (GrammarAST)idAST.getParent().getChild(1);
			if ( rrefAST.getType() == ANTLRParser.RULE_REF ) {
				Rule targetRule = factory.getGrammar().getRule(rrefAST.getText());
				String ctxName = gen.getTarget().getRuleFunctionContextStructName(targetRule);
				RuleContextDecl d;
				if (idAST.getParent().getType() == ANTLRParser.ASSIGN) {
					d = new RuleContextDecl(factory, label, ctxName);
				}
				else {
					d = new RuleContextListDecl(factory, label, ctxName);
				}

				StructDecl struct = ruleCtx;
				if ( altLabelCtxs!=null ) {
					StructDecl s = altLabelCtxs.get(altLabel);
					if ( s!=null ) struct = s; // if alt label, use subctx
				}
				struct.addDecl(d); // stick in overall rule's ctx
			}
		}

		addAltHooks(factory, r);
	}

	/**
	 * Labeled primary alternatives replace the rule's context; operator alternatives replace the
	 * context of their iteration and label the context of the previous one.
	 */
	private void addAltHooks(OutputModelFactory factory, LeftRecursiveRule r) {
		Target target = factory.getGenerator().getTarget();
		// Rules store alternative 1 of their outermost block, as generated parsers do.
		altHooks.add(new AltHook(startState.getStateNumber(), 1, null, 1, null, false));
		List<LeftRecursiveRuleAltInfo> primaryAlts = r.recPrimaryAlts;
		for (int i = 0; i < primaryAlts.size(); i++) {
			String label = primaryAlts.get(i).altLabel;
			if ( label==null ) continue;
			int state = primaryAlts.size() > 1 ? startState.transition(0).getTarget().getStateNumber() : startState.getStateNumber();
			altHooks.add(new AltHook(state, primaryAlts.size() > 1 ? i + 1 : 1, target.getAltLabelContextStructName(label), null, null, false));
		}
		int opBlock = findPrecedenceLoopEntry(factory, r).transition(0).getTarget().getStateNumber();
		for (int i = 0; i < r.recOpAlts.size(); i++) {
			LeftRecursiveRuleAltInfo altInfo = r.recOpAlts.getElement(i);
			String ctxName = altInfo.altLabel != null ? target.getAltLabelContextStructName(altInfo.altLabel) : null;
			String previousLabel = altInfo.leftRecursiveRuleRefLabel != null ? target.escapeIfNeeded(altInfo.leftRecursiveRuleRefLabel) : null;
			if ( ctxName==null && previousLabel==null ) continue;
			altHooks.add(new AltHook(opBlock, i + 1, ctxName, null, previousLabel, altInfo.isListLabel));
		}
	}

	/** The star loop entry that decides whether the rule continues, like ANTLR's {@code markPrecedenceDecisions}. */
	private static StarLoopEntryState findPrecedenceLoopEntry(OutputModelFactory factory, Rule r) {
		for (ATNState state : factory.getGrammar().atn.getStates()) {
			if ( !(state instanceof StarLoopEntryState) || state.getRuleIndex()!=r.index ) continue;
			ATNState loopEnd = state.transition(state.getNumberOfTransitions() - 1).getTarget();
			if ( loopEnd instanceof LoopEndState && loopEnd.onlyHasEpsilonTransitions()
				 && loopEnd.transition(0).getTarget() instanceof RuleStopState ) {
				return (StarLoopEntryState)state;
			}
		}
		throw new IllegalStateException("left-recursive rule " + r.name + " has no precedence loop");
	}
}
