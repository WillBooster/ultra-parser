//! Lexes and parses with the grammar of `examples/arithmetic`, reading the ATNs and names from the
//! TypeScript that the tool generated for it.

use ultra_parser_runtime::{Grammar, NodeId, ParserHost, PredictionMode, SyntaxError, Vocabulary};

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

fn parse(source: &str) -> (String, Vec<String>) {
    let (lexer, parser) = grammars();
    let (tokens, lexer_errors) = lexer.tokenize(source);
    let mut errors = lexer_errors;
    let tree = parser.parse(&tokens, 0, 0, PredictionMode::Ll, &mut errors);
    (
        tree.to_string_tree(&tokens, parser.rule_names()),
        errors.iter().map(ToString::to_string).collect(),
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
        recursive: bool,
    ) {
        self.events.push(format!(
            "enter {ctx} rule {rule_index} recursive {recursive}"
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
    parser.parse(&tokens, 0, 0, PredictionMode::Ll, &mut recorder);
    assert_eq!(
        recorder.events,
        [
            "enter 0 rule 0 recursive false",
            "enter 1 rule 1 recursive true",
            "token 0 in 1 at Some(11) error false",
            "push 2 over 1",
            "token 1 in 2 at Some(21) error false",
            "enter 3 rule 1 recursive true",
            "token 2 in 3 at Some(11) error false",
            "unroll 3 into Some(2)",
            "unroll 2 into Some(0)",
            "token 3 in 0 at Some(4) error false",
            "exit 0",
        ]
    );
}
