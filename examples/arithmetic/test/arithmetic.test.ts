import { expect, test } from 'bun:test';
import { CharStream, CommonToken, CommonTokenStream, type ErrorListener, type ParserHooks, Token } from 'ultra-parser';

import { evaluate, parse } from '../src/index.js';
import { ArithmeticLexer } from '../src/generated/ArithmeticLexer.js';
import { ArithmeticParser } from '../src/generated/ArithmeticParser.js';

test('follows precedence and associativity', () => {
  expect(parse('1 + 2 * 3').treeText).toBe('(program (expr (expr 1) + (expr (expr 2) * (expr 3))) <EOF>)');
  expect(evaluate('1 + 2 * 3')).toBe(7);
  expect(evaluate('(1 + 2) * 3')).toBe(9);
  expect(evaluate('10 - 4 - 3')).toBe(3);
  expect(evaluate('2 ^ 3 ^ 2')).toBe(512);
  expect(evaluate('-2 ^ 2')).toBe(-4);
  expect(evaluate('1.5 * 4 / 2')).toBe(3);
});

test('recovers from syntax errors', () => {
  const missing = parse('1 + (2');
  expect(missing.treeText).toBe("(program (expr (expr 1) + (expr ( (expr 2) <missing ')'>)) <EOF>)");
  expect(missing.errors).toEqual(["line 1:6 missing ')' at '<EOF>'"]);
  expect(missing.tree.getText()).toBe("1+(2<missing ')'><EOF>");
  expect(parse('1 ) 2').tree.getText()).toBe('1)2');
  const incomplete = parse('1 +');
  expect(incomplete.treeText).toBe('(program (expr (expr 1) + expr) <EOF>)');
  expect(incomplete.errors).toEqual(["line 1:3 mismatched input '<EOF>' expecting {'-', '(', NUMBER}"]);
  expect(() => evaluate('1 # 2')).toThrow("line 1:2 token recognition error at: '#'");
});

test('handles deeply nested and long inputs', () => {
  const chain = `1${'+1'.repeat(49_999)}`;
  expect(parse(chain).errors).toEqual([]);
  expect(evaluate(chain)).toBe(50_000);
  const nested = `${'('.repeat(20_000)}1${')'.repeat(20_000)}`;
  expect(parse(nested).errors).toEqual([]);
  expect(evaluate(nested)).toBe(1);
  expect(parse(`${'-'.repeat(20_000)}(1 1`).errors).toEqual([
    "line 1:20003 mismatched input '1' expecting {'^', '-', '*', '/', '+', ')'}",
  ]);
}, 60_000);

test('parses from where the previous rule stopped', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 2 * 3 4'))));
  const exprs: string[] = [];
  while (parser.getCurrentToken().type !== Token.EOF) exprs.push(parser.expr().toStringTree(parser));
  expect(exprs).toEqual(['(expr 1)', '(expr (expr 2) * (expr 3))', '(expr 4)']);
});

test('continues a parse after grammar code parses again', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 + 2'))));
  let nested = '';
  let states: number[] = [];
  let expected = '';
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() !== '+' || nested) return;
      const state = parser.state;
      nested = parser.expr().toStringTree(parser);
      states = [state, parser.state];
      expected = parser.getExpectedTokens().toString(parser.vocabulary);
    },
  });
  expect(parser.program().toStringTree(parser)).toBe('(program (expr (expr 1) + (expr 2)) <EOF>)');
  expect(nested).toBe('(expr 2)');
  expect(states[1]).toBe(states[0]);
  expect(expected).toBe("{'-', '+'}");
});

