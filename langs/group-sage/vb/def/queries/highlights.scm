; VB.NET highlights query

; Comments
(comment) @comment

; Keyword statements - target the statement nodes themselves
(imports_statement) @keyword.import
(namespace_block) @keyword

; Strings
(string_literal) @string
(interpolated_string_literal) @string
(character_literal) @character

; Numbers
(integer_literal) @number
(floating_point_literal) @number

; Boolean
(boolean_literal) @constant.builtin

; Date literals
(date_literal) @string.special

; Type declarations
(class_block
  (identifier) @type.definition)

(module_block
  (identifier) @type.definition)

(structure_block
  (identifier) @type.definition)

(interface_block
  (identifier) @type.definition)

(enum_block
  (identifier) @type.definition)

; Method declarations
(method_declaration
  (identifier) @function.definition)

; Property declarations
(property_declaration
  (identifier) @property)

; Parameter list
(parameter
  (identifier) @variable.parameter)

; Type references
(primitive_type) @type.builtin

; Member access
(member_access
  (identifier) @property)

; Invocations
(invocation
  (identifier) @function.call)

; Identifiers
(identifier) @variable

; Enum members
(enum_member
  (identifier) @constant)

; Modifiers
(modifier) @keyword.modifier

; Attributes
(attribute) @attribute
