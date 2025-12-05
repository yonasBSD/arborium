class Person {
  final String name;
  final int age;

  Person(this.name, this.age);

  String greet() => 'Hello, my name is $name';
}

void main() {
  final person = Person('Alice', 30);
  print(person.greet());

  final numbers = [1, 2, 3, 4, 5];
  final doubled = numbers.map((n) => n * 2).toList();
  print(doubled);
}
