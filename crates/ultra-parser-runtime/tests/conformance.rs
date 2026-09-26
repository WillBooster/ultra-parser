//! Checks the runtime against ANTLR's own interpreters, using the fixtures that
//! `conformance/GenerateFixtures.java` records.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use ultra_parser_runtime::{GrammarData, LexerInterpreter, ParserInterpreter, Token};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    grammar: String,
    start_rule: String,
    lexer: Data,
    parser: Data,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    recognizer_name: String,
    serialized_atn: Vec<i32>,
    rule_names: Vec<String>,
    literal_names: Vec<Option<String>>,
    symbolic_names: Vec<Option<String>>,
    channel_names: Vec<String>,
    mode_names: Vec<String>,
}

#[derive(Deserialize)]
struct Case {
    input: String,
    tokens: Vec<String>,
    tree: String,
    errors: Vec<String>,
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn leak_strs(v: Vec<String>) -> &'static [&'static str] {
    Box::leak(v.into_iter().map(leak_str).collect())
}

fn leak_opt_strs(v: Vec<Option<String>>) -> &'static [Option<&'static str>] {
    Box::leak(v.into_iter().map(|s| s.map(leak_str)).collect())
}

fn leak_grammar_data(grammar: &str, data: Data) -> &'static GrammarData {
    Box::leak(Box::new(GrammarData {
        grammar_file_name: leak_str(grammar.to_string()),
        recognizer_name: leak_str(data.recognizer_name),
        serialized_atn: Box::leak(data.serialized_atn.into_boxed_slice()),
        rule_names: leak_strs(data.rule_names),
        literal_names: leak_opt_strs(data.literal_names),
        symbolic_names: leak_opt_strs(data.symbolic_names),
        channel_names: leak_strs(data.channel_names),
        mode_names: leak_strs(data.mode_names),
    }))
}

/// Formats a token like ANTLR's `CommonToken.toString()`.
fn format_token(t: &Token) -> String {
    let text = t
        .text
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let channel = if t.channel > 0 {
        format!(",channel={}", t.channel)
    } else {
        String::new()
    };
    format!(
        "[@{},{}:{}='{}',<{}>{},{}:{}]",
        t.token_index,
        t.start,
        t.end as isize - 1,
        text,
        t.token_type,
        channel,
        t.line,
        t.column
    )
}

#[test]
fn matches_antlr_interpreters() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures");
    let mut paths: Vec<_> = fs::read_dir(&fixtures_dir)
        .expect("fixtures directory")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no fixtures in {}",
        fixtures_dir.display()
    );

    let mut failures = Vec::new();
    let mut case_count = 0;
    for path in paths {
        let fixture: Fixture = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let lexer =
            LexerInterpreter::new(leak_grammar_data(&fixture.grammar, fixture.lexer)).unwrap();
        let parser =
            ParserInterpreter::new(leak_grammar_data(&fixture.grammar, fixture.parser)).unwrap();
        let start_rule = parser
            .data()
            .rule_names
            .iter()
            .position(|name| *name == fixture.start_rule)
            .expect("start rule");
        for case in fixture.cases {
            case_count += 1;
            let (tokens, mut errors) = lexer.tokenize(&case.input);
            let output = parser.parse(&tokens, start_rule);
            errors.extend(output.errors);
            let actual_tokens: Vec<String> = tokens.iter().map(format_token).collect();
            let actual_errors: Vec<String> = errors.iter().map(ToString::to_string).collect();
            let actual_tree = output.tree.to_string_tree();
            if actual_tokens != case.tokens
                || actual_tree != case.tree
                || actual_errors != case.errors
            {
                failures.push(format!(
                    "{} {:?}\n  expected tokens: {:?}\n  actual tokens:   {:?}\n  expected tree:   {}\n  actual tree:     {}\n  expected errors: {:?}\n  actual errors:   {:?}",
                    fixture.grammar,
                    case.input,
                    case.tokens,
                    actual_tokens,
                    case.tree,
                    actual_tree,
                    case.errors,
                    actual_errors
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {case_count} cases differ from ANTLR:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
