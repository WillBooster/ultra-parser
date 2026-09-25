//! Adaptive LL(*) prediction, ported from ANTLR's `ParserATNSimulator` (prediction mode `LL`)
//! without DFA caching: SLL prediction first, then full-context LL prediction on SLL conflicts.

use std::collections::{HashMap, HashSet};

use crate::atn::{Atn, INVALID_ALT, StateKind, Transition};
use crate::config::{AltSet, Config, ConfigSet};
use crate::context::{Ctx, EMPTY_RETURN_STATE, PredictionContext};
use crate::semantic::SemanticContext;
use crate::token::{EOF, EPSILON, TokenStream};
use crate::tree::{NodeId, ParseTree, RuleNode};

/// No alternative of a decision matches the input.
pub(crate) struct NoViableAlt {
    /// Index of the first token of the decision.
    pub(crate) start_token: usize,
    /// Index of the token where prediction failed.
    pub(crate) offending_token: usize,
}

/// The parser state that prediction depends on.
pub(crate) struct Outer<'o> {
    /// The rule contexts built so far.
    pub(crate) nodes: &'o [RuleNode],
    /// The current rule context, whose invocation stack full-context prediction starts from.
    pub(crate) ctx: NodeId,
    /// The precedence of the innermost left-recursive rule invocation, or -1.
    pub(crate) precedence: i32,
}

pub(crate) fn adaptive_predict(
    atn: &Atn,
    input: &mut TokenStream<'_>,
    decision: usize,
    outer: &Outer<'_>,
) -> Result<usize, NoViableAlt> {
    let decision_state = atn.decision_to_state[decision];
    let mut simulator = Simulator {
        atn,
        input,
        start_index: 0,
        outer,
        decision_state,
        is_precedence_decision: atn.states[decision_state].is_precedence_decision,
    };
    simulator.start_index = simulator.input.index();
    let mut s0 = simulator.compute_start_state(decision_state, &[], false);
    if simulator.is_precedence_decision {
        s0 = simulator.apply_precedence_filter(&s0);
    }
    let result = simulator.exec_atn(s0);
    let start_index = simulator.start_index;
    input.seek(start_index);
    result
}

/// The outcome of one SLL prediction step (a DFA state in ANTLR).
struct Step {
    configs: ConfigSet,
    is_accept: bool,
    prediction: usize,
    requires_full_context: bool,
    predicates: Option<Vec<(SemanticContext, usize)>>,
}

struct Simulator<'a, 'i, 't, 'o> {
    atn: &'a Atn,
    input: &'i mut TokenStream<'t>,
    start_index: usize,
    outer: &'o Outer<'o>,
    decision_state: usize,
    is_precedence_decision: bool,
}

impl Simulator<'_, '_, '_, '_> {
    fn exec_atn(&mut self, s0: ConfigSet) -> Result<usize, NoViableAlt> {
        let mut previous = s0;
        let mut t = self.input.la(1);
        loop {
            let Some(reach) = self.compute_reach_set(&previous, t, false) else {
                return self.no_viable_alt_or_finished_alt(&previous);
            };
            let step = self.compute_step(reach);
            if step.requires_full_context {
                if let Some(predicates) = &step.predicates {
                    let alts = self.eval_predicates(predicates, true);
                    if alts.len() == 1 {
                        return Ok(alts.min());
                    }
                }
                // Only full-context prediction needs the invocation stack, which is as long as
                // the input is nested.
                let invoking_states = ParseTree::invoking_states(self.outer.nodes, self.outer.ctx);
                let s0 = self.compute_start_state(self.decision_state, &invoking_states, true);
                return self.exec_atn_with_full_context(s0);
            }
            if step.is_accept {
                let Some(predicates) = &step.predicates else {
                    return Ok(step.prediction);
                };
                self.input.seek(self.start_index);
                let alts = self.eval_predicates(predicates, true);
                if alts.len() == 0 {
                    return Err(self.no_viable_alt());
                }
                return Ok(alts.min());
            }
            previous = step.configs;
            if t != EOF {
                self.input.consume();
                t = self.input.la(1);
            }
        }
    }

