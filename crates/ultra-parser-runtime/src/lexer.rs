//! The lexer simulator, ported from ANTLR's `LexerATNSimulator` without DFA caching, and a driver
//! ported from `Lexer.nextToken()`.

use crate::hash::FxHashSet;
use std::rc::Rc;

use crate::SyntaxError;
use crate::atn::{
    Atn, INVALID_ALT, LexerAction, MAX_CHAR_VALUE, MIN_CHAR_VALUE, StateKind, Transition,
};
use crate::context::{Ctx, EMPTY_RETURN_STATE, PredictionContext};
use crate::token::{DEFAULT_CHANNEL, EOF, INVALID_TYPE, Token, Tokens, escape_ws, to_code_points};

const MORE: i32 = -2;
const SKIP: i32 = -3;

/// A position in the input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    /// Offset in code points.
    pub index: usize,
    /// 1-based line.
    pub line: usize,
    /// 0-based column in code points.
    pub column: usize,
}

impl Position {
    pub const START: Self = Self {
        index: 0,
        line: 1,
        column: 0,
    };

    /// The position after consuming the code point at this position of `input`.
    pub fn advance(self, input: &[u32]) -> Self {
        match input.get(self.index) {
            None => self,
            Some(&c) if c == '\n' as u32 => Self {
                index: self.index + 1,
                line: self.line + 1,
                column: 0,
            },
            Some(_) => Self {
                index: self.index + 1,
                column: self.column + 1,
                ..self
            },
        }
    }
}

/// Runs the grammar code that the lexer ATN refers to.
pub trait LexerHost {
    /// Evaluates predicate `pred_index` of lexer rule `rule_index` with the input at `at`.
    fn sempred(&mut self, _rule_index: usize, _pred_index: usize, _at: Position) -> bool {
        true
    }
}

/// Grammar code is not run: predicates succeed.
impl LexerHost for () {}

/// A lexer action to run when a token is accepted: an index into `Atn::lexer_actions` and, for a
/// custom action reached before the end of the token, how many code points into the token it is.
pub type LexerActionRef = (usize, Option<usize>);

type Executor = Rc<Vec<LexerActionRef>>;

/// The token a lexer rule matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexerMatch {
    /// The token type, or EOF at the end of the input.
    pub token_type: i32,
    /// The position after the token.
    pub stop: Position,
    /// The actions to run, in order.
    pub actions: Vec<LexerActionRef>,
}

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
    seen: FxHashSet<LexerConfig>,
}

impl LexerConfigSet {
    fn add(&mut self, config: LexerConfig) {
        if self.seen.insert(config.clone()) {
            self.configs.push(config);
        }
    }
}

/// Matches one token of `input` in `mode` from `start` (`LexerATNSimulator.match`). On failure,
/// returns the position where no rule could continue.
pub(crate) fn match_token(
    atn: &Atn,
    input: &[u32],
    mode: usize,
    start: Position,
    host: &mut dyn LexerHost,
) -> Result<LexerMatch, Position> {
    Simulator {
        atn,
        input,
        pos: start,
        start_index: start.index,
        host,
    }
    .match_token(mode)
}

struct Simulator<'a, 'h> {
    atn: &'a Atn,
    input: &'a [u32],
    pos: Position,
    start_index: usize,
    host: &'h mut dyn LexerHost,
}

struct Accept {
    pos: Position,
    token_type: i32,
    executor: Option<Executor>,
}

impl Simulator<'_, '_> {
    fn la(&self) -> i32 {
        self.input.get(self.pos.index).map_or(EOF, |&c| c as i32)
    }

