//! The parser interpreter, ported from ANTLR's `ParserInterpreter`, the tree-building parts of
//! `Parser`, and `DefaultErrorStrategy`.

use crate::SyntaxError;
use crate::atn::{Atn, StateKind, Transition};
use crate::interval_set::IntervalSet;
use crate::ll1;
use crate::prediction::{self, Outer};
use crate::token::{
    DEFAULT_CHANNEL, EOF, EPSILON, INVALID_TYPE, MIN_USER_TOKEN_TYPE, Token, TokenStream,
    Vocabulary,
};
use crate::tree::{Child, NodeId, ParseTree, RuleNode};

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
        predicate: String,
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

pub(crate) struct Parser<'a, 't> {
    atn: &'a Atn,
    vocabulary: Vocabulary,
    rule_names: &'static [&'static str],
    input: TokenStream<'t>,
    nodes: Vec<RuleNode>,
    conjured_tokens: Vec<Token>,
    ctx: NodeId,
    state: usize,
    precedence_stack: Vec<i32>,
    /// The parent and invoking state of each left-recursive rule invocation being parsed.
    parent_context_stack: Vec<(Option<NodeId>, Option<usize>)>,
    matched_eof: bool,
    errors: Vec<SyntaxError>,
    // Error strategy state
    error_recovery_mode: bool,
    last_error_index: Option<usize>,
    last_error_states: Option<IntervalSet>,
    /// Where `sync` last saw a state that can be skipped (`nextTokensContext`/`nextTokensState`).
    next_tokens: Option<(NodeId, usize)>,
}

