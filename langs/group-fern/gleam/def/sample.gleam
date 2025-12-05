import gleam/io
import gleam/list

pub type Person {
  Person(name: String, age: Int)
}

pub fn greet(person: Person) -> String {
  "Hello, " <> person.name <> "!"
}

pub fn main() {
  let people = [
    Person("Alice", 30),
    Person("Bob", 25),
  ]

  list.each(people, fn(p) { io.println(greet(p)) })
}
