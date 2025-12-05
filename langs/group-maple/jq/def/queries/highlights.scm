; jq highlights query

; Comments
(comment) @comment

; Strings
(string) @string
(string_interp) @string
(format) @string.special

; Numbers
(number) @number

; Keywords (grammar has a 'keyword' node type)
(keyword) @keyword

; Built-in keywords (anonymous tokens)
[
  "if"
  "then"
  "else"
  "elif"
  "end"
  "as"
  "def"
  "reduce"
  "foreach"
  "try"
  "catch"
  "and"
  "or"
  "import"
  "include"
  "module"
  "label"
  "break"
] @keyword

; Function definitions
(funcdef
  (identifier) @function.definition)

; Function names in calls
(funcname) @function.call

; Variables
(variable) @variable

; Identifiers (field access)
(identifier) @property

; Object keys
(objectkey) @property

; Index access
(index) @punctuation.bracket

; Recursion operator
(recurse) @operator

; Module/import
(import_) @keyword.import
(moduleheader) @keyword.import

; Operators
[
  "|"
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
  "//"
  "="
  "|="
  "+="
  "-="
  "*="
  "/="
  "%="
  "//="
  "?"
  "."
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
  ":"
  ";"
  ","
] @punctuation.delimiter

; Special values (anonymous tokens)
[
  "null"
  "true"
  "false"
] @constant.builtin
