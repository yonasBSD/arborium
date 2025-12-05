; D language highlights query

; Comments
(line_comment) @comment
(block_comment) @comment
(nesting_block_comment) @comment

; Strings
(double_quoted_string) @string
(wysiwyg_string) @string
(alternate_wysiwyg_string) @string
(hex_string) @string
(delimited_string) @string
(token_string) @string

; Characters
(character_literal) @character

; Numbers
(integer_literal) @number
(float_literal) @number

; Boolean and null literals
"true" @constant.builtin
"false" @constant.builtin
"null" @constant.builtin

; This reference
"this" @variable.builtin

; Keywords
[
  "abstract"
  "alias"
  "align"
  "asm"
  "assert"
  "auto"
  "body"
  "break"
  "case"
  "cast"
  "catch"
  "class"
  "const"
  "continue"
  "debug"
  "default"
  "delete"
  "deprecated"
  "do"
  "else"
  "enum"
  "export"
  "extern"
  "final"
  "finally"
  "for"
  "foreach"
  "foreach_reverse"
  "function"
  "goto"
  "if"
  "immutable"
  "import"
  "in"
  "inout"
  "interface"
  "invariant"
  "is"
  "lazy"
  "mixin"
  "module"
  "new"
  "nothrow"
  "out"
  "override"
  "package"
  "pragma"
  "private"
  "protected"
  "public"
  "pure"
  "ref"
  "return"
  "scope"
  "shared"
  "static"
  "struct"
  "super"
  "switch"
  "synchronized"
  "template"
  "throw"
  "try"
  "typeid"
  "typeof"
  "union"
  "unittest"
  "version"
  "while"
  "with"
] @keyword

; Fundamental types
[
  "void"
  "bool"
  "byte"
  "ubyte"
  "short"
  "ushort"
  "int"
  "uint"
  "long"
  "ulong"
  "float"
  "double"
  "real"
  "char"
  "wchar"
  "dchar"
] @type.builtin

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "^^"
  "&"
  "|"
  "^"
  "~"
  "!"
  "&&"
  "||"
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "="
  "?"
  ":"
  "=>"
] @operator

; Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ";"
  ","
  "."
] @punctuation.delimiter

; Identifiers (fallback)
(identifier) @variable
