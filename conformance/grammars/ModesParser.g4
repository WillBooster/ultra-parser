parser grammar ModesParser;

options {
    tokenVocab = ModesLexer;
}

document
    : content* EOF
    ;

content
    : TEXT
    | ENTITY
    | element
    ;

element
    : OPEN SLASH? NAME attribute* (CLOSE | SLASH_CLOSE)
    ;

attribute
    : NAME (EQ STRING)?
    ;
