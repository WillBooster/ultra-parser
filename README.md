# ultra-parser

An experimental parser generator for WebAssembly, derived from [ANTLR 5](https://github.com/antlr/antlr5).

> [!NOTE]
> This project is derived from [antlr/antlr5](https://github.com/antlr/antlr5) at commit [`354c8e9`](https://github.com/antlr/antlr5/tree/354c8e90747c768caf6f09cbc7df3513798af9ec).
> It is not affiliated with or endorsed by the ANTLR project.
> All credit for the original work goes to the ANTLR 5 authors listed below and to the [ANTLR 4 contributors](https://github.com/antlr/antlr4).

## How it works

- The tool (`tool/`, Java) reads ANTLR grammars. Its `Rust` target (`-Dlanguage=Rust`) emits one Rust module per recognizer that holds the grammar's serialized ATN and names, not parsing code.
- The runtime (`crates/ultra-parser-runtime`) interprets that ATN like ANTLR's `LexerInterpreter` and `ParserInterpreter`: adaptive LL(\*) prediction, left-recursive rules, lexer modes and actions, and ANTLR's default error recovery.
- Grammar actions and semantic predicates are not run. Predicates always succeed.
- The runtime has no dependencies and builds for `wasm32-unknown-unknown`. `examples/arithmetic` shows a grammar compiled to WebAssembly and called from JavaScript.

## Development

Install the tools pinned in `mise.toml` and `rust-toolchain.toml` with `mise install`, then:

```sh
mvn -B install -DskipTests                                     # build the tool
mvn -B test -pl runtime/Core,tool-testsuite,runtime-testsuite  # the tests of antlr5-maven-plugin already failed upstream
cargo test                                                     # includes the conformance tests
bun install
bun run test                                                   # build the example for WebAssembly and test it from JavaScript
```

After changing the tool, a grammar under `examples/` or `conformance/grammars/`, or an input under `conformance/grammars/`, run `bun run generate` and commit the regenerated files.

- `conformance/` checks the runtime against ANTLR's own interpreters. `GenerateFixtures.java` records how they lex and parse each grammar's inputs into `conformance/fixtures/`, and `crates/ultra-parser-runtime/tests/conformance.rs` requires identical tokens, trees, and error messages. Add inputs there when changing the runtime.
- `doc/` is ANTLR's documentation, inherited from upstream. Read it for grammar syntax; its pages about other targets do not apply.

## ANTLR 5 authors

* [Terence Parr](http://www.cs.usfca.edu/~parrt/), ANTLR project lead
* [Eric Vergnaud](https://github.com/ericvergnaud), ANTLR 5 project lead
* [Ivan Kochurkin](https://github.com/KvanTTT), major contributor
* [Ken Domino](https://github.com/kaby76), major contributor
* [Jim Idle](https://github.com/jimidle), major contributor
* [Federico Tomassetti](https://github.com/ftomassetti), major contributor

## License

BSD 3-Clause; see [LICENSE.txt](LICENSE.txt).
