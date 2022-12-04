# Welcome to cargo-mutants

cargo-mutants is a mutation testing tool for Rust. It helps you improve your
program's quality by finding functions whose body could be replaced without
causing any tests to fail. Each such case indicates, perhaps, a gap in semantic
code coverage by your tests, where a bug might be lurking.

Coverage measurements can be helpful, but they really tell you what code is
_reached_ by a test, and not whether the test really _checks_ anything about the
behavior of the code. Mutation tests give different information, about whether
the tests really check the code's behavior.

**The goal of cargo-mutants is to be _easy_ to run on any Rust source tree, and
to tell you something _interesting_ about areas where bugs might be lurking or
the tests might be insufficient.** ([More about these goals](goals.md).)

TODO: Some motivating examples. How does mutation testing help?

TODO: How is this different to coverage, etc?

## Cases where cargo-mutants _can't_ help

Running cargo-mutants on your code won't, by itself, make your code better. It
only helps suggest places you might want to improve your tests, and that might
indirectly find bugs, or prevent future bugs. It's on you to follow up.
(However, it's really easy to run, so  you might as well look!)

cargo-mutants typically can't do much to help with crates that primarily
generate code using macros or build scripts, because it can't "see" the code
that's generated. (You can still run it, but it's may generate very few
mutants.)