    fn compute_step(&self, mut configs: ConfigSet) -> Step {
        let mut step_prediction = INVALID_ALT;
        let mut is_accept = false;
        let mut requires_full_context = false;
        let predicted = unique_alt(&configs);
        if predicted != INVALID_ALT {
            is_accept = true;
            configs.unique_alt = predicted;
            step_prediction = predicted;
        } else if has_sll_conflict_terminating_prediction(self.atn, &configs) {
            let conflicting = conflicting_alts(&configs);
            step_prediction = conflicting.min();
            configs.conflicting_alts = Some(conflicting);
            requires_full_context = true;
            is_accept = true;
        }
        let mut step = Step {
            configs,
            is_accept,
            prediction: step_prediction,
            requires_full_context,
            predicates: None,
        };
        if step.is_accept && step.configs.has_semantic_context {
            self.predicate_step(&mut step);
        }
        step
    }

    fn predicate_step(&self, step: &mut Step) {
        let n_alts = self.atn.states[self.decision_state].transitions.len();
        let alts = if step.configs.unique_alt != INVALID_ALT {
            AltSet::of(step.configs.unique_alt)
        } else {
            step.configs.conflicting_alts.clone().unwrap_or_default()
        };
        match preds_for_ambiguous_alts(&alts, &step.configs, n_alts) {
            Some(alt_to_pred) => {
                step.predicates = predicate_predictions(&alts, &alt_to_pred);
                step.prediction = INVALID_ALT;
            }
            None => step.prediction = alts.min(),
        }
    }

    fn exec_atn_with_full_context(&mut self, s0: ConfigSet) -> Result<usize, NoViableAlt> {
        let mut previous = s0;
        self.input.seek(self.start_index);
        let mut t = self.input.la(1);
        loop {
            let Some(reach) = self.compute_reach_set(&previous, t, true) else {
                return self.no_viable_alt_or_finished_alt(&previous);
            };
            let predicted = unique_alt(&reach);
            if predicted != INVALID_ALT {
                return Ok(predicted);
            }
            let predicted = single_viable_alt(&conflicting_alt_subsets(&reach));
            if predicted != INVALID_ALT {
                return Ok(predicted);
            }
            previous = reach;
            if t != EOF {
                self.input.consume();
                t = self.input.la(1);
            }
        }
    }

    fn no_viable_alt(&self) -> NoViableAlt {
        NoViableAlt {
            start_token: self.start_index,
            offending_token: self.input.index(),
        }
    }

    fn no_viable_alt_or_finished_alt(
        &mut self,
        previous: &ConfigSet,
    ) -> Result<usize, NoViableAlt> {
        let error = self.no_viable_alt();
        self.input.seek(self.start_index);
        let alt = self.syn_valid_or_sem_invalid_alt_that_finished_decision_entry_rule(previous);
        if alt != INVALID_ALT {
            Ok(alt)
        } else {
            Err(error)
        }
    }

    fn compute_reach_set(&self, closure: &ConfigSet, t: i32, full_ctx: bool) -> Option<ConfigSet> {
        let mut intermediate = ConfigSet::new(full_ctx);
        let mut skipped_stop_states = Vec::new();
        for c in closure {
            if self.atn.states[c.state].kind == StateKind::RuleStop {
                if full_ctx || t == EOF {
                    skipped_stop_states.push(c.clone());
                }
                continue;
            }
            for transition in &self.atn.states[c.state].transitions {
                if transition.matches(self.atn, t, 0, self.atn.max_token_type) {
                    intermediate.add(c.with_state(transition.target()));
                }
            }
        }

        let use_intermediate = skipped_stop_states.is_empty()
            && t != EOF
            && (intermediate.len() == 1 || unique_alt(&intermediate) != INVALID_ALT);
        let mut reach = if use_intermediate {
            intermediate
        } else {
            let mut reach = ConfigSet::new(full_ctx);
            let mut busy = HashSet::new();
            for c in &intermediate {
                self.closure(c.clone(), &mut reach, &mut busy, false, full_ctx, t == EOF);
            }
            reach
        };
        if t == EOF {
            reach = self.remove_all_configs_not_in_rule_stop_state(reach, use_intermediate);
        }
        if !skipped_stop_states.is_empty()
            && (!full_ctx || !self.has_config_in_rule_stop_state(&reach))
        {
            for c in skipped_stop_states {
                reach.add(c);
            }
        }
        if reach.is_empty() { None } else { Some(reach) }
    }

