//! The parser interpreter, ported from ANTLR's `ParserInterpreter`, the tree-building parts of
//! `Parser`, and `DefaultErrorStrategy`. Where the generated parsers of ANTLR's code generation
//! targets behave differently from `ParserInterpreter`, it follows the generated parsers: it syncs
//! at the same states and recovers without adding error nodes.

use crate::SyntaxError;
use crate::atn::{Atn, StateKind, Transition};
use crate::interval_set::IntervalSet;
use crate::ll1;
use crate::prediction::{self, Diagnostic, Outer, PredictionHost, PredictionMode};
use crate::token::{
    DEFAULT_CHANNEL, EOF, EPSILON, INVALID_TYPE, MIN_USER_TOKEN_TYPE, Token, TokenStream, Tokens,
    Vocabulary,
};
use crate::tree::{Child, NodeId, ParseTree, RuleNode};

/// Receives what the parser builds and runs the grammar code that the ATN refers to. Rule
/// contexts are identified by their node in the parse tree the parser returns.
///
/// Every method has a default that does nothing, lets predicates succeed, or ignores errors.
#[allow(unused_variables)]
pub trait ParserHost {
    /// The parser entered rule `rule_index` at its start state `state` with a new context `ctx`,
    /// invoked from `invoking_state` in `parent`. The context of a left-recursive rule
    /// (`recursive`) becomes a child of its parent only when the rule returns (see `unroll`).
    #[allow(clippy::too_many_arguments)]
    fn enter_rule(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        invoking_state: Option<usize>,
        rule_index: usize,
        state: usize,
        start_token: usize,
        recursive: bool,
    ) {
    }

    /// Another iteration of a left-recursive rule: the new context `ctx` takes the place of
    /// `previous`, which becomes its first child, invoked from `state` (the rule's start state),
    /// and ends at `previous_stop`.
    fn push_recursion(
        &mut self,
        ctx: NodeId,
        previous: NodeId,
        state: usize,
        previous_stop: Option<usize>,
    ) {
    }

    /// The parser chose alternative `alt` of the block at `state`. Blocks of outermost
    /// alternatives report this, as does the start state of every rule with alternative 1 when
    /// the rule is entered.
    fn outer_alt(&mut self, ctx: NodeId, state: usize, alt: usize) {}

    /// The parser left the rule of `ctx`, which ends at `stop_token`; `error` tells whether the
    /// rule ended early because of a syntax error in it.
    fn exit_rule(&mut self, ctx: NodeId, stop_token: Option<usize>, error: bool) {}

    /// A left-recursive rule returned `ctx`, which ends at `stop_token` and becomes a child of
    /// `parent`; `error` tells whether the rule ended early because of a syntax error in it.
    fn unroll(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        stop_token: Option<usize>,
        error: bool,
    ) {
    }

    /// The parser consumed a token. `state` is the state that matched it, or `None` when error
    /// recovery skipped it; `error` tells whether the token is an error node.
    fn token(&mut self, ctx: NodeId, state: Option<usize>, token_index: usize, error: bool) {}

    /// Error recovery made up `token` where `state` expected it; `add_to_tree` tells whether it
    /// becomes an error node.
    fn conjure(&mut self, ctx: NodeId, state: usize, token: &Token, add_to_tree: bool) {}

    /// The parser passed the action at `state` with the input at `input_index`.
    fn action(&mut self, ctx: NodeId, state: usize, input_index: usize) {}

    /// Evaluates predicate `pred_index` of rule `rule_index` with the input at `input_index`;
    /// `ctx` is `None` for predicates that do not depend on the rule context during prediction.
    fn sempred(
        &mut self,
        ctx: Option<NodeId>,
        rule_index: usize,
        pred_index: usize,
        input_index: usize,
    ) -> bool {
        true
    }

    fn syntax_error(&mut self, error: SyntaxError) {}

    /// A predicate failed while parsing `ctx`; `error` has a default message that does not show
    /// the predicate.
    fn failed_predicate(
        &mut self,
        ctx: NodeId,
        rule_index: usize,
        pred_index: usize,
        error: SyntaxError,
    ) {
        self.syntax_error(error);
    }

    fn diagnostic(&mut self, diagnostic: Diagnostic) {}

