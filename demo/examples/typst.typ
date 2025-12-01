// Typst document example
#set page(paper: "a4", margin: 2cm)
#set text(font: "Linux Libertine", size: 11pt)

= Introduction

This is a #strong[Typst] document demonstrating various features.

== Math Support

Inline math: $x^2 + y^2 = z^2$

Block equation:
$ integral_0^infinity e^(-x^2) dif x = sqrt(pi) / 2 $

== Code Blocks

```rust
fn main() {
    println!("Hello, Typst!");
}
```

== Tables

#table(
  columns: (auto, auto, auto),
  [*Name*], [*Age*], [*City*],
  [Alice], [25], [Paris],
  [Bob], [30], [London],
)

#let greet(name) = [Hello, #name!]
#greet("World")
