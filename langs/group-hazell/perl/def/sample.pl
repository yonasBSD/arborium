#!/usr/bin/perl
use strict;
use warnings;
use feature 'say';

# Hash of arrays
my %data = (
    fruits  => ['apple', 'banana', 'cherry'],
    numbers => [1, 2, 3, 4, 5],
);

# Subroutine with regex
sub process_text {
    my ($text) = @_;
    $text =~ s/\bfoo\b/bar/gi;
    return $text;
}

# File handling
open(my $fh, '<', 'input.txt') or die "Cannot open: $!";
while (my $line = <$fh>) {
    chomp $line;
    say "Line: $line" if $line =~ /pattern/;
}
close($fh);

# Object-oriented style
package Counter;
sub new { bless { count => 0 }, shift }
sub increment { $_[0]->{count}++ }
