//! The ultra-parser runtime as a WebAssembly module with a small C ABI, which the `ultra-parser`
//! npm package drives.
//!
//! Objects are passed to JavaScript as pointers and freed with the matching `*_free` export.
//! Results that do not fit in a return value are written to a buffer that stays valid until the
//! next call. Grammar code runs in the host through the functions imported from module `host`;
//! each returns a status so that an exception in grammar code stops lexing or parsing.

use std::alloc::{Layout, alloc, dealloc};
use std::cell::{Cell, RefCell};

use ultra_parser_runtime::{
    Diagnostic, DiagnosticKind, Grammar, LexerAction, LexerHost, NodeId, ParserHost, Position,
    PredictionMode, SyntaxError, Token, Tokens, Vocabulary,
};

/// Returned by host functions to stop lexing or parsing, e.g., because grammar code threw.
const ABORT: i32 = 1;
/// Returned by host predicates to stop lexing or parsing.
const PREDICATE_ABORT: i32 = 2;

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn lexer_sempred(rule_index: u32, pred_index: u32, index: u32, line: u32, column: u32) -> i32;

    fn enter_rule(
        ctx: u32,
        parent: i32,
        invoking_state: i32,
        rule_index: u32,
        state: u32,
        start_token: u32,
        precedence: i32,
    ) -> i32;
    fn push_recursion(ctx: u32, previous: u32, state: u32, previous_stop: i32) -> i32;
    fn outer_alt(ctx: u32, state: u32, alt: u32) -> i32;
    fn exit_rule(ctx: u32, stop_token: i32, error: u32) -> i32;
    fn unroll(ctx: u32, parent: i32, stop_token: i32, error: u32) -> i32;
    fn token(ctx: u32, state: i32, token_index: u32, error: u32) -> i32;
    fn conjure(
        ctx: u32,
        state: u32,
        token_type: i32,
        line: u32,
        column: u32,
        add_to_tree: u32,
    ) -> i32;
    fn action(ctx: u32, state: u32, input_index: u32) -> i32;
    fn sempred(ctx: i32, rule_index: u32, pred_index: u32, input_index: u32) -> i32;
    fn syntax_error(
        token_index: i32,
        line: u32,
        column: u32,
        message: *const u8,
        len: usize,
    ) -> i32;
    fn failed_predicate(
        ctx: u32,
        rule_index: u32,
        pred_index: u32,
        token_index: u32,
        message: *const u8,
        len: usize,
    ) -> i32;
    fn fetched(token_index: u32) -> i32;
    fn diagnostic(
        kind: u32,
        decision: u32,
        rule_index: u32,
        start_index: u32,
        stop_index: u32,
        alts: *const u32,
        n_alts: usize,
    ) -> i32;
}

thread_local! {
    static RESULT: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    /// The prediction mode of the current parse, which grammar code may change while parsing.
    static PREDICTION_MODE: Cell<u32> = const { Cell::new(1) };
}

fn prediction_mode(mode: u32) -> PredictionMode {
    match mode {
        0 => PredictionMode::Sll,
        2 => PredictionMode::LlExactAmbigDetection,
        _ => PredictionMode::Ll,
    }
}

/// Changes the prediction mode of the current parse; see `parse`.
#[unsafe(no_mangle)]
pub extern "C" fn set_prediction_mode(mode: u32) {
    PREDICTION_MODE.set(mode);
}

/// Stores `values` as the result of the current call and returns a pointer to them.
fn result(values: impl IntoIterator<Item = i32>) -> *const i32 {
    RESULT.with_borrow_mut(|buf| {
        buf.clear();
        buf.extend(values);
        buf.as_ptr()
    })
}

fn set_last_error(message: String) {
    LAST_ERROR.with_borrow_mut(|e| *e = message);
}

fn to_i32(value: Option<usize>) -> i32 {
    value.map_or(-1, |v| v as i32)
}

