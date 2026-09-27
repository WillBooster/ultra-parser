# ultra-parser

A parser generator for ANTLR grammars whose parsers run in WebAssembly, derived from [ANTLR 5](https://github.com/antlr/antlr5).

> [!NOTE]
> This project is derived from [antlr/antlr5](https://github.com/antlr/antlr5) at commit [`354c8e9`](https://github.com/antlr/antlr5/tree/354c8e90747c768caf6f09cbc7df3513798af9ec).
> It is not affiliated with or endorsed by the ANTLR project.
> All credit for the original work goes to the ANTLR 5 authors listed below and to the [ANTLR 4 contributors](https://github.com/antlr/antlr4).

ANTLR 5 aims at a single runtime in WebAssembly instead of one runtime per language. ultra-parser pursues that goal for TypeScript: one prebuilt WebAssembly module parses for every grammar, and the generated code only holds the grammar and its actions.

## How it works

- The tool (`tool/`, Java) reads ANTLR grammars and generates TypeScript: a lexer and a parser class holding the grammar's serialized ATN, typed rule contexts, a listener, and a visitor.
- The runtime (`crates/ultra-parser-runtime`, Rust) interprets the ATN like ANTLR's generated parsers behave: adaptive LL(\*) prediction with SLL and LL modes, left-recursive rules, lexer modes and commands, semantic predicates, and ANTLR's default error recovery. `crates/ultra-parser-wasm` compiles it to WebAssembly with a small C ABI.
- The `ultra-parser` npm package (`packages/ultra-parser`) loads the module and builds the parse tree in JavaScript from what the runtime reports. Grammar code runs as TypeScript hooks that the runtime reaches through ATN states: actions, predicates, labels, rule arguments, and the contexts of labeled alternatives.

The runtime passes all of ANTLR's runtime tests (`runtime-testsuite/`) except those that print ANTLR's DFA, which the runtime does not build. It differs from ANTLR's TypeScript target in these ways:

- The parser reads all tokens before it parses, so the parser cannot switch lexer modes, and lexer actions run before parser actions. Lexer errors are still reported in ANTLR's order.
- Prediction does not cache DFA states; it caches only LL(1) sets. Grammars that ANTLR predicts quickly are also fast here, but the same decisions are recomputed each time they are made.
- `catch` clauses of rules are ignored, and the runtime has no pluggable error strategies (only the default and `BailErrorStrategy`), token stream rewriters, parse tree patterns, or XPath.

See [doc/typescript-target.md](doc/typescript-target.md) for the generated code and the runtime API.

## Usage

Generate TypeScript from a grammar with the tool (Java 21 or later; see "Development" for building it):

```sh
java -jar tool/target/antlr5-0.0.1-SNAPSHOT-complete.jar -visitor -o src/generated Expr.g4
```

Then parse with the `ultra-parser` package, which is not published to npm yet: after `bun run build`, add `packages/ultra-parser` of this repository to your project, e.g., with `bun add /path/to/ultra-parser/packages/ultra-parser`.

```ts
import { CharStream, CommonTokenStream } from 'ultra-parser';

import { ExprLexer } from './generated/ExprLexer.js';
import { ExprParser } from './generated/ExprParser.js';

const lexer = new ExprLexer(CharStream.fromString('1 + 2 * 3'));
const parser = new ExprParser(new CommonTokenStream(lexer));
const tree = parser.prog();
console.log(tree.toStringTree(parser));
```

Node.js, Bun, and Deno load the WebAssembly module on first use. Elsewhere, such as in browsers and Cloudflare Workers, call `init()` or `initSync()` with the module (`ultra-parser/ultra_parser.wasm`) before parsing.

`examples/arithmetic` evaluates arithmetic expressions with a generated parser.

## Development

Install the tools pinned in `mise.toml` and `rust-toolchain.toml` with `mise install`, then:

```sh
mvn -B install -DskipTests                                    # build the tool
bun install
bun run build                                                 # build the WebAssembly runtime
mvn -B test -pl runtime/Core,tool-testsuite,runtime-testsuite # test the tool, and the runtime with ANTLR's runtime tests
cargo test
bun run typecheck
bun test
```

After changing the tool or a grammar under `examples/*/grammar/`, run `bun run generate` and commit the regenerated files.

- `runtime-testsuite/` runs ANTLR's runtime test descriptors: it generates TypeScript for each grammar and runs it with Bun against the package in this repository.
- `runtime/Core` is ANTLR 5's Kotlin runtime, which the tool uses to build and serialize ATNs.
- `doc/` is ANTLR's documentation of grammars, inherited from upstream, with pages about ultra-parser's target.

## ANTLR 5 authors

* [Terence Parr](http://www.cs.usfca.edu/~parrt/), ANTLR project lead
* [Eric Vergnaud](https://github.com/ericvergnaud), ANTLR 5 project lead
* [Ivan Kochurkin](https://github.com/KvanTTT), major contributor
* [Ken Domino](https://github.com/kaby76), major contributor
* [Jim Idle](https://github.com/jimidle), major contributor
* [Federico Tomassetti](https://github.com/ftomassetti), major contributor

## License

BSD 3-Clause; see [LICENSE.txt](LICENSE.txt).
