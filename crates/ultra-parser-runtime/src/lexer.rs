//! The lexer simulator, ported from ANTLR's `LexerATNSimulator` and `Lexer.nextToken()`
//! without DFA caching.

use std::collections::HashSet;
use std::rc::Rc;

use crate::SyntaxError;
use crate::atn::{
    Atn, INVALID_ALT, LexerAction, MAX_CHAR_VALUE, MIN_CHAR_VALUE, StateKind, Transition,
};
use crate::context::{Ctx, EMPTY_RETURN_STATE, PredictionContext};
use crate::token::{DEFAULT_CHANNEL, EOF, INVALID_TYPE, Token};

const MORE: i32 = -2;
const SKIP: i32 = -3;

/// Indexes into `Atn::lexer_actions` to run when a token is accepted.
type Executor = Rc<Vec<usize>>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LexerConfig {
    state: usize,
    alt: usize,
    context: Ctx,
    executor: Option<Executor>,
    passed_through_non_greedy_decision: bool,
}

impl LexerConfig {
    fn derive(&self, atn: &Atn, state: usize, context: Ctx, executor: Option<Executor>) -> Self {
        let target = &atn.states[state];
        Self {
            state,
            alt: self.alt,
            context,
            executor,
            passed_through_non_greedy_decision: self.passed_through_non_greedy_decision
                || (target.kind.is_decision() && target.non_greedy),
        }
    }

    fn with_state(&self, atn: &Atn, state: usize) -> Self {
        self.derive(atn, state, self.context.clone(), self.executor.clone())
    }
}

/// An ordered set of configurations without context merging (`OrderedATNConfigSet`).
#[derive(Default)]
struct LexerConfigSet {
    configs: Vec<LexerConfig>,
    seen: HashSet<LexerConfig>,
}

impl LexerConfigSet {
    fn add(&mut self, config: LexerConfig) {
        if self.seen.insert(config.clone()) {
            self.configs.push(config);
        }
    }
}

struct Accept {
    index: usize,
    line: usize,
    column: usize,
    token_type: i32,
    executor: Option<Executor>,
}

