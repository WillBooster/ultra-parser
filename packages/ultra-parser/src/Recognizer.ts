import { ConsoleErrorListener, type ErrorListener } from './errors.js';
import type { ParserRuleContext } from './tree.js';
import { Vocabulary } from './Vocabulary.js';
import { WasmGrammar } from './wasm.js';

const grammars = new WeakMap<object, WasmGrammar>();
const vocabularies = new WeakMap<object, Vocabulary>();

/** What lexers and parsers share, like ANTLR's `Recognizer`. Generated recognizers provide the grammar. */
export abstract class Recognizer {
  #listeners: ErrorListener[] = [ConsoleErrorListener.INSTANCE];
  /** The ATN state the recognizer is in, where grammar code can see it. */
  state = -1;

  abstract get grammarFileName(): string;
  abstract get ruleNames(): readonly string[];
  abstract get literalNames(): readonly (string | null)[];
  abstract get symbolicNames(): readonly (string | null)[];
  abstract get serializedATN(): readonly number[];

  get vocabulary(): Vocabulary {
    let vocabulary = vocabularies.get(this.constructor);
    if (!vocabulary) {
      vocabulary = new Vocabulary(this.literalNames, this.symbolicNames);
      vocabularies.set(this.constructor, vocabulary);
    }
    return vocabulary;
  }

  /** The grammar in the runtime, loaded once per recognizer class. */
  protected get grammar(): WasmGrammar {
    let grammar = grammars.get(this.constructor);
    if (!grammar) {
      grammar = WasmGrammar.load(this.serializedATN, this.ruleNames, this.literalNames, this.symbolicNames);
      grammars.set(this.constructor, grammar);
    }
    return grammar;
  }

  get errorListeners(): readonly ErrorListener[] {
    return this.#listeners;
  }

  addErrorListener(listener: ErrorListener): void {
    this.#listeners.push(listener);
  }

  removeErrorListener(listener: ErrorListener): void {
    this.#listeners = this.#listeners.filter((l) => l !== listener);
  }

  removeErrorListeners(): void {
    this.#listeners = [];
  }

  /** Reports a syntax error to the error listeners. */
  abstract notifyErrorListeners(message: string): void;

  /** The text of the tokens from `start` to `stop`; parsers override it for diagnostics. */
  getTokenText(_start: number, _stop: number): string {
    return '';
  }

  /** Evaluates a predicate of the grammar; generated recognizers override it. */
  sempred(_localctx: ParserRuleContext | null, _ruleIndex: number, _predIndex: number): boolean {
    return true;
  }

  /** Runs an action of a lexer grammar; generated lexers override it. */
  action(_localctx: ParserRuleContext | null, _ruleIndex: number, _actionIndex: number): void {}
}