/// # Safety
///
/// `ptr` must point to `len` readable values of `T`, or `len` must be 0.
unsafe fn slice<'a, T>(ptr: *const T, len: usize) -> &'a [T] {
    if len == 0 {
        &[]
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

// Memory

/// Allocates `len` bytes aligned for 32-bit values.
#[unsafe(no_mangle)]
pub extern "C" fn alloc_bytes(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u32>::dangling().as_ptr().cast();
    }
    // SAFETY: the layout has a non-zero size.
    unsafe { alloc(Layout::from_size_align(len, 4).expect("valid layout")) }
}

/// # Safety
///
/// `ptr` must come from `alloc_bytes(len)` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_bytes(ptr: *mut u8, len: usize) {
    if len != 0 {
        // SAFETY: guaranteed by the caller.
        unsafe { dealloc(ptr, Layout::from_size_align(len, 4).expect("valid layout")) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn last_error_ptr() -> *const u8 {
    LAST_ERROR.with_borrow(|e| e.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn last_error_len() -> usize {
    LAST_ERROR.with_borrow(|e| e.len())
}

// Grammars

/// Loads a grammar. `names` holds the rule names, literal names, and symbolic names in this
/// order, each terminated by NUL; a name of a single U+0001 stands for a missing name. Returns
/// null when the grammar is invalid, with the reason in `last_error`.
///
/// # Safety
///
/// The pointers must point to `atn_len` values and `names_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn grammar_new(
    atn: *const i32,
    atn_len: usize,
    names: *const u8,
    names_len: usize,
    n_rules: usize,
    n_literal_names: usize,
) -> *mut Grammar {
    // SAFETY: guaranteed by the caller.
    let (atn, names) = unsafe { (slice(atn, atn_len), slice(names, names_len)) };
    let Ok(names) = std::str::from_utf8(names) else {
        set_last_error("the names are not UTF-8".to_string());
        return std::ptr::null_mut();
    };
    let mut names = names.split_terminator('\0').map(|name| match name {
        "\u{1}" => None,
        name => Some(name.to_string()),
    });
    let rule_names = names.by_ref().take(n_rules).flatten().collect();
    let literal_names = names.by_ref().take(n_literal_names).collect();
    let symbolic_names = names.collect();
    match Grammar::new(
        atn,
        rule_names,
        Vocabulary {
            literal_names,
            symbolic_names,
        },
    ) {
        Ok(grammar) => Box::into_raw(Box::new(grammar)),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// # Safety
///
/// `grammar` must come from `grammar_new` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn grammar_free(grammar: *mut Grammar) {
    // SAFETY: guaranteed by the caller.
    drop(unsafe { Box::from_raw(grammar) });
}

/// The lexer actions of a lexer grammar as `[n, (kind, data1, data2)...]` with ANTLR's action
/// type numbers.
///
/// # Safety
///
/// `grammar` must be a live grammar.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn grammar_lexer_actions(grammar: *const Grammar) -> *const i32 {
    // SAFETY: guaranteed by the caller.
    let grammar = unsafe { &*grammar };
    let actions = grammar.lexer_actions();
    let mut values = vec![actions.len() as i32];
    for action in actions {
        values.extend(match *action {
            LexerAction::Channel(c) => [0, c, 0],
            LexerAction::Custom {
                rule_index,
                action_index,
            } => [1, rule_index as i32, action_index as i32],
            LexerAction::Mode(m) => [2, m as i32, 0],
            LexerAction::More => [3, 0, 0],
            LexerAction::PopMode => [4, 0, 0],
            LexerAction::PushMode(m) => [5, m as i32, 0],
            LexerAction::Skip => [6, 0, 0],
            LexerAction::Type(t) => [7, t, 0],
        });
    }
    result(values)
}

/// The tokens that can follow `state` in a rule invoked from the given invoking states,
/// innermost first, as `[n, (from, to)...]`, or `[-1]` for a state that is not in the ATN.
///
/// # Safety
///
/// `grammar` must be a live grammar and `invoking_states` must point to `n` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn grammar_expected_tokens(
    grammar: *const Grammar,
    state: usize,
    invoking_states: *const u32,
    n: usize,
) -> *const i32 {
    // SAFETY: guaranteed by the caller.
    let (grammar, invoking_states) = unsafe { (&*grammar, slice(invoking_states, n)) };
    let invoking_states: Vec<usize> = invoking_states.iter().map(|&s| s as usize).collect();
    let Some(set) = grammar.expected_tokens(state, &invoking_states) else {
        return result([-1]);
    };
    let intervals = set.intervals();
    result(
        std::iter::once(intervals.len() as i32).chain(intervals.iter().flat_map(|&(a, b)| [a, b])),
    )
}

// Token lists

/// Creates a token list over an input of `len` code points.
///
/// # Safety
///
/// `input` must point to `len` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tokens_new(input: *const u32, len: usize) -> *mut Tokens {
    // SAFETY: guaranteed by the caller.
    let input = unsafe { slice(input, len) }.to_vec();
    Box::into_raw(Box::new(Tokens {
        input,
        tokens: Vec::new(),
    }))
}

/// # Safety
///
/// `tokens` must come from `tokens_new` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tokens_free(tokens: *mut Tokens) {
    // SAFETY: guaranteed by the caller.
    drop(unsafe { Box::from_raw(tokens) });
}

/// Replaces the tokens of the list with `n` tokens given as
/// `(type, channel, start, stop, line, column)`, where `stop` is the offset of the last code
/// point, as in ANTLR.
///
/// # Safety
///
/// `tokens` must be a live token list and `data` must point to `6 * n` values. No lexer match or
/// parse may be reading `tokens`, including from its host callbacks, since they borrow the tokens.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tokens_set(tokens: *mut Tokens, data: *const i32, n: usize) {
    // SAFETY: guaranteed by the caller.
    let (tokens, data) = unsafe { (&mut *tokens, slice(data, 6 * n)) };
    tokens.tokens = data
        .chunks_exact(6)
        .enumerate()
        .map(|(i, t)| Token {
            token_type: t[0],
            channel: t[1],
            start: t[2].max(0) as usize,
            end: (t[3] + 1).max(t[2]).max(0) as usize,
            line: t[4].max(0) as usize,
            column: t[5].max(0) as usize,
            token_index: i,
            text: None,
        })
        .collect();
}

