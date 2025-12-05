module Main

data Vect : Nat -> Type -> Type where
    Nil  : Vect Z a
    (::) : a -> Vect n a -> Vect (S n) a

append : Vect n a -> Vect m a -> Vect (n + m) a
append Nil       ys = ys
append (x :: xs) ys = x :: append xs ys

total
head : Vect (S n) a -> a
head (x :: _) = x

main : IO ()
main = printLn (head [1, 2, 3])
