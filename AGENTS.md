# Repository guidelines

## Project structure and module organization

This repository is a Rust workspace:

- `fuzzer/` contains the Nautilus fuzzer and the `fuzzer`, `generator`, and
  `mutator` binaries.
- `grammartec/` contains grammar parsing, tree generation, mutation, and chunk
  storage.
- `forksrv/` runs targets through the AFL-compatible fork server.
- `regex_mutator/` generates values for regex terminals.
- `grammars/` contains example Python grammars.
- `test_cases/` contains test grammars and inputs.

`config.ron` is the example runtime configuration. Keep build output under
`target/` and fuzzing corpora or findings in an external work directory, such
as `/tmp/workdir`.

## Build, test, and development commands

Run commands from the repository root:

```bash
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets

# Run the fuzzer against the bundled example target.
cargo run --release -- -g grammars/grammar_py_example.py \
  -o /tmp/workdir -- ./test @@

# Generate 100 inputs without fuzzing a target.
cargo run --bin generator -- \
  -g grammars/grammar_py_example.py -t 100
```

Compile fuzz targets with AFL++ instrumentation. See `README.md` for AFL++ and
QEMU examples.

## Coding style

Follow the existing Rust module boundaries and naming conventions. Run
`cargo fmt` on Rust changes. Prefer existing workspace crates and standard
library features over new dependencies. Keep target paths, grammar paths,
timeouts, and work-directory settings in `config.ron` or command-line
arguments rather than hard-coding them.

Python files in `grammars/` are grammar definitions loaded by the Rust fuzzer.
Keep them compatible with the `ctx.rule`, `ctx.script`, and `ctx.regex` API
shown in `README.md`.

## Testing guidelines

Place focused unit tests beside the Rust module they exercise. Validate with
the smallest affected command, then run `cargo test --workspace` when shared
grammar, mutation, tree, queue, or fork-server behavior changes. Run the
generator against an affected grammar when grammar loading or unparsing
changes. Do not commit generated fuzzing output unless the change explicitly
requires a fixture.

If formatting, linting, or type checking fully verifies a change, do not add a
redundant test.

## Commit and pull request guidelines

Use the `conventional-commit` skill for both commit messages and PR titles.
Keep commits small and trackable. PRs should state the target and grammar,
commands run, observed behavior or output changes, and linked issue when one
exists. Add logs, crash artifacts, or screenshots only when they help explain
the change.

Pull requests are documentation for my honours thesis submission and write-up.
Record the decisions behind code changes, the engineering work performed,
trade-offs, and relevant documentation or sources. Use diagrams when they make
the design or execution flow easier to understand.

## Configuration and fuzzing safety

Treat `config.ron` and command-line arguments as the execution contract. Check
the target binary, input placeholder, grammar, seed corpus, output directory,
timeouts, and AFL/QEMU mode before a run. Preserve crashing inputs and the
configuration needed to reproduce them. Do not run untrusted fuzz targets
outside an appropriate sandbox.
