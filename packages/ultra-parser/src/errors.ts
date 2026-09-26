import type { Recognizer } from './Recognizer.js';
import type { Token } from './Token.js';

/** An error that the lexer or parser recovered from; the message is ANTLR's. */
export class RecognitionException extends Error {
  constructor(
    message: string,
    readonly offendingToken: Token | null = null
  ) {
    super(message);
    this.name = 'RecognitionException';
  }
}

/** Thrown out of a parse by a parser whose `_errHandler` is a {@link BailErrorStrategy}. */
export class ParseCancellationException extends Error {
  constructor(override readonly cause: RecognitionException) {
    super(cause.message);
    this.name = 'ParseCancellationException';
  }
}

/** Stops parsing at the first syntax error by throwing {@link ParseCancellationException}. */
export class BailErrorStrategy {}

/** Receives syntax errors and, for diagnostics, reports about prediction. */
export interface ErrorListener {
  syntaxError(
    recognizer: Recognizer,
    offendingSymbol: Token | null,
    line: number,
    column: number,
    message: string,
    e: RecognitionException | null
  ): void;
  /** Full-context prediction found `ambigAlts` ambiguous between the tokens `startIndex` and `stopIndex`. */
  reportAmbiguity?(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    exact: boolean,
    ambigAlts: readonly number[]
  ): void;
  /** SLL prediction found a conflict between `conflictingAlts` and falls back to full-context prediction. */
  reportAttemptingFullContext?(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    conflictingAlts: readonly number[]
  ): void;
  /** Full-context prediction found a unique alternative where SLL prediction found a conflict. */
  reportContextSensitivity?(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number
  ): void;
}

/** Prints syntax errors to the console like ANTLR, e.g., `line 1:4 missing ')' at '<EOF>'`. */
export class ConsoleErrorListener implements ErrorListener {
  static readonly INSTANCE = new ConsoleErrorListener();

  syntaxError(
    _recognizer: Recognizer,
    _offendingSymbol: Token | null,
    line: number,
    column: number,
    message: string
  ): void {
    console.error(`line ${line}:${column} ${message}`);
  }
}

function decisionDescription(recognizer: Recognizer, decision: number, ruleIndex: number): string {
  const ruleName = recognizer.ruleNames[ruleIndex];
  return ruleName ? `${decision} (${ruleName})` : String(decision);
}

function formatAlts(alts: readonly number[]): string {
  return `{${alts.join(', ')}}`;
}

/** Reports prediction events as syntax errors, like ANTLR's `DiagnosticErrorListener`. */
export class DiagnosticErrorListener implements ErrorListener {
  /** @param exactOnly whether to report only exact ambiguities. */
  constructor(readonly exactOnly = true) {}

  syntaxError(): void {}

  reportAmbiguity(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number,
    exact: boolean,
    ambigAlts: readonly number[]
  ): void {
    if (this.exactOnly && !exact) return;
    recognizer.notifyErrorListeners(
      `reportAmbiguity d=${decisionDescription(recognizer, decision, ruleIndex)}: ambigAlts=${formatAlts(ambigAlts)}, input='${recognizer.getTokenText(startIndex, stopIndex)}'`
    );
  }

  reportAttemptingFullContext(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number
  ): void {
    recognizer.notifyErrorListeners(
      `reportAttemptingFullContext d=${decisionDescription(recognizer, decision, ruleIndex)}, input='${recognizer.getTokenText(startIndex, stopIndex)}'`
    );
  }

  reportContextSensitivity(
    recognizer: Recognizer,
    decision: number,
    ruleIndex: number,
    startIndex: number,
    stopIndex: number
  ): void {
    recognizer.notifyErrorListeners(
      `reportContextSensitivity d=${decisionDescription(recognizer, decision, ruleIndex)}, input='${recognizer.getTokenText(startIndex, stopIndex)}'`
    );
  }
}
