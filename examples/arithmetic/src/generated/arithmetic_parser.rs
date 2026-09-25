// Generated from Arithmetic.g4 by ultra-parser. Do not edit.

// Users rarely need every token type and rule index.
#![allow(dead_code)]

use ultra_parser_runtime::GrammarData;

pub static GRAMMAR: GrammarData = GrammarData {
    grammar_file_name: "Arithmetic.g4",
    recognizer_name: "ArithmeticParser",
    serialized_atn: &[
        4,1,9,29,2,0,7,0,2,1,7,1,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,3,1,13,
        8,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,5,1,24,8,1,10,1,12,1,27,
        9,1,1,1,0,1,2,2,0,2,0,2,1,0,3,4,2,0,2,2,5,5,31,0,4,3,2,1,0,2,12,
        1,0,0,0,4,1,5,0,0,1,5,6,6,1,-1,0,6,7,5,2,0,0,7,13,3,2,1,5,8,9,5,
        6,0,0,9,10,3,2,1,0,10,13,5,7,0,0,11,13,5,8,0,0,12,5,1,0,0,0,12,8,
        1,0,0,0,12,11,1,0,0,0,13,25,1,0,0,0,14,15,10,6,0,0,15,16,5,1,0,0,
        16,24,3,2,1,6,17,18,10,4,0,0,18,19,7,0,0,0,19,24,3,2,1,5,20,21,10,
        3,0,0,21,22,7,1,0,0,22,24,3,2,1,4,23,14,1,0,0,0,23,17,1,0,0,0,23,
        20,1,0,0,0,24,27,1,0,0,0,25,23,1,0,0,0,25,26,1,0,0,0,26,3,1,0,0,
        0,27,25,1,0,0,0,3,12,23,25
    ],
    rule_names: &[
        "program",
        "expr"
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
    channel_names: &[],
    mode_names: &[],
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
    pub const PROGRAM: usize = 0;
    pub const EXPR: usize = 1;
}
