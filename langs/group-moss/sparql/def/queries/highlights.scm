; SPARQL highlights query

; Comments
(comment) @comment

; Strings
(string) @string
(rdf_literal) @string
(lang_tag) @string.special

; Numbers
(integer) @number
(decimal) @number
(double) @number

; Booleans
(boolean_literal) @constant.builtin

; IRIs and prefixed names
(iri_reference) @string.special.url
(prefixed_name) @type
(namespace) @namespace
(pn_prefix) @namespace
(pn_local) @property

; Variables
(var) @variable

; Blank nodes
(blank_node_label) @variable.special
(anon) @variable.special

; Prefix/base declarations
(prefix_declaration) @keyword.import
(base_declaration) @keyword.import

; Query types
(select_query) @keyword
(construct_query) @keyword
(describe_query) @keyword
(ask_query) @keyword

; Clauses
(select_clause) @keyword
(where_clause) @keyword
(group_clause) @keyword
(having_clause) @keyword
(order_clause) @keyword
(limit_clause) @keyword
(offset_clause) @keyword
(values_clause) @keyword
(dataset_clause) @keyword

; Graph patterns
(optional_graph_pattern) @keyword
(minus_graph_pattern) @keyword
(graph_graph_pattern) @keyword
(service_graph_pattern) @keyword
(group_or_union_graph_pattern) @keyword
(filter) @keyword.conditional
(bind) @keyword

; SPARQL Update keywords
(insert_data) @keyword
(delete_data) @keyword
(insert_clause) @keyword
(delete_clause) @keyword
(delete_where) @keyword
(modify) @keyword
(load) @keyword
(clear) @keyword
(drop) @keyword
(create) @keyword
(add) @keyword
(move) @keyword
(copy) @keyword

; Built-in functions
(build_in_function) @function.builtin
(aggregate) @function.builtin
(exists_func) @function.builtin
(not_exists_func) @function.builtin
(regex_expression) @function.builtin
(substring_expression) @function.builtin
(string_replace_expression) @function.builtin

; Function calls
(function_call) @function.call

; Keywords
[
  "SELECT"
  "CONSTRUCT"
  "DESCRIBE"
  "ASK"
  "WHERE"
  "FROM"
  "NAMED"
  "PREFIX"
  "BASE"
  "OPTIONAL"
  "UNION"
  "MINUS"
  "GRAPH"
  "SERVICE"
  "SILENT"
  "FILTER"
  "BIND"
  "AS"
  "VALUES"
  "GROUP"
  "BY"
  "HAVING"
  "ORDER"
  "ASC"
  "DESC"
  "LIMIT"
  "OFFSET"
  "DISTINCT"
  "REDUCED"
  "INSERT"
  "DELETE"
  "LOAD"
  "CLEAR"
  "DROP"
  "CREATE"
  "ADD"
  "MOVE"
  "COPY"
  "TO"
  "INTO"
  "DEFAULT"
  "ALL"
  "USING"
  "WITH"
  "a"
  "true"
  "false"
] @keyword

; Operators
[
  "||"
  "&&"
  "="
  "!="
  "<"
  ">"
  "<="
  ">="
  "+"
  "-"
  "*"
  "/"
  "!"
  "^"
  "^^"
] @operator

; Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
  "<"
  ">"
] @punctuation.bracket

[
  "."
  ","
  ";"
  ":"
] @punctuation.delimiter

; Property paths
(path_element) @property
(path_inverse) @operator
(path_mod) @operator

; Unit (empty collection)
(unit) @punctuation.bracket

; Nil
(nil) @constant.builtin
