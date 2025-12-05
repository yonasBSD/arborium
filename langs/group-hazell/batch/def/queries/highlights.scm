; Batch/CMD highlights query

; Comments
(comment) @comment

; Strings
(string) @string

; Numbers
(number) @number

; Variables
(variable_reference) @variable
(variable_declaration
  (identifier) @variable)

; Labels (functions)
(function_definition) @function

; Echo off directive
(echooff) @keyword

; Keywords (the grammar bundles all keywords into a single node type)
(keyword) @keyword

; Identifiers
(identifier) @variable