test('reports lexer errors when the parser reaches them, as ANTLR does', () => {
  const run = (parseAll: (parser: ArithmeticParser, log: string[]) => void): string[] => {
    const log: string[] = [];
    const listener: ErrorListener = {
      syntaxError: (_recognizer, _symbol, line, column, message) => log.push(`${line}:${column} ${message}`),
    };
    const lexer = new ArithmeticLexer(CharStream.fromString('1 2 3 4 # 5'));
    lexer.removeErrorListeners();
    lexer.addErrorListener(listener);
    const parser = new ArithmeticParser(new CommonTokenStream(lexer));
    parser.removeErrorListeners();
    parser.addErrorListener(listener);
    parseAll(parser, log);
    return log;
  };
  const expected = ['(expr 1)', '(expr 2)', '(expr 3)', "1:8 token recognition error at: '#'", '(expr 4)', '(expr 5)'];
  expect(
    run((parser, log) => {
      while (parser.getCurrentToken().type !== Token.EOF) log.push(parser.expr().toStringTree(parser));
    })
  ).toEqual(expected);
  expect(
    run((parser, log) => {
      for (let i = 0; i < 5; i++) log.push(parser.expr().toStringTree(parser));
    })
  ).toEqual(expected);
});

test('returns all tokens from a stream that has not been read', () => {
  const tokens = new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 + 2'))).getTokens();
  expect(tokens.map((token) => token.text)).toEqual(['1', '+', '2', '<EOF>']);
});

test('reports all lexer errors when the whole text of a stream is read', () => {
  const log: string[] = [];
  const lexer = new ArithmeticLexer(CharStream.fromString('1 + 2 # 3'));
  lexer.removeErrorListeners();
  const listener: ErrorListener = {
    syntaxError: (_recognizer, _symbol, line, column, message) => log.push(`${line}:${column} ${message}`),
  };
  lexer.addErrorListener(listener);
  expect(new CommonTokenStream(lexer).getText()).toBe('1+23');
  expect(log).toEqual(["1:6 token recognition error at: '#'"]);
});

test('reports the lexer errors before the next token when reading hidden tokens', () => {
  const log: string[] = [];
  const lexer = new ArithmeticLexer(CharStream.fromString('1 # 2'));
  lexer.removeErrorListeners();
  const listener: ErrorListener = {
    syntaxError: (_recognizer, _symbol, line, column, message) => log.push(`${line}:${column} ${message}`),
  };
  lexer.addErrorListener(listener);
  expect(new CommonTokenStream(lexer).getHiddenTokensToRight(0)).toBeNull();
  expect(log).toEqual(["1:2 token recognition error at: '#'"]);
});

test('loads the tokens once for successive rules', () => {
  const stream = new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 2 3')));
  const parser = new ArithmeticParser(stream);
  parser.expr();
  let visits = 0;
  const every = stream.tokens.every.bind(stream.tokens);
  stream.tokens.every = ((...args: Parameters<typeof every>) => {
    visits++;
    return every(...args);
  }) as typeof every;
  parser.expr();
  parser.expr();
  expect(visits).toBe(0);
});

test('keeps the tokens of a parse when grammar code parses other tokens of the same input', () => {
  const input = CharStream.fromString('1 + 2');
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(input)));
  let nested = '';
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() !== '+' || nested) return;
      const tokens = [
        new CommonToken(ArithmeticLexer.NUMBER, 0, 0, 0, 1, 0, null, input, '9'),
        new CommonToken(Token.EOF, 0, 1, 0, 1, 1, null, input),
      ];
      const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
      const inner = new ArithmeticParser(new CommonTokenStream(source));
      nested = inner.expr().toStringTree(inner);
    },
  });
  expect(parser.program().toStringTree(parser)).toBe('(program (expr (expr 1) + (expr 2)) <EOF>)');
  expect(nested).toBe('(expr 9)');
});

