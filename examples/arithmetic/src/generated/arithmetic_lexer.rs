// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

// Users rarely need every token type and rule index.
#![allow(dead_code)]

use ultra_parser_runtime::GrammarData;

pub static GRAMMAR: GrammarData = GrammarData {
    grammar_file_name: "Arithmetic.g4",
    recognizer_name: "ArithmeticLexer",
    serialized_atn: &[
        4,0,9,34,6,-1,2,0,7,0,2,1,7,1,2,2,7,2,2,3,7,3,2,4,7,4,2,5,7,5,2,
        6,7,6,2,7,7,7,2,8,7,8,4,7,20,8,7,11,7,12,7,21,1,7,4,7,25,8,7,11,
        7,12,7,26,3,7,29,8,7,4,8,31,8,8,11,8,12,8,32,0,0,9,1,1,3,2,5,3,7,
        4,9,5,11,6,13,7,15,8,17,9,1,0,2,1,0,48,57,3,0,9,10,13,13,32,32,37,
        0,1,1,0,0,0,0,3,1,0,0,0,0,5,1,0,0,0,0,7,1,0,0,0,0,9,1,0,0,0,0,11,
        1,0,0,0,0,13,1,0,0,0,0,15,1,0,0,0,0,17,1,0,0,0,1,2,5,94,0,0,3,4,
        5,45,0,0,5,6,5,42,0,0,7,8,5,47,0,0,9,10,5,43,0,0,11,12,5,40,0,0,
        13,14,5,41,0,0,15,19,1,0,0,0,17,30,1,0,0,0,19,20,7,0,0,0,20,21,1,
        0,0,0,21,19,1,0,0,0,21,22,1,0,0,0,22,28,1,0,0,0,23,24,5,46,0,0,24,
        25,7,0,0,0,25,26,1,0,0,0,26,24,1,0,0,0,26,27,1,0,0,0,27,29,1,0,0,
        0,28,23,1,0,0,0,28,29,1,0,0,0,29,16,1,0,0,0,30,31,7,1,0,0,31,32,
        1,0,0,0,32,30,1,0,0,0,32,33,1,0,0,0,33,18,6,8,0,0,5,0,21,26,28,32,
        1,6,0,0
    ],
    rule_names: &[
        "T__0",
        "T__1",
        "T__2",
        "T__3",
        "T__4",
        "T__5",
        "T__6",
        "NUMBER",
        "WS"
    ],
    literal_names: &[
        None,
        Some("'^'"),
        Some("'-'"),
        Some("'*'"),
        Some("'/'"),
        Some("'+'"),
        Some("'('"),
        Some("')'"),
        None,
        None
    ],
    symbolic_names: &[
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some("NUMBER"),
        Some("WS")
    ],
    channel_names: &["DEFAULT_TOKEN_CHANNEL", "HIDDEN"],
    mode_names: &["DEFAULT_MODE"],
};

/// Token types.
pub mod token {
    pub const T__0: i32 = 1;
    pub const T__1: i32 = 2;
    pub const T__2: i32 = 3;
    pub const T__3: i32 = 4;
    pub const T__4: i32 = 5;
    pub const T__5: i32 = 6;
    pub const T__6: i32 = 7;
    pub const NUMBER: i32 = 8;
    pub const WS: i32 = 9;
}

/// Rule indexes.
pub mod rule {
    pub const T__0: usize = 0;
    pub const T__1: usize = 1;
    pub const T__2: usize = 2;
    pub const T__3: usize = 3;
    pub const T__4: usize = 4;
    pub const T__5: usize = 5;
    pub const T__6: usize = 6;
    pub const NUMBER: usize = 7;
    pub const WS: usize = 8;
}
