use std::borrow::Cow;
use std::cell::Cell;

pub const EOF: i32 = -1;
pub const EPSILON: i32 = -2;
pub const INVALID_TYPE: i32 = 0;
pub const MIN_USER_TOKEN_TYPE: i32 = 1;

pub const DEFAULT_CHANNEL: i32 = 0;
pub const HIDDEN_CHANNEL: i32 = 1;

/// A token produced by the lexer. Offsets count Unicode code points, as ANTLR's `CharStreams` do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub token_type: i32,
    pub channel: i32,
    /// Offset of the first code point.
    pub start: usize,
    /// Offset just past the last code point.
    pub end: usize,
    /// 1-based line of the first code point.
    pub line: usize,
    /// 0-based column (in code points) of the first code point.
    pub column: usize,
    /// Index of this token in the token list, including tokens on hidden channels.
    pub token_index: usize,
    /// Code points of the text that the lexer set instead of the matched input.
    pub text: Option<Vec<u32>>,
}

/// The tokens of an input together with the input, which holds the text of most tokens.
#[derive(Clone, Debug, Default)]
pub struct Tokens {
    /// The input as code points; lone surrogates are kept as they are.
    pub input: Vec<u32>,
    /// The tokens, ending with an EOF token.
    pub tokens: Vec<Token>,
}

impl Tokens {
    /// The code points of `token`'s text like ANTLR's `CommonToken.getText()`: `<EOF>` for an EOF
    /// token past the end of the input.
    pub fn code_points<'a>(&'a self, token: &'a Token) -> Cow<'a, [u32]> {
        if let Some(text) = &token.text {
            return Cow::Borrowed(text);
        }
        if token.start >= self.input.len() && token.token_type == EOF {
            return Cow::Owned(to_code_points("<EOF>"));
        }
        Cow::Borrowed(
            &self.input[token.start.min(self.input.len())..token.end.min(self.input.len())],
        )
    }

    /// The text of `token`, replacing lone surrogates with U+FFFD.
    pub fn text(&self, token: &Token) -> String {
        code_points_to_string(&self.code_points(token))
    }
}

/// The code points of `text`, for messages and token text.
pub fn to_code_points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