/// Sets the text of token `index`, which the lexer changed.
///
/// # Safety
///
/// `tokens` must be a live token list with a token `index`, and `text` must point to `len` bytes
/// of UTF-8. No lexer match or parse may be reading `tokens`, including from its host callbacks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tokens_set_text(
    tokens: *mut Tokens,
    index: usize,
    text: *const u8,
    len: usize,
) {
    // SAFETY: guaranteed by the caller.
    let (tokens, text) = unsafe { (&mut *tokens, slice(text, len)) };
    tokens.tokens[index].text = Some(String::from_utf8_lossy(text).into_owned());
}

// Lexing

struct WasmLexerHost {
    aborted: bool,
}

impl LexerHost for WasmLexerHost {
    fn sempred(&mut self, rule_index: usize, pred_index: usize, at: Position) -> bool {
        if self.aborted {
            return true;
        }
        // SAFETY: calls into the host.
        match unsafe {
            lexer_sempred(
                rule_index as u32,
                pred_index as u32,
                at.index as u32,
                at.line as u32,
                at.column as u32,
            )
        } {
            0 => false,
            PREDICATE_ABORT => {
                self.aborted = true;
                true
            }
            _ => true,
        }
    }
}

/// Matches one token of the input of `tokens` in `mode` from the given position. The result is
/// `[status, type, index, line, column, n, (action, offset)...]`: status 0 means a token was
/// matched, 1 that no rule matched up to the position given instead, and 2 that a predicate
/// aborted. An offset of -1 means the end of the token.
///
/// # Safety
///
/// `grammar` must be a live lexer grammar and `tokens` a live token list, which must not change
/// (`tokens_set`, `tokens_set_text`, `tokens_free`) until the match returns, even from a host
/// callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lexer_match(
    grammar: *const Grammar,
    tokens: *const Tokens,
    mode: usize,
    index: usize,
    line: usize,
    column: usize,
) -> *const i32 {
    // SAFETY: guaranteed by the caller.
    let (grammar, tokens) = unsafe { (&*grammar, &*tokens) };
    let mut host = WasmLexerHost { aborted: false };
    let start = Position {
        index,
        line,
        column,
    };
    let outcome = grammar.match_token(&tokens.input, mode, start, &mut host);
    if host.aborted {
        return result([2]);
    }
    match outcome {
        Ok(m) => result(
            [
                0,
                m.token_type,
                m.stop.index as i32,
                m.stop.line as i32,
                m.stop.column as i32,
                m.actions.len() as i32,
            ]
            .into_iter()
            .chain(
                m.actions
                    .iter()
                    .flat_map(|&(action, offset)| [action as i32, to_i32(offset)]),
            ),
        ),
        Err(stop) => result([
            1,
            0,
            stop.index as i32,
            stop.line as i32,
            stop.column as i32,
            0,
        ]),
    }
}

// Parsing

struct WasmParserHost {
    aborted: bool,
}

impl WasmParserHost {
    fn check(&mut self, status: i32) {
        if status == ABORT {
            self.aborted = true;
        }
    }
}

fn ctx_or_none(ctx: Option<NodeId>) -> i32 {
    to_i32(ctx)
}

