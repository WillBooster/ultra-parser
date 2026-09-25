/*
 * Copyright (c) 2026-present WillBooster Inc. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.test.tool;

import org.antlr.v5.codegen.target.RustTarget;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class TestRustTarget {
	@Test
	public void keepsConstantNamesDistinct() {
		Map<String, String> names = new RustTarget(null).getConstantNames(List.of("FooBar", "FOO_BAR", "T__0", "multiplyExpr", "WS"));
		assertEquals(
			Map.of("FooBar", "FOO_BAR_", "FOO_BAR", "FOO_BAR", "T__0", "T__0", "multiplyExpr", "MULTIPLY_EXPR", "WS", "WS"),
			names);
	}
}
