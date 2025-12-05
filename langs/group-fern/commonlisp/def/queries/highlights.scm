; Common Lisp highlights query

; Comments
(comment) @comment
(block_comment) @comment

; Strings
(str_lit) @string

; Characters
(char_lit) @character

; Numbers
(num_lit) @number
(complex_num_lit) @number

; Nil literal
(nil_lit) @constant.builtin

; Keywords (keyword symbols starting with :)
(kwd_lit) @constant
(kwd_symbol) @constant

; Function definitions
(defun
  (defun_header
    (defun_keyword) @keyword.function
    (sym_lit) @function))

; Defun keyword
(defun_keyword) @keyword.function

; Symbols (identifiers)
(sym_lit) @variable

; Package literals
(package_lit) @namespace

; Path literals
(path_lit) @string.special

; Quoting
(quoting_lit) @punctuation.special
(syn_quoting_lit) @punctuation.special
(unquoting_lit) @punctuation.special
(unquote_splicing_lit) @punctuation.special
(var_quoting_lit) @punctuation.special

; Reader macros
(include_reader_macro) @keyword.import
(self_referential_reader_macro) @punctuation.special
(read_cond_lit) @keyword.conditional
(splicing_read_cond_lit) @keyword.conditional

; Disabled expressions
(dis_expr) @comment

; Lists
(list_lit
  "(" @punctuation.bracket
  ")" @punctuation.bracket)

; Vectors
(vec_lit
  ")" @punctuation.bracket)

; Set literals
(set_lit) @punctuation.bracket

; Format specifiers
(format_specifier) @string.special
(format_directive_type) @string.special.symbol

; Loop macros and clauses
(loop_macro) @keyword
(loop_clause) @keyword
(for_clause) @keyword
(for_clause_word) @keyword
(do_clause) @keyword
(while_clause) @keyword
(repeat_clause) @keyword
(with_clause) @keyword
(condition_clause) @keyword
(termination_clause) @keyword
(accumulation_clause) @keyword
(accumulation_verb) @keyword

; Meta literals (reader macros)
(meta_lit) @attribute
(old_meta_lit) @attribute

; Fancy literals (like structures)
(fancy_literal) @type

; Array dimensions
(array_dimension) @number
