  layers::T
end

Chain(xs...) = Chain(xs)
function Chain(; kw...)
  :layers in keys(kw) && throw(ArgumentError("a Chain cannot have a named layer called `layers`"))
  isempty(kw) && return Chain(())
  Chain(values(kw))
end

@forward Chain.layers Base.getindex, Base.length, Base.first, Base.last,
  Base.iterate, Base.lastindex, Base.keys, Base.firstindex

@layer Chain

(c::Chain)(x) = _applychain(c.layers, x)
(c::Chain)(x, ys...) = _applychain(c.layers, (x, ys...))

@generated function _applychain(layers::Tuple{Vararg{Any,N}}, x) where {N}
  symbols = vcat(:x, [gensym() for _ in 1:N])
  calls = [:($(symbols[i+1]) = layers[$i]($(symbols[i]))) for i in 1:N]
  Expr(:block, calls...)
end

_applychain(layers::NamedTuple, x) = _applychain(Tuple(layers), x)

function _applychain(layers::AbstractVector, x)  # type-unstable path, helps compile times
  for f in layers
    x = f(x)
  end
  return x
end

# An easy error to make is to pass result of explicit gradient(...), not gradient(...)[1]
# Can't catch every case, but can catch many simple Flux models:
function Optimisers.update!(opt, model::Chain, grads::Tuple)
  # Zygote will make a NamedTuple{(:layers,)} for the gradient of Chain, Diffractor a Tangent
  @warn """explicit `update!(opt, model, grad)` wants the gradient for the model alone,
    not the whole tuple from `gradient(m -> loss(m, x, y), model)`. You probably want `grads[1]`."""
  return Optimisers.update!(opt, model, grads[1])
end

Base.getindex(c::Chain, i::AbstractArray) = Chain(c.layers[i])
Base.getindex(c::Chain{<:NamedTuple}, i::AbstractArray) =
  Chain(NamedTuple{keys(c)[i]}(Tuple(c.layers)[i]))

function Base.show(io::IO, c::Chain)
  print(io, "Chain(")
  _show_layers(io, c.layers)
  print(io, ")")
end

_show_layers(io, layers::Tuple) = join(io, layers, ", ")
_show_layers(io, layers::NamedTuple) = join(io, [lazy"$k = $v" for (k, v) in pairs(layers)], ", ")
_show_layers(io, layers::AbstractVector) = (print(io, "["); join(io, layers, ", "); print(io, "]"))

# This is a temporary and naive implementation
# it might be replaced in the future for better performance
# see issue https://github.com/FluxML/Flux.jl/issues/702
# Johnny Chen -- @johnnychen94
# only slightly changed to better handle interaction with Zygote @dsweber2
"""
    activations(c::Chain, input)

Like calling a `Chain`, but saves the result of each layer as an output.

# Examples

```jldoctest
julia> using Flux: activations

julia> c = Chain(x -> x + 1, x -> x * 2, x -> x ^ 3);

julia> activations(c, 1)
(2, 4, 64)
```
"""
activations(c::Chain, input) = _extraChain(Tuple(c.layers), input)

# Calculates the forward results of each layer provided in a `Tuple` with `x` as model input.
function _extraChain(fs::Tuple, x)
  res = first(fs)(x)
  return (res, _extraChain(Base.tail(fs), res)...)
end
_extraChain(::Tuple{}, x) = ()


"""
    Dense(in => out, σ=identity; bias=true, init=glorot_uniform)
    Dense(W::AbstractMatrix, [bias, σ])

Create a traditional fully connected layer, whose forward pass is given by:

    y = σ.(W * x .+ bias)

The input `x` should be a vector of length `in`, or batch of vectors represented
as an `in × N` matrix, or any array with `size(x,1) == in`.
The out `y` will be a vector  of length `out`, or a batch with
`size(y) == (out, size(x)[2:end]...)`

Keyword `bias=false` will switch off trainable bias for the layer.
The initialisation of the weight matrix is `W = init(out, in)`, calling the function
given to keyword `init`, with default [`glorot_uniform`](@ref Flux.glorot_uniform).
The weight matrix and/or the bias vector (of length `out`) may also be provided explicitly.

# Examples
```jldoctest
julia> model = Dense(5 => 2)
Dense(5 => 2)       # 12 parameters

julia> model(rand32(5, 64)) |> size
(2, 64)

julia> model(rand32(5, 6, 4, 64)) |> size  # treated as three batch dimensions
(2, 6, 4, 64)

julia> model2 = Dense(ones(2, 5), false, tanh)  # using provided weight matrix
Dense(5 => 2, tanh; bias=false)  # 10 parameters

julia> model2(ones(5))
2-element Vector{Float64}:
