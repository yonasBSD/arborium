-- Lean 4 example
def fibonacci : Nat → Nat
  | 0 => 0
  | 1 => 1
  | n + 2 => fibonacci n + fibonacci (n + 1)

theorem fib_pos : ∀ n, fibonacci n ≥ 0 := by
  intro n
  exact Nat.zero_le _

#eval fibonacci 10

structure Point where
  x : Float
  y : Float
deriving Repr

def Point.add (p q : Point) : Point :=
  { x := p.x + q.x, y := p.y + q.y }
