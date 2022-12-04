# How is mutation testing different to fuzzing?

Fuzzing is a technique for finding bugs by feeding pseudo-random inputs to a
program, and is particularly useful on programs that parse complex or untrusted
inputs such as binary file formats or network protocols.

Mutation testing makes algorithmically-generated changes to a copy of the
program source, and measures whether the test suite catches the change.

The two techniques are complementary. Although some bugs might be found by
either technique, fuzzing will tend to find bugs that are triggered by complex
or unusual inputs, whereas mutation testing will tend to point out logic that
might be correct but that's not tested.
