grammar Misc;

// Parser wildcards and negated sets, `type()` actions, fragments, escapes, non-greedy loops, and
// non-BMP input.
file
    : item+ EOF
    ;

item
    : 'let' ID '=' value ';'
    | 'raw' ~';'* ';'
    | 'any' . . ';'
    | list
    ;

value
    : STRING
    | NUMBER
    | ID
    | list
    ;

list
    : '[' (value (',' value)*)? ']'
    ;

ID
    : LETTER (LETTER | DIGIT)*
    ;

NUMBER
    : DIGIT+
    ;

STRING
    : '"' ('\\' . | ~["\\])* '"'
    ;

TAG
    : '#' [a-z]+ -> type(ID)
    ;

COMMENT
    : '(*' .*? '*)' -> skip
    ;

WS
    : [ \t\r\n]+ -> skip
    ;

fragment LETTER
    : [a-zA-Z_À-\u{10FFFF}]
    ;

fragment DIGIT
    : [0-9]
    ;