    fn remove_all_configs_not_in_rule_stop_state(
        &self,
        configs: ConfigSet,
        look_to_end_of_rule: bool,
    ) -> ConfigSet {
        if configs
            .iter()
            .all(|c| self.atn.states[c.state].kind == StateKind::RuleStop)
        {
            return configs;
        }
        let mut result = ConfigSet::new(configs.full_ctx);
        for c in &configs {
            let state = &self.atn.states[c.state];
            if state.kind == StateKind::RuleStop {
                result.add(c.clone());
            } else if look_to_end_of_rule
                && state.epsilon_only
                && self.atn.next_tokens(c.state).contains(EPSILON)
            {
                result.add(c.with_state(self.atn.rule_to_stop_state[state.rule_index]));
            }
        }
        result
    }

    fn has_config_in_rule_stop_state(&self, configs: &ConfigSet) -> bool {
        configs
            .iter()
            .any(|c| self.atn.states[c.state].kind == StateKind::RuleStop)
    }

    fn compute_start_state(
        &self,
        p: usize,
        invoking_states: &[usize],
        full_ctx: bool,
    ) -> ConfigSet {
        let initial_context = PredictionContext::from_invoking_states(self.atn, invoking_states);
        let mut configs = ConfigSet::new(full_ctx);
        for (i, transition) in self.atn.states[p].transitions.iter().enumerate() {
            let config = Config::new(transition.target(), i + 1, initial_context.clone());
            let mut busy = HashSet::new();
            self.closure(config, &mut configs, &mut busy, true, full_ctx, false);
        }
        configs
    }

    /// Removes the configurations of alternatives other than 1 that the precedence of the current
    /// rule invocation rules out, as ANTLR's `applyPrecedenceFilter` does.
    fn apply_precedence_filter(&self, configs: &ConfigSet) -> ConfigSet {
        let mut states_from_alt1: HashMap<usize, Ctx> = HashMap::new();
        let mut result = ConfigSet::new(configs.full_ctx);
        for c in configs.iter().filter(|c| c.alt == 1) {
            let Some(updated) = c.semantic.eval_precedence(self.outer.precedence) else {
                continue;
            };
            states_from_alt1.insert(c.state, c.context.clone());
            result.add(Config {
                semantic: updated,
                ..c.clone()
            });
        }
        for c in configs.iter().filter(|c| c.alt != 1) {
            if !c.precedence_filter_suppressed
                && states_from_alt1
                    .get(&c.state)
                    .is_some_and(|ctx| *ctx == c.context)
            {
                continue;
            }
            result.add(c.clone());
        }
        result
    }

    fn syn_valid_or_sem_invalid_alt_that_finished_decision_entry_rule(
        &self,
        configs: &ConfigSet,
    ) -> usize {
        let mut valid = ConfigSet::new(configs.full_ctx);
        let mut invalid = ConfigSet::new(configs.full_ctx);
        for c in configs {
            if c.semantic == SemanticContext::Empty || c.semantic.eval(self.outer.precedence) {
                valid.add(c.clone());
            } else {
                invalid.add(c.clone());
            }
        }
        let alt = self.alt_that_finished_decision_entry_rule(&valid);
        if alt != INVALID_ALT {
            return alt;
        }
        self.alt_that_finished_decision_entry_rule(&invalid)
    }

