//! LL(1) lookahead analysis, ported from ANTLR's `LL1Analyzer`.
//!
//! The analysis follows rule invocation stacks as deep as the input is nested, so it runs on an
//! explicit work list instead of recursing, and it interns the stacks so that each one is an id.

use std::collections::{HashMap, HashSet};

use crate::atn::{Atn, StateKind, Transition};
use crate::interval_set::IntervalSet;
use crate::token::{EOF, EPSILON, MIN_USER_TOKEN_TYPE};

/// Computes the symbols that can follow `state` within its rule; adds `EPSILON` when the end of
/// the rule is reachable.
pub(crate) fn look(atn: &Atn, state: usize) -> IntervalSet {
    analyze(atn, state, &[], false)
}

/// Computes the symbols that can follow `state` when the current rule was invoked from the rule
/// invocations with the given follow states (innermost first); adds `EOF` when the end of the
/// outermost rule is reachable.
pub(crate) fn look_in_context(atn: &Atn, state: usize, follow_states: &[usize]) -> IntervalSet {
    analyze(atn, state, follow_states, true)
}

/// The id of the empty stack.
const EMPTY_STACK: usize = 0;

/// Rule invocation stacks as a tree of interned nodes: node `i` pushes a follow state onto a
/// parent stack, so equal stacks share one id.
struct Stacks {
    /// The parent stack and follow state of each node; entry 0 is the empty stack.
    nodes: Vec<(usize, usize)>,
    ids: HashMap<(usize, usize), usize>,
}

impl Stacks {
    fn push(&mut self, parent: usize, follow_state: usize) -> usize {
        *self.ids.entry((parent, follow_state)).or_insert_with(|| {
            self.nodes.push((parent, follow_state));
            self.nodes.len() - 1
        })
    }
}

enum Work {
    Visit {
        state: usize,
        stack: usize,
    },
    /// Enters a rule unless it is already being analyzed on this path.
    EnterRule {
        rule_index: usize,
        state: usize,
        stack: usize,
    },
    /// Returns from a rule to a follow state, allowing the rule to be entered again meanwhile.
    ReturnFromRule {
        rule_index: usize,
        state: usize,
        stack: usize,
    },
    RestoreCalledRule {
        rule_index: usize,
        called: bool,
    },
}

fn analyze(atn: &Atn, state: usize, follow_states: &[usize], in_context: bool) -> IntervalSet {
    let mut stacks = Stacks {
        nodes: vec![(EMPTY_STACK, 0)],
        ids: HashMap::new(),
    };
    let mut stack = EMPTY_STACK;
    for &follow_state in follow_states.iter().rev() {
        stack = stacks.push(stack, follow_state);
    }
    let mut look = IntervalSet::new();
    let mut busy = HashSet::new();
    let mut called_rules = vec![false; atn.rule_to_start_state.len()];
    // Items are pushed in reverse so that they run in the order ANTLR's recursion visits them;
    // the order matters because `busy` keeps the first visit of each state and stack.
    let mut work = vec![Work::Visit { state, stack }];
    while let Some(item) = work.pop() {
        let (s, stack) = match item {
            Work::Visit { state, stack } => (state, stack),
            Work::EnterRule {
                rule_index,
                state,
                stack,
            } => {
                if !called_rules[rule_index] {
                    called_rules[rule_index] = true;
                    work.push(Work::RestoreCalledRule {
                        rule_index,
                        called: false,
                    });
                    work.push(Work::Visit { state, stack });
                }
                continue;
            }
            Work::ReturnFromRule {
                rule_index,
                state,
                stack,
            } => {
                let called = std::mem::replace(&mut called_rules[rule_index], false);
                work.push(Work::RestoreCalledRule { rule_index, called });
                work.push(Work::Visit { state, stack });
                continue;
            }
            Work::RestoreCalledRule { rule_index, called } => {
                called_rules[rule_index] = called;
                continue;
            }
        };
        if !busy.insert((s, stack)) {
            continue;
        }
        let state = &atn.states[s];
        if state.kind == StateKind::RuleStop {
            if stack == EMPTY_STACK {
                look.add(if in_context { EOF } else { EPSILON });
            } else {
                let (parent, follow_state) = stacks.nodes[stack];
                work.push(Work::ReturnFromRule {
                    rule_index: state.rule_index,
                    state: follow_state,
                    stack: parent,
                });
            }
            continue;
        }
        for transition in state.transitions.iter().rev() {
            match *transition {
                Transition::Rule {
                    target,
                    follow_state,
                    ..
                } => work.push(Work::EnterRule {
                    rule_index: atn.states[target].rule_index,
                    state: target,
                    stack: stacks.push(stack, follow_state),
                }),
                Transition::Predicate { target, .. }
                | Transition::Precedence { target, .. }
                | Transition::Epsilon { target, .. }
                | Transition::Action { target, .. } => work.push(Work::Visit {
                    state: target,
                    stack,
                }),
                Transition::Wildcard { .. } => {
                    look.add_range(MIN_USER_TOKEN_TYPE, atn.max_token_type)
                }
                Transition::NotSet { set, .. } => {
                    look.add_set(
                        &atn.sets[set].complement(MIN_USER_TOKEN_TYPE, atn.max_token_type),
                    );
                }
                _ => {
                    if let Some(label) = transition.label(atn) {
                        look.add_set(&label);
                    }
                }
            }
        }
    }
    look
}