test('bounds hidden tokens by the default channel on a stream of another channel', () => {
  const input = CharStream.fromString('a b');
  const tokens = [
    new CommonToken(1, Token.DEFAULT_CHANNEL, 0, 0, 1, 0, null, input),
    new CommonToken(2, Token.HIDDEN_CHANNEL, 1, 1, 1, 1, null, input),
    new CommonToken(3, Token.DEFAULT_CHANNEL, 2, 2, 1, 2, null, input),
    new CommonToken(Token.EOF, Token.DEFAULT_CHANNEL, 3, 2, 1, 3, null, input),
  ];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const stream = new CommonTokenStream(source, Token.HIDDEN_CHANNEL);
  stream.fill();
  expect(stream.getHiddenTokensToRight(1)).toBeNull();
  expect(stream.getHiddenTokensToLeft(1)).toBeNull();
  expect(stream.getHiddenTokensToRight(0)?.map((token) => token.text)).toEqual([' ']);
});

test('rejects the expected tokens of an invalid state without breaking the runtime', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1'))));
  parser.program();
  expect(parser.state).toBe(-1);
  expect(() => parser.getExpectedTokens()).toThrow(RangeError);
  expect(parse('1 + 2').errors).toEqual([]);
});

test('parses the tokens on the channel of the token stream', () => {
  const input = CharStream.fromString('1 2');
  const tokens = [
    new CommonToken(ArithmeticParser.NUMBER, Token.DEFAULT_CHANNEL, 0, 0, 1, 0, null, input),
    new CommonToken(ArithmeticParser.NUMBER, Token.HIDDEN_CHANNEL, 2, 2, 1, 2, null, input),
    new CommonToken(Token.EOF, Token.DEFAULT_CHANNEL, 3, 2, 1, 3, null, input),
  ];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const parser = new ArithmeticParser(new CommonTokenStream(source, Token.HIDDEN_CHANNEL));
  const errors: string[] = [];
  parser.removeErrorListeners();
  const listener: ErrorListener = { syntaxError: (_recognizer, _symbol, _line, _column, message) => errors.push(message) };
  parser.addErrorListener(listener);
  expect(parser.getCurrentToken().text).toBe('2');
  expect(parser.expr().toStringTree(parser)).toBe('(expr 2)');
  expect(errors).toEqual([]);
});

test('parses the current tokens after a token changed', () => {
  const stream = new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1')));
  const parser = new ArithmeticParser(stream);
  parser.removeErrorListeners();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(0);
  (stream.tokens[0] as Token).type = ArithmeticParser.T__4;
  parser.reset();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(1);
});

test('parses the current tokens after a token of another class changed', () => {
  const input = CharStream.fromString('1');
  const token = (type: number, text: string): Token => ({
    type,
    channel: Token.DEFAULT_CHANNEL,
    start: 0,
    stop: 0,
    line: 1,
    column: 0,
    tokenIndex: -1,
    text,
    inputStream: input,
    tokenSource: null,
    hasText: true,
    getText() {
      return this.text;
    },
  });
  const first = token(ArithmeticParser.NUMBER, '1');
  const tokens = [first, token(Token.EOF, '<EOF>')];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const parser = new ArithmeticParser(new CommonTokenStream(source));
  parser.removeErrorListeners();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(0);
  first.type = ArithmeticParser.T__4;
  parser.reset();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(1);
});

test('parses the current tokens after a token of a CommonToken subclass changed', () => {
  class Derived extends CommonToken {
    #value = 0;
    override get type(): number {
      return this.#value;
    }
    override set type(value: number) {
      this.#value = value;
    }
  }
  const input = CharStream.fromString('1');
  const first = new Derived(ArithmeticParser.NUMBER, 0, 0, 0, 1, 0, null, input, '1');
  first.type = ArithmeticParser.NUMBER;
  const eof = new Derived(Token.EOF, 0, 1, 0, 1, 1, null, input, '<EOF>');
  eof.type = Token.EOF;
  const tokens: Token[] = [first, eof];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const parser = new ArithmeticParser(new CommonTokenStream(source));
  parser.removeErrorListeners();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(0);
  first.type = ArithmeticParser.T__4;
  parser.reset();
  parser.program();
  expect(parser.numberOfSyntaxErrors).toBe(1);
});

