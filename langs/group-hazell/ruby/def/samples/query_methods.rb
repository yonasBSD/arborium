        where_clause = @scope.send(:build_where_clause, opts, rest)

        @scope.where_clause += where_clause.invert

        @scope
      end

      # Returns a new relation with joins and where clause to identify
      # associated relations.
      #
      # For example, posts that are associated to a related author:
      #
      #    Post.where.associated(:author)
      #    # SELECT "posts".* FROM "posts"
      #    # INNER JOIN "authors" ON "authors"."id" = "posts"."author_id"
      #    # WHERE "authors"."id" IS NOT NULL
      #
      # Additionally, multiple relations can be combined. This will return posts
      # associated to both an author and any comments:
      #
      #    Post.where.associated(:author, :comments)
      #    # SELECT "posts".* FROM "posts"
      #    # INNER JOIN "authors" ON "authors"."id" = "posts"."author_id"
      #    # INNER JOIN "comments" ON "comments"."post_id" = "posts"."id"
      #    # WHERE "authors"."id" IS NOT NULL AND "comments"."id" IS NOT NULL
      #
      # You can define join type in the scope and +associated+ will not use `JOIN` by default.
      #
      #    Post.left_joins(:author).where.associated(:author)
      #    # SELECT "posts".* FROM "posts"
      #    # LEFT OUTER JOIN "authors" "authors"."id" = "posts"."author_id"
      #    # WHERE "authors"."id" IS NOT NULL
      #
      #    Post.left_joins(:comments).where.associated(:author)
      #    # SELECT "posts".* FROM "posts"
      #    # INNER JOIN "authors" ON "authors"."id" = "posts"."author_id"
      #    # LEFT OUTER JOIN "comments" ON "comments"."post_id" = "posts"."id"
      #   #  WHERE "author"."id" IS NOT NULL
      def associated(*associations)
        associations.each do |association|
          reflection = scope_association_reflection(association)
          unless @scope.joins_values.include?(reflection.name) || @scope.left_outer_joins_values.include?(reflection.name)
            @scope.joins!(association)
          end

          association_conditions = Array(reflection.association_primary_key).index_with(nil)
          if reflection.options[:class_name]
            self.not(association => association_conditions)
          else
            self.not(reflection.table_name => association_conditions)
          end
        end

        @scope
      end

      # Returns a new relation with left outer joins and where clause to identify
      # missing relations.
      #
      # For example, posts that are missing a related author:
      #
      #    Post.where.missing(:author)
      #    # SELECT "posts".* FROM "posts"
      #    # LEFT OUTER JOIN "authors" ON "authors"."id" = "posts"."author_id"
      #    # WHERE "authors"."id" IS NULL
      #
      # Additionally, multiple relations can be combined. This will return posts
      # that are missing both an author and any comments:
      #
      #    Post.where.missing(:author, :comments)
      #    # SELECT "posts".* FROM "posts"
      #    # LEFT OUTER JOIN "authors" ON "authors"."id" = "posts"."author_id"
      #    # LEFT OUTER JOIN "comments" ON "comments"."post_id" = "posts"."id"
      #    # WHERE "authors"."id" IS NULL AND "comments"."id" IS NULL
      def missing(*associations)
        associations.each do |association|
          reflection = scope_association_reflection(association)
          @scope.left_outer_joins!(association)
          association_conditions = Array(reflection.association_primary_key).index_with(nil)
          if reflection.options[:class_name]
            @scope.where!(association => association_conditions)
          else
            @scope.where!(reflection.table_name => association_conditions)
          end
        end

        @scope
      end

      private
        def scope_association_reflection(association)
          model = @scope.model
          reflection = model._reflect_on_association(association)
          unless reflection
            raise ArgumentError.new("An association named `:#{association}` does not exist on the model `#{model.name}`.")
          end
          reflection
        end
    end

    # A wrapper to distinguish CTE joins from other nodes.
    class CTEJoin # :nodoc:
      attr_reader :name

      def initialize(name)
        @name = name
      end
    end

    FROZEN_EMPTY_ARRAY = [].freeze
    FROZEN_EMPTY_HASH = {}.freeze

    Relation::VALUE_METHODS.each do |name|
      method_name, default =
        case name
        when *Relation::MULTI_VALUE_METHODS
          ["#{name}_values", "FROZEN_EMPTY_ARRAY"]
        when *Relation::SINGLE_VALUE_METHODS
          ["#{name}_value", name == :create_with ? "FROZEN_EMPTY_HASH" : "nil"]
        when *Relation::CLAUSE_METHODS
          ["#{name}_clause", name == :from ? "Relation::FromClause.empty" : "Relation::WhereClause.empty"]