    /// The parser has looked at the tokens up to `index`, as far as ANTLR's token stream, which
    /// lexes lazily, would have lexed. Called before reporting errors and at the end of parsing,
    /// so that lexer errors can be reported in the order ANTLR reports them.
    fn fetched(&mut self, index: usize) {}

    /// The prediction mode to use from now on, which grammar code may change while parsing;
    /// `None` keeps the mode parsing started with.
    fn prediction_mode(&self) -> Option<PredictionMode> {
        None
    }

    /// Whether to stop parsing, e.g., because grammar code threw an exception.
    fn is_aborted(&self) -> bool {
        false
    }
}

/// Runs no grammar code and collects syntax errors.
impl ParserHost for Vec<SyntaxError> {
    fn syntax_error(&mut self, error: SyntaxError) {
        self.push(error);
    }
}

enum RecognitionError {
    NoViableAlt {
        start_token: usize,
        offending_token: usize,
    },
    /// The expected tokens are those that can follow `state` in `ctx`.
    InputMismatch {
        offending_token: usize,
        state: usize,
        ctx: NodeId,
    },
    FailedPredicate {
        offending_token: usize,
        /// The rule and predicate index of a user predicate.
        predicate: Option<(usize, usize)>,
        message: String,
    },
}

impl RecognitionError {
    fn offending_token(&self) -> usize {
        match *self {
            Self::NoViableAlt {
                offending_token, ..
            }
            | Self::InputMismatch {
                offending_token, ..
            }
            | Self::FailedPredicate {
                offending_token, ..
            } => offending_token,
        }
    }
}

/// How `recover_inline` got past a token that did not match.
enum Recovered {
    /// The unexpected token was deleted and the next token matched.
    Matched,
    /// The expected token was assumed to be missing.
    Conjured(Token),
}

pub(crate) struct Parser<'a, 't, 'h, H: ParserHost> {
    atn: &'a Atn,
    vocabulary: &'a Vocabulary,
    rule_names: &'a [String],
    input: TokenStream<'t>,
    mode: PredictionMode,
    host: &'h mut H,
    nodes: Vec<RuleNode>,
    conjured_tokens: Vec<Token>,
    ctx: NodeId,
    state: usize,
    precedence_stack: Vec<i32>,
    /// The parent and invoking state of each left-recursive rule invocation being parsed.
    parent_context_stack: Vec<(Option<NodeId>, Option<usize>)>,
    matched_eof: bool,
    // Error strategy state
    error_recovery_mode: bool,
    last_error_index: Option<usize>,
    last_error_states: Option<IntervalSet>,
    /// Where `sync` last saw a state that can be skipped (`nextTokensContext`/`nextTokensState`).
    next_tokens: Option<(NodeId, usize)>,
    /// The `fetched` index last reported to the host.
    reported_fetched: Option<usize>,
}

impl<'a, 't, 'h, H: ParserHost> Parser<'a, 't, 'h, H> {
    pub(crate) fn new(
        atn: &'a Atn,
        vocabulary: &'a Vocabulary,
        rule_names: &'a [String],
        tokens: &'t Tokens,
        mode: PredictionMode,
        host: &'h mut H,
    ) -> Self {
        Self {
            atn,
            vocabulary,
            rule_names,
            input: TokenStream::new(tokens),
            mode,
            host,
            nodes: Vec::new(),
            conjured_tokens: Vec::new(),
            ctx: 0,
            state: 0,
            precedence_stack: Vec::new(),
            parent_context_stack: Vec::new(),
            matched_eof: false,
            error_recovery_mode: false,
            last_error_index: None,
            last_error_states: None,
            next_tokens: None,
            reported_fetched: None,
        }
    }

    /// Tells the host how far the tokens have been fetched, if that changed.
    fn report_fetched(&mut self) {
        let fetched = self.input.fetched();
        if self.reported_fetched != Some(fetched) {
            self.reported_fetched = Some(fetched);
            self.host.fetched(fetched);
        }
    }