    fn match_token(&mut self, mode: usize) -> Result<LexerMatch, Position> {
        let s0 = self.compute_start_state(self.atn.mode_to_start_state[mode]);
        let mut accept = None;
        if let Some((token_type, executor)) = self.accepted(&s0) {
            accept = Some(Accept {
                pos: self.pos,
                token_type,
                executor,
            });
        }
        let mut t = self.la();
        let mut s = s0;
        loop {
            let reach = self.compute_reach_set(&s, t);
            if reach.configs.is_empty() {
                break;
            }
            if t != EOF {
                self.pos = self.pos.advance(self.input);
            }
            if let Some((token_type, executor)) = self.accepted(&reach) {
                accept = Some(Accept {
                    pos: self.pos,
                    token_type,
                    executor,
                });
                if t == EOF {
                    break;
                }
            }
            t = self.la();
            s = reach;
        }
        match accept {
            Some(accept) => Ok(LexerMatch {
                token_type: accept.token_type,
                stop: accept.pos,
                actions: accept.executor.map(|e| e.to_vec()).unwrap_or_default(),
            }),
            None if t == EOF && self.pos.index == self.start_index => Ok(LexerMatch {
                token_type: EOF,
                stop: self.pos,
                actions: Vec::new(),
            }),
            None => Err(self.pos),
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

    fn compute_start_state(&mut self, start_state: usize) -> LexerConfigSet {
        let mut configs = LexerConfigSet::default();
        let atn = self.atn;
        for (i, transition) in atn.states[start_state].transitions.iter().enumerate() {
            let config = LexerConfig {
                state: transition.target(),
                alt: i + 1,
                context: PredictionContext::empty(),
                executor: None,
                passed_through_non_greedy_decision: false,
            };
            self.closure(config, &mut configs, false, false, false);
        }
        configs
    }

    fn compute_reach_set(&mut self, closure: &LexerConfigSet, t: i32) -> LexerConfigSet {
        let mut reach = LexerConfigSet::default();
        let mut skip_alt = INVALID_ALT;
        let atn = self.atn;
        for c in &closure.configs {
            let current_alt_reached_accept_state = c.alt == skip_alt;
            if current_alt_reached_accept_state && c.passed_through_non_greedy_decision {
                continue;
            }
            for transition in &atn.states[c.state].transitions {
                if !transition.matches(atn, t, MIN_CHAR_VALUE, MAX_CHAR_VALUE) {
                    continue;
                }
                let executor = c
                    .executor
                    .as_ref()
                    .map(|e| fix_offset_before_match(atn, e, self.pos.index - self.start_index));
                let config = c.derive(atn, transition.target(), c.context.clone(), executor);
                if self.closure(
                    config,
                    &mut reach,
                    current_alt_reached_accept_state,
                    true,
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
        &mut self,
        config: LexerConfig,
        configs: &mut LexerConfigSet,
        mut current_alt_reached_accept_state: bool,
        speculative: bool,
        treat_eof_as_epsilon: bool,
    ) -> bool {
        let atn = self.atn;
        let state = &atn.states[config.state];
        if state.kind == StateKind::RuleStop {
            if config.context.has_empty_path() {
                if config.context.is_empty() {
                    configs.add(config);
                    return true;
                }
                configs.add(config.derive(
                    atn,
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
                    let c = config.derive(atn, return_state, parent, config.executor.clone());
                    current_alt_reached_accept_state = self.closure(
                        c,
                        configs,
                        current_alt_reached_accept_state,
                        speculative,
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
            if let Some(c) =
                self.epsilon_target(&config, transition, speculative, treat_eof_as_epsilon)
            {
                current_alt_reached_accept_state = self.closure(
                    c,
                    configs,
                    current_alt_reached_accept_state,
                    speculative,
                    treat_eof_as_epsilon,
                );
            }
        }
        current_alt_reached_accept_state
    }

    fn epsilon_target(
        &mut self,
        config: &LexerConfig,
        transition: &Transition,
        speculative: bool,
        treat_eof_as_epsilon: bool,
    ) -> Option<LexerConfig> {
        let atn = self.atn;
        match *transition {
            Transition::Rule {
                target,
                follow_state,
                ..
            } => {
                let context =
                    PredictionContext::singleton(Some(config.context.clone()), follow_state);
                Some(config.derive(atn, target, context, config.executor.clone()))
            }
            // Precedence predicates only appear in parsers.
            Transition::Precedence { .. } => None,
            Transition::Predicate {
                target,
                rule_index,
                pred_index,
                ..
            } => {
                // Like ANTLR, a speculative evaluation sees the input after the current symbol.
                let at = if speculative {
                    self.pos.advance(self.input)
                } else {
                    self.pos
                };
                self.host
                    .sempred(rule_index, pred_index, at)
                    .then(|| config.with_state(atn, target))
            }
            Transition::Epsilon { target, .. }
            | Transition::Action {
                target,
                action_index: None,
                ..
            } => Some(config.with_state(atn, target)),
            Transition::Action {
                target,
                action_index: Some(action_index),
                ..
            } => {
                if config.context.has_empty_path() {
                    let mut actions = config.executor.as_deref().cloned().unwrap_or_default();
                    actions.push((action_index, None));
                    Some(config.derive(atn, target, config.context.clone(), Some(Rc::new(actions))))
                } else {
                    // Actions in rules that the token rule invokes are not run.
                    Some(config.with_state(atn, target))
                }
            }
            Transition::Atom { target, .. }
            | Transition::Range { target, .. }
            | Transition::Set { target, .. }
                if treat_eof_as_epsilon
                    && transition.matches(atn, EOF, MIN_CHAR_VALUE, MAX_CHAR_VALUE) =>
            {
                Some(config.with_state(atn, target))
            }
            _ => None,
        }
    }
}

/// Records how far into the token the custom actions of `executor` are
/// (`LexerActionExecutor.fixOffsetBeforeMatch`).
fn fix_offset_before_match(atn: &Atn, executor: &Executor, offset: usize) -> Executor {
    if !executor
        .iter()
        .any(|&(i, o)| o.is_none() && matches!(atn.lexer_actions[i], LexerAction::Custom { .. }))
    {
        return executor.clone();
    }
    Rc::new(
        executor
            .iter()
            .map(|&(i, o)| match atn.lexer_actions[i] {
                LexerAction::Custom { .. } if o.is_none() => (i, Some(offset)),
                _ => (i, o),
            })
            .collect(),
    )
}

/// Splits `input` into tokens like `Lexer.nextToken()` does, ending with an EOF token. Custom
/// actions are not run.
pub(crate) fn tokenize(
    atn: &Atn,
    input: Vec<u32>,
    host: &mut dyn LexerHost,
) -> (Tokens, Vec<SyntaxError>) {
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut pos = Position::START;
    let mut mode = 0;
    let mut mode_stack = Vec::new();
    let mut hit_eof = false;
    'outer: loop {
        if hit_eof {
            tokens.push(Token {
                token_type: EOF,
                channel: DEFAULT_CHANNEL,
                start: pos.index,
                end: pos.index,
                line: pos.line,
                column: pos.column,
                token_index: tokens.len(),
                text: None,
            });
            break;
        }
        let token_start = pos;
        let mut channel = DEFAULT_CHANNEL;
        let mut token_type;
        loop {
            token_type = INVALID_TYPE;
            let predicted = match match_token(atn, &input, mode, pos, host) {
                Ok(m) => {
                    pos = m.stop;
                    for (action, _) in m.actions {
                        match atn.lexer_actions[action] {
                            LexerAction::Channel(c) => channel = c,
                            LexerAction::Custom { .. } => {}
                            LexerAction::Mode(m) => mode = m,
                            LexerAction::More => token_type = MORE,
                            LexerAction::PopMode => match mode_stack.pop() {
                                Some(m) => mode = m,
                                // ANTLR throws `EmptyStackException` here; reporting keeps
                                // lexing without aborting the WebAssembly instance.
                                None => errors.push(SyntaxError {
                                    offending_token: None,
                                    line: token_start.line,
                                    column: token_start.column,
                                    message: to_code_points(
                                        "cannot pop a mode: the mode stack is empty",
                                    ),
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
                    m.token_type
                }
                Err(stop) => {
                    let end = (stop.index + 1).min(input.len());
                    errors.push(SyntaxError {
                        offending_token: None,
                        line: token_start.line,
                        column: token_start.column,
                        message: [
                            to_code_points("token recognition error at: '"),
                            escape_ws(&input[token_start.index..end]),
                            to_code_points("'"),
                        ]
                        .concat(),
                    });
                    pos = stop.advance(&input);
                    SKIP
                }
            };
            if pos.index >= input.len() {
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
        tokens.push(Token {
            token_type,
            channel,
            start: token_start.index,
            end: pos.index.max(token_start.index),
            line: token_start.line,
            column: token_start.column,
            token_index: tokens.len(),
            text: None,
        });
        if token_type == EOF {
            break;
        }
    }
    (Tokens { input, tokens }, errors)
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
        let (tokens, errors) = tokenize(&atn, vec!['a' as u32], &mut ());
        assert_eq!(
            tokens
                .tokens
                .iter()
                .map(|t| t.token_type)
                .collect::<Vec<_>>(),
            [1, EOF]
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].to_string(),
            "line 1:0 cannot pop a mode: the mode stack is empty"
        );
    }
}
