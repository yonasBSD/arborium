#!/bin/zsh
# Zsh configuration example

# Enable options
setopt AUTO_CD EXTENDED_GLOB NULL_GLOB

# Array and associative array
typeset -a fruits=("apple" "banana" "cherry")
typeset -A colors=([red]=ff0000 [green]=00ff00 [blue]=0000ff)

# Function with local variables
greet() {
    local name=$1
    local greeting=${2:-Hello}
    print "$greeting, $name!"
}

# Parameter expansion
path_parts=(${(s:/:)PATH})
upper_name=${name:u}
first_three=${fruits[1,3]}

# Glob qualifiers
recent_files=(*.txt(om[1,5]))  # 5 most recent .txt files
large_files=(*(Lm+10))         # files larger than 10MB

# Completion
compdef _gnu_generic mycommand