pub(crate) struct Lexer<'a> {
    atn: &'a Atn,
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
    start_index: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(atn: &'a Atn, source: &str) -> Self {
        Self {
            atn,
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 0,
            start_index: 0,
        }
    }

    pub(crate) fn tokenize(mut self) -> (Vec<Token>, Vec<SyntaxError>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        let mut mode = 0;
        let mut mode_stack = Vec::new();
        let mut hit_eof = false;
        'outer: loop {
            if hit_eof {
                tokens.push(self.token(
                    EOF,
                    DEFAULT_CHANNEL,
                    self.index,
                    self.line,
                    self.column,
                    tokens.len(),
                ));
                break;
            }
            let token_start = self.index;
            let start_line = self.line;
            let start_column = self.column;
            let mut channel = DEFAULT_CHANNEL;
            let mut token_type;
            loop {
                token_type = INVALID_TYPE;
                let predicted = match self.match_token(mode) {
                    Some((predicted, executor)) => {
                        for &action in executor.iter().flat_map(|e| e.iter()) {
                            match self.atn.lexer_actions[action] {
                                LexerAction::Channel(c) => channel = c,
                                // Custom actions are grammar code, which the runtime does not run.
                                LexerAction::Custom { .. } => {}
                                LexerAction::Mode(m) => mode = m,
                                LexerAction::More => token_type = MORE,
                                LexerAction::PopMode => match mode_stack.pop() {
                                    Some(m) => mode = m,
                                    // ANTLR throws `EmptyStackException` here; reporting keeps
                                    // lexing without aborting the WebAssembly instance.
                                    None => errors.push(SyntaxError {
                                        line: start_line,
                                        column: start_column,
                                        start: token_start,
                                        end: self.index,
                                        message: "cannot pop a mode: the mode stack is empty"
                                            .to_string(),
                                    }),
                                },
                                LexerAction::PushMode(m) => {
                                    mode_stack.push(mode);
                                    mode = m;
                                }
                                LexerAction::Skip => token_type = SKIP,
                                LexerAction::Type(t) => token_type = t,
                            }
                        }
                        predicted
                    }
                    None => {
                        let end = (self.index + 1).min(self.chars.len());
                        let text: String = self.chars[token_start..end]
                            .iter()
                            .map(|&c| error_display(c))
                            .collect();
                        errors.push(SyntaxError {
                            line: start_line,
                            column: start_column,
                            start: token_start,
                            end,
                            message: format!("token recognition error at: '{text}'"),
                        });
                        if self.la() != EOF {
                            self.consume();
                        }
                        SKIP
                    }
                };
                if self.la() == EOF {
                    hit_eof = true;
                }
                if token_type == INVALID_TYPE {
                    token_type = predicted;
                }
                if token_type == SKIP {
                    continue 'outer;
                }
                if token_type != MORE {
                    break;
                }
            }
            tokens.push(self.token(
                token_type,
                channel,
                token_start,
                start_line,
                start_column,
                tokens.len(),
            ));
            if token_type == EOF {
                break;
            }
        }
        (tokens, errors)
    }

    fn token(
        &self,
        token_type: i32,
        channel: i32,
        start: usize,
        line: usize,
        column: usize,
        token_index: usize,
    ) -> Token {
        let end = self.index.max(start);
        let text = if token_type == EOF && start >= self.chars.len() {
            "<EOF>".to_string()
        } else {
            self.chars[start..end].iter().collect()
        };
        Token {
            token_type,
            channel,
            start,
            end,
            line,
            column,
            token_index,
            text,
        }
    }

    fn la(&self) -> i32 {
        self.chars.get(self.index).map_or(EOF, |&c| c as i32)
    }

    fn consume(&mut self) {
        if self.chars[self.index] == '\n' {
            self.line += 1;
            self.column = 0;
        } else {
            self.column += 1;
        }
        self.index += 1;
    }

    /// Matches the next token in `mode`; returns its type and the actions to run, or `None` when no
    /// rule matches.
    fn match_token(&mut self, mode: usize) -> Option<(i32, Option<Executor>)> {
        self.start_index = self.index;
        let s0 = self.compute_start_state(self.atn.mode_to_start_state[mode]);
        let mut accept = None;
        if let Some((token_type, executor)) = self.accepted(&s0) {
            accept = Some(self.capture(token_type, executor));
        }
        let mut t = self.la();
        let mut s = s0;
        loop {
            let reach = self.compute_reach_set(&s, t);
            if reach.configs.is_empty() {
                break;
            }
            if t != EOF {
                self.consume();
            }
            if let Some((token_type, executor)) = self.accepted(&reach) {
                accept = Some(self.capture(token_type, executor));
                if t == EOF {
                    break;
                }
            }
            t = self.la();
            s = reach;
        }
        match accept {
            Some(accept) => {
                self.index = accept.index;
                self.line = accept.line;
                self.column = accept.column;
                Some((accept.token_type, accept.executor))
            }
            None if t == EOF && self.index == self.start_index => Some((EOF, None)),
            None => None,
        }
    }

    fn capture(&self, token_type: i32, executor: Option<Executor>) -> Accept {
        Accept {
            index: self.index,
            line: self.line,
            column: self.column,
            token_type,
            executor,
        }
    }

    /// The token type predicted by the first configuration that reached the end of a rule.
    fn accepted(&self, configs: &LexerConfigSet) -> Option<(i32, Option<Executor>)> {
        configs
            .configs
            .iter()
            .find(|c| self.atn.states[c.state].kind == StateKind::RuleStop)
            .map(|c| {
                let rule_index = self.atn.states[c.state].rule_index;
                (self.atn.rule_to_token_type[rule_index], c.executor.clone())
            })
    }

    fn compute_start_state(&self, start_state: usize) -> LexerConfigSet {
        let mut configs = LexerConfigSet::default();
        for (i, transition) in self.atn.states[start_state].transitions.iter().enumerate() {
            let config = LexerConfig {
                state: transition.target(),
                alt: i + 1,
                context: PredictionContext::empty(),
                executor: None,
                passed_through_non_greedy_decision: false,
            };
            self.closure(config, &mut configs, false, false);
        }
        configs
    }

    fn compute_reach_set(&self, closure: &LexerConfigSet, t: i32) -> LexerConfigSet {
        let mut reach = LexerConfigSet::default();
        let mut skip_alt = INVALID_ALT;
        for c in &closure.configs {
            let current_alt_reached_accept_state = c.alt == skip_alt;
            if current_alt_reached_accept_state && c.passed_through_non_greedy_decision {
                continue;
            }
            for transition in &self.atn.states[c.state].transitions {
                if !transition.matches(self.atn, t, MIN_CHAR_VALUE, MAX_CHAR_VALUE) {
                    continue;
                }
                let config = c.with_state(self.atn, transition.target());
                if self.closure(
                    config,
                    &mut reach,
                    current_alt_reached_accept_state,
                    t == EOF,
                ) {
                    // This alternative reached an accept state; later configurations of the same
                    // alternative cannot produce a better (longer) match.
                    skip_alt = c.alt;
                    break;
                }
            }
        }
        reach
    }

    fn closure(
        &self,
        config: LexerConfig,
        configs: &mut LexerConfigSet,
        mut current_alt_reached_accept_state: bool,
        treat_eof_as_epsilon: bool,
    ) -> bool {
        let state = &self.atn.states[config.state];
        if state.kind == StateKind::RuleStop {
            if config.context.has_empty_path() {
                if config.context.is_empty() {
                    configs.add(config);
                    return true;
                }
                configs.add(config.derive(
                    self.atn,
                    config.state,
                    PredictionContext::empty(),
                    config.executor.clone(),
                ));
                current_alt_reached_accept_state = true;
            }
            if !config.context.is_empty() {
                for i in 0..config.context.len() {
                    let return_state = config.context.return_state(i);
                    if return_state == EMPTY_RETURN_STATE {
                        continue;
                    }
                    let parent = config
                        .context
                        .parent(i)
                        .expect("non-empty return state")
                        .clone();
                    let c = config.derive(self.atn, return_state, parent, config.executor.clone());
                    current_alt_reached_accept_state = self.closure(
                        c,
                        configs,
                        current_alt_reached_accept_state,
                        treat_eof_as_epsilon,
                    );
                }
            }
            return current_alt_reached_accept_state;
        }

        if !state.epsilon_only
            && (!current_alt_reached_accept_state || !config.passed_through_non_greedy_decision)
        {
            configs.add(config.clone());
        }
        for transition in &state.transitions {
            if let Some(c) = self.epsilon_target(&config, transition, treat_eof_as_epsilon) {
                current_alt_reached_accept_state = self.closure(
                    c,
                    configs,
                    current_alt_reached_accept_state,
                    treat_eof_as_epsilon,
                );
            }
        }
        current_alt_reached_accept_state
    }

    fn epsilon_target(
        &self,
        config: &LexerConfig,
        transition: &Transition,
        treat_eof_as_epsilon: bool,
    ) -> Option<LexerConfig> {
        match *transition {
            Transition::Rule {
                target,
                follow_state,
                ..
            } => {
                let context =
                    PredictionContext::singleton(Some(config.context.clone()), follow_state);
                Some(config.derive(self.atn, target, context, config.executor.clone()))
            }
            // Precedence predicates only appear in parsers.
            Transition::Precedence { .. } => None,
            // Predicates are grammar code, which the runtime does not run; they always succeed.
            Transition::Predicate { target, .. }
            | Transition::Epsilon { target, .. }
            | Transition::Action {
                target,
                action_index: None,
                ..
            } => Some(config.with_state(self.atn, target)),
            Transition::Action {
                target,
                action_index: Some(action_index),
                ..
            } => {
                if config.context.has_empty_path() {
                    let mut actions = config.executor.as_deref().cloned().unwrap_or_default();
                    actions.push(action_index);
                    Some(config.derive(
                        self.atn,
                        target,
                        config.context.clone(),
                        Some(Rc::new(actions)),
                    ))
                } else {
                    Some(config.with_state(self.atn, target))
                }
            }
            Transition::Atom { target, .. }
            | Transition::Range { target, .. }
            | Transition::Set { target, .. }
                if treat_eof_as_epsilon
                    && transition.matches(self.atn, EOF, MIN_CHAR_VALUE, MAX_CHAR_VALUE) =>
            {
                Some(config.with_state(self.atn, target))
            }
            _ => None,
        }
    }
}

fn error_display(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_pop_mode_on_an_empty_mode_stack() {
        // lexer grammar Pop; A : 'a' -> popMode ;
        let atn = Atn::deserialize(&[
            4, 0, 1, 4, 6, -1, 2, 0, 7, 0, 1, 0, 0, 0, 1, 1, 1, 1, 0, 0, 3, 0, 1, 1, 0, 0, 0, 1, 3,
            5, 97, 0, 0, 3, 2, 6, 0, 0, 0, 1, 0, 1, 4, 0, 0,
        ])
        .unwrap();
        let (tokens, errors) = Lexer::new(&atn, "a").tokenize();
        assert_eq!(
            tokens.iter().map(|t| t.token_type).collect::<Vec<_>>(),
            [1, EOF]
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].to_string(),
            "line 1:0 cannot pop a mode: the mode stack is empty"
        );
    }
}
