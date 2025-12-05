; Verilog highlights query

; Comments
(comment) @comment

; Strings
(double_quoted_string) @string
(string_literal) @string

; Numbers
(binary_number) @number
(octal_number) @number
(decimal_number) @number
(hex_number) @number
(unsigned_number) @number
(fixed_point_number) @number
(real_number) @number

; Module declarations
(module_declaration
  (module_header
    (module_keyword) @keyword
    (simple_identifier) @type.definition))

; Module instantiation
(module_instantiation
  (simple_identifier) @type)

; Function declarations
(function_declaration
  (function_body_declaration
    (function_identifier
      (simple_identifier) @function.definition)))

; Task declarations
(task_declaration
  (task_body_declaration
    (task_identifier
      (simple_identifier) @function.definition)))

; Port declarations
(port_direction) @keyword
(port_identifier
  (simple_identifier) @variable)

; Identifiers
(simple_identifier) @variable
(escaped_identifier) @variable
(system_tf_identifier) @function.builtin

; Keywords
[
  "module"
  "endmodule"
  "input"
  "output"
  "inout"
  "wire"
  "reg"
  "logic"
  "integer"
  "real"
  "time"
  "realtime"
  "parameter"
  "localparam"
  "assign"
  "always"
  "always_comb"
  "always_ff"
  "always_latch"
  "initial"
  "begin"
  "end"
  "if"
  "else"
  "case"
  "casex"
  "casez"
  "default"
  "endcase"
  "for"
  "while"
  "repeat"
  "forever"
  "function"
  "endfunction"
  "task"
  "endtask"
  "generate"
  "endgenerate"
  "genvar"
  "posedge"
  "negedge"
  "or"
  "and"
  "not"
  "xor"
  "nand"
  "nor"
  "xnor"
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
  "==="
  "!=="
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
  "<<"
  ">>"
  "="
  "<="
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
  ":"
  "."
  "@"
  "#"
] @punctuation.delimiter
