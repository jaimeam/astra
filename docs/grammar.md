# Astra Language Grammar

Formal EBNF grammar for the Astra programming language.

This grammar is derived from the parser implementation in `src/parser/parser.rs`,
the lexer in `src/parser/lexer.rs`, and the AST definitions in `src/parser/ast.rs`.

## Notation

- `::=` defines a production rule
- `|` separates alternatives
- `[...]` denotes optional elements (zero or one)
- `{...}` denotes repetition (zero or more)
- `'...'` denotes terminal strings (keywords, operators, punctuation)
- `(...)` groups elements
- `,` within a rule is literal only when quoted; otherwise it is sequencing

---

## Module

```ebnf
Module         ::= 'module' ModulePath { Item }

ModulePath     ::= IDENT { '.' IDENT }
```

---

## Items

```ebnf
Item           ::= ImportDecl
                  | TypeDef
                  | EnumDef
                  | FnDef
                  | TraitDef
                  | ImplBlock
                  | EffectDef
                  | TestBlock
                  | PropertyBlock
```

### Import Declarations

```ebnf
ImportDecl     ::= [ 'public' ] 'import' ModulePath [ ImportTail ]

ImportTail     ::= 'as' IDENT
                  | '.' '{' IDENT { ',' IDENT } [ ',' ] '}'
```

### Type Definitions

```ebnf
TypeDef        ::= 'type' IDENT [ TypeParams ] '=' TypeExpr [ 'invariant' Expr ]
```

### Enum Definitions

```ebnf
EnumDef        ::= 'enum' IDENT [ TypeParams ] '=' [ '|' ] Variant { '|' Variant }

Variant        ::= IDENT [ '(' Field { ',' Field } [ ',' ] ')' ]

Field          ::= IDENT ':' TypeExpr
```

### Function Definitions

```ebnf
FnDef          ::= [ 'public' ] [ 'async' ] 'fn' IDENT [ TypeParams ] '(' [ Params ] ')'
                   [ '->' TypeExpr ]
                   [ EffectsClause ]
                   { RequiresClause }
                   { EnsuresClause }
                   Block

Params         ::= Param { ',' Param } [ ',' ]

Param          ::= IDENT ':' TypeExpr
                  | 'self'
                  | Pattern ':' TypeExpr

EffectsClause  ::= 'effects' '(' IDENT { ',' IDENT } ')'

RequiresClause ::= 'requires' Expr

EnsuresClause  ::= 'ensures' Expr
```

### Trait Definitions

```ebnf
TraitDef       ::= 'trait' IDENT [ TypeParams ] '{' { FnSignature } '}'

FnSignature    ::= 'fn' IDENT '(' [ Params ] ')' [ '->' TypeExpr ]
```

### Impl Blocks

```ebnf
ImplBlock      ::= 'impl' IDENT 'for' TypeExpr '{' { FnDef } '}'
```

### Effect Definitions

```ebnf
EffectDef      ::= 'effect' IDENT '{' { FnSignature } '}'
```

### Test Blocks

```ebnf
TestBlock      ::= 'test' TEXT_LIT [ UsingClause ] Block
```

### Property Blocks

```ebnf
PropertyBlock  ::= 'property' TEXT_LIT [ UsingClause ] Block
```

### Using Clause

```ebnf
UsingClause    ::= 'using' 'effects' '(' [ EffectBinding { ',' EffectBinding } [ ',' ] ] ')'

EffectBinding  ::= IDENT '=' Expr
```

---

## Type Parameters and Arguments

```ebnf
TypeParams     ::= '[' TypeParam { ',' TypeParam } ']'

TypeParam      ::= IDENT [ ':' IDENT ]

TypeArgs       ::= '[' TypeExpr { ',' TypeExpr } ']'
```

---

## Type Expressions

```ebnf
TypeExpr       ::= NamedType
                  | RecordType
                  | FunctionType
                  | TupleType
                  | UnitType
                  | '(' TypeExpr ')'

NamedType      ::= IDENT [ TypeArgs ]

RecordType     ::= '{' [ Field { ',' Field } [ ',' ] ] '}'

FunctionType   ::= '(' [ TypeExpr { ',' TypeExpr } [ ',' ] ] ')' '->' TypeExpr [ EffectsClause ]

TupleType      ::= '(' TypeExpr ',' TypeExpr { ',' TypeExpr } [ ',' ] ')'

UnitType       ::= '(' ')'
```

