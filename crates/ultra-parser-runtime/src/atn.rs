//! The augmented transition network (ATN) and its deserializer, following ANTLR's serialization
//! format version 4.

use std::fmt;

use crate::interval_set::IntervalSet;
use crate::token::EOF;

const SERIALIZED_VERSION: i32 = 4;

pub const INVALID_ALT: usize = 0;

/// The rule index of states that belong to no rule.
pub const NO_RULE: usize = usize::MAX;

/// Lexers match code points in `0..=MAX_CHAR_VALUE`.
pub const MIN_CHAR_VALUE: i32 = 0;
pub const MAX_CHAR_VALUE: i32 = 0x10FFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrammarType {
    Lexer,
    Parser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateKind {
    Invalid,
    Basic,
    RuleStart,
    BlockStart,
    PlusBlockStart,
    StarBlockStart,
    TokenStart,
    RuleStop,
    BlockEnd,
    StarLoopBack,
    StarLoopEntry,
    PlusLoopBack,
    LoopEnd,
}

impl StateKind {
    fn from_serialized(value: i32) -> Result<Self, AtnError> {
        Ok(match value {
            0 => Self::Invalid,
            1 => Self::Basic,
            2 => Self::RuleStart,
            3 => Self::BlockStart,
            4 => Self::PlusBlockStart,
            5 => Self::StarBlockStart,
            6 => Self::TokenStart,
            7 => Self::RuleStop,
            8 => Self::BlockEnd,
            9 => Self::StarLoopBack,
            10 => Self::StarLoopEntry,
            11 => Self::PlusLoopBack,
            12 => Self::LoopEnd,
            _ => return Err(AtnError::new(format!("invalid state type {value}"))),
        })
    }

    pub fn is_decision(self) -> bool {
        matches!(
            self,
            Self::BlockStart
                | Self::PlusBlockStart
                | Self::StarBlockStart
                | Self::TokenStart
                | Self::StarLoopEntry
                | Self::PlusLoopBack
        )
    }

    fn is_block_start(self) -> bool {
        matches!(
            self,
            Self::BlockStart | Self::PlusBlockStart | Self::StarBlockStart
        )
    }
}

#[derive(Clone, Debug)]
pub struct AtnState {
    pub kind: StateKind,
    /// The rule the state belongs to, or `NO_RULE`.
    pub rule_index: usize,
    pub transitions: Vec<Transition>,
    /// Whether every transition is an epsilon transition; false for states without transitions.
    pub epsilon_only: bool,
    pub decision: Option<usize>,
    pub non_greedy: bool,
    /// The matching block end state of a block start state.
    pub end_state: Option<usize>,
    /// The matching block start state of a block end state.
    pub start_state: Option<usize>,
    /// The loop back state of a loop end, plus block start, or star loop entry state.
    pub loop_back_state: Option<usize>,
    /// Whether this rule start state begins a left-recursive (precedence) rule.
    pub is_left_recursive_rule: bool,
    /// Whether this star loop entry state decides whether a precedence rule continues.
    pub is_precedence_decision: bool,
}

impl AtnState {
    fn new(kind: StateKind, rule_index: usize) -> Self {
        Self {
            kind,
            rule_index,
            transitions: Vec::new(),
            epsilon_only: false,
            decision: None,
            non_greedy: false,
            end_state: None,
            start_state: None,
            loop_back_state: None,
            is_left_recursive_rule: false,
            is_precedence_decision: false,
        }
    }

    fn add_transition(&mut self, transition: Transition) {
        if self.transitions.is_empty() {
            self.epsilon_only = transition.is_epsilon();
        } else if self.epsilon_only != transition.is_epsilon() {
            self.epsilon_only = false;
        }
        self.transitions.push(transition);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transition {
    Epsilon {
        target: usize,
        /// The rule index of a left-recursive rule that this return edge leaves at its outermost
        /// invocation (precedence 0).
        outermost_precedence_return: Option<usize>,
    },
    Range {
        target: usize,
        from: i32,
        to: i32,
    },
    Rule {
        /// The start state of the invoked rule.
        target: usize,
        rule_index: usize,
        precedence: i32,
        follow_state: usize,
    },
    Predicate {
        target: usize,
        rule_index: usize,
        pred_index: usize,
        ctx_dependent: bool,
    },
    Atom {
        target: usize,
        label: i32,
    },
    Action {
        target: usize,
        rule_index: usize,
        /// `None` for the placeholder actions of rewritten left-recursive rules.
        action_index: Option<usize>,
        ctx_dependent: bool,
    },
    Set {
        target: usize,
        set: usize,
    },
    NotSet {
        target: usize,
        set: usize,
    },
    Wildcard {
        target: usize,
    },
    Precedence {
        target: usize,
        precedence: i32,
    },
}

impl Transition {
    pub fn target(&self) -> usize {
        match *self {
            Self::Epsilon { target, .. }
            | Self::Range { target, .. }
            | Self::Rule { target, .. }
            | Self::Predicate { target, .. }
            | Self::Atom { target, .. }
            | Self::Action { target, .. }
            | Self::Set { target, .. }
            | Self::NotSet { target, .. }
            | Self::Wildcard { target }
            | Self::Precedence { target, .. } => target,
        }
    }

    pub fn is_epsilon(&self) -> bool {
        matches!(
            self,
            Self::Epsilon { .. }
                | Self::Rule { .. }
                | Self::Predicate { .. }
                | Self::Action { .. }
                | Self::Precedence { .. }
        )
    }

    pub fn matches(&self, atn: &Atn, symbol: i32, min: i32, max: i32) -> bool {
        match *self {
            Self::Range { from, to, .. } => from <= symbol && symbol <= to,
            Self::Atom { label, .. } => label == symbol,
            Self::Set { set, .. } => atn.sets[set].contains(symbol),
            Self::NotSet { set, .. } => {
                min <= symbol && symbol <= max && !atn.sets[set].contains(symbol)
            }
            Self::Wildcard { .. } => min <= symbol && symbol <= max,
            _ => false,
        }
    }

    /// The symbols matched by a non-epsilon transition other than a wildcard.
    pub fn label(&self, atn: &Atn) -> Option<IntervalSet> {
        match *self {
            Self::Range { from, to, .. } => Some(IntervalSet::of(from, to)),
            Self::Atom { label, .. } => Some(IntervalSet::of(label, label)),
            Self::Set { set, .. } | Self::NotSet { set, .. } => Some(atn.sets[set].clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LexerAction {
    Channel(i32),
    Custom {
        rule_index: usize,
        action_index: usize,
    },
    Mode(usize),
    More,
    PopMode,
    PushMode(usize),
    Skip,
    Type(i32),
}

#[derive(Clone, Debug)]
pub struct Atn {
    pub grammar_type: GrammarType,
    pub max_token_type: i32,
    pub states: Vec<AtnState>,
    pub sets: Vec<IntervalSet>,
    pub rule_to_start_state: Vec<usize>,
    pub rule_to_stop_state: Vec<usize>,
    /// The token type each lexer rule emits; empty for parsers.
    pub rule_to_token_type: Vec<i32>,
    pub mode_to_start_state: Vec<usize>,
    pub decision_to_state: Vec<usize>,
    pub lexer_actions: Vec<LexerAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtnError {
    message: String,
}

impl AtnError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn wrong_grammar_type(recognizer_name: &str) -> Self {
        Self::new(format!("{recognizer_name} has the wrong grammar type"))
    }
}

impl fmt::Display for AtnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid serialized ATN: {}", self.message)
    }
}

impl std::error::Error for AtnError {}

struct Reader<'a> {
    data: &'a [i32],
    p: usize,
}

impl Reader<'_> {
    fn next(&mut self) -> Result<i32, AtnError> {
        let value = *self
            .data
            .get(self.p)
            .ok_or_else(|| AtnError::new("unexpected end of data"))?;
        self.p += 1;
        Ok(value)
    }

    fn next_usize(&mut self) -> Result<usize, AtnError> {
        let value = self.next()?;
        usize::try_from(value)
            .map_err(|_| AtnError::new(format!("unexpected negative value {value}")))
    }
}

impl Atn {
    pub fn deserialize(data: &[i32]) -> Result<Self, AtnError> {
        let mut r = Reader { data, p: 0 };
        let version = r.next()?;
        if version != SERIALIZED_VERSION {
            return Err(AtnError::new(format!(
                "version {version} is not supported (expected {SERIALIZED_VERSION})"
            )));
        }
        let grammar_type = match r.next()? {
            0 => GrammarType::Lexer,
            1 => GrammarType::Parser,
            other => return Err(AtnError::new(format!("invalid grammar type {other}"))),
        };
        let max_token_type = r.next()?;

        // States
        let n_states = r.next_usize()?;
        let mut states = Vec::with_capacity(n_states);
        let mut loop_back_numbers = Vec::new();
        let mut end_numbers = Vec::new();
        for i in 0..n_states {
            let kind = StateKind::from_serialized(r.next()?)?;
            if kind == StateKind::Invalid {
                states.push(AtnState::new(kind, 0));
                continue;
            }
            // Token start states of lexers belong to no rule (-1).
            let rule_index = usize::try_from(r.next()?).unwrap_or(NO_RULE);
            if kind == StateKind::LoopEnd {
                loop_back_numbers.push((i, r.next_usize()?));
            } else if kind.is_block_start() {
                end_numbers.push((i, r.next_usize()?));
            }
            states.push(AtnState::new(kind, rule_index));
        }
        let state_count = states.len();
        let check_state = |s: usize| {
            if s < state_count {
                Ok(s)
            } else {
                Err(AtnError::new(format!("state {s} is out of range")))
            }
        };
        for (s, loop_back) in loop_back_numbers {
            states[s].loop_back_state = Some(check_state(loop_back)?);
        }
        for (s, end) in end_numbers {
            states[s].end_state = Some(check_state(end)?);
        }
        for _ in 0..r.next_usize()? {
            let s = check_state(r.next_usize()?)?;
            states[s].non_greedy = true;
        }
        for _ in 0..r.next_usize()? {
            let s = check_state(r.next_usize()?)?;
            states[s].is_left_recursive_rule = true;
        }

        // Rules
        let n_rules = r.next_usize()?;
        let mut rule_to_start_state = Vec::with_capacity(n_rules);
        let mut rule_to_token_type = Vec::new();
        for _ in 0..n_rules {
            rule_to_start_state.push(check_state(r.next_usize()?)?);
            if grammar_type == GrammarType::Lexer {
                rule_to_token_type.push(r.next()?);
            }
        }
        let mut rule_to_stop_state = vec![usize::MAX; n_rules];
        for (i, state) in states.iter().enumerate() {
            if state.kind == StateKind::RuleStop {
                let slot = rule_to_stop_state
                    .get_mut(state.rule_index)
                    .ok_or_else(|| AtnError::new("rule stop state has an invalid rule index"))?;
                *slot = i;
            }
        }
        if rule_to_stop_state.contains(&usize::MAX) {
            return Err(AtnError::new("a rule has no stop state"));
        }

        // Modes
        let mut mode_to_start_state = Vec::new();
        for _ in 0..r.next_usize()? {
            mode_to_start_state.push(check_state(r.next_usize()?)?);
        }

        // Sets
        let mut sets = Vec::new();
        for _ in 0..r.next_usize()? {
            let n_intervals = r.next_usize()?;
            let mut set = IntervalSet::new();
            if r.next()? != 0 {
                set.add(EOF);
            }
            for _ in 0..n_intervals {
                let a = r.next()?;
                let b = r.next()?;
                set.add_range(a, b);
            }
            sets.push(set);
        }

        // Edges
        for _ in 0..r.next_usize()? {
            let src = check_state(r.next_usize()?)?;
            let trg = check_state(r.next_usize()?)?;
            let kind = r.next()?;
            let arg1 = r.next()?;
            let arg2 = r.next()?;
            let arg3 = r.next()?;
            let as_usize = |v: i32| {
                usize::try_from(v)
                    .map_err(|_| AtnError::new(format!("unexpected negative value {v}")))
            };
            let transition = match kind {
                1 => Transition::Epsilon {
                    target: trg,
                    outermost_precedence_return: None,
                },
                2 => Transition::Range {
                    target: trg,
                    from: if arg3 != 0 { EOF } else { arg1 },
                    to: arg2,
                },
                3 => Transition::Rule {
                    target: check_state(as_usize(arg1)?)?,
                    rule_index: as_usize(arg2)?,
                    precedence: arg3,
                    follow_state: trg,
                },
                4 => Transition::Predicate {
                    target: trg,
                    rule_index: as_usize(arg1)?,
                    pred_index: as_usize(arg2)?,
                    ctx_dependent: arg3 != 0,
                },
                5 => Transition::Atom {
                    target: trg,
                    label: if arg3 != 0 { EOF } else { arg1 },
                },
                6 => Transition::Action {
                    target: trg,
                    rule_index: as_usize(arg1)?,
                    action_index: usize::try_from(arg2).ok(),
                    ctx_dependent: arg3 != 0,
                },
                7 | 8 => {
                    let set = as_usize(arg1)?;
                    if set >= sets.len() {
                        return Err(AtnError::new(format!("set {set} is out of range")));
                    }
                    if kind == 7 {
                        Transition::Set { target: trg, set }
                    } else {
                        Transition::NotSet { target: trg, set }
                    }
                }
                9 => Transition::Wildcard { target: trg },
                10 => Transition::Precedence {
                    target: trg,
                    precedence: arg1,
                },
                _ => return Err(AtnError::new(format!("invalid transition type {kind}"))),
            };
            states[src].add_transition(transition);
        }

        // Edges out of rule stop states are derived from the rule transitions invoking the rule.
        let mut return_edges = Vec::new();
        for state in &states {
            for transition in &state.transitions {
                if let Transition::Rule {
                    target,
                    precedence,
                    follow_state,
                    ..
                } = *transition
                {
                    let rule_index = states[target].rule_index;
                    let outermost_precedence_return = (states[rule_to_start_state[rule_index]]
                        .is_left_recursive_rule
                        && precedence == 0)
                        .then_some(rule_index);
                    return_edges.push((
                        rule_to_stop_state[rule_index],
                        Transition::Epsilon {
                            target: follow_state,
                            outermost_precedence_return,
                        },
                    ));
                }
            }
        }
        for (stop_state, transition) in return_edges {
            states[stop_state].add_transition(transition);
        }

        for i in 0..states.len() {
            if states[i].kind.is_block_start() {
                let end = states[i]
                    .end_state
                    .ok_or_else(|| AtnError::new("block start state has no end state"))?;
                if states[end].kind != StateKind::BlockEnd || states[end].start_state.is_some() {
                    return Err(AtnError::new("block end state is shared or invalid"));
                }
                states[end].start_state = Some(i);
            }
            let loop_back_kind = match states[i].kind {
                StateKind::PlusLoopBack => StateKind::PlusBlockStart,
                StateKind::StarLoopBack => StateKind::StarLoopEntry,
                _ => continue,
            };
            let targets: Vec<usize> = states[i]
                .transitions
                .iter()
                .map(Transition::target)
                .collect();
            for target in targets {
                if states[target].kind == loop_back_kind {
                    states[target].loop_back_state = Some(i);
                }
            }
        }

        // Decisions
        let n_decisions = r.next_usize()?;
        let mut decision_to_state = Vec::with_capacity(n_decisions);
        for decision in 0..n_decisions {
            let s = check_state(r.next_usize()?)?;
            if !states[s].kind.is_decision() {
                return Err(AtnError::new(format!("state {s} is not a decision state")));
            }
            states[s].decision = Some(decision);
            decision_to_state.push(s);
        }

        // Lexer actions
        let mut lexer_actions = Vec::new();
        if grammar_type == GrammarType::Lexer {
            for _ in 0..r.next_usize()? {
                let kind = r.next()?;
                let data1 = r.next()?;
                let data2 = r.next()?;
                let as_usize = |v: i32| {
                    usize::try_from(v)
                        .map_err(|_| AtnError::new(format!("unexpected negative value {v}")))
                };
                lexer_actions.push(match kind {
                    0 => LexerAction::Channel(data1),
                    1 => LexerAction::Custom {
                        rule_index: as_usize(data1)?,
                        action_index: as_usize(data2)?,
                    },
                    2 => LexerAction::Mode(as_usize(data1)?),
                    3 => LexerAction::More,
                    4 => LexerAction::PopMode,
                    5 => LexerAction::PushMode(as_usize(data1)?),
                    6 => LexerAction::Skip,
                    7 => LexerAction::Type(data1),
                    _ => return Err(AtnError::new(format!("invalid lexer action type {kind}"))),
                });
            }
        }

        let mut atn = Self {
            grammar_type,
            max_token_type,
            states,
            sets,
            rule_to_start_state,
            rule_to_stop_state,
            rule_to_token_type,
            mode_to_start_state,
            decision_to_state,
            lexer_actions,
        };
        atn.mark_precedence_decisions();
        atn.verify()?;
        Ok(atn)
    }

    /// Marks the star loop entry states that decide whether a precedence rule continues or completes.
    fn mark_precedence_decisions(&mut self) {
        for i in 0..self.states.len() {
            let state = &self.states[i];
            if state.kind != StateKind::StarLoopEntry
                || !self.states[self.rule_to_start_state[state.rule_index]].is_left_recursive_rule
            {
                continue;
            }
            let Some(last) = state.transitions.last() else {
                continue;
            };
            let loop_end = &self.states[last.target()];
            if loop_end.kind == StateKind::LoopEnd
                && loop_end.epsilon_only
                && self.states[loop_end.transitions[0].target()].kind == StateKind::RuleStop
            {
                self.states[i].is_precedence_decision = true;
            }
        }
    }

    fn verify(&self) -> Result<(), AtnError> {
        let check = |condition: bool, message: &str| {
            if condition {
                Ok(())
            } else {
                Err(AtnError::new(message))
            }
        };
        for state in &self.states {
            if state.kind == StateKind::Invalid {
                continue;
            }
            check(
                state.epsilon_only || state.transitions.len() <= 1,
                "a state mixes epsilon and non-epsilon transitions",
            )?;
            match state.kind {
                StateKind::PlusBlockStart | StateKind::LoopEnd => {
                    check(
                        state.loop_back_state.is_some(),
                        "a loop has no loop back state",
                    )?;
                }
                StateKind::StarLoopEntry => {
                    check(
                        state.loop_back_state.is_some(),
                        "a loop has no loop back state",
                    )?;
                    check(
                        state.transitions.len() == 2,
                        "a star loop entry needs two transitions",
                    )?;
                }
                StateKind::StarLoopBack => {
                    check(
                        state.transitions.len() == 1
                            && self.states[state.transitions[0].target()].kind
                                == StateKind::StarLoopEntry,
                        "a star loop back state must return to its entry",
                    )?;
                }
                StateKind::BlockEnd => {
                    check(
                        state.start_state.is_some(),
                        "a block end state has no start state",
                    )?;
                }
                _ => {}
            }
            if state.kind.is_decision() {
                check(
                    state.transitions.len() <= 1 || state.decision.is_some(),
                    "a decision state has no decision number",
                )?;
            } else {
                check(
                    state.transitions.len() <= 1 || state.kind == StateKind::RuleStop,
                    "a non-decision state has several transitions",
                )?;
            }
        }
        Ok(())
    }

    /// The token types (or code points) the ATN can match next from `state` within its rule;
    /// includes `EPSILON` when the end of the rule is reachable.
    pub fn next_tokens(&self, state: usize) -> IntervalSet {
        crate::ll1::look(self, state)
    }
}
