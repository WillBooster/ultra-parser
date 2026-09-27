//! Runtime for grammars compiled by the ultra-parser tool.
//!
//! A [`Grammar`] holds the serialized ATN that the tool generates for a lexer or parser.
//! [`Grammar::match_token`] and [`Grammar::parse`] interpret it like ANTLR's `LexerATNSimulator`
//! and generated parsers do, including adaptive LL(*) prediction and ANTLR's default error
//! recovery. The grammar's code (actions and predicates) runs in a host: [`LexerHost`] and
//! [`ParserHost`] receive what the ATN refers to and what the parser builds.

mod atn;
mod config;
mod context;
mod hash;
mod interval_set;
mod lexer;
mod ll1;
mod parser;
mod prediction;
mod semantic;
mod token;
mod tree;

use std::fmt;

pub use atn::{AtnError, LexerAction};
pub use interval_set::IntervalSet;
pub use lexer::{LexerActionRef, LexerHost, LexerMatch, Position};
pub use parser::ParserHost;
pub use prediction::{Diagnostic, DiagnosticKind, PredictionMode};
pub use token::{
    DEFAULT_CHANNEL, EOF, HIDDEN_CHANNEL, Token, Tokens, Vocabulary, code_points_to_string,
};
pub use tree::NodeId;

use atn::{Atn, GrammarType};

/// A lexer or parser grammar.
#[derive(Debug)]
pub struct Grammar {
    atn: Atn,
    rule_names: Vec<String>,
    vocabulary: Vocabulary,
}

/// A syntax error, located at a token or at the text the lexer could not match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    /// Index of the offending token; `None` for errors of the lexer.
    pub offending_token: Option<usize>,
    /// 1-based line.
    pub line: usize,
    /// 0-based column in code points.
    pub column: usize,
    pub message: String,
}

impl fmt::Display for SyntaxError {
    /// Formats the error like ANTLR's `ConsoleErrorListener`, e.g., `line 1:4 missing ')' at '<EOF>'`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}:{} {}", self.line, self.column, self.message)
    }
}

impl Grammar {
    /// Loads a grammar from its serialized ATN, rule names, and token names.
    pub fn new(
        serialized_atn: &[i32],
        rule_names: Vec<String>,
        vocabulary: Vocabulary,
    ) -> Result<Self, AtnError> {
        let atn = Atn::deserialize(serialized_atn)?;
        if rule_names.len() != atn.rule_to_start_state.len() {
            return Err(AtnError::new(format!(
                "the ATN has {} rules but {} rule names were given",
                atn.rule_to_start_state.len(),
                rule_names.len()
            )));
        }
        Ok(Self {
            atn,
            rule_names,
            vocabulary,
        })
    }

    pub fn is_lexer(&self) -> bool {
        self.atn.grammar_type == GrammarType::Lexer
    }

    pub fn rule_names(&self) -> &[String] {
        &self.rule_names
    }

    /// The actions that `LexerMatch::actions` refer to.
    pub fn lexer_actions(&self) -> &[LexerAction] {
        &self.atn.lexer_actions
    }

    /// Matches one token of `input` in lexer mode `mode` from `start`. On failure, returns the
    /// position where no lexer rule could continue.
    pub fn match_token(
        &self,
        input: &[u32],
        mode: usize,
        start: Position,
        host: &mut dyn LexerHost,
    ) -> Result<LexerMatch, Position> {
        assert!(self.is_lexer(), "match_token needs a lexer grammar");
        assert!(
            mode < self.atn.mode_to_start_state.len(),
            "invalid mode {mode}"
        );
        lexer::match_token(&self.atn, input, mode, start, host)
    }

    /// Splits `source` into tokens, ending with an EOF token. Tokens skipped by the grammar are
    /// dropped; tokens on other channels are kept. Custom actions do not run, and predicates
    /// succeed.
    pub fn tokenize(&self, source: &str) -> (Tokens, Vec<SyntaxError>) {
        assert!(self.is_lexer(), "tokenize needs a lexer grammar");
        lexer::tokenize(&self.atn, source.chars().map(u32::from).collect(), &mut ())
    }

    /// Parses `tokens`, which must end with an EOF token, from token `start_token` (or the next
    /// token on the default channel) with rule `start_rule`, reporting the parse tree to `host`,
    /// and returns the root context. Error recovery produces a tree even for invalid input.
    pub fn parse<H: ParserHost>(
        &self,
        tokens: &Tokens,
        start_token: usize,
        start_rule: usize,
        mode: PredictionMode,
        host: &mut H,
    ) -> NodeId {
        assert!(!self.is_lexer(), "parse needs a parser grammar");
        assert!(
            tokens.tokens.last().is_some_and(|t| t.token_type == EOF),
            "tokens must end with an EOF token"
        );
        assert!(
            start_rule < self.rule_names.len(),
            "invalid start rule {start_rule}"
        );
        parser::Parser::new(
            &self.atn,
            &self.vocabulary,
            &self.rule_names,
            tokens,
            start_token,
            mode,
            host,
        )
        .parse(start_rule)
    }

    /// The tokens that can follow `state` in a rule invoked from `invoking_states`, innermost
    /// first (`ATN.getExpectedTokens`).
    pub fn expected_tokens(&self, state: usize, invoking_states: &[usize]) -> IntervalSet {
        let follow_states: Vec<usize> = invoking_states
            .iter()
            .map(|&s| match self.atn.states[s].transitions[0] {
                atn::Transition::Rule { follow_state, .. } => follow_state,
                _ => panic!("state {s} does not invoke a rule"),
            })
            .collect();
        parser::expected_tokens(&self.atn, state, &follow_states)
    }
}
