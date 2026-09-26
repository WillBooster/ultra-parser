/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.codegen.model;

/**
 * What the parser does when it chooses one of a rule's outermost alternatives: the runtime reports
 * alternative {@link #alt} of the block at ATN state {@link #state}, or alternative 1 of the
 * rule's start state for a rule without such a block.
 */
public class AltHook {
	public final int state;
	public final int alt;
	/** The context class of a labeled alternative, which replaces the rule's context. */
	public final String ctxName;
	/** The alternative number to store in the context, if any. */
	public final Integer altNumber;
	/**
	 * The label of the left-recursive rule reference that an operator alternative of a
	 * left-recursive rule starts with; the context of the previous iteration goes there.
	 */
	public final String previousLabel;
	public final boolean previousIsList;

	public AltHook(int state, int alt, String ctxName, Integer altNumber, String previousLabel, boolean previousIsList) {
		this.state = state;
		this.alt = alt;
		this.ctxName = ctxName;
		this.altNumber = altNumber;
		this.previousLabel = previousLabel;
		this.previousIsList = previousIsList;
	}
}
