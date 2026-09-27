/*
 * Copyright (c) 2012-present The ANTLR Project. All rights reserved.
 * Use of this file is governed by the BSD 3-clause license that
 * can be found in the LICENSE.txt file in the project root.
 */

package org.antlr.v5.test.tool;

import org.antlr.v5.test.runtime.PredictionMode;
import org.antlr.v5.test.runtime.RunOptions;
import org.antlr.v5.test.runtime.Stage;
import org.antlr.v5.test.runtime.states.ExecutedState;
import org.antlr.v5.test.runtime.states.State;
import org.antlr.v5.test.runtime.typescript.TypeScriptRunner;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.antlr.v5.test.tool.ToolTestUtils.*;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;

/** Test parser execution.
 *
 *  For the non-greedy stuff, the rule is that .* or any other non-greedy loop
 *  (any + or * loop that has an alternative with '.' in it is automatically
 *  non-greedy) never sees past the end of the rule containing that loop.
 *  There is no automatic way to detect when the exit branch of a non-greedy
 *  loop has seen enough input to determine how much the loop should consume
 *  yet still allow matching the entire input. Of course, this is extremely
 *  inefficient, particularly for things like
 *
 *     block : '{' (block|.)* '}' ;
 *
 *  that need only see one symbol to know when it hits a '}'. So, I
 *  came up with a practical solution.  During prediction, the ATN
 *  simulator never fall off the end of a rule to compute the global
 *  FOLLOW. Instead, we terminate the loop, choosing the exit branch.
 *  Otherwise, we predict to reenter the loop.  For example, input
 *  "{ foo }" will allow the loop to match foo, but that's it. During
 *  prediction, the ATN simulator will see that '}' reaches the end of a
 *  rule that contains a non-greedy loop and stop prediction. It will choose
 *  the exit branch of the inner loop. So, the way in which you construct
 *  the rule containing a non-greedy loop dictates how far it will scan ahead.
 *  Include everything after the non-greedy loop that you know it must scan
 *  in order to properly make a prediction decision. these beasts are tricky,
 *  so be careful. don't liberally sprinkle them around your code.
 *
 *  To simulate filter mode, use ( .* (pattern1|pattern2|...) )*
 *
 *  Nongreedy loops match as much input as possible while still allowing
 *  the remaining input to match.
 */
public class TestParserExec {
	/**
	 * This is a regression test for antlr/antlr4#588 "ClassCastException during
	 * semantic predicate handling".
	 * https://github.com/antlr/antlr4/issues/588
	 */
	// TODO: port to test framework (can we simplify the Psl grammar?)
	@Test public void testFailedPredicateExceptionState() throws Exception {
		String grammar = load("Psl.g4");
		ExecutedState executedState = execParser(grammar,"floating_constant", " . 234", false);
		assertEquals("", executedState.output);
		assertEquals("line 1:6 rule floating_constant DEC:A floating-point constant cannot have internal white space\n", executedState.errors);
	}

	/** Like the finally blocks of ANTLR's generated rule methods, finally runs when grammar code throws. */
	@Test public void testFinallyRunsWhenGrammarCodeThrows() {
		String grammar =
			"grammar T;\n" +
			"s : {\n" +
			"  try { this.a(); } catch (e) { console.log('caught ' + (e as Error).message); }\n" +
			"} ;\n" +
			"a\n" +
			"@after {console.log('after a');}\n" +
			"  : e ;\n" +
			"finally {console.log('finally a');}\n" +
			"e : e '+' e | b ;\n" +
			"b : ID {if ($ID.text === 'b') throw new Error('boom');} ;\n" +
			"finally {console.log('finally b');}\n" +
			"ID : [a-z]+ ;\n" +
			"WS : [ \\t\\n]+ -> skip ;\n";
		ExecutedState executedState = execParser(grammar, "s", "a + b", false);
		assertEquals("finally b\nfinally b\nfinally a\ncaught boom\n", executedState.output);
		assertEquals("", executedState.errors);
	}

