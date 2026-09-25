# Runtime Libraries and Code Generation Targets

The tool, written in Java, generates code for every target through command line options. The available targets are the following:

* [Java](java-target.md). The [ANTLR v4 book](http://pragprog.com/book/tpantlr2/the-definitive-antlr-4-reference) has a decent summary of the runtime library.  We have added a useful XPath feature since the book was printed that lets you select bits of parse trees. See [Runtime API](http://www.antlr.org/api/Java/index.html) and [Getting Started with ANTLR v4](getting-started.md)
* Rust (`-Dlanguage=Rust`), which builds for WebAssembly. See the [README](../README.md).
