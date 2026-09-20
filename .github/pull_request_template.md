## What changes

<!-- One paragraph. What is different after this merges? -->

## Why

<!-- The problem, not the solution. Link the issue or RFC if there is one. -->

## Evidence

<!-- How do you know it works? Tests, a benchmark run, a trace, a screenshot. -->

- [ ] `cargo test --locked` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] Schemas and examples still validate, if either changed

## Semantics

- [ ] This changes no portable semantics, **or** it links the RFC that does
- [ ] It does not change a published schema `$id`, **or** it says how old artifacts keep resolving

## Claims

<!-- If this moves a line on docs/regeneration.md, say which one. If it makes a
     performance claim, link the report; a claim without a number is not a claim. -->
