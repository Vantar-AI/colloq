# Colloq RFCs

RFCs define changes to Colloq's portable semantics, canonical graph, public tooling contracts, and governance model.

An RFC begins as a problem statement. It is not accepted merely because an implementation exists.

## Statuses

- **Draft** — open design with unresolved questions.
- **Experimental** — precise enough to implement and test, but not stable.
- **Accepted** — part of the intended portable semantics.
- **Superseded** — replaced by a later RFC.
- **Rejected** — preserved as design history.

## Required sections

Every semantic RFC should contain:

1. Motivation and concrete workloads.
2. Decision and terminology.
3. Static semantics.
4. Runtime semantics.
5. Failure and security consequences.
6. Canonical representation.
7. Compatibility and migration.
8. Alternatives and prior art.
9. Testable acceptance criteria.

The implementation may evolve quickly. Accepted semantics require a conformance suite and at least two independent consumers before they are called stable.

## Index

| RFC | Status | Title |
|---|---|---|
| [0001](0001-colloq-kernel.md) | Draft | The Colloq language kernel |
| [0002](0002-conversation-is-the-computation.md) | Draft | The conversation is the computation |
| [0003](0003-pluggable-substrates.md) | Experimental | Pluggable collaboration, connectivity, and deployment substrates |
| [0004](0004-evidence-protocol.md) | Experimental | Colloq evidence protocol |
| [0005](0005-binary-wire-codec.md) | Draft | Binary Colloq Wire codec |
| [0006](0006-identity-lifecycle.md) | Draft | Identity rotation and revocation |
