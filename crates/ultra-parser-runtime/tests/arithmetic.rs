//! Lexes and parses with the grammar of `examples/arithmetic`, whose ATNs the tool generated.

use ultra_parser_runtime::{Grammar, NodeId, ParserHost, PredictionMode, SyntaxError, Vocabulary};

#[rustfmt::skip]
const LEXER_ATN: &[i32] = &[
    4, 0, 9, 34, 6, -1, 2, 0, 7, 0, 2, 1, 7, 1, 2, 2, 7, 2, 2, 3,
    7, 3, 2, 4, 7, 4, 2, 5, 7, 5, 2, 6, 7, 6, 2, 7, 7, 7, 2, 8,
    7, 8, 4, 7, 20, 8, 7, 11, 7, 12, 7, 21, 1, 7, 4, 7, 25, 8, 7, 11,
    7, 12, 7, 26, 3, 7, 29, 8, 7, 4, 8, 31, 8, 8, 11, 8, 12, 8, 32, 0,
    0, 9, 1, 1, 3, 2, 5, 3, 7, 4, 9, 5, 11, 6, 13, 7, 15, 8, 17, 9,
    1, 0, 2, 1, 0, 48, 57, 3, 0, 9, 10, 13, 13, 32, 32, 37, 0, 1, 1, 0,
    0, 0, 0, 3, 1, 0, 0, 0, 0, 5, 1, 0, 0, 0, 0, 7, 1, 0, 0, 0,
    0, 9, 1, 0, 0, 0, 0, 11, 1, 0, 0, 0, 0, 13, 1, 0, 0, 0, 0, 15,
    1, 0, 0, 0, 0, 17, 1, 0, 0, 0, 1, 2, 5, 94, 0, 0, 3, 4, 5, 45,
    0, 0, 5, 6, 5, 42, 0, 0, 7, 8, 5, 47, 0, 0, 9, 10, 5, 43, 0, 0,
    11, 12, 5, 40, 0, 0, 13, 14, 5, 41, 0, 0, 15, 19, 1, 0, 0, 0, 17, 30,
    1, 0, 0, 0, 19, 20, 7, 0, 0, 0, 20, 21, 1, 0, 0, 0, 21, 19, 1, 0,
    0, 0, 21, 22, 1, 0, 0, 0, 22, 28, 1, 0, 0, 0, 23, 24, 5, 46, 0, 0,
    24, 25, 7, 0, 0, 0, 25, 26, 1, 0, 0, 0, 26, 24, 1, 0, 0, 0, 26, 27,
    1, 0, 0, 0, 27, 29, 1, 0, 0, 0, 28, 23, 1, 0, 0, 0, 28, 29, 1, 0,
    0, 0, 29, 16, 1, 0, 0, 0, 30, 31, 7, 1, 0, 0, 31, 32, 1, 0, 0, 0,
    32, 30, 1, 0, 0, 0, 32, 33, 1, 0, 0, 0, 33, 18, 6, 8, 0, 0, 5, 0,
    21, 26, 28, 32, 1, 6, 0, 0,
];

#[rustfmt::skip]
const PARSER_ATN: &[i32] = &[
    4, 1, 9, 29, 2, 0, 7, 0, 2, 1, 7, 1, 1, 0, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 13, 8, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 5, 1, 24, 8, 1, 10, 1, 12, 1,
    27, 9, 1, 1, 1, 0, 1, 2, 2, 0, 2, 0, 2, 1, 0, 3, 4, 2, 0, 2,
    2, 5, 5, 31, 0, 4, 3, 2, 1, 0, 2, 12, 1, 0, 0, 0, 4, 1, 5, 0,
    0, 1, 5, 6, 6, 1, -1, 0, 6, 7, 5, 2, 0, 0, 7, 13, 3, 2, 1, 5,
    8, 9, 5, 6, 0, 0, 9, 10, 3, 2, 1, 0, 10, 13, 5, 7, 0, 0, 11, 13,
    5, 8, 0, 0, 12, 5, 1, 0, 0, 0, 12, 8, 1, 0, 0, 0, 12, 11, 1, 0,
    0, 0, 13, 25, 1, 0, 0, 0, 14, 15, 10, 6, 0, 0, 15, 16, 5, 1, 0, 0,
    16, 24, 3, 2, 1, 6, 17, 18, 10, 4, 0, 0, 18, 19, 7, 0, 0, 0, 19, 24,
    3, 2, 1, 5, 20, 21, 10, 3, 0, 0, 21, 22, 7, 1, 0, 0, 22, 24, 3, 2,
    1, 4, 23, 14, 1, 0, 0, 0, 23, 17, 1, 0, 0, 0, 23, 20, 1, 0, 0, 0,
    24, 27, 1, 0, 0, 0, 25, 23, 1, 0, 0, 0, 25, 26, 1, 0, 0, 0, 26, 3,
    1, 0, 0, 0, 27, 25, 1, 0, 0, 0, 3, 12, 23, 25,
];

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn grammars() -> (Grammar, Grammar) {
    let vocabulary = Vocabulary {
        literal_names: [
            None,
            Some("'^'"),
            Some("'-'"),
            Some("'*'"),
            Some("'/'"),
            Some("'+'"),
            Some("'('"),
            Some("')'"),
        ]
        .into_iter()
        .map(|name: Option<&str>| name.map(str::to_string))
        .collect(),
        symbolic_names: [
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("NUMBER"),
            Some("WS"),
        ]
        .into_iter()
        .map(|name: Option<&str>| name.map(str::to_string))
        .collect(),
    };
    let lexer = Grammar::new(
        LEXER_ATN,
        names(&[
            "T__0", "T__1", "T__2", "T__3", "T__4", "T__5", "T__6", "NUMBER", "WS",
        ]),
        vocabulary.clone(),
    )
    .unwrap();
    let parser = Grammar::new(PARSER_ATN, names(&["program", "expr"]), vocabulary).unwrap();
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