    pub(crate) fn parse(mut self, start_rule: usize) -> ParseTree {
        let start_state = self.atn.rule_to_start_state[start_rule];
        let is_left_recursive = self.atn.states[start_state].is_left_recursive_rule;
        let root = self.new_node(None, None, start_rule);
        if is_left_recursive {
            self.enter_recursion_rule(root, start_state, 0);
        } else {
            self.enter_rule(root, start_state);
        }
        let mut previous = start_state;
        let result = loop {
            if self.host.is_aborted() {
                break root;
            }
            let p = self.state;
            if self.atn.states[p].kind == StateKind::RuleStop {
                if self.nodes[self.ctx].invoking_state.is_none() {
                    if is_left_recursive {
                        let result = self.ctx;
                        let (parent, _) =
                            self.parent_context_stack.pop().expect("recursion context");
                        self.unroll_recursion_contexts(parent);
                        break result;
                    }
                    self.exit_rule();
                    break root;
                }
                self.visit_rule_stop_state(p);
            } else if let Err(e) = self.visit_state(p, previous) {
                self.nodes[self.ctx].error = true;
                self.report_error(&e);
                self.recover(&e);
                self.state = self.atn.rule_to_stop_state[self.atn.states[p].rule_index];
            }
            previous = p;
        };
        self.report_fetched();
        ParseTree {
            nodes: self.nodes,
            root: result,
            conjured_tokens: self.conjured_tokens,
        }
    }

    fn new_node(
        &mut self,
        parent: Option<NodeId>,
        invoking_state: Option<usize>,
        rule_index: usize,
    ) -> NodeId {
        self.nodes.push(RuleNode {
            rule_index,
            parent,
            children: Vec::new(),
            start: 0,
            stop: None,
            invoking_state,
            error: false,
        });
        self.nodes.len() - 1
    }

