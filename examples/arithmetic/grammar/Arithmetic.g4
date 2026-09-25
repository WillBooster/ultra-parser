grammar Arithmetic;

program
    : expr EOF
    ;

expr
    : <assoc = right> expr '^' expr
    | '-' expr
    | expr op = ('*' | '/') expr
    | expr op = ('+' | '-') expr
    | '(' expr ')'
    | NUMBER
    ;

NUMBER
    : [0-9]+ ('.' [0-9]+)?
    ;

WS
    : [ \t\r\n]+ -> skip
    ;