test('keeps CommonToken fields behind the setters that parsers track', () => {
  const token = new CommonToken(ArithmeticParser.NUMBER, 0, 0, 0, 1, 0, null, null, '1');
  expect(() => Object.defineProperty(token, 'type', { value: ArithmeticParser.T__4 })).toThrow(TypeError);
});

test('reports the text of tokens of other classes', () => {
  class Custom extends CommonToken {
    override get text(): string {
      return this.type === Token.EOF ? '<EOF>' : 'custom';
    }
  }
  const input = CharStream.fromString('1');
  const tokens: Token[] = [
    new Custom(ArithmeticParser.T__4, 0, 0, 0, 1, 0, null, input),
    new Custom(Token.EOF, 0, 1, 0, 1, 1, null, input),
  ];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const parser = new ArithmeticParser(new CommonTokenStream(source));
  const errors: string[] = [];
  parser.removeErrorListeners();
  const listener: ErrorListener = { syntaxError: (_recognizer, _symbol, _line, _column, message) => errors.push(message) };
  parser.addErrorListener(listener);
  parser.expr();
  expect(errors[0]).toBe("mismatched input 'custom' expecting {'-', '(', NUMBER}");
});

test('returns the hidden tokens to the left in order', () => {
  const input = CharStream.fromString('1 \t2');
  const tokens = [
    new CommonToken(ArithmeticParser.NUMBER, Token.DEFAULT_CHANNEL, 0, 0, 1, 0, null, input),
    new CommonToken(2, Token.HIDDEN_CHANNEL, 1, 1, 1, 1, null, input),
    new CommonToken(3, Token.HIDDEN_CHANNEL, 2, 2, 1, 2, null, input),
    new CommonToken(ArithmeticParser.NUMBER, Token.DEFAULT_CHANNEL, 3, 3, 1, 3, null, input),
    new CommonToken(Token.EOF, Token.DEFAULT_CHANNEL, 4, 3, 1, 4, null, input),
  ];
  const source = { nextToken: () => tokens.shift() as Token, line: 1, column: 0, inputStream: input, sourceName: '' };
  const stream = new CommonTokenStream(source);
  stream.fill();
  expect(stream.getHiddenTokensToLeft(3)?.map((token) => token.text)).toEqual([' ', '\t']);
});

test('sends each exit event once when a listener throws at a left-recursive iteration', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1+2'))));
  let exits = 0;
  parser.addParseListener({
    exitEveryRule: () => {
      exits++;
      if (exits === 1) throw new Error('stop');
    },
  });
  expect(() => parser.expr()).toThrow('stop');
  expect(exits).toBe(1);
});

test('reports each deferred lexer error once and then lets it go', () => {
  const log: string[] = [];
  const lexer = new ArithmeticLexer(CharStream.fromString('1 # 2 # 3'));
  const stream = new CommonTokenStream(lexer);
  lexer.removeErrorListeners();
  const listener: ErrorListener = {
    syntaxError: (_recognizer, _symbol, line, column, message) => {
      log.push(`${line}:${column} ${message}`);
      // A report that reads the whole stream drains the queue while it is being drained.
      stream.getText();
    },
  };
  lexer.addErrorListener(listener);
  stream.getText();
  expect(log).toEqual(["1:2 token recognition error at: '#'", "1:6 token recognition error at: '#'"]);
  expect((stream as unknown as { deferredLexerErrors: unknown[] }).deferredLexerErrors).toEqual([]);
});

test('rejects a lexer mode that does not exist', () => {
  const lexer = new ArithmeticLexer(CharStream.fromString('1'));
  lexer.mode(5);
  expect(() => lexer.nextToken()).toThrow(RangeError);
  lexer.mode(0);
  expect(lexer.nextToken().text).toBe('1');
});

