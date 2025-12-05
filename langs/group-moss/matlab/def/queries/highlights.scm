; MATLAB highlights query

; Comments
(comment) @comment

; Strings
(string) @string
(string_content) @string
(formatting_sequence) @string.special
(escape_sequence) @string.escape

; Numbers
(number) @number

; Function definitions
(function_definition
  (function_signature
    (identifier) @function.definition))

; Function calls
(function_call
  (identifier) @function.call)

; Command syntax
(command
  (command_name) @function.call)

; Class definitions
(class_definition
  (identifier) @type.definition)

; Properties
(property
  (property_name) @property)

; Class property access
(class_property
  (identifier) @property)

; Field expressions
(field_expression
  (identifier) @property)

; Identifiers
(identifier) @variable

; Ignored argument
(ignored_argument) @comment

; Iterator in for loops
(iterator
  (identifier) @variable)

; Keywords
[
  "function"
  "end"
  "return"
  "classdef"
  "properties"
  "methods"
  "events"
  "enumeration"
] @keyword

[
  "if"
  "elseif"
  "else"
  "switch"
  "case"
  "otherwise"
] @keyword.conditional

[
  "for"
  "parfor"
  "while"
  "spmd"
] @keyword.repeat

[
  "try"
  "catch"
] @keyword.exception

[
  "break"
  "continue"
] @keyword

[
  "global"
  "persistent"
] @keyword

; Operators (simplified - just the common punctuation)
[
  "="
  "+"
  "-"
  "*"
  "/"
  "^"
  "'"
  "@"
  ":"
  "~"
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
  ","
  ";"
  "."
] @punctuation.delimiter

; Line continuation
(line_continuation) @punctuation.special

; Attributes
(attribute) @attribute
(attributes) @attribute
