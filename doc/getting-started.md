# Getting Started with ultra-parser

ultra-parser generates TypeScript parsers from ANTLR 4 grammars, and its `ultra-parser` package parses with them in WebAssembly. The grammar syntax is ANTLR's; the other pages of this documentation describe it.

## Building the tool

The tool is a Java program. With Java 21 and Maven (both pinned in `mise.toml`), build it from the root of the repository:

```bash
mvn -B install -DskipTests
```

This creates `tool/target/antlr5-0.0.1-SNAPSHOT-complete.jar`. Build the WebAssembly module of the runtime with Rust and Bun:

```bash
bun install
bun run build
```

## A first example

Save this grammar as `Expr.g4`:

```
grammar Expr;
prog:	expr EOF ;
expr:	expr ('*'|'/') expr
    |	expr ('+'|'-') expr
    |	INT
    |	'(' expr ')'
    ;
NEWLINE : [\r\n]+ -> skip;
INT     : [0-9]+ ;
```

Generate the lexer, the parser, a listener, and a visitor:

```bash
java -jar tool/target/antlr5-0.0.1-SNAPSHOT-complete.jar -visitor -o src/generated Expr.g4
```

The `ultra-parser` package is not published to npm yet. Build it as above and add it to your project from this repository, e.g., with `bun add /path/to/ultra-parser/packages/ultra-parser` (Node.js loads its `dist/` output, which `bun run build` creates). Then parse an expression and print its tree:

```ts
import { CharStream, CommonTokenStream } from 'ultra-parser';

import { ExprLexer } from './generated/ExprLexer.js';
import { ExprParser } from './generated/ExprParser.js';

const lexer = new ExprLexer(CharStream.fromString('10+20*30'));
const parser = new ExprParser(new CommonTokenStream(lexer));
console.log(parser.prog().toStringTree(parser));
// (prog (expr (expr 10) + (expr (expr 20) * (expr 30))) <EOF>)
```

See [TypeScript Target](typescript-target.md) for the generated code and the runtime API, and `examples/arithmetic` for a complete example.