test('lets go of reported lexer errors when a report throws', () => {
  const lexer = new ArithmeticLexer(CharStream.fromString('1 # 2 #'));
  const stream = new CommonTokenStream(lexer);
  let calls = 0;
  lexer.removeErrorListeners();
  const listener: ErrorListener = {
    syntaxError: () => {
      calls++;
      if (calls === 2) throw new Error('stop');
    },
  };
  lexer.addErrorListener(listener);
  expect(() => stream.getText()).toThrow('stop');
  expect(calls).toBe(2);
  expect((stream as unknown as { deferredLexerErrors: unknown[] }).deferredLexerErrors).toEqual([]);
});

test('reports the lexer errors before a token when reading hidden tokens to its left', () => {
  const log: string[] = [];
  const lexer = new ArithmeticLexer(CharStream.fromString('1 # 2'));
  lexer.removeErrorListeners();
  const listener: ErrorListener = {
    syntaxError: (_recognizer, _symbol, line, column, message) => log.push(`${line}:${column} ${message}`),
  };
  lexer.addErrorListener(listener);
  const stream = new CommonTokenStream(lexer);
  stream.fill(true);
  expect(stream.getHiddenTokensToLeft(1)).toBeNull();
  expect(log).toEqual(["1:2 token recognition error at: '#'"]);
});

test('answers precpred with the precedence of the left-recursive rule being parsed', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('2 ^ 3'))));
  const answers: boolean[] = [];
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() === '3') answers.push(parser.precpred(parser.context, 5), parser.precpred(parser.context, 6));
    },
  });
  parser.program();
  expect(answers).toEqual([false, true]);
  expect(parser.precpred(null, 0)).toBe(true);
});

test('pops the precedence of a left-recursive rule before its exit event', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('2 ^ 3'))));
  const answers: boolean[] = [];
  parser.addParseListener({
    exitEveryRule: (ctx) => {
      if (ctx.getText() === '3') answers.push(parser.precpred(ctx, 5));
    },
  });
  parser.program();
  expect(answers).toEqual([true]);
});

test('leaves deeply nested rules quickly when grammar code throws', () => {
  const input = `${'('.repeat(20_000)}1${')'.repeat(20_000)}`;
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString(input))));
  let exits = 0;
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() === '1') throw new Error('stop');
    },
    exitEveryRule: () => {
      exits++;
    },
  });
  expect(() => parser.program()).toThrow('stop');
  expect(exits).toBe(20_002);
}, 60_000);

test('pops the precedence of left-recursive rules left because grammar code threw', () => {
  const seen: [string, boolean][] = [];
  class Parser extends ArithmeticParser {
    protected static override registerHooks(hooks: ParserHooks): void {
      super.registerHooks(hooks);
      hooks.onFinally(ArithmeticParser.RULE_expr, function (this: Parser, ctx) {
        seen.push([ctx.getText(), this.precpred(ctx, 5)]);
      });
    }
  }
  const parser = new Parser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('2 ^ 3'))));
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() === '3') throw new Error('stop');
    },
  });
  expect(() => parser.program()).toThrow('stop');
  // Like ANTLR's, a rule's finally runs before its precedence is popped.
  expect(seen).toEqual([
    ['3', false],
    ['2^', true],
  ]);
});

test('defers errors that lexer code reports until the parser reaches their token', () => {
  class Lexer extends ArithmeticLexer {
    override nextToken(): Token {
      const token = super.nextToken();
      if (token.text === '2') this.notifyErrorListeners('action error');
      return token;
    }
  }
  const log: string[] = [];
  const lexer = new Lexer(CharStream.fromString('1 2'));
  lexer.removeErrorListeners();
  const listener: ErrorListener = { syntaxError: (_recognizer, _symbol, _line, _column, message) => log.push(message) };
  lexer.addErrorListener(listener);
  const stream = new CommonTokenStream(lexer);
  expect(stream.LT(1)?.text).toBe('1');
  expect(log).toEqual([]);
  stream.consume();
  expect(log).toEqual(['action error']);
});