/// Escapes line feeds, carriage returns, and tabs for error messages, like
/// `Lexer.getErrorDisplay` and `DefaultErrorStrategy.escapeWSAndQuote`.
pub(crate) fn escape_ws(code_points: &[u32]) -> Vec<u32> {
    let mut escaped = Vec::with_capacity(code_points.len());
    for &c in code_points {
        match char::from_u32(c) {
            Some('\n') => escaped.extend(to_code_points("\\n")),
            Some('\r') => escaped.extend(to_code_points("\\r")),
            Some('\t') => escaped.extend(to_code_points("\\t")),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Converts code points to a string, replacing lone surrogates with U+FFFD.
pub fn code_points_to_string(code_points: &[u32]) -> String {
    code_points
        .iter()
        .map(|&c| char::from_u32(c).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Token names used for error messages and parse tree output.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    pub literal_names: Vec<Option<String>>,
    pub symbolic_names: Vec<Option<String>>,
}

impl Vocabulary {
    pub fn display_name(&self, token_type: i32) -> String {
        if token_type >= 0 {
            let index = token_type as usize;
            if let Some(Some(name)) = self.literal_names.get(index) {
                return name.clone();
            }
            if let Some(Some(name)) = self.symbolic_names.get(index) {
                return name.clone();
            }
        } else if token_type == EOF {
            return "EOF".to_string();
        }
        token_type.to_string()
    }
}

/// A view over lexed tokens that skips tokens off one channel, like `CommonTokenStream`.
pub(crate) struct TokenStream<'a> {
    tokens: &'a Tokens,
    channel: i32,
    p: usize,
    /// The highest token index that ANTLR's `BufferedTokenStream`, which lexes lazily, would have
    /// fetched so far; the host reports lexer errors when their tokens are fetched.
    fetched: Cell<usize>,
}

impl<'a> TokenStream<'a> {
    /// Starts at token `start`, or the next one on `channel`; `tokens` must end with an EOF token.
    pub(crate) fn new(tokens: &'a Tokens, start: usize, channel: i32) -> Self {
        let mut stream = Self {
            tokens,
            channel,
            p: 0,
            fetched: Cell::new(0),
        };
        stream.p = stream.next_on_channel(start);
        stream
    }

    pub(crate) fn fetched(&self) -> usize {
        self.fetched.get()
    }

    fn fetch(&self, i: usize) {
        if i > self.fetched.get() {
            self.fetched.set(i.min(self.tokens.tokens.len() - 1));
        }
    }

    pub(crate) fn tokens(&self) -> &'a [Token] {
        &self.tokens.tokens
    }

    pub(crate) fn token_code_points(&self, index: usize) -> Cow<'a, [u32]> {
        self.tokens.code_points(&self.tokens.tokens[index])
    }

    pub(crate) fn index(&self) -> usize {
        self.p
    }

    pub(crate) fn seek(&mut self, index: usize) {
        self.p = self.next_on_channel(index);
    }

    pub(crate) fn consume(&mut self) {
        if self.tokens()[self.p].token_type != EOF {
            self.p = self.next_on_channel(self.p + 1);
        }
    }

    pub(crate) fn la(&self, k: isize) -> i32 {
        self.lt(k).map_or(INVALID_TYPE, |t| t.token_type)
    }

    pub(crate) fn lt(&self, k: isize) -> Option<&'a Token> {
        match k {
            0 => None,
            k if k < 0 => self.lb(k.unsigned_abs()),
            k => {
                let mut i = self.p;
                for _ in 1..k {
                    if i + 1 < self.tokens().len() {
                        i = self.next_on_channel(i + 1);
                    }
                }
                Some(&self.tokens()[i])
            }
        }
    }

    fn lb(&self, k: usize) -> Option<&'a Token> {
        if k > self.p {
            return None;
        }
        let mut i = self.p as isize;
        for _ in 0..k {
            if i <= 0 {
                return None;
            }
            i = self.previous_on_channel(i - 1);
        }
        if i < 0 {
            None
        } else {
            Some(&self.tokens()[i as usize])
        }
    }

    fn next_on_channel(&self, mut i: usize) -> usize {
        let tokens = self.tokens();
        if i >= tokens.len() {
            return tokens.len() - 1;
        }
        self.fetch(i);
        while tokens[i].channel != self.channel {
            if tokens[i].token_type == EOF {
                return i;
            }
            i += 1;
            self.fetch(i);
        }
        i
    }

    fn previous_on_channel(&self, mut i: isize) -> isize {
        let tokens = self.tokens();
        while i >= 0 && tokens[i as usize].channel != self.channel {
            if tokens[i as usize].token_type == EOF {
                return i;
            }
            i -= 1;
        }
        i
    }

    /// Concatenates the code points of the tokens from `start` to `stop` (inclusive) on all
    /// channels.
    pub(crate) fn code_points(&self, start: usize, stop: usize) -> Vec<u32> {
        let tokens = self.tokens();
        tokens[start..=stop.min(tokens.len() - 1)]
            .iter()
            .take_while(|t| t.token_type != EOF)
            .flat_map(|t| self.tokens.code_points(t).into_owned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(token_type: i32, channel: i32, index: usize) -> Token {
        Token {
            token_type,
            channel,
            start: index,
            end: index + 1,
            line: 1,
            column: index,
            token_index: index,
            text: None,
        }
    }

    #[test]
    fn lookbehind_past_the_first_token_on_the_channel_is_none() {
        let tokens = Tokens {
            input: "a b".chars().map(u32::from).collect(),
            tokens: vec![
                token(1, DEFAULT_CHANNEL, 0),
                token(2, HIDDEN_CHANNEL, 1),
                token(1, DEFAULT_CHANNEL, 2),
                token(EOF, DEFAULT_CHANNEL, 3),
            ],
        };
        let stream = TokenStream::new(&tokens, 2, DEFAULT_CHANNEL);
        assert_eq!(stream.lt(-1).map(|t| t.token_index), Some(0));
        assert_eq!(stream.lt(-2), None);
    }
}
