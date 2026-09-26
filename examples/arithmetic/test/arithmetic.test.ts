import { expect, test } from 'bun:test';
import { CharStream, CommonTokenStream, Token } from 'ultra-parser';

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
});

test('parses from where the previous rule stopped', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 2 * 3 4'))));
  const exprs: string[] = [];
  while (parser.getCurrentToken().type !== Token.EOF) exprs.push(parser.expr().toStringTree(parser));
  expect(exprs).toEqual(['(expr 1)', '(expr (expr 2) * (expr 3))', '(expr 4)']);
});

test('continues a parse after grammar code parses again', () => {
  const parser = new ArithmeticParser(new CommonTokenStream(new ArithmeticLexer(CharStream.fromString('1 + 2'))));
  let nested = '';
  parser.addParseListener({
    visitTerminal: (node) => {
      if (node.getText() === '+' && !nested) nested = parser.expr().toStringTree(parser);
    },
  });
  expect(parser.program().toStringTree(parser)).toBe('(program (expr (expr 1) + (expr 2)) <EOF>)');
  expect(nested).toBe('(expr 2)');
});
