//! Lexes and parses with the grammar of `examples/arithmetic`, reading the ATNs and names from the
//! TypeScript that the tool generated for it.

use std::collections::HashMap;

use ultra_parser_runtime::{
    DEFAULT_CHANNEL, Grammar, NodeId, ParserHost, PredictionMode, SyntaxError, Token, Tokens,
    Vocabulary,
};

/// Reads the static array `name` of the recognizer that the tool generated for the example.
fn generated_array(recognizer: &str, name: &str) -> Vec<String> {
    let path = format!(
        "{}/../../examples/arithmetic/src/generated/{recognizer}.ts",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let start = source
        .find(&format!("static readonly {name}"))
        .unwrap_or_else(|| panic!("{path} has no {name}"));
    let open = start + source[start..].find("= [").expect("array") + 3;
    let close = open + source[open..].find("];").expect("end of array");
    source[open..close]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn atn(recognizer: &str) -> Vec<i32> {
    generated_array(recognizer, "_serializedATN")
        .iter()
        .map(|n| n.parse().expect("an integer"))
        .collect()
}

fn names(recognizer: &str, name: &str) -> Vec<Option<String>> {
    generated_array(recognizer, name)
        .into_iter()
        .map(|item| (item != "null").then(|| item.trim_matches('"').to_string()))
        .collect()
}

fn grammars() -> (Grammar, Grammar) {
    let rule_names = |recognizer| {
        names(recognizer, "ruleNames")
            .into_iter()
            .flatten()
            .collect()
    };
    let vocabulary = Vocabulary {
        literal_names: names("ArithmeticParser", "literalNames"),
        symbolic_names: names("ArithmeticParser", "symbolicNames"),
    };
    let lexer = Grammar::new(
        &atn("ArithmeticLexer"),
        rule_names("ArithmeticLexer"),
        vocabulary.clone(),
    )
    .unwrap();
    let parser = Grammar::new(
        &atn("ArithmeticParser"),
        rule_names("ArithmeticParser"),
        vocabulary,
    )
    .unwrap();
    (lexer, parser)
}

/// Builds the parse tree from the events of the parser, as `ultra-parser`'s `Parser` does, and
/// collects the syntax errors.
struct TreeBuilder<'a> {
    tokens: &'a Tokens,
    rules: HashMap<NodeId, (usize, Vec<Node>)>,
    errors: Vec<String>,
}

enum Node {
    Rule(NodeId),
    Token(String),
}

impl TreeBuilder<'_> {
    fn add(&mut self, ctx: NodeId, node: Node) {
        self.rules
            .get_mut(&ctx)
            .expect("a rule context")
            .1
            .push(node);
    }

    fn to_string_tree(&self, ctx: NodeId, rule_names: &[String]) -> String {
        let (rule_index, children) = &self.rules[&ctx];
        let name = &rule_names[*rule_index];
        if children.is_empty() {
            return name.clone();
        }
        let children: Vec<String> = children
            .iter()
            .map(|child| match child {
                Node::Rule(child) => self.to_string_tree(*child, rule_names),
                Node::Token(text) => text.clone(),
            })
            .collect();
        format!("({name} {})", children.join(" "))
    }
}

impl ParserHost for TreeBuilder<'_> {
    fn enter_rule(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        _invoking_state: Option<usize>,
        rule_index: usize,
        _state: usize,
        _start_token: usize,
        precedence: Option<i32>,
    ) {
        self.rules.insert(ctx, (rule_index, Vec::new()));
        if let (Some(parent), None) = (parent, precedence) {
            self.add(parent, Node::Rule(ctx));
        }
    }

    fn push_recursion(
        &mut self,
        ctx: NodeId,
        previous: NodeId,
        _state: usize,
        _previous_stop: Option<usize>,
    ) {
        let rule_index = self.rules[&previous].0;
        self.rules
            .insert(ctx, (rule_index, vec![Node::Rule(previous)]));
    }

    fn unroll(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        _stop_token: Option<usize>,
        _error: bool,
    ) {
        if let Some(parent) = parent {
            self.add(parent, Node::Rule(ctx));
        }
    }

    fn token(&mut self, ctx: NodeId, _state: Option<usize>, token_index: usize, _error: bool) {
        let text = self.tokens.text(&self.tokens.tokens[token_index]);
        self.add(ctx, Node::Token(text));
    }

    fn conjure(&mut self, ctx: NodeId, _state: usize, token: &Token, add_to_tree: bool) {
        if add_to_tree {
            let text = self.tokens.text(token);
            self.add(ctx, Node::Token(text));
        }
    }

    fn syntax_error(&mut self, error: SyntaxError) {
        self.errors.push(error.to_string());
    }
}

