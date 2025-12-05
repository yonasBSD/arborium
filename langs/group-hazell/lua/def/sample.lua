-- Fibonacci generator
local function fibonacci(n)
    local a, b = 0, 1
    local result = {}

    for i = 1, n do
        table.insert(result, a)
        a, b = b, a + b
    end

    return result
end

local fib = fibonacci(10)
for i, v in ipairs(fib) do
    print(string.format("fib[%d] = %d", i, v))
end
