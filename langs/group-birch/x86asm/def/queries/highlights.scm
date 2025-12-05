; Comments
(line_comment) @comment

; Labels
(label_definition
  name: (label) @label)

; Directives
(directive_name) @keyword.directive

; Instructions
(mnemonic) @function.builtin
(prefix) @keyword

; Registers
(register) @variable.builtin

; Numbers
(number) @number
(immediate (number) @number)

; Strings
(string) @string

; Memory
(memory_operand) @punctuation.bracket
(ptr_size) @type

; Operators in memory expressions
(binary_expression
  ["+" "-" "*"] @operator)

; Identifiers (symbols)
(identifier) @variable