---

## Blocks

```ebnf
Block          ::= '{' { BlockElement } [ Expr ] '}'

BlockElement   ::= Stmt
                  | LocalFnDef
                  | Expr '=' Expr
                  | Expr CompoundAssignOp Expr
                  | Expr
```

---

## Statements

```ebnf
Stmt           ::= LetStmt
                  | LetPatternStmt
                  | ReturnStmt

LetStmt        ::= 'let' [ 'mut' ] IDENT [ ':' TypeExpr ] '=' Expr

LetPatternStmt ::= 'let' Pattern [ ':' TypeExpr ] '=' Expr

ReturnStmt     ::= 'return' [ Expr ]
```

### Local Function Definition

```ebnf
LocalFnDef    ::= 'fn' IDENT [ TypeParams ] '(' [ LambdaParams ] ')' [ '->' TypeExpr ] Block
```

---

## Expressions

### Precedence (lowest to highest)

| Precedence | Operators                  | Associativity |
|------------|----------------------------|---------------|
| 0          | `\|>`                      | Left          |
| 1          | `..` `..=`                 | Left          |
| 2          | `or`                       | Left          |
| 3          | `and`                      | Left          |
| 4          | `==` `!=`                  | Left          |
| 5          | `<` `<=` `>` `>=`          | Left          |
| 6          | `+` `-`                    | Left          |
| 7          | `*` `/` `%`               | Left          |
| (prefix)   | `not` `-` `await`          | Right (unary) |
| (postfix)  | `()` `.` `?` `?else` `[]` | Left          |

### Expression Grammar

```ebnf
Expr           ::= BinaryExpr

BinaryExpr     ::= PipeExpr

PipeExpr       ::= RangeExpr { '|>' RangeExpr }

RangeExpr      ::= OrExpr [ ( '..' | '..=' ) OrExpr ]

OrExpr         ::= AndExpr { 'or' AndExpr }

AndExpr        ::= EqExpr { 'and' EqExpr }

EqExpr         ::= CmpExpr { ( '==' | '!=' ) CmpExpr }

CmpExpr        ::= AddExpr { ( '<' | '<=' | '>' | '>=' ) AddExpr }

AddExpr        ::= MulExpr { ( '+' | '-' ) MulExpr }

MulExpr        ::= UnaryExpr { ( '*' | '/' | '%' ) UnaryExpr }

UnaryExpr      ::= 'not' UnaryExpr
                  | '-' UnaryExpr
                  | 'await' UnaryExpr
                  | PostfixExpr

PostfixExpr    ::= PrimaryExpr { PostfixOp }

PostfixOp      ::= '(' [ Expr { ',' Expr } [ ',' ] ] ')'
                  | '.' IDENT '(' [ Expr { ',' Expr } [ ',' ] ] ')'
                  | '.' IDENT
                  | '.' INT_LIT
                  | '?else' Expr
                  | '?'
                  | '[' Expr ']'
```

### Primary Expressions

```ebnf
PrimaryExpr    ::= INT_LIT
                  | FLOAT_LIT
                  | 'true'
                  | 'false'
                  | TEXT_LIT
                  | MULTILINE_TEXT_LIT
                  | IDENT
                  | UnitLit
                  | TupleLit
                  | ParenExpr
                  | ListLit
                  | RecordExpr
                  | BlockExpr
                  | LambdaExpr
                  | ForExpr
                  | WhileExpr
                  | IfExpr
                  | MatchExpr
                  | AssertExpr
                  | 'break'
                  | 'continue'
                  | '???'

UnitLit        ::= '(' ')'

TupleLit       ::= '(' Expr ',' [ Expr { ',' Expr } ] [ ',' ] ')'

ParenExpr      ::= '(' Expr ')'

ListLit        ::= '[' [ Expr { ',' Expr } [ ',' ] ] ']'

RecordExpr     ::= '{' '}'
                  | '{' IDENT '=' Expr { ',' IDENT '=' Expr } [ ',' ] '}'

BlockExpr      ::= '{' { BlockElement } [ Expr ] '}'

LambdaExpr     ::= 'fn' '(' [ LambdaParams ] ')' [ '->' TypeExpr ] Block

LambdaParams   ::= LambdaParam { ',' LambdaParam } [ ',' ]

LambdaParam    ::= IDENT [ ':' TypeExpr ]

ForExpr        ::= 'for' ( IDENT | Pattern ) 'in' Expr Block

WhileExpr      ::= 'while' Expr Block

IfExpr         ::= 'if' Expr Block [ 'else' ( IfExpr | Block ) ]
                  | 'if' Expr 'then' Expr [ 'else' Expr ]

MatchExpr      ::= 'match' Expr '{' { MatchArm [ ',' ] } '}'

MatchArm       ::= Pattern [ 'if' Expr ] '=>' Expr

AssertExpr     ::= 'assert' '(' Expr [ ',' Expr ] ')'
                  | 'assert' Expr
```

