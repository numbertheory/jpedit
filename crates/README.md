# crates

This directory contains the crates that make up JPEdit and its supporting tooling.

* `jpedit`: Main editor binary and library<br>
  It is split apart into a library to allow for benchmarks.
* `lsh`: Syntax-highlighting compiler and runtime
* `lsh-bin`: A small CLI for experimenting with and debugging LSH output
* `stdext`: Shared utility code used across the workspace
* `unicode-gen`: Code generation utilities for Unicode LUTs
