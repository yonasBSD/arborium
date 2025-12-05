; VHDL syntax highlighting queries (simplified)

; Comments
(comment) @comment

; Strings
(string_literal) @string
(character_literal) @character
(bit_string_literal) @string.special

; Numbers
(integer_decimal) @number
(real_decimal) @number
(based_integer) @number
(based_real) @number

; Keywords
[
  "abs"
  "access"
  "after"
  "alias"
  "all"
  "and"
  "architecture"
  "array"
  "assert"
  "attribute"
  "begin"
  "block"
  "body"
  "buffer"
  "bus"
  "case"
  "component"
  "configuration"
  "constant"
  "disconnect"
  "downto"
  "else"
  "elsif"
  "end"
  "entity"
  "exit"
  "file"
  "for"
  "function"
  "generate"
  "generic"
  "group"
  "guarded"
  "if"
  "impure"
  "in"
  "inertial"
  "inout"
  "is"
  "label"
  "library"
  "linkage"
  "literal"
  "loop"
  "map"
  "mod"
  "nand"
  "new"
  "next"
  "nor"
  "not"
  "null"
  "of"
  "on"
  "open"
  "or"
  "others"
  "out"
  "package"
  "port"
  "postponed"
  "procedure"
  "process"
  "protected"
  "pure"
  "range"
  "record"
  "register"
  "reject"
  "rem"
  "report"
  "return"
  "rol"
  "ror"
  "select"
  "severity"
  "signal"
  "shared"
  "sla"
  "sll"
  "sra"
  "srl"
  "subtype"
  "then"
  "to"
  "transport"
  "type"
  "unaffected"
  "units"
  "until"
  "use"
  "variable"
  "wait"
  "when"
  "while"
  "with"
  "xnor"
  "xor"
] @keyword

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "**"
  "&"
  "="
  "/="
  "<"
  ">"
  ":="
  "=>"
] @operator

; Punctuation
[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  ","
  ";"
  ":"
  "."
] @punctuation.delimiter

; Identifiers (fallback)
(identifier) @variable
