//! Parses and evaluates arithmetic expressions with a grammar interpreted by the ultra-parser
//! runtime, exported to JavaScript through wasm-bindgen.

use std::sync::OnceLock;

use ultra_parser_runtime::{Child, LexerInterpreter, NodeId, ParseTree, ParserInterpreter};
use wasm_bindgen::prelude::*;

#[rustfmt::skip]
mod generated {
    pub mod arithmetic_lexer;
    pub mod arithmetic_parser;
}

use generated::{arithmetic_lexer, arithmetic_parser};

fn interpreters() -> &'static (LexerInterpreter, ParserInterpreter) {
    static INTERPRETERS: OnceLock<(LexerInterpreter, ParserInterpreter)> = OnceLock::new();
    INTERPRETERS.get_or_init(|| {
        (
            LexerInterpreter::new(&arithmetic_lexer::GRAMMAR)
                .expect("the generated lexer ATN is valid"),
            ParserInterpreter::new(&arithmetic_parser::GRAMMAR)
                .expect("the generated parser ATN is valid"),
        )
    })
}

#[wasm_bindgen(getter_with_clone)]
pub struct ParseResult {
    /// The parse tree as an S-expression, e.g., `(program (expr 1) <EOF>)`.
    pub tree: String,
    /// Syntax errors formatted like `line 1:4 missing ')' at '<EOF>'`.
    pub errors: Vec<String>,
}

fn parse_tree(source: &str) -> (ParseTree, Vec<String>) {
    let (lexer, parser) = interpreters();
    let (tokens, mut errors) = lexer.tokenize(source);
    let output = parser.parse(&tokens, arithmetic_parser::rule::PROGRAM);
    errors.extend(output.errors);
    (
        output.tree,
        errors.iter().map(ToString::to_string).collect(),
    )
}

/// Parses `source`; the tree is available even when there are syntax errors.
#[wasm_bindgen]
pub fn parse(source: &str) -> ParseResult {
    let (tree, errors) = parse_tree(source);
    ParseResult {
        tree: tree.to_string_tree(),
        errors,
    }
}

/// Evaluates `source`; throws the syntax errors when there are any.
#[wasm_bindgen]
pub fn evaluate(source: &str) -> Result<f64, String> {
    let (tree, errors) = parse_tree(source);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    let Some(Child::Rule(expr)) = tree.nodes[tree.root].children.first() else {
        unreachable!("a valid program starts with an expression");
    };
    Ok(evaluate_expr(&tree, *expr))
}

/// Evaluates the expression node `root` bottom-up with an explicit stack, since left-recursive
/// rules nest one node per operator and recursion would overflow the WebAssembly stack.
fn evaluate_expr(tree: &ParseTree, root: NodeId) -> f64 {
    let text = |child: &Child| match *child {
        Child::Token(i) => tree.tokens[i].text.as_str(),
        _ => "",
    };
    let mut values: Vec<f64> = Vec::new();
    // Each entry is a node and whether its operand subexpressions are already on `values`.
    let mut stack = vec![(root, false)];
    while let Some((id, operands_evaluated)) = stack.pop() {
        let children = tree.nodes[id].children.as_slice();
        if !operands_evaluated {
            stack.push((id, true));
            for child in children.iter().rev() {
                if let Child::Rule(operand) = *child {
                    stack.push((operand, false));
                }
            }
            continue;
        }
        let value = match children {
            [number] => text(number).parse().expect("NUMBER tokens are numbers"),
            [minus, _] if text(minus) == "-" => -values.pop().expect("operand"),
            [open, _, _] if text(open) == "(" => values.pop().expect("operand"),
            [_, op, _] => {
                let right = values.pop().expect("right operand");
                let left = values.pop().expect("left operand");
                match text(op) {
                    "^" => left.powf(right),
                    "*" => left * right,
                    "/" => left / right,
                    "+" => left + right,
                    "-" => left - right,
                    op => unreachable!("unknown operator {op}"),
                }
            }
            _ => unreachable!("unexpected expression shape"),
        };
        values.push(value);
    }
    values.pop().expect("the root value")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_precedence_and_associativity() {
        assert_eq!(evaluate("1 + 2 * 3").unwrap(), 7.0);
        assert_eq!(evaluate("(1 + 2) * 3").unwrap(), 9.0);
        assert_eq!(evaluate("10 - 4 - 3").unwrap(), 3.0);
        assert_eq!(evaluate("2 ^ 3 ^ 2").unwrap(), 512.0);
        assert_eq!(evaluate("-2 ^ 2").unwrap(), -4.0);
        assert_eq!(evaluate("1.5 * 4 / 2").unwrap(), 3.0);
    }

    #[test]
    fn reports_syntax_errors() {
        assert_eq!(
            evaluate("(1 + 2").unwrap_err(),
            "line 1:6 missing ')' at '<EOF>'"
        );
        let result = parse("1 +");
        assert_eq!(
            result.tree,
            "(program (expr (expr 1) + (expr <EOF>)) <EOF>)"
        );
        assert_eq!(
            result.errors,
            ["line 1:3 mismatched input '<EOF>' expecting {'-', '(', NUMBER}"]
        );
    }
}
