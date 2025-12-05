; HLSL (High Level Shading Language) highlights query

; Comments
(comment) @comment

; Strings
(string_literal) @string
(char_literal) @character
(concatenated_string) @string
(raw_string_literal) @string

; Numbers
(number_literal) @number

; Boolean literals
(true) @constant.builtin
(false) @constant.builtin
(null) @constant.builtin

; Preprocessor
(preproc_include) @keyword.import
(preproc_def) @keyword
(preproc_function_def) @keyword
(preproc_if) @keyword.conditional
(preproc_ifdef) @keyword.conditional
(preproc_elifdef) @keyword.conditional
(preproc_else) @keyword.conditional
(preproc_elif) @keyword.conditional
(preproc_directive) @keyword

; Types
(primitive_type) @type.builtin
(type_identifier) @type

; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function.definition))

; Function calls
(call_expression
  function: (identifier) @function.call)

; Field access
(field_expression
  field: (field_identifier) @property)

; Keywords
[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "extern"
  "for"
  "if"
  "inline"
  "return"
  "static"
  "struct"
  "switch"
  "typedef"
  "while"
  "cbuffer"
  "register"
  "in"
  "out"
  "inout"
  "uniform"
  "volatile"
  "discard"
] @keyword

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "~"
  "="
  "+="
  "-="
  "*="
  "/="
  "++"
  "--"
  "?"
  ":"
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
(field_identifier) @property
