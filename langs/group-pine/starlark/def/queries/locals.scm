;;; Program structure
(module) @scope

; Function with parameters, defines parameters
(parameters
  (parameter
    (identifier) @definition.parameter))

(parameters
  (parameter
    (default_parameter
      name: (identifier) @definition.parameter)))

; *args parameter
(parameters
  (parameter
    (list_splat_pattern
      (identifier) @definition.parameter)))

; **kwargs parameter
(parameters
  (parameter
    (dictionary_splat_pattern
      (identifier) @definition.parameter)))

; Function defines function and scope
((function_definition
  name: (identifier) @definition.function) @scope
 (#set! definition.function.scope "parent"))

;;; Loops
(for_statement
  left: (tuple_pattern
          (identifier) @definition.var))
(for_statement
  left: (identifier) @definition.var)

; for in list comprehension
(for_in_clause
  left: (identifier) @definition.var)
(for_in_clause
  left: (tuple_pattern
          (identifier) @definition.var))

(dictionary_comprehension) @scope
(list_comprehension) @scope

;;; Assignments

(assignment
 left: (identifier) @definition.var)

(assignment
 left: (tuple_pattern
   (identifier) @definition.var))

(assignment
 left: (attribute
   attribute: (identifier) @definition.field))

;;; REFERENCES
(identifier) @reference
