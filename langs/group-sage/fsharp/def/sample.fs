module App

type Person = { Name: string; Age: int }

let greet person =
    sprintf "Hello, %s!" person.Name

let people = [
    { Name = "Alice"; Age = 30 }
    { Name = "Bob"; Age = 25 }
]

people
|> List.map greet
|> List.iter (printfn "%s")
