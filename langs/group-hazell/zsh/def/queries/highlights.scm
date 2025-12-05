; Comments
(comment) @comment

; Strings
(string) @string
(raw_string) @string
(escape_sequence) @string.escape

; Variables
(expansion) @variable

; Command substitution
(command_substitution) @embedded

; Numbers
(number) @number

; Words (commands, arguments, etc) - low priority
(word) @string.special