    fn lt(&self, k: isize) -> Option<&'t Token> {
        self.input.lt(k)
    }

    fn current_token(&self) -> &'t Token {
        self.input.lt(1).expect("the token stream ends with EOF")
    }

    fn precedence(&self) -> i32 {
        self.precedence_stack.last().copied().unwrap_or(-1)
    }

    fn invoking_states(&self, ctx: NodeId) -> Vec<usize> {
        ParseTree::invoking_states(&self.nodes, ctx)
    }

    fn follow_state(&self, invoking_state: usize) -> usize {
        match self.atn.states[invoking_state].transitions[0] {
            Transition::Rule { follow_state, .. } => follow_state,
            _ => unreachable!("an invoking state must start with a rule transition"),
        }
    }

    // Tree building (`Parser`)

    fn enter_rule(&mut self, ctx: NodeId, state: usize) {
        self.state = state;
        self.ctx = ctx;
        let node = &mut self.nodes[ctx];
        node.start = self.input.lt(1).expect("EOF").token_index;
        let (parent, invoking_state, rule_index, start) = (
            node.parent,
            node.invoking_state,
            node.rule_index,
            node.start,
        );
        if let Some(parent) = parent {
            self.nodes[parent].children.push(Child::Rule(ctx));
        }
        self.host
            .enter_rule(ctx, parent, invoking_state, rule_index, state, start, false);
        self.host.outer_alt(ctx, state, 1);
    }

    fn exit_rule(&mut self) {
        let stop = if self.matched_eof {
            self.lt(1)
        } else {
            self.lt(-1)
        };
        let stop = stop.map(|t| t.token_index);
        let ctx = self.ctx;
        let node = &mut self.nodes[ctx];
        node.stop = stop;
        self.state = node.invoking_state.unwrap_or(usize::MAX);
        if let Some(parent) = node.parent {
            self.ctx = parent;
        }
        let error = node.error;
        self.host.exit_rule(ctx, stop, error);
    }

    fn enter_recursion_rule(&mut self, ctx: NodeId, state: usize, precedence: i32) {
        let node = &self.nodes[ctx];
        let (parent, invoking_state, rule_index) =
            (node.parent, node.invoking_state, node.rule_index);
        self.parent_context_stack.push((parent, invoking_state));
        self.state = state;
        self.precedence_stack.push(precedence);
        self.ctx = ctx;
        let start = self.current_token().token_index;
        self.nodes[ctx].start = start;
        self.host
            .enter_rule(ctx, parent, invoking_state, rule_index, state, start, true);
        self.host.outer_alt(ctx, state, 1);
    }

    fn push_new_recursion_context(&mut self, ctx: NodeId, state: usize) {
        let previous = self.ctx;
        let stop = self.lt(-1).map(|t| t.token_index);
        let previous_node = &mut self.nodes[previous];
        previous_node.parent = Some(ctx);
        previous_node.invoking_state = Some(state);
        previous_node.stop = stop;
        let start = previous_node.start;
        self.ctx = ctx;
        self.nodes[ctx].start = start;
        self.nodes[ctx].children.push(Child::Rule(previous));
        self.host.push_recursion(ctx, previous, state, stop);
    }

    fn unroll_recursion_contexts(&mut self, parent: Option<NodeId>) {
        self.precedence_stack.pop();
        let stop = self.lt(-1).map(|t| t.token_index);
        let ret = self.ctx;
        self.nodes[ret].stop = stop;
        self.nodes[ret].parent = parent;
        if let Some(parent) = parent {
            self.ctx = parent;
            self.nodes[parent].children.push(Child::Rule(ret));
        }
        let error = self.nodes[ret].error;
        self.host.unroll(ret, parent, stop, error);
    }

    /// Consumes the current token; `state` is the state that matches it, if any.
    fn consume(&mut self, state: Option<usize>) {
        let token = self.current_token();
        if token.token_type != EOF {
            self.input.consume();
        }
        let error = self.error_recovery_mode;
        let child = if error {
            Child::SkippedToken(token.token_index)
        } else {
            Child::Token(token.token_index)
        };
        self.nodes[self.ctx].children.push(child);
        self.host.token(self.ctx, state, token.token_index, error);
    }

    fn add_conjured_token(&mut self, state: usize, token: Token) {
        self.host.conjure(self.ctx, state, &token, true);
        self.conjured_tokens.push(token);
        let child = Child::ConjuredToken(self.conjured_tokens.len() - 1);
        self.nodes[self.ctx].children.push(child);
    }

    /// Matches a token of `token_type` like `Parser.match`.
    fn match_token(&mut self, state: usize, token_type: i32) -> Result<(), RecognitionError> {
        if self.current_token().token_type == token_type {
            if token_type == EOF {
                self.matched_eof = true;
            }
            self.end_error_condition();
            self.consume(Some(state));
        } else if let Recovered::Conjured(token) = self.recover_inline(state)? {
            self.add_conjured_token(state, token);
        }
        Ok(())
    }

    /// Matches any token like `Parser.matchWildcard`.
    fn match_wildcard(&mut self, state: usize) -> Result<(), RecognitionError> {
        if self.current_token().token_type > 0 {
            self.end_error_condition();
            self.consume(Some(state));
        } else if let Recovered::Conjured(token) = self.recover_inline(state)? {
            self.add_conjured_token(state, token);
        }
        Ok(())
    }

    /// Matches a set of tokens like the code that ANTLR generates for sets, which does not add a
    /// token made up during recovery to the tree.
    fn match_set(&mut self, state: usize, transition: &Transition) -> Result<(), RecognitionError> {
        if transition.matches(self.atn, self.input.la(1), MIN_USER_TOKEN_TYPE, 65535) {
            self.end_error_condition();
            self.consume(Some(state));
        } else if let Recovered::Conjured(token) = self.recover_inline(state)? {
            self.host.conjure(self.ctx, state, &token, false);
        }
        Ok(())
    }

    // Interpretation (`ParserInterpreter`)

    fn visit_state(&mut self, p: usize, previous: usize) -> Result<(), RecognitionError> {
        let atn = self.atn;
        let state = &atn.states[p];
        // Generated parsers sync at the end of each loop iteration instead of where the loop
        // continues, and at the start of a `+` loop even when its block has one alternative.
        let continues_loop = state.loop_back_state == Some(previous);
        match state.kind {
            StateKind::StarLoopBack => self.sync(p)?,
            StateKind::PlusBlockStart if state.transitions.len() <= 1 && !continues_loop => {
                self.sync(p)?
            }
            _ => {}
        }
        let predicted_alt = if state.kind.is_decision() {
            self.visit_decision_state(p, !continues_loop)?
        } else {
            1
        };
        if state.is_outer_alt_block {
            self.host.outer_alt(self.ctx, p, predicted_alt);
        }
        let transition = &state.transitions[predicted_alt - 1];
        match *transition {
            Transition::Epsilon { target, .. } => {
                if state.kind == StateKind::StarLoopEntry
                    && state.is_precedence_decision
                    && atn.states[target].kind != StateKind::LoopEnd
                {
                    // Another iteration of a left-recursive rule: the tree so far becomes the
                    // first child of a new context.
                    let (parent, invoking_state) =
                        *self.parent_context_stack.last().expect("recursion context");
                    let rule_index = self.nodes[self.ctx].rule_index;
                    let ctx = self.new_node(parent, invoking_state, rule_index);
                    self.push_new_recursion_context(ctx, atn.rule_to_start_state[state.rule_index]);
                }
            }
            Transition::Atom { label, .. } => self.match_token(p, label)?,
            Transition::Range { .. } | Transition::Set { .. } | Transition::NotSet { .. } => {
                self.match_set(p, transition)?
            }
            Transition::Wildcard { .. } => self.match_wildcard(p)?,
            Transition::Rule {
                target, precedence, ..
            } => {
                let rule_index = atn.states[target].rule_index;
                let ctx = self.new_node(Some(self.ctx), Some(p), rule_index);
                if atn.states[target].is_left_recursive_rule {
                    self.enter_recursion_rule(ctx, target, precedence);
                } else {
                    self.enter_rule(ctx, target);
                }
                return Ok(());
            }
            Transition::Predicate {
                rule_index,
                pred_index,
                ..
            } => {
                if !self
                    .host
                    .sempred(Some(self.ctx), rule_index, pred_index, self.input.index())
                {
                    return Err(RecognitionError::FailedPredicate {
                        offending_token: self.current_token().token_index,
                        predicate: Some((rule_index, pred_index)),
                        message: format!("failed predicate: {{predicate {pred_index}}}?"),
                    });
                }
            }
            Transition::Action { .. } => self.host.action(self.ctx, p, self.input.index()),
            Transition::Precedence { precedence, .. } => {
                if precedence < self.precedence() {
                    return Err(RecognitionError::FailedPredicate {
                        offending_token: self.current_token().token_index,
                        predicate: None,
                        message: format!("failed predicate: {{precpred(_ctx, {precedence})}}?"),
                    });
                }
            }
        }
        self.state = transition.target();
        Ok(())
    }

    fn visit_decision_state(&mut self, p: usize, sync: bool) -> Result<usize, RecognitionError> {
        let state = &self.atn.states[p];
        if state.transitions.len() <= 1 {
            return Ok(1);
        }
        if sync {
            self.sync(p)?;
        }
        let decision = state
            .decision
            .expect("decision states have a decision number");
        let outer = Outer {
            nodes: &self.nodes,
            ctx: self.ctx,
            precedence: self.precedence_stack.last().copied().unwrap_or(-1),
        };
        let mode = self.host.prediction_mode().unwrap_or(self.mode);
        let mut host = PredictionHostAdapter {
            host: &mut *self.host,
        };
        prediction::adaptive_predict(self.atn, &mut self.input, decision, &outer, mode, &mut host)
            .map_err(|e| RecognitionError::NoViableAlt {
                start_token: e.start_token,
                offending_token: e.offending_token,
            })
    }

    fn visit_rule_stop_state(&mut self, p: usize) {
        let rule_start = self.atn.rule_to_start_state[self.atn.states[p].rule_index];
        if self.atn.states[rule_start].is_left_recursive_rule {
            let (parent, invoking_state) =
                self.parent_context_stack.pop().expect("recursion context");
            self.unroll_recursion_contexts(parent);
            self.state = invoking_state.expect("nested rule invocations have an invoking state");
        } else {
            self.exit_rule();
        }
        self.state = self.follow_state(self.state);
    }

    // Error handling (`DefaultErrorStrategy`)

    fn begin_error_condition(&mut self) {
        self.error_recovery_mode = true;
    }

    fn end_error_condition(&mut self) {
        self.error_recovery_mode = false;
        self.last_error_states = None;
        self.last_error_index = None;
    }

    fn expected_tokens(&self) -> IntervalSet {
        self.expected_tokens_at(self.state, self.ctx)
    }

    /// The tokens that can follow `state` in `ctx` (`ATN.getExpectedTokens`).
    fn expected_tokens_at(&self, state: usize, ctx: NodeId) -> IntervalSet {
        let follow_states: Vec<usize> = self
            .invoking_states(ctx)
            .into_iter()
            .map(|s| self.follow_state(s))
            .collect();
        expected_tokens(self.atn, state, &follow_states)
    }

    fn notify(&mut self, token_index: usize, message: String) {
        self.report_fetched();
        let token = &self.input.tokens()[token_index];
        self.host.syntax_error(SyntaxError {
            offending_token: Some(token_index),
            line: token.line,
            column: token.column,
            message,
        });
    }

    fn token_error_display(&self, token_index: usize) -> String {
        escape_ws_and_quote(&self.input.token_text(token_index))
    }

    fn report_error(&mut self, e: &RecognitionError) {
        if self.error_recovery_mode {
            return;
        }
        self.begin_error_condition();
        let message = match e {
            RecognitionError::NoViableAlt {
                start_token,
                offending_token,
            } => {
                let input = if self.input.tokens()[*start_token].token_type == EOF {
                    "<EOF>".to_string()
                } else {
                    self.input.text(*start_token, *offending_token)
                };
                format!(
                    "no viable alternative at input {}",
                    escape_ws_and_quote(&input)
                )
            }
            RecognitionError::InputMismatch {
                offending_token,
                state,
                ctx,
            } => format!(
                "mismatched input {} expecting {}",
                self.token_error_display(*offending_token),
                self.expected_tokens_at(*state, *ctx)
                    .to_string_with(self.vocabulary)
            ),
            RecognitionError::FailedPredicate {
                offending_token,
                predicate,
                message,
            } => {
                self.report_fetched();
                let rule_index = self.nodes[self.ctx].rule_index;
                let token = &self.input.tokens()[*offending_token];
                let error = SyntaxError {
                    offending_token: Some(*offending_token),
                    line: token.line,
                    column: token.column,
                    message: format!("rule {} {message}", self.rule_names[rule_index]),
                };
                match *predicate {
                    Some((rule_index, pred_index)) => self
                        .host
                        .failed_predicate(self.ctx, rule_index, pred_index, error),
                    None => self.host.syntax_error(error),
                }
                return;
            }
        };
        self.notify(e.offending_token(), message);
    }

    fn report_unwanted_token(&mut self) {
        if self.error_recovery_mode {
            return;
        }
        self.begin_error_condition();
        let token = self.current_token().token_index;
        let message = format!(
            "extraneous input {} expecting {}",
            self.token_error_display(token),
            self.expected_tokens().to_string_with(self.vocabulary)
        );
        self.notify(token, message);
    }

    fn report_missing_token(&mut self) {
        if self.error_recovery_mode {
            return;
        }
        self.begin_error_condition();
        let token = self.current_token().token_index;
        let message = format!(
            "missing {} at {}",
            self.expected_tokens().to_string_with(self.vocabulary),
            self.token_error_display(token)
        );
        self.notify(token, message);
    }

    /// Recovers from an error in a rule by skipping tokens until one that can follow the rule.
    fn recover(&mut self, _e: &RecognitionError) {
        let index = self.input.index();
        if self.last_error_index == Some(index)
            && self
                .last_error_states
                .as_ref()
                .is_some_and(|s| s.contains(self.state as i32))
        {
            // The previous recovery made no progress at this position; skip a token to avoid
            // looping forever.
            self.consume(None);
        }
        self.last_error_index = Some(self.input.index());
        self.last_error_states
            .get_or_insert_default()
            .add(self.state as i32);
        let follow = self.error_recovery_set();
        self.consume_until(&follow);
    }

    fn sync(&mut self, state: usize) -> Result<(), RecognitionError> {
        self.state = state;
        if self.error_recovery_mode {
            return Ok(());
        }
        let la = self.input.la(1);
        let next_tokens = self.atn.next_tokens(self.state);
        if next_tokens.contains(la) {
            self.next_tokens = None;
            return Ok(());
        }
        if next_tokens.contains(EPSILON) {
            if self.next_tokens.is_none() {
                self.next_tokens = Some((self.ctx, self.state));
            }
            return Ok(());
        }
        match self.atn.states[self.state].kind {
            StateKind::BlockStart
            | StateKind::StarBlockStart
            | StateKind::PlusBlockStart
            | StateKind::StarLoopEntry => {
                if self.single_token_deletion() {
                    return Ok(());
                }
                Err(RecognitionError::InputMismatch {
                    offending_token: self.current_token().token_index,
                    state: self.state,
                    ctx: self.ctx,
                })
            }
            StateKind::PlusLoopBack | StateKind::StarLoopBack => {
                self.report_unwanted_token();
                let mut follow = self.expected_tokens();
                follow.add_set(&self.error_recovery_set());
                self.consume_until(&follow);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn recover_inline(&mut self, state: usize) -> Result<Recovered, RecognitionError> {
        if self.single_token_deletion() {
            self.consume(Some(state));
            return Ok(Recovered::Matched);
        }
        if self.single_token_insertion() {
            return Ok(Recovered::Conjured(self.missing_token()));
        }
        let (ctx, state) = self.next_tokens.unwrap_or((self.ctx, self.state));
        Err(RecognitionError::InputMismatch {
            offending_token: self.current_token().token_index,
            state,
            ctx,
        })
    }

    fn single_token_insertion(&mut self) -> bool {
        let current = self.input.la(1);
        let next = self.atn.states[self.state].transitions[0].target();
        let follow_states: Vec<usize> = self
            .invoking_states(self.ctx)
            .into_iter()
            .map(|s| self.follow_state(s))
            .collect();
        if ll1::look_in_context(self.atn, next, &follow_states).contains(current) {
            self.report_missing_token();
            return true;
        }
        false
    }

    /// Deletes the current token when the next one is expected; the caller consumes that one.
    fn single_token_deletion(&mut self) -> bool {
        if self.expected_tokens().contains(self.input.la(2)) {
            self.report_unwanted_token();
            self.consume(None);
            self.end_error_condition();
            return true;
        }
        false
    }

    fn missing_token(&self) -> Token {
        let expected = self.expected_tokens().min_element().unwrap_or(INVALID_TYPE);
        let text = if expected == EOF {
            "<missing EOF>".to_string()
        } else {
            format!("<missing {}>", self.vocabulary.display_name(expected))
        };
        let mut current = self.current_token();
        if current.token_type == EOF
            && let Some(previous) = self.lt(-1)
        {
            current = previous;
        }
        Token {
            token_type: expected,
            channel: DEFAULT_CHANNEL,
            end: current.start,
            text: Some(text),
            ..current.clone()
        }
    }

    fn error_recovery_set(&self) -> IntervalSet {
        let mut set = IntervalSet::new();
        for invoking_state in self.invoking_states(self.ctx) {
            set.add_set(self.atn.next_tokens(self.follow_state(invoking_state)));
        }
        set.remove(EPSILON);
        set
    }

    fn consume_until(&mut self, set: &IntervalSet) {
        loop {
            let token_type = self.input.la(1);
            if token_type == EOF || set.contains(token_type) {
                break;
            }
            self.consume(None);
        }
    }
}

/// The tokens that can follow `state` when the current rule was invoked from rule invocations
/// with the given follow states, innermost first (`ATN.getExpectedTokens`).
pub(crate) fn expected_tokens(atn: &Atn, state: usize, follow_states: &[usize]) -> IntervalSet {
    let mut following = atn.next_tokens(state);
    if !following.contains(EPSILON) {
        return following.clone();
    }
    let mut expected = following.clone();
    expected.remove(EPSILON);
    for &follow_state in follow_states {
        if !following.contains(EPSILON) {
            break;
        }
        following = atn.next_tokens(follow_state);
        expected.add_set(following);
        expected.remove(EPSILON);
    }
    if following.contains(EPSILON) {
        expected.add(EOF);
    }
    expected
}

struct PredictionHostAdapter<'h, H: ParserHost> {
    host: &'h mut H,
}

impl<H: ParserHost> PredictionHost for PredictionHostAdapter<'_, H> {
    fn sempred(
        &mut self,
        ctx: Option<NodeId>,
        rule_index: usize,
        pred_index: usize,
        input_index: usize,
    ) -> bool {
        self.host.is_aborted() || self.host.sempred(ctx, rule_index, pred_index, input_index)
    }

    fn diagnostic(&mut self, diagnostic: Diagnostic) {
        if !self.host.is_aborted() {
            self.host.diagnostic(diagnostic);
        }
    }

    fn fetched(&mut self, index: usize) {
        if !self.host.is_aborted() {
            self.host.fetched(index);
        }
    }
}

fn escape_ws_and_quote(s: &str) -> String {
    format!(
        "'{}'",
        s.replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}
