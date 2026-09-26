grammar Lookahead;

// The alternatives of `member` share an arbitrarily long prefix.
unit
    : member* EOF
    ;

member
    : modifier* type ID ';'
    | modifier* type ID '(' params? ')' block
    | modifier* 'class' ID '{' member* '}'
    ;

modifier
    : 'public'
    | 'static'
    | 'final'
    ;

type
    : ID ('.' ID)* ('[' ']')*
    | 'void'
    ;

params
    : param (',' param)*
    ;

param
    : type ID
    ;

block
    : '{' stmt* '}'
    ;

stmt
    : block
    | type ID ('=' expr)? ';'
    | expr ';'
    | 'return' expr? ';'
    ;

expr
    : expr '.' ID
    | expr '(' (expr (',' expr)*)? ')'
    | <assoc = right> expr '=' expr
    | ID
    | INT
    ;

ID
    : [a-zA-Z_] [a-zA-Z_0-9]*
    ;

INT
    : [0-9]+
    ;

LINE_COMMENT
    : '//' ~[\r\n]* -> channel(HIDDEN)
    ;

BLOCK_COMMENT
    : '/*' .*? '*/' -> skip
    ;

WS
    : [ \t\r\n]+ -> skip
    ;
