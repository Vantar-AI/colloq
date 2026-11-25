# Contributing to Colloq

Colloq is a research project with a working Rust implementation. Contributions should make the thesis more precise, more testable, or easier to falsify.

## Useful contributions

- A real distributed AI workload and its constraints.
- An incident or failure mode current abstractions make difficult to prevent.
- A minimal syntax proposal paired with exact IR and runtime semantics.
- A comparison with prior work that changes or narrows Colloq's direction.
- A benchmark design with a conventional baseline.
- A security analysis of generated or evolutionary programs.
- A typed graph operation, canonicalization fixture, or projection round-trip case.

## Proposal format

Open an issue or pull request covering:

1. Problem and concrete workload.
2. Current solution and why it is insufficient.
3. Proposed source-level behavior.
4. Proposed IR/runtime semantics.
5. Failure and security implications.
6. Measurement or acceptance criterion.

Syntax without semantics is considered a sketch, not a language proposal.

Changes to portable semantics should use the [RFC process](rfcs/README.md). RFC-0001 establishes the current graph-native direction but remains a Draft that implementation evidence may revise.

## How a change reaches main

`main` is protected by a ruleset. Nobody pushes to it, including the maintainers.

1. Work on a branch and open a pull request. Draft is fine, and preferred while the design
   is still moving.
2. These checks must pass before merge: formatting, clippy with `-D warnings`, the release
   build, the schema and artifact validation, the Linux test matrix on the minimum
   supported Rust version and on stable, and the two-node Iroh smoke test.
   The macOS jobs run and are visible, but they are not required, because their network
   tests are still occasionally flaky under a starved runner.
3. Review conversations must be resolved before merge.
4. Merges are squash merges, so the history stays linear. Force pushes to `main` and
   deleting `main` are blocked.

A pull request that changes what Colloq *means*, rather than how it is implemented, needs
an RFC. A pull request that makes a performance claim needs a report, not a sentence.

## Documentation style

- Distinguish current behavior from proposed behavior.
- Prefer precise examples over broad claims.
- Do not claim a performance advantage without a reproducible benchmark.
- Use “must” only for required semantics and “should” for design direction.
- Define new terms in the document or link to the definition.

## Development

Colloq is a Rust crate (edition 2024, minimum Rust 1.91). The `colloq` binary compiles
conversations, bootstraps nodes and runs the benchmarks.

```sh
cargo build --locked
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --check
```

These are the checks CI runs on every pull request. Run them before you push.

Two further CI jobs are worth running locally when you touch their area:

- **Schemas and examples.** Install `check-jsonschema` (`pip install check-jsonschema==0.37.4`)
  and run the commands in the `schemas` job of `.github/workflows/ci.yml`.
- **Two-node smoke test.** `scripts/two-node-smoke.sh` after `cargo build --locked`. It needs `jq`.

New here? Start with an issue labelled
[good first issue](https://github.com/Vantar-AI/colloq/labels/good%20first%20issue).

Documentation changes still follow the style rules above: examples carry their experimental
status, and design documents stay consistent with `docs/design.md`.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