    fn alt_that_finished_decision_entry_rule(&self, configs: &ConfigSet) -> usize {
        configs
            .iter()
            .filter(|c| {
                c.outer_context_depth > 0
                    || (self.atn.states[c.state].kind == StateKind::RuleStop
                        && c.context.has_empty_path())
            })
            .map(|c| c.alt)
            .min()
            .unwrap_or(INVALID_ALT)
    }

    fn eval_predicates(&self, predicates: &[(SemanticContext, usize)], complete: bool) -> AltSet {
        let mut alts = AltSet::default();
        for (pred, alt) in predicates {
            if *pred == SemanticContext::Empty || pred.eval(self.outer.precedence) {
                alts.insert(*alt);
                if !complete {
                    break;
                }
            }
        }
        alts
    }

    fn closure(
        &self,
        config: Config,
        configs: &mut ConfigSet,
        busy: &mut HashSet<Config>,
        collect_predicates: bool,
        full_ctx: bool,
        treat_eof_as_epsilon: bool,
    ) {
        self.closure_checking_stop_state(
            config,
            configs,
            busy,
            collect_predicates,
            full_ctx,
            0,
            treat_eof_as_epsilon,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn closure_checking_stop_state(
        &self,
        config: Config,
        configs: &mut ConfigSet,
        busy: &mut HashSet<Config>,
        collect_predicates: bool,
        full_ctx: bool,
        depth: i32,
        treat_eof_as_epsilon: bool,
    ) {
        if self.atn.states[config.state].kind == StateKind::RuleStop {
            if !config.context.is_empty() {
                let context = config.context.clone();
                for i in 0..context.len() {
                    let return_state = context.return_state(i);
                    if return_state == EMPTY_RETURN_STATE {
                        if full_ctx {
                            configs.add(
                                config.with_state_and_context(
                                    config.state,
                                    PredictionContext::empty(),
                                ),
                            );
                        } else {
                            // Reached the end of the decision rule in SLL mode: follow the rule's
                            // return edges into any outer context.
                            self.closure_(
                                config.clone(),
                                configs,
                                busy,
                                collect_predicates,
                                full_ctx,
                                depth,
                                treat_eof_as_epsilon,
                            );
                        }
                        continue;
                    }
                    let parent = context.parent(i).expect("non-empty return state").clone();
                    let c = config.with_state_and_context(return_state, parent);
                    self.closure_checking_stop_state(
                        c,
                        configs,
                        busy,
                        collect_predicates,
                        full_ctx,
                        depth - 1,
                        treat_eof_as_epsilon,
                    );
                }
                return;
            } else if full_ctx {
                configs.add(config);
                return;
            }
        }
        self.closure_(
            config,
            configs,
            busy,
            collect_predicates,
            full_ctx,
            depth,
            treat_eof_as_epsilon,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn closure_(
        &self,
        config: Config,
        configs: &mut ConfigSet,
        busy: &mut HashSet<Config>,
        collect_predicates: bool,
        full_ctx: bool,
        depth: i32,
        treat_eof_as_epsilon: bool,
    ) {
        let p = &self.atn.states[config.state];
        if !p.epsilon_only {
            configs.add(config.clone());
        }
        for (i, transition) in p.transitions.iter().enumerate() {
            if i == 0 && self.can_drop_loop_entry_edge_in_left_recursive_rule(&config) {
                continue;
            }
            let continue_collecting =
                !matches!(transition, Transition::Action { .. }) && collect_predicates;
            let Some(mut c) = self.epsilon_target(
                &config,
                transition,
                continue_collecting,
                depth == 0,
                full_ctx,
                treat_eof_as_epsilon,
            ) else {
                continue;
            };
            let mut new_depth = depth;
            if p.kind == StateKind::RuleStop {
                // Leaving the decision rule for the outer context (SLL only).
                if self.is_precedence_decision
                    && let Transition::Epsilon {
                        outermost_precedence_return: Some(rule_index),
                        ..
                    } = *transition
                    && rule_index == self.atn.states[self.decision_state].rule_index
                {
                    c.precedence_filter_suppressed = true;
                }
                c.outer_context_depth += 1;
                if !busy.insert(c.clone()) {
                    continue;
                }
                configs.dips_into_outer_context = true;
                new_depth -= 1;
            } else {
                if !transition.is_epsilon() && !busy.insert(c.clone()) {
                    continue;
                }
                if matches!(transition, Transition::Rule { .. }) && new_depth >= 0 {
                    new_depth += 1;
                }
            }
            self.closure_checking_stop_state(
                c,
                configs,
                busy,
                continue_collecting,
                full_ctx,
                new_depth,
                treat_eof_as_epsilon,
            );
        }
    }

    /// Implements ANTLR's optimization that skips re-entering the loop of a left-recursive rule
    /// when every stack returns to the same loop (`canDropLoopEntryEdgeInLeftRecursiveRule`).
    fn can_drop_loop_entry_edge_in_left_recursive_rule(&self, config: &Config) -> bool {
        let atn = self.atn;
        let p = &atn.states[config.state];
        if p.kind != StateKind::StarLoopEntry
            || !p.is_precedence_decision
            || config.context.is_empty()
            || config.context.has_empty_path()
        {
            return false;
        }
        let n = config.context.len();
        for i in 0..n {
            if atn.states[config.context.return_state(i)].rule_index != p.rule_index {
                return false;
            }
        }
        let block_start = p.transitions[0].target();
        let block_end = atn.states[block_start]
            .end_state
            .expect("block start has an end state");
        for i in 0..n {
            let return_state_number = config.context.return_state(i);
            let return_state = &atn.states[return_state_number];
            if return_state.transitions.len() != 1 || !return_state.transitions[0].is_epsilon() {
                return false;
            }
            let return_target = return_state.transitions[0].target();
            if return_state.kind == StateKind::BlockEnd && return_target == config.state {
                continue;
            }
            if return_state_number == block_end || return_target == block_end {
                continue;
            }
            let target_state = &atn.states[return_target];
            if target_state.kind == StateKind::BlockEnd
                && target_state.transitions.len() == 1
                && target_state.transitions[0].is_epsilon()
                && target_state.transitions[0].target() == config.state
            {
                continue;
            }
            return false;
        }
        true
    }

    fn epsilon_target(
        &self,
        config: &Config,
        transition: &Transition,
        collect_predicates: bool,
        in_context: bool,
        full_ctx: bool,
        treat_eof_as_epsilon: bool,
    ) -> Option<Config> {
        match *transition {
            Transition::Rule {
                target,
                follow_state,
                ..
            } => {
                let context =
                    PredictionContext::singleton(Some(config.context.clone()), follow_state);
                Some(config.with_state_and_context(target, context))
            }
            Transition::Precedence { target, precedence } => {
                if collect_predicates && in_context {
                    let predicate = SemanticContext::Precedence(precedence);
                    if full_ctx {
                        predicate
                            .eval(self.outer.precedence)
                            .then(|| config.with_state(target))
                    } else {
                        Some(Config {
                            state: target,
                            semantic: SemanticContext::and(&config.semantic, &predicate),
                            ..config.clone()
                        })
                    }
                } else {
                    Some(config.with_state(target))
                }
            }
            Transition::Predicate {
                target,
                rule_index,
                pred_index,
                ctx_dependent,
            } => {
                // In full-context mode the predicate would be evaluated here; the runtime does
                // not run grammar code, so it always succeeds.
                if collect_predicates && (!ctx_dependent || in_context) && !full_ctx {
                    let predicate = SemanticContext::Predicate {
                        rule_index,
                        pred_index,
                        ctx_dependent,
                    };
                    Some(Config {
                        state: target,
                        semantic: SemanticContext::and(&config.semantic, &predicate),
                        ..config.clone()
                    })
                } else {
                    Some(config.with_state(target))
                }
            }
            Transition::Action { target, .. } | Transition::Epsilon { target, .. } => {
                Some(config.with_state(target))
            }
            Transition::Atom { target, .. }
            | Transition::Range { target, .. }
            | Transition::Set { target, .. } => (treat_eof_as_epsilon
                && transition.matches(self.atn, EOF, 0, 1))
            .then(|| config.with_state(target)),
            Transition::NotSet { .. } | Transition::Wildcard { .. } => None,
        }
    }
}

fn unique_alt(configs: &ConfigSet) -> usize {
    let mut alt = INVALID_ALT;
    for c in configs {
        if alt == INVALID_ALT {
            alt = c.alt;
        } else if c.alt != alt {
            return INVALID_ALT;
        }
    }
    alt
}

/// Groups the alternatives of configurations by their state and context.
fn conflicting_alt_subsets(configs: &ConfigSet) -> Vec<AltSet> {
    let mut index: HashMap<(usize, &Ctx), usize> = HashMap::new();
    let mut subsets: Vec<AltSet> = Vec::new();
    for c in configs {
        let i = *index.entry((c.state, &c.context)).or_insert_with(|| {
            subsets.push(AltSet::default());
            subsets.len() - 1
        });
        subsets[i].insert(c.alt);
    }
    subsets
}

fn conflicting_alts(configs: &ConfigSet) -> AltSet {
    let mut alts = AltSet::default();
    for subset in conflicting_alt_subsets(configs) {
        alts.union_with(&subset);
    }
    alts
}

fn has_sll_conflict_terminating_prediction(atn: &Atn, configs: &ConfigSet) -> bool {
    if configs
        .iter()
        .all(|c| atn.states[c.state].kind == StateKind::RuleStop)
    {
        return true;
    }
    let has_conflicting_alt_set = conflicting_alt_subsets(configs)
        .iter()
        .any(|alts| alts.len() > 1);
    has_conflicting_alt_set && !has_state_associated_with_one_alt(configs)
}

fn has_state_associated_with_one_alt(configs: &ConfigSet) -> bool {
    let mut state_to_alts: HashMap<usize, AltSet> = HashMap::new();
    for c in configs {
        state_to_alts.entry(c.state).or_default().insert(c.alt);
    }
    state_to_alts.values().any(|alts| alts.len() == 1)
}

fn single_viable_alt(subsets: &[AltSet]) -> usize {
    let mut viable = AltSet::default();
    for alts in subsets {
        viable.insert(alts.min());
        if viable.len() > 1 {
            return INVALID_ALT;
        }
    }
    viable.min()
}

fn preds_for_ambiguous_alts(
    alts: &AltSet,
    configs: &ConfigSet,
    n_alts: usize,
) -> Option<Vec<SemanticContext>> {
    let mut alt_to_pred: Vec<Option<SemanticContext>> = vec![None; n_alts + 1];
    for c in configs {
        if alts.contains(c.alt) {
            let pred = &mut alt_to_pred[c.alt];
            *pred = Some(match pred.take() {
                None => c.semantic.clone(),
                Some(existing) => SemanticContext::or(&existing, &c.semantic),
            });
        }
    }
    let alt_to_pred: Vec<SemanticContext> = alt_to_pred
        .into_iter()
        .map(|p| p.unwrap_or(SemanticContext::Empty))
        .collect();
    let n_pred_alts = alt_to_pred[1..]
        .iter()
        .filter(|p| **p != SemanticContext::Empty)
        .count();
    (n_pred_alts > 0).then_some(alt_to_pred)
}

fn predicate_predictions(
    alts: &AltSet,
    alt_to_pred: &[SemanticContext],
) -> Option<Vec<(SemanticContext, usize)>> {
    let mut pairs = Vec::new();
    let mut contains_predicate = false;
    for (alt, pred) in alt_to_pred.iter().enumerate().skip(1) {
        if alts.contains(alt) {
            pairs.push((pred.clone(), alt));
        }
        if *pred != SemanticContext::Empty {
            contains_predicate = true;
        }
    }
    contains_predicate.then_some(pairs)
}
