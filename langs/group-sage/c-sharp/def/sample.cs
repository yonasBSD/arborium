using System;
using System.Collections.Generic;
using System.Linq;

namespace Example
{
    public record Person(string Name, int Age);

    public class Program
    {
        public static async Task<int> Main(string[] args)
        {
            var people = new List<Person>
            {
                new("Alice", 30),
                new("Bob", 25),
                new("Charlie", 35)
            };

            var adults = people.Where(p => p.Age >= 18)
                               .OrderBy(p => p.Name);

            foreach (var person in adults)
            {
                Console.WriteLine($"{person.Name} is {person.Age}");
            }

            return 0;
        }
    }
}
