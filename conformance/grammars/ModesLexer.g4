lexer grammar ModesLexer;

// Markup with tags: modes, a mode stack, `more`, custom channels, and non-greedy loops.
channels {
    COMMENTS
}

COMMENT
    : '<!--' .*? '-->' -> channel(COMMENTS)
    ;

OPEN
    : '<' -> pushMode(TAG)
    ;

ENTITY
    : '&' [a-z]+ ';'
    ;

TEXT
    : ~[<&]+
    ;

mode TAG;

CLOSE
    : '>' -> popMode
    ;

SLASH_CLOSE
    : '/>' -> popMode
    ;

NAME
    : [a-zA-Z] [a-zA-Z0-9-]*
    ;

SLASH
    : '/'
    ;

EQ
    : '='
    ;

QUOTE
    : '"' -> more, pushMode(STRING_MODE)
    ;

TAG_WS
    : [ \t\r\n]+ -> skip
    ;

mode STRING_MODE;

STRING
    : '"' -> popMode
    ;

STRING_TEXT
    : ~'"' -> more
    ;
