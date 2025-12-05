defmodule MyApp.User do
  @moduledoc "User module for handling user data"

  defstruct [:name, :email, :age]

  def new(name, email, age) do
    %__MODULE__{name: name, email: email, age: age}
  end

  def greet(%__MODULE__{name: name}) do
    "Hello, #{name}!"
  end
end

user = MyApp.User.new("Alice", "alice@example.com", 30)
IO.puts(MyApp.User.greet(user))
