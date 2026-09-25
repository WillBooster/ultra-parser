//! LL(1) lookahead analysis, ported from ANTLR's `LL1Analyzer`.

use std::collections::HashSet;

use crate::atn::{Atn, StateKind, Transition};
use crate::interval_set::IntervalSet;
use crate::token::{EOF, EPSILON, MIN_USER_TOKEN_TYPE};

/// Computes the symbols that can follow `state` within its rule; adds `EPSILON` when the end of
/// the rule is reachable.
pub(crate) fn look(atn: &Atn, state: usize) -> IntervalSet {
    analyze(atn, state, Vec::new(), false)
}

/// Computes the symbols that can follow `state` when the current rule was invoked from the rule
/// invocations with the given follow states (innermost first); adds `EOF` when the end of the
/// outermost rule is reachable.
pub(crate) fn look_in_context(atn: &Atn, state: usize, follow_states: &[usize]) -> IntervalSet {
    analyze(
        atn,
        state,
        follow_states.iter().rev().copied().collect(),
        true,
    )
}

fn analyze(atn: &Atn, state: usize, stack: Vec<usize>, in_context: bool) -> IntervalSet {
    let mut analyzer = Analyzer {
        atn,
        look: IntervalSet::new(),
        busy: HashSet::new(),
        called_rules: vec![false; atn.rule_to_start_state.len()],
        stack,
        in_context,
    };
    analyzer.visit(state);
    analyzer.look
}

struct Analyzer<'a> {
    atn: &'a Atn,
    look: IntervalSet,
    busy: HashSet<(usize, Vec<usize>)>,
    called_rules: Vec<bool>,
    /// Follow states of the rule invocations, outermost first.
    stack: Vec<usize>,
    /// Whether the stack started with the outer context, whose end is followed by `EOF`.
    in_context: bool,
}

impl Analyzer<'_> {
    fn visit(&mut self, s: usize) {
        if !self.busy.insert((s, self.stack.clone())) {
            return;
        }
        let atn = self.atn;
        let state = &atn.states[s];
        if state.kind == StateKind::RuleStop {
            let Some(return_state) = self.stack.pop() else {
                self.look.add(if self.in_context { EOF } else { EPSILON });
                return;
            };
            let removed = std::mem::replace(&mut self.called_rules[state.rule_index], false);
            self.visit(return_state);
            self.called_rules[state.rule_index] = removed;
            self.stack.push(return_state);
            return;
        }
        for transition in &state.transitions {
            match *transition {
                Transition::Rule {
                    target,
                    follow_state,
                    ..
                } => {
                    let rule_index = atn.states[target].rule_index;
                    if self.called_rules[rule_index] {
                        continue;
                    }
                    self.called_rules[rule_index] = true;
                    self.stack.push(follow_state);
                    self.visit(target);
                    self.stack.pop();
                    self.called_rules[rule_index] = false;
                }
                Transition::Predicate { target, .. }
                | Transition::Precedence { target, .. }
                | Transition::Epsilon { target, .. }
                | Transition::Action { target, .. } => self.visit(target),
                Transition::Wildcard { .. } => {
                    self.look.add_range(MIN_USER_TOKEN_TYPE, atn.max_token_type)
                }
                Transition::NotSet { set, .. } => {
                    let complement =
                        atn.sets[set].complement(MIN_USER_TOKEN_TYPE, atn.max_token_type);
                    self.look.add_set(&complement);
                }
                _ => {
                    if let Some(label) = transition.label(atn) {
                        self.look.add_set(&label);
                    }
                }
            }
        }
    }
}