impl ParserHost for WasmParserHost {
    fn enter_rule(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        invoking_state: Option<usize>,
        rule_index: usize,
        state: usize,
        start_token: usize,
        precedence: Option<i32>,
    ) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            enter_rule(
                ctx as u32,
                ctx_or_none(parent),
                to_i32(invoking_state),
                rule_index as u32,
                state as u32,
                start_token as u32,
                precedence.unwrap_or(-1),
            )
        };
        self.check(status);
    }

    fn fetched(&mut self, index: usize) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe { fetched(index as u32) };
        self.check(status);
    }

    fn prediction_mode(&self) -> Option<PredictionMode> {
        Some(prediction_mode(PREDICTION_MODE.get()))
    }

    fn push_recursion(
        &mut self,
        ctx: NodeId,
        previous: NodeId,
        state: usize,
        previous_stop: Option<usize>,
    ) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            push_recursion(
                ctx as u32,
                previous as u32,
                state as u32,
                to_i32(previous_stop),
            )
        };
        self.check(status);
    }

    fn outer_alt(&mut self, ctx: NodeId, state: usize, alt: usize) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe { outer_alt(ctx as u32, state as u32, alt as u32) };
        self.check(status);
    }

    fn exit_rule(&mut self, ctx: NodeId, stop_token: Option<usize>, error: bool) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe { exit_rule(ctx as u32, to_i32(stop_token), error as u32) };
        self.check(status);
    }

    fn unroll(
        &mut self,
        ctx: NodeId,
        parent: Option<NodeId>,
        stop_token: Option<usize>,
        error: bool,
    ) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            unroll(
                ctx as u32,
                ctx_or_none(parent),
                to_i32(stop_token),
                error as u32,
            )
        };
        self.check(status);
    }

    fn token(&mut self, ctx: NodeId, state: Option<usize>, token_index: usize, error: bool) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe { token(ctx as u32, to_i32(state), token_index as u32, error as u32) };
        self.check(status);
    }

    fn conjure(&mut self, ctx: NodeId, state: usize, token: &Token, add_to_tree: bool) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            conjure(
                ctx as u32,
                state as u32,
                token.token_type,
                token.line as u32,
                token.column as u32,
                add_to_tree as u32,
            )
        };
        self.check(status);
    }

    fn action(&mut self, ctx: NodeId, state: usize, input_index: usize) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe { action(ctx as u32, state as u32, input_index as u32) };
        self.check(status);
    }

    fn sempred(
        &mut self,
        ctx: Option<NodeId>,
        rule_index: usize,
        pred_index: usize,
        input_index: usize,
    ) -> bool {
        if self.aborted {
            return true;
        }
        // SAFETY: calls into the host.
        match unsafe {
            sempred(
                ctx_or_none(ctx),
                rule_index as u32,
                pred_index as u32,
                input_index as u32,
            )
        } {
            0 => false,
            PREDICATE_ABORT => {
                self.aborted = true;
                true
            }
            _ => true,
        }
    }

    fn syntax_error(&mut self, error: SyntaxError) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            syntax_error(
                to_i32(error.offending_token),
                error.line as u32,
                error.column as u32,
                error.message.as_ptr(),
                error.message.len(),
            )
        };
        self.check(status);
    }

    fn failed_predicate(
        &mut self,
        ctx: NodeId,
        rule_index: usize,
        pred_index: usize,
        error: SyntaxError,
    ) {
        if self.aborted {
            return;
        }
        // SAFETY: calls into the host.
        let status = unsafe {
            failed_predicate(
                ctx as u32,
                rule_index as u32,
                pred_index as u32,
                error.offending_token.unwrap_or_default() as u32,
                error.message.as_ptr(),
                error.message.len(),
            )
        };
        self.check(status);
    }

    fn diagnostic(&mut self, d: Diagnostic) {
        if self.aborted {
            return;
        }
        let kind = match d.kind {
            DiagnosticKind::AttemptingFullContext => 0,
            DiagnosticKind::ContextSensitivity => 1,
            DiagnosticKind::Ambiguity { exact: false } => 2,
            DiagnosticKind::Ambiguity { exact: true } => 3,
        };
        let alts: Vec<u32> = d.alts.iter().map(|&a| a as u32).collect();
        // SAFETY: calls into the host.
        let status = unsafe {
            diagnostic(
                kind,
                d.decision as u32,
                d.rule_index as u32,
                d.start_index as u32,
                d.stop_index as u32,
                alts.as_ptr(),
                alts.len(),
            )
        };
        self.check(status);
    }

    fn is_aborted(&self) -> bool {
        self.aborted
    }
}

/// Parses the tokens on `channel` of `tokens` from token `start_token` with rule `start_rule`,
/// reporting to the host as it goes. Returns the id of the root context, or -1 when the host aborted. `mode` is 0 for SLL, 1 for LL, and 2 for LL
/// with exact ambiguity detection.
///
/// # Safety
///
/// `grammar` must be a live parser grammar and `tokens` a live token list ending with EOF, which
/// must not change (`tokens_set`, `tokens_set_text`, `tokens_free`) until the parse returns, even
/// from a host callback; a host that parses other tokens from a callback needs another list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn parse(
    grammar: *const Grammar,
    tokens: *const Tokens,
    start_token: usize,
    channel: i32,
    start_rule: usize,
    mode: u32,
) -> i32 {
    // SAFETY: guaranteed by the caller.
    let (grammar, tokens) = unsafe { (&*grammar, &*tokens) };
    let outer_mode = PREDICTION_MODE.replace(mode);
    let mut host = WasmParserHost { aborted: false };
    let root = grammar.parse(
        tokens,
        start_token,
        channel,
        start_rule,
        prediction_mode(mode),
        &mut host,
    );
    PREDICTION_MODE.set(outer_mode);
    if host.aborted { -1 } else { root as i32 }
}
