; Tree-sitter query example
; Highlight function definitions
(function_definition
  name: (identifier) @function)

; Highlight method calls
(call_expression
  function: (attribute
    attribute: (identifier) @method))

; Capture string literals
(string) @string

; Match if statements
(if_statement
  condition: (_) @condition
  consequence: (_) @consequence
  alternative: (_)? @alternative)

; Fold class definitions
(class_definition) @fold

; Locals for variable scoping
(function_definition) @local.scope
(parameter) @local.definition

; Injections for embedded languages
((string) @injection.content
 (#match? @injection.content "^r\"")
 (#set! injection.language "regex"))
