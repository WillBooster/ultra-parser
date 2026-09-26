# TypeScript Target

ultra-parser's tool generates TypeScript for the `ultra-parser` package, whose runtime interprets the grammar in WebAssembly. It is the tool's only target and its default, so `-Dlanguage` is not needed.

## Generated files

For a combined grammar `T.g4`, the tool generates:

- `TLexer.ts` with class `TLexer`, which extends `Lexer`.
- `TParser.ts` with class `TParser`, which extends `Parser`, and a context class per rule and per labeled alternative, e.g., `ExprContext` and `AddContext`.
- `TListener.ts` with class `TListener`, whose `enter*` and `exit*` methods are optional. Pass `-no-listener` to skip it.
- `TVisitor.ts` with class `TVisitor<Result>`, which extends `ParseTreeVisitor<Result>` and whose `visit*` methods are optional, with `-visitor`.

The recognizers hold the serialized ATN and the names of tokens, rules, channels, and modes as static members, e.g., `TParser.ID` and `TParser.RULE_expr`. Parsers have a method per rule that parses the whole input from that rule and returns its context:

```ts
const parser = new TParser(new CommonTokenStream(new TLexer(CharStream.fromString(input))));
const tree = parser.expr();
```

Context classes have getters for the elements of their rule, as in ANTLR's TypeScript target: `ID()` for a token that appears once, `ID_list()` and `ID(i)` for a token that appears several times, and likewise for rules. Labels are fields with an underscore, e.g., `_op` for `op=('+'|'-')`, and arguments, return values, and locals are fields with their names.

Names that the runtime uses get an underscore appended: rule methods named like members of `Parser`, such as `state` and `context`, and getters, arguments, and return values named like members of `ParserRuleContext`, such as `start` and `parent`.

## Grammar code

Actions, predicates, and the `@init`, `@after`, and `@finally` actions of rules are TypeScript. They run with the parser (or lexer) as `this`:

```antlr
grammar T;

@parser::members {
count = 0;
}

s : (ID {this.count++;})+ {console.log(`${this.count} identifiers`);} ;
ID : [a-z]+ ;
WS : [ \t\r\n]+ -> skip ;
```

Unlike ANTLR's generated code, parser actions do not run inside rule methods; they are hooks that the runtime calls when it passes their place in the grammar. They see the rule context as `localctx`, which `$ctx`, `$x`, `$x.text`, and so on refer to, and the rule's arguments as variables. `$text`, `this._input`, `this._ctx`, `this.state`, `getExpectedTokens()`, and `getRuleInvocationStack()` reflect the position of the hook.

The parser reads all tokens before it parses, so lexer actions run before parser actions, and the parser cannot switch lexer modes. `catch` clauses of rules are ignored.

## Runtime API

The `ultra-parser` package exports classes like ANTLR's: `CharStream`, `CommonToken`, `CommonTokenStream`, `Lexer`, `Parser`, `ParserRuleContext`, `TerminalNode`, `ErrorNode`, `ParseTreeWalker`, `ParseTreeVisitor`, `ConsoleErrorListener`, `DiagnosticErrorListener`, and `BailErrorStrategy`.

- Offsets and columns count Unicode code points, as in ANTLR's Java runtime.
- `Parser.predictionMode` is `PredictionMode.SLL`, `LL` (the default), or `LL_EXACT_AMBIG_DETECTION`, and can change while parsing.
- `Parser.buildParseTrees = false` skips building the tree; `addParseListener()` receives events as the tree is built.
- `ctx.toStringTree(parser)` formats a tree like ANTLR; trees of any depth can be formatted and walked, since the runtime and these functions do not recurse.

Node.js, Bun, and Deno load the WebAssembly module on first use. Elsewhere, call `await init()`, which fetches `ultra_parser.wasm` next to the package, or `initSync(module)` with a compiled module, e.g., one that Cloudflare Workers import.
