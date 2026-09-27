# Parse Tree Listeners

*Partially taken from publically visible [excerpt from ANTLR 4 book](http://media.pragprog.com/titles/tpantlr2/picture.pdf)*

By default, generated parsers build a data structure called a parse tree or syntax tree that records how the parser recognized the structure of the input sentence and component phrases.

<img src=images/process.png>

The interior nodes of the parse tree are phrase names that group and identify their children. The root node is the most abstract phrase name, in this case `stat` (short for statement). The leaves of a parse tree are always the input tokens. Parse trees sit between a language recognizer and an interpreter or translator implementation. They are extremely effective data structures because they contain all of the input and complete knowledge of how the parser grouped the symbols into phrases. Better yet, they are easy to understand and the parser generates them automatically (unless you turn them off with `parser.buildParseTrees = false`).

Because we specify phrase structure with a set of rules, parse tree subtree roots correspond to grammar rule names. The `ultra-parser` package has a `ParseTreeWalker` that knows how to walk these parse trees and trigger events in listener objects that you create. The tool generates a listener class for you also, unless you turn that off with `-no-listener`. You can also have it generate visitors with `-visitor`. For example, from a `Java.g4` grammar, the tool generates:

```ts
export class JavaListener implements ParseTreeListener {
  enterClassDeclaration?(ctx: ClassDeclarationContext): void;
  exitClassDeclaration?(ctx: ClassDeclarationContext): void;
  enterMethodDeclaration?(ctx: MethodDeclarationContext): void;
  ...
}
```

where there is an optional enter and exit method for each rule in the parser grammar. You build your listener by subclassing `JavaListener`, or by writing an object of its type, with the methods of interest.

Assuming you've created a listener object called `extractor`, here is how to call the parser and walk the parse tree:

```ts
import { CharStream, CommonTokenStream, ParseTreeWalker } from 'ultra-parser';

import { JavaLexer } from './generated/JavaLexer.js';
import { JavaParser } from './generated/JavaParser.js';

const lexer = new JavaLexer(CharStream.fromString(input));
const parser = new JavaParser(new CommonTokenStream(lexer));
const tree = parser.compilationUnit(); // parse a compilationUnit

ParseTreeWalker.DEFAULT.walk(extractor, tree); // walk the tree with the listener
```

Listeners and visitors are great because they keep application-specific code out of grammars, making grammars easier to read and preventing them from getting entangled with a particular application.

See the book for more information on listeners and to learn how to use visitors. (The biggest difference between the listener and visitor mechanisms is that listener methods are called independently by a walker object, whereas visitor methods must walk their children with explicit visit calls.  Forgetting to invoke visitor methods on a node’s children, means those subtrees don’t get visited.)

## Listening during the parse

We can also use listeners to execute code during the parse instead of waiting for a tree walker walks the resulting parse tree. Let's say we have the following simple expression grammar.

```
grammar CalcNoLR;

s : expr EOF ;

expr:	add ((MUL | DIV) add)* ;

add :   atom ((ADD | SUB) atom)* ;

atom : INT ;

INT : [0-9]+;
MUL : '*';
DIV : '/';
ADD : '+';
SUB : '-';
WS : [ \t]+ -> channel(HIDDEN);
```

We can create a listener that executes during the parse by subclassing the listener as before:

```ts
class CountListener extends CalcNoLRListener {
  nums = 0;
  execExitS = false;

  override exitS(_ctx: SContext): void {
    this.execExitS = true;
  }

  override exitAtom(_ctx: AtomContext): void {
    this.nums++;
  }
}
```

And then passing it to `addParseListener()`:

```ts
const lexer = new CalcNoLRLexer(CharStream.fromString('2 + 8 / 2'));
const parser = new CalcNoLRParser(new CommonTokenStream(lexer));
const counter = new CountListener();
parser.addParseListener(counter);

const tree = parser.s();
console.log(tree.toStringTree(parser)); // (s (expr (add (atom 2) + (atom 8)) / (add (atom 2))) <EOF>)
console.log(counter.nums); // 3
console.log(counter.execExitS); // true
```

If a listener method throws, the parse stops matching input and leaves the open rules, innermost first, as the `finally` blocks of ANTLR's generated rule methods do: each runs its `finally` action and sends its exit events to the listeners. Then the exception propagates out of the rule function that started the parse, unless a `finally` action or listener throws another one, which propagates instead:

```ts
class ErrorListener extends CalcNoLRListener {
  override exitAtom(_ctx: AtomContext): void {
    throw new Error('bail out');
  }
}

parser.addParseListener(new ErrorListener());
parser.s(); // throws Error: bail out
```