	/**
	 * Like Java finally blocks, a finally that throws still lets the enclosing rules' finally run,
	 * and the last error propagates; every rule runs its finally and exit event once.
	 */
	@Test public void testFinallyThatThrowsUnwindsEnclosingRules() {
		String grammar =
			"grammar T;\n" +
			"@parser::members {\n" +
			"failIn = '';\n" +
			"}\n" +
			"r : {\n" +
			"  this.addParseListener({exitEveryRule: (ctx) => console.log('exit ' + this.ruleNames[ctx.ruleIndex])});\n" +
			"  for (const place of ['action', 'finally']) {\n" +
			"    this.failIn = place;\n" +
			"    try { this.s(); } catch (e) { console.log('caught ' + (e as Error).message); }\n" +
			"  }\n" +
			"} ;\n" +
			"s : t ;\n" +
			"finally {console.log('outer finally');}\n" +
			"t : ID {if (this.failIn === 'action') throw new Error('boom');} ;\n" +
			"finally {console.log('inner finally'); throw new Error('cleanup');}\n" +
			"ID : [a-z]+ ;\n";
		ExecutedState executedState = execParser(grammar, "r", "a", false);
		assertEquals("inner finally\nexit t\nouter finally\nexit s\ncaught cleanup\n" +
			"inner finally\nexit t\nouter finally\nexit s\ncaught cleanup\nexit r\n", executedState.output);
		assertEquals("", executedState.errors);
	}

		/** An argument written by one action is what later actions of the rule read. */
	@Test public void testArgumentWrittenByAnAction() {
		String grammar =
			"grammar T;\n" +
			"s : a[1] ;\n" +
			"a[int i] : {$i = $i + 1;} ID {console.log($i);} ;\n" +
			"ID : [a-z]+ ;\n";
		ExecutedState executedState = execParser(grammar, "s", "abc", false);
		assertEquals("2\n", executedState.output);
		assertEquals("", executedState.errors);
	}

		/** Arguments may have names that strict code and modules cannot bind. */
	@Test public void testStrictModeArgumentNames() {
		String grammar =
			"grammar T;\n" +
			"s : a[1, 2, 3] ;\n" +
			"a[int arguments, int eval, int await] : ID {console.log($arguments + $eval + $await);} ;\n" +
			"ID : [a-z]+ ;\n";
		ExecutedState executedState = execParser(grammar, "s", "abc", false);
		assertEquals("6\n", executedState.output);
		assertEquals("", executedState.errors);
	}

	/** Actions written for ANTLR read token text with {@code $t.getText()}. */
	@Test public void testTokenGetText() {
		String grammar =
			"grammar T;\n" +
			"s : id=ID {console.log($id.getText() + ' ' + $start.getText());} ;\n" +
			"ID : [a-z]+ ;\n";
		ExecutedState executedState = execParser(grammar, "s", "abc", false);
		assertEquals("abc abc\n", executedState.output);
		assertEquals("", executedState.errors);
	}

	/**
	 * Grammar code of another parser may change the prediction mode of a running parse, which then
	 * predicts the context-sensitive decision of {@code e} with SLL and reports no full-context
	 * prediction.
	 */
	@Test public void testPredictionModeSetFromNestedParser() {
		String grammar =
			"grammar T;\n" +
			"@parser::header {\n" +
			"import { CharStream, CommonTokenStream, PredictionMode } from 'ultra-parser';\n" +
			"import { TLexer } from './TLexer.js';\n" +
			"}\n" +
			"@parser::members {\n" +
			"outer: TParser | null = null;\n" +
			"}\n" +
			"s : {\n" +
			"  const inner = new TParser(new CommonTokenStream(new TLexer(CharStream.fromString('x'))));\n" +
			"  inner.outer = this;\n" +
			"  inner.q();\n" +
			"} '$' a ;\n" +
			"q : ID {this.outer!.predictionMode = PredictionMode.SLL;} ;\n" +
			"a : e ID ;\n" +
			"b : e INT ID ;\n" +
			"e : INT | ;\n" +
			"t : '@' b ;\n" +
			"ID : [a-z]+ ;\n" +
			"INT : [0-9]+ ;\n" +
			"WS : [ \\t\\n]+ -> skip ;\n";
		ExecutedState executedState = execParser(grammar, "s", "$ 34 abc", true);
		assertEquals("", executedState.output);
		assertEquals("", executedState.errors);
	}

