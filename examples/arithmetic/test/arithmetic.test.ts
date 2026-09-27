import { expect, test } from 'bun:test';
import { CharStream, CommonToken, CommonTokenStream, type ErrorListener, Token } from 'ultra-parser';

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