impl<'a, 't> Parser<'a, 't> {
    pub(crate) fn new(
        atn: &'a Atn,
        vocabulary: Vocabulary,
        rule_names: &'static [&'static str],
        tokens: &'t [Token],
    ) -> Self {
        Self {
            atn,
            vocabulary,
            rule_names,
            input: TokenStream::new(tokens),
            nodes: Vec::new(),
            conjured_tokens: Vec::new(),
            ctx: 0,
            state: 0,
            precedence_stack: Vec::new(),
            parent_context_stack: Vec::new(),
            matched_eof: false,
            errors: Vec::new(),
            error_recovery_mode: false,
            last_error_index: None,
            last_error_states: None,
            next_tokens: None,
        }
    }

    pub(crate) fn parse(mut self, start_rule: usize) -> (ParseTree, Vec<SyntaxError>) {
        let start_state = self.atn.rule_to_start_state[start_rule];
        let is_left_recursive = self.atn.states[start_state].is_left_recursive_rule;
        let root = self.new_node(None, None, start_rule);
        if is_left_recursive {
            self.enter_recursion_rule(root, start_state, 0);
        } else {
            self.enter_rule(root, start_state);
        }
        let result = loop {
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
            } else if let Err(e) = self.visit_state(p) {
                self.state = self.atn.rule_to_stop_state[self.atn.states[p].rule_index];
                self.report_error(&e);
                self.recover(&e);
            }
        };
        let tree = ParseTree {
            nodes: self.nodes,
            root: result,
            tokens: self.input.tokens().to_vec(),
            conjured_tokens: self.conjured_tokens,
            rule_names: self.rule_names,
        };
        (tree, self.errors)
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
        self.nodes[ctx].start = self.current_token().token_index;
        if let Some(parent) = self.nodes[ctx].parent {
            self.nodes[parent].children.push(Child::Rule(ctx));
        }
    }

    fn exit_rule(&mut self) {
        let stop = if self.matched_eof {
            self.lt(1)
        } else {
            self.lt(-1)
        };
        let node = &mut self.nodes[self.ctx];
        node.stop = stop.map(|t| t.token_index);
        self.state = node.invoking_state.unwrap_or(usize::MAX);
        if let Some(parent) = node.parent {
            self.ctx = parent;
        }
    }

    fn enter_recursion_rule(&mut self, ctx: NodeId, state: usize, precedence: i32) {
        let node = &self.nodes[ctx];
        self.parent_context_stack
            .push((node.parent, node.invoking_state));
        self.state = state;
        self.precedence_stack.push(precedence);
        self.ctx = ctx;
        self.nodes[ctx].start = self.current_token().token_index;
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
    }

    fn consume(&mut self) {
        let token = self.current_token();
        if token.token_type != EOF {
            self.input.consume();
        }
        let child = if self.error_recovery_mode {
            Child::SkippedToken(token.token_index)
        } else {
            Child::Token(token.token_index)
        };
        self.nodes[self.ctx].children.push(child);
    }

    fn add_conjured_token(&mut self, token: Token) {
        self.conjured_tokens.push(token);
        let child = Child::ConjuredToken(self.conjured_tokens.len() - 1);
        self.nodes[self.ctx].children.push(child);
    }

    fn match_token(&mut self, token_type: i32) -> Result<(), RecognitionError> {
        if self.current_token().token_type == token_type {
            if token_type == EOF {
                self.matched_eof = true;
            }
            self.end_error_condition();
            self.consume();
        } else if let Recovered::Conjured(token) = self.recover_inline()? {
            self.add_conjured_token(token);
        }
        Ok(())
    }

    fn match_wildcard(&mut self) -> Result<(), RecognitionError> {
        if self.current_token().token_type > 0 {
            self.end_error_condition();
            self.consume();
        } else if let Recovered::Conjured(token) = self.recover_inline()? {
            self.add_conjured_token(token);
        }
        Ok(())
    }

    // Interpretation (`ParserInterpreter`)

    fn visit_state(&mut self, p: usize) -> Result<(), RecognitionError> {
        let atn = self.atn;
        let state = &atn.states[p];
        let predicted_alt = if state.kind.is_decision() {
            self.visit_decision_state(p)?
        } else {
            1
        };
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
            Transition::Atom { label, .. } => self.match_token(label)?,
            Transition::Range { .. } | Transition::Set { .. } | Transition::NotSet { .. } => {
                if !transition.matches(atn, self.input.la(1), MIN_USER_TOKEN_TYPE, 65535) {
                    self.recover_inline()?;
                }
                self.match_wildcard()?;
            }
            Transition::Wildcard { .. } => self.match_wildcard()?,
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
            }
            // Predicates and actions are grammar code, which the runtime does not run.
            Transition::Predicate { .. } | Transition::Action { .. } => {}
            Transition::Precedence { precedence, .. } => {
                if precedence < self.precedence() {
                    return Err(RecognitionError::FailedPredicate {
                        offending_token: self.current_token().token_index,
                        predicate: format!("precpred(_ctx, {precedence})"),
                    });
                }
            }
        }
        self.state = transition.target();
        Ok(())
    }

    fn visit_decision_state(&mut self, p: usize) -> Result<usize, RecognitionError> {
        let state = &self.atn.states[p];
        if state.transitions.len() <= 1 {
            return Ok(1);
        }
        self.sync()?;
        let decision = state
            .decision
            .expect("decision states have a decision number");
        let outer = Outer {
            nodes: &self.nodes,
            ctx: self.ctx,
            precedence: self.precedence(),
        };
        prediction::adaptive_predict(self.atn, &mut self.input, decision, &outer).map_err(|e| {
            RecognitionError::NoViableAlt {
                start_token: e.start_token,
                offending_token: e.offending_token,
            }
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

    // Error handling (`DefaultErrorStrategy` and `ParserInterpreter.recover`)

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
    fn expected_tokens_at(&self, state: usize, mut ctx: NodeId) -> IntervalSet {
        let mut following = self.atn.next_tokens(state);
        if !following.contains(EPSILON) {
            return following.clone();
        }
        let mut expected = following.clone();
        expected.remove(EPSILON);
        while following.contains(EPSILON) {
            let (Some(parent), Some(invoking_state)) =
                (self.nodes[ctx].parent, self.nodes[ctx].invoking_state)
            else {
                break;
            };
            following = self.atn.next_tokens(self.follow_state(invoking_state));
            expected.add_set(following);
            expected.remove(EPSILON);
            ctx = parent;
        }
        if following.contains(EPSILON) {
            expected.add(EOF);
        }
        expected
    }

    fn notify(&mut self, token_index: usize, message: String) {
        let token = &self.input.tokens()[token_index];
        self.errors.push(SyntaxError {
            line: token.line,
            column: token.column,
            start: token.start,
            end: token.end,
            message,
        });
    }

    fn token_error_display(&self, token_index: usize) -> String {
        escape_ws_and_quote(&self.input.tokens()[token_index].text)
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
                    .to_string_with(&self.vocabulary)
            ),
            RecognitionError::FailedPredicate { predicate, .. } => format!(
                "rule {} failed predicate: {{{predicate}}}?",
                self.rule_names[self.nodes[self.ctx].rule_index]
            ),
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
            self.expected_tokens().to_string_with(&self.vocabulary)
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
            self.expected_tokens().to_string_with(&self.vocabulary),
            self.token_error_display(token)
        );
        self.notify(token, message);
    }

    /// Recovers from an error in a rule by skipping tokens until one that can follow the rule
    /// (`ParserInterpreter.recover` wrapping `DefaultErrorStrategy.recover`).
    fn recover(&mut self, e: &RecognitionError) {
        let index = self.input.index();
        if self.last_error_index == Some(index)
            && self
                .last_error_states
                .as_ref()
                .is_some_and(|s| s.contains(self.state as i32))
        {
            // The previous recovery made no progress at this position; skip a token to avoid
            // looping forever.
            self.consume();
        }
        self.last_error_index = Some(self.input.index());
        self.last_error_states
            .get_or_insert_default()
            .add(self.state as i32);
        let follow = self.error_recovery_set();
        self.consume_until(&follow);

        if self.input.index() == index {
            let offending = &self.input.tokens()[e.offending_token()];
            let token_type = match *e {
                RecognitionError::InputMismatch { state, ctx, .. } => self
                    .expected_tokens_at(state, ctx)
                    .min_element()
                    .unwrap_or(INVALID_TYPE),
                _ => INVALID_TYPE,
            };
            let token = Token {
                token_type,
                channel: DEFAULT_CHANNEL,
                ..offending.clone()
            };
            self.add_conjured_token(token);
        }
    }

    fn sync(&mut self) -> Result<(), RecognitionError> {
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

    fn recover_inline(&mut self) -> Result<Recovered, RecognitionError> {
        if self.single_token_deletion() {
            self.consume();
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

    fn single_token_deletion(&mut self) -> bool {
        if self.expected_tokens().contains(self.input.la(2)) {
            self.report_unwanted_token();
            self.consume();
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
            text,
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
            self.consume();
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
