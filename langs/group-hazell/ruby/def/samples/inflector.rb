    #   pluralize('words')            # => "words"
    #   pluralize('CamelOctopus')     # => "CamelOctopi"
    #   pluralize('ley', :es)         # => "leyes"
    def pluralize(word, locale = :en)
      apply_inflections(word, inflections(locale).plurals, locale)
    end

    # The reverse of #pluralize, returns the singular form of a word in a
    # string.
    #
    # If passed an optional +locale+ parameter, the word will be
    # singularized using rules defined for that language. By default,
    # this parameter is set to <tt>:en</tt>.
    #
    #   singularize('posts')            # => "post"
    #   singularize('octopi')           # => "octopus"
    #   singularize('sheep')            # => "sheep"
    #   singularize('word')             # => "word"
    #   singularize('CamelOctopi')      # => "CamelOctopus"
    #   singularize('leyes', :es)       # => "ley"
    def singularize(word, locale = :en)
      apply_inflections(word, inflections(locale).singulars, locale)
    end

    # Converts strings to UpperCamelCase.
    # If the +uppercase_first_letter+ parameter is set to false, then produces
    # lowerCamelCase.
    #
    # Also converts '/' to '::' which is useful for converting
    # paths to namespaces.
    #
    #   camelize('active_model')                # => "ActiveModel"
    #   camelize('active_model', false)         # => "activeModel"
    #   camelize('active_model/errors')         # => "ActiveModel::Errors"
    #   camelize('active_model/errors', false)  # => "activeModel::Errors"
    #
    # As a rule of thumb you can think of +camelize+ as the inverse of
    # #underscore, though there are cases where that does not hold:
    #
    #   camelize(underscore('SSLError'))        # => "SslError"
    def camelize(term, uppercase_first_letter = true)
      string = term.to_s
      # String#camelize takes a symbol (:upper or :lower), so here we also support :lower to keep the methods consistent.
      if !uppercase_first_letter || uppercase_first_letter == :lower
        string = string.sub(inflections.acronyms_camelize_regex) { |match| match.downcase! || match }
      elsif string.match?(/\A[a-z\d]*\z/)
        return inflections.acronyms[string]&.dup || string.capitalize
      else
        string = string.sub(/^[a-z\d]*/) { |match| inflections.acronyms[match] || match.capitalize! || match }
      end
      string.gsub!(/(?:_|(\/))([a-z\d]*)/i) do
        word = $2
        substituted = inflections.acronyms[word] || word.capitalize! || word
        $1 ? "::#{substituted}" : substituted
      end
      string
    end

    # Makes an underscored, lowercase form from the expression in the string.
    #
    # Changes '::' to '/' to convert namespaces to paths.
    #
    #   underscore('ActiveModel')         # => "active_model"
    #   underscore('ActiveModel::Errors') # => "active_model/errors"
    #
    # As a rule of thumb you can think of +underscore+ as the inverse of
    # #camelize, though there are cases where that does not hold:
    #
    #   camelize(underscore('SSLError'))  # => "SslError"
    def underscore(camel_cased_word)
      return camel_cased_word.to_s.dup unless /[A-Z-]|::/.match?(camel_cased_word)
      word = camel_cased_word.to_s.gsub("::", "/")
      word.gsub!(inflections.acronyms_underscore_regex) { "#{$1 && '_' }#{$2.downcase}" }
      word.gsub!(/(?<=[A-Z])(?=[A-Z][a-z])|(?<=[a-z\d])(?=[A-Z])/, "_")
      word.tr!("-", "_")
      word.downcase!
      word
    end

    # Tweaks an attribute name for display to end users.
    #
    # Specifically, performs these transformations:
    #
    # * Applies human inflection rules to the argument.
    # * Deletes leading underscores, if any.
    # * Removes an "_id" suffix if present.
    # * Replaces underscores with spaces, if any.
    # * Downcases all words except acronyms.
    # * Capitalizes the first word.
    # The capitalization of the first word can be turned off by setting the
    # +:capitalize+ option to false (default is true).
    #
    # The trailing '_id' can be kept and capitalized by setting the
    # optional parameter +keep_id_suffix+ to true (default is false).
    #
    #   humanize('employee_salary')                  # => "Employee salary"
    #   humanize('author_id')                        # => "Author"
    #   humanize('author_id', capitalize: false)     # => "author"
    #   humanize('_id')                              # => "Id"
    #   humanize('author_id', keep_id_suffix: true)  # => "Author id"
    #
    # If "SSL" was defined to be an acronym:
    #
    #   humanize('ssl_error') # => "SSL error"
    #
    def humanize(lower_case_and_underscored_word, capitalize: true, keep_id_suffix: false)
      result = lower_case_and_underscored_word.to_s.dup

      inflections.humans.each { |(rule, replacement)| break if result.sub!(rule, replacement) }

      result.tr!("_", " ")
      result.lstrip!
      if !keep_id_suffix && lower_case_and_underscored_word&.end_with?("_id")
        result.delete_suffix!(" id")
      end

      result.gsub!(/([[[:alpha:]]\d]+)/i) do |match|
        match.downcase!
        inflections.acronyms[match] || match
      end

