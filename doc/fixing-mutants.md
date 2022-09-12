# How to fix missed mutants

Your goal is to get well-written and well-tested code, and you get to decide (with your co-authors) just what kind of structure and testing you prefer. Cargo-mutants provides information to help you get there, but does not dictate what you should do.

When faced with a missed mutant there are many ways to "cheat" and make the mutant no longer missed, without really improving  the program. For example, you could:

* inline the function into its callers, so that cargo-mutants no longer sees it as a function that can be mutated
* mark it as skipped
* add a unit test that checks the return value of the specific function
* delete the function and the code that calls it

These might be appropriate choices but they also might not move the program in the right direction.

One good question is: why didn't an existing test catch this? Some programs, like cargo-mutants itself, are designed to be tested primarily on their public interface, either an API, a command-line interface or a network API. Ask which part of the public interface's behavior should have failed if this function was mutated. Why didn't a test already catch that? Could you extend an existing test to check it?
