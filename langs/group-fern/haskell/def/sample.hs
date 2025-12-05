module Main where

import Data.List (sort)

quicksort :: Ord a => [a] -> [a]
quicksort [] = []
quicksort (x:xs) =
    quicksort smaller ++ [x] ++ quicksort larger
  where
    smaller = [a | a <- xs, a <= x]
    larger  = [a | a <- xs, a > x]

main :: IO ()
main = print $ quicksort [3, 1, 4, 1, 5, 9, 2, 6]
