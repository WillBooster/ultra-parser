import fs from 'node:fs';
import path from 'node:path';

import { beforeAll, expect, test } from 'bun:test';

import { evaluate, initSync, parse } from '../dist/arithmetic.js';

beforeAll(() => {
  const wasm = fs.readFileSync(path.join(import.meta.dirname, '..', 'dist', 'arithmetic_bg.wasm'));
  initSync({ module: new WebAssembly.Module(wasm) });
});

test('parses and evaluates expressions in WebAssembly', () => {
  const result = parse('1 + 2 * 3');
  expect(result.tree).toBe('(program (expr (expr 1) + (expr (expr 2) * (expr 3))) <EOF>)');
  expect(result.errors).toEqual([]);
  expect(evaluate('2 ^ 3 ^ 2 - (4 - 1)')).toBe(509);
});

test('recovers from syntax errors', () => {
  const result = parse('1 + (2');
  expect(result.tree).toBe("(program (expr (expr 1) + (expr ( (expr 2) <missing ')'>)) <EOF>)");
  expect(result.errors).toEqual(["line 1:6 missing ')' at '<EOF>'"]);
  expect(() => evaluate('1 # 2')).toThrow("line 1:2 token recognition error at: '#'");
});