fn parse(source: &str) -> (String, Vec<String>) {
    let (lexer, parser) = grammars();
    let (tokens, lexer_errors) = lexer.tokenize(source);
    let mut builder = TreeBuilder {
        tokens: &tokens,
        rules: HashMap::new(),
        errors: lexer_errors.iter().map(ToString::to_string).collect(),
    };
    let root = parser.parse(
        &tokens,
        0,
        DEFAULT_CHANNEL,
        0,
        PredictionMode::Ll,
        &mut builder,
    );
    (
        builder.to_string_tree(root, parser.rule_names()),
        builder.errors,
    )
}

#[test]
fn parses_by_precedence_and_associativity() {
    assert_eq!(
        parse("1 + 2 * 3").0,
        "(program (expr (expr 1) + (expr (expr 2) * (expr 3))) <EOF>)"
    );
    assert_eq!(
        parse("2 ^ 3 ^ 2").0,
        "(program (expr (expr 2) ^ (expr (expr 3) ^ (expr 2))) <EOF>)"
    );
}

#[test]
fn recovers_from_syntax_errors() {
    assert_eq!(
        parse("1 + (2"),
        (
            "(program (expr (expr 1) + (expr ( (expr 2) <missing ')'>)) <EOF>)".to_string(),
            vec!["line 1:6 missing ')' at '<EOF>'".to_string()]
        )
    );
    assert_eq!(
        parse("1 # 2").1,
        [
            "line 1:2 token recognition error at: '#'",
            "line 1:4 extraneous input '2' expecting <EOF>"
        ]
    );
}

/// Records the events that a host receives.
#[derive(Default)]
struct Recorder {
    events: Vec<String>,
}

impl ParserHost for Recorder {
    fn enter_rule(
        &mut self,
        ctx: NodeId,
        _parent: Option<NodeId>,
        _invoking_state: Option<usize>,
        rule_index: usize,
        _state: usize,
        _start_token: usize,
        precedence: Option<i32>,
    ) {
        self.events.push(format!(
            "enter {ctx} rule {rule_index} precedence {precedence:?}"
        ));
    }

    fn push_recursion(
        &mut self,
        ctx: NodeId,
        previous: NodeId,
        _state: usize,
        _previous_stop: Option<usize>,
    ) {
        self.events.push(format!("push {ctx} over {previous}"));
    }

    fn token(&mut self, ctx: NodeId, state: Option<usize>, token_index: usize, error: bool) {
        self.events.push(format!(
            "token {token_index} in {ctx} at {state:?} error {error}"
        ));
    }

    fn unroll(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        _stop_token: Option<usize>,
        _error: bool,
    ) {
        self.events.push(format!("unroll {ctx} into {parent:?}"));
    }

    fn exit_rule(&mut self, ctx: NodeId, _stop_token: Option<usize>, _error: bool) {
        self.events.push(format!("exit {ctx}"));
    }

    fn syntax_error(&mut self, error: SyntaxError) {
        self.events.push(error.to_string());
    }
}

#[test]
fn reports_the_tree_to_the_host() {
    let (lexer, parser) = grammars();
    let (tokens, _) = lexer.tokenize("1+2");
    let mut recorder = Recorder::default();
    parser.parse(
        &tokens,
        0,
        DEFAULT_CHANNEL,
        0,
        PredictionMode::Ll,
        &mut recorder,
    );
    assert_eq!(
        recorder.events,
        [
            "enter 0 rule 0 precedence None",
            "enter 1 rule 1 precedence Some(0)",
            "token 0 in 1 at Some(11) error false",
            "push 2 over 1",
            "token 1 in 2 at Some(21) error false",
            "enter 3 rule 1 precedence Some(4)",
            "token 2 in 3 at Some(11) error false",
            "unroll 3 into Some(2)",
            "unroll 2 into Some(0)",
            "token 3 in 0 at Some(4) error false",
            "exit 0",
        ]
    );
}