---

## Patterns

```ebnf
Pattern        ::= '_'
                  | INT_LIT
                  | FLOAT_LIT
                  | 'true'
                  | 'false'
                  | TEXT_LIT
                  | IDENT
                  | VariantPattern
                  | RecordPattern
                  | TuplePattern

VariantPattern ::= UPPER_IDENT [ '(' [ Pattern { ',' Pattern } [ ',' ] ] ')' ]

RecordPattern  ::= '{' [ RecordFieldPat { ',' RecordFieldPat } [ ',' ] ] '}'

RecordFieldPat ::= IDENT [ '=' Pattern ]

TuplePattern   ::= '(' [ Pattern { ',' Pattern } [ ',' ] ] ')'
```

Note: An identifier starting with an uppercase letter is parsed as a variant
pattern (with no fields), while a lowercase identifier is parsed as a binding
pattern.

---

## Assignment and Compound Assignment

Within blocks, assignment and compound assignment are parsed as block elements
rather than as standalone statements:

```ebnf
CompoundAssignOp ::= '+=' | '-=' | '*=' | '/=' | '%='
```

Compound assignment `x += expr` is desugared to `x = x + expr` (and similarly
for the other operators).

---

## String Interpolation

String interpolation is supported within both regular and multiline string
literals. Interpolation segments use the `${expr}` syntax:

```ebnf
InterpolatedString ::= '"' { TextSegment | '${' Expr '}' } '"'
                      | '"""' { TextSegment | '${' Expr '}' } '"""'
```

Multiline strings (`"""..."""`) are automatically dedented: the first line (if
empty) and last line (if whitespace-only) are stripped, then the minimum common
leading whitespace is removed from all remaining lines.

---

## Escape Sequences

Within string literals, the following escape sequences are recognized:

```
\n   newline
\r   carriage return
\t   tab
\\   backslash
\"   double quote
\0   null
\$   literal dollar sign (prevents interpolation)
```

---

## Comments

```ebnf
LineComment    ::= '#' { any character except newline }

DocComment     ::= '##' { any character except newline }
```

Comments are lexed and discarded by the parser. Doc comments (`##`) are
preserved for tooling (e.g., the formatter) but do not appear in the AST.

---

## Lexical Elements

```ebnf
INT_LIT        ::= DIGIT { DIGIT }

FLOAT_LIT      ::= DIGIT { DIGIT } '.' DIGIT { DIGIT }

TEXT_LIT       ::= '"' { CHAR | ESCAPE } '"'

MULTILINE_TEXT_LIT ::= '"""' { any character } '"""'

IDENT          ::= ( LETTER | '_' ) { LETTER | DIGIT | '_' }

UPPER_IDENT    ::= UPPER_LETTER { LETTER | DIGIT | '_' }

LETTER         ::= 'a'..'z' | 'A'..'Z'

UPPER_LETTER   ::= 'A'..'Z'

DIGIT          ::= '0'..'9'

CHAR           ::= any character except '"' and '\'

ESCAPE         ::= '\' ( 'n' | 'r' | 't' | '\' | '"' | '0' | '$' )
```

---

## Keywords

The following identifiers are reserved keywords:

```
and       as        assert    async     await     break     continue
effect    effects   else      ensures   enum      false     fn
for       forall    if        impl      import    in        invariant
let       match     module    mut       not       or        property
public    requires  return    test      then      trait     true
type      using     while
```

---

## Operators and Punctuation

```
+    -    *    /    %
+=   -=   *=   /=   %=
==   !=   <    >    <=   >=
|>   ->   =>   ..   ..=
(    )    {    }    [    ]
,    :    =    .    |    ?    ?else
_    ???
```
