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
    pub text: String,
}

/// Token names used for error messages and parse tree output.
#[derive(Clone, Copy, Debug)]
pub struct Vocabulary {
    pub literal_names: &'static [Option<&'static str>],
    pub symbolic_names: &'static [Option<&'static str>],
}

impl Vocabulary {
    pub fn display_name(&self, token_type: i32) -> String {
        if token_type >= 0 {
            let index = token_type as usize;
            if let Some(Some(name)) = self.literal_names.get(index) {
                return (*name).to_string();
            }
            if let Some(Some(name)) = self.symbolic_names.get(index) {
                return (*name).to_string();
            }
        } else if token_type == EOF {
            return "EOF".to_string();
        }
        token_type.to_string()
    }
}

/// A view over lexed tokens that skips tokens off the default channel, like `CommonTokenStream`.
pub(crate) struct TokenStream<'a> {
    tokens: &'a [Token],
    p: usize,
}

impl<'a> TokenStream<'a> {
    /// `tokens` must end with an EOF token.
    pub(crate) fn new(tokens: &'a [Token]) -> Self {
        let mut stream = Self { tokens, p: 0 };
        stream.p = stream.next_on_channel(0);
        stream
    }

    pub(crate) fn tokens(&self) -> &'a [Token] {
        self.tokens
    }

    pub(crate) fn index(&self) -> usize {
        self.p
    }

    pub(crate) fn seek(&mut self, index: usize) {
        self.p = self.next_on_channel(index);
    }

    pub(crate) fn consume(&mut self) {
        if self.tokens[self.p].token_type != EOF {
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
                    if i + 1 < self.tokens.len() {
                        i = self.next_on_channel(i + 1);
                    }
                }
                Some(&self.tokens[i])
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
                break;
            }
            i = self.previous_on_channel(i - 1);
        }
        if i < 0 {
            None
        } else {
            Some(&self.tokens[i as usize])
        }
    }

    fn next_on_channel(&self, mut i: usize) -> usize {
        if i >= self.tokens.len() {
            return self.tokens.len() - 1;
        }
        while self.tokens[i].channel != DEFAULT_CHANNEL {
            if self.tokens[i].token_type == EOF {
                return i;
            }
            i += 1;
        }
        i
    }

    fn previous_on_channel(&self, mut i: isize) -> isize {
        while i >= 0 && self.tokens[i as usize].channel != DEFAULT_CHANNEL {
            if self.tokens[i as usize].token_type == EOF {
                return i;
            }
            i -= 1;
        }
        i
    }

    /// Concatenates the text of the tokens from `start` to `stop` (inclusive) on all channels.
    pub(crate) fn text(&self, start: usize, stop: usize) -> String {
        self.tokens[start..=stop.min(self.tokens.len() - 1)]
            .iter()
            .take_while(|t| t.token_type != EOF)
            .map(|t| t.text.as_str())
            .collect()
    }
}
