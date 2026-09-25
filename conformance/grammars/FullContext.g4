grammar FullContext;

// Deciding whether `e` matches INT depends on how the rule was invoked, so SLL prediction
// conflicts and full-context LL prediction decides.
s
    : '$' a
    | '@' b
    ;

a
    : e ID
    ;

b
    : e INT ID
    ;

e
    : INT
    |
    ;

ID
    : [a-z]+
    ;

INT
    : [0-9]+
    ;

WS
    : [ \t\n]+ -> skip
    ;