	/**
	 * This is a regression test for antlr/antlr4#563 "Inconsistent token
	 * handling in ANTLR4".
	 * https://github.com/antlr/antlr4/issues/563
	 */
	// TODO: port to test framework (missing templates)
	@Test public void testAlternateQuotes(@TempDir Path tempDir) {
		String lexerGrammar =
			"lexer grammar ModeTagsLexer;\n" +
			"\n" +
			"// Default mode rules (the SEA)\n" +
			"OPEN  : '«'     -> mode(ISLAND) ;       // switch to ISLAND mode\n" +
			"TEXT  : ~'«'+ ;                         // clump all text together\n" +
			"\n" +
			"mode ISLAND;\n" +
			"CLOSE : '»'     -> mode(DEFAULT_MODE) ; // back to SEA mode \n" +
			"SLASH : '/' ;\n" +
			"ID    : [a-zA-Z]+ ;                     // match/send ID in tag to parser\n";
		String parserGrammar =
			"parser grammar ModeTagsParser;\n" +
			"\n" +
			"options { tokenVocab=ModeTagsLexer; } // use tokens from ModeTagsLexer.g4\n" +
			"\n" +
			"file: (tag | TEXT)* ;\n" +
			"\n" +
			"tag : '«' ID '»'\n" +
			"    | '«' '/' ID '»'\n" +
			"    ;";

		RunOptions runOptions = new RunOptions(new String[] {parserGrammar, lexerGrammar}, null, false, false, "file",
				"«id» text «/id»", false, false, false, false, Stage.Execute, null, PredictionMode.LL, true, null);
		try (TypeScriptRunner runner = new TypeScriptRunner(tempDir, false)) {
			State state = runner.run(runOptions);
			assertInstanceOf(ExecutedState.class, state, state.getErrorMessage());
			ExecutedState executedState = (ExecutedState) state;
			assertEquals("", executedState.output);
			assertEquals("", executedState.errors);
		}
	}

	/**
	 * This is a regression test for antlr/antlr4#672 "Initialization failed in
	 * locals".
	 * https://github.com/antlr/antlr4/issues/672
	 */
	// TODO: port to test framework (missing templates)
	@Test public void testAttributeValueInitialization() {
		String grammar =
			"grammar Data; \n" +
			"\n" +
			"file : group+ EOF; \n" +
			"\n" +
			"group: INT sequence {console.log($sequence.values.length);} ; \n" +
			"\n" +
			"sequence returns [values: number[] = []] \n" +
			"  locals[localValues: number[] = []]\n" +
			"         : (INT {$localValues.push($INT.int);})* {$values.push(...$localValues);}\n" +
			"; \n" +
			"\n" +
			"INT : [0-9]+ ; // match integers \n" +
			"WS : [ \\t\\n\\r]+ -> skip ; // toss out all whitespace\n";

		String input = "2 9 10 3 1 2 3";
		ExecutedState executedState = execParser(grammar, "file", input, false);
		assertEquals("6\n", executedState.output);
		assertEquals("", executedState.errors);
	}

	@Test public void testCaseInsensitiveInCombinedGrammar() throws Exception {
		String grammar =
				"grammar CaseInsensitiveGrammar;\n" +
				"options { caseInsensitive = true; }\n" +
				"e\n" +
				"    : ID\n" +
				"    | 'not' e\n" +
				"    | e 'and' e\n" +
				"    | 'new' ID '(' e ')'\n" +
				"    ;\n" +
				"ID: [a-z_][a-z_0-9]*;\n" +
				"WS: [ \\t\\n\\r]+ -> skip;";

		String input = "NEW Abc (Not a AND not B)";
		ExecutedState executedState = execParser(grammar,"e", input, false);
		assertEquals("", executedState.output);
		assertEquals("", executedState.errors);
	}
}
