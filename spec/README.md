# Colloq Graph specification

This directory contains machine-readable experiments for the canonical Colloq Graph described by [RFC-0001](../rfcs/0001-colloq-kernel.md).

## Files

- [`colloq-graph-v0.schema.json`](colloq-graph-v0.schema.json) — JSON Schema for the first interchange experiment.
- [`colloq-conversation-v0.schema.json`](colloq-conversation-v0.schema.json) — executable two-role conversation interchange used by the Rust prototype.
- [`colloq-plan-v0.schema.json`](colloq-plan-v0.schema.json) — compiled two-endpoint execution-plan artifact, including the optional compact transition dictionary used to start reusable sessions.
- [`colloq-session-v0.schema.json`](colloq-session-v0.schema.json) — plan-bound network preface exchanged before reference or compact Colloq frames.
- [`colloq-node-v0.schema.json`](colloq-node-v0.schema.json) — local-only persistent Iroh identity; the secret-bearing file is written mode `0600` on Unix and must never be shared.
- [`colloq-endpoint-v0.schema.json`](colloq-endpoint-v0.schema.json) — public direct-address ticket for a persistent endpoint identity.
- [`colloq-authorization-v0.schema.json`](colloq-authorization-v0.schema.json) — local allow-list binding authenticated endpoint IDs to exact roles and plan identities.
- [`colloq-evidence-v0.schema.json`](colloq-evidence-v0.schema.json) — reproducible correctness, security, or performance result tied to one exact revision and protocol configuration.
- [`colloq-jev-v0.schema.json`](colloq-jev-v0.schema.json) — binds one choice state to one TypeSafe Jev Choice question, with an explicit threshold and escalation branch. See [the Jev chooser](../docs/jev.md).

Published schemas are available at `https://colloq.dev/spec/<schema-file>`.

The JSON representation is an interchange and debugging format. It is not yet the canonical binary encoding and must not be treated as stable.

The original Colloq Graph v0 schema predates the conversation-state model in [RFC-0002](../rfcs/0002-conversation-is-the-computation.md). The separate Conversation v0 experiment now represents two roles, sends, choices, loops, cancellation, declared failures, success terminals, and failure terminals without pretending the broader Graph schema is already stable. In v0, an `on_failure` edge must target a terminal `fail` state carrying the same declared failure ID; retry and recovery graphs are deferred.

Colloq Plan v0 is a derived artifact, not another semantic source. It carries the experimental conversation identity, deterministic plan identity, projected endpoint graphs, and a compiler-derived compact-wire dictionary. JSON Schema checks its representation; the Rust plan verifier additionally recalculates the digest, validates state and role references, and rejects a noncanonical transition table before sessions are created. Older v0 artifacts may omit `wire`; the runtime derives it when preparing the plan.

Colloq Session Preface v0 binds one network connection to a session version, conversation and plan identity, endpoint role, and exact wire encoding. A mismatch aborts before frame zero. Optional `endpoint_identity` and `channel_binding` fields carry transport-authenticated evidence. The Iroh adapter requires both, checks the mutually authenticated endpoint keys, and verifies a TLS exporter bound to the concrete connection and plan. On the standalone QUIC adapter the client authenticates the server through a pinned certificate; TCP provides no cryptographic authentication, and the standalone QUIC server does not authenticate its client.

Colloq's node artifacts separate three concerns. The secret-bearing node identity proves who the local Iroh endpoint is. The public endpoint ticket says where that identity can be reached. The authorization policy says which authenticated remote identity may assume which Colloq role under which exact compiled plan. v0 has no wildcard grants: changing plan semantics requires an explicit new authorization.

Colloq Evidence v0 is a research artifact rather than an executable language input. It records the
revision, environment, pre-registered protocol, semantic identities, correctness outcome,
measurement aggregates, and digests of raw artifacts. Its reporting rules are defined by
[RFC-0004](../rfcs/0004-evidence-protocol.md).

## Canonicalization experiment

Colloq Graph v0 intends to calculate semantic identity from:

1. The declared semantic format version.
2. Definitions after reference resolution and alpha-normalization.
3. Types, behavior, effects, failures, contracts, capabilities, budgets, placement requirements, policies, and reachable holes.
4. External semantic dependencies by content identity.

The following are excluded:

- object key order and insignificant representation details;
- friendly names and aliases;
- comments and documentation;
- source locations;
- UI positions and view state;
- authorship and timestamps;
- cached derived facts and execution measurements.

The initial implementation must publish canonicalization fixtures before any produced digest is called an Colloq `ContentId`.

## Validation boundary

JSON Schema validates representation shape. The Colloq checker must additionally validate:

- uniqueness and resolution of local references;
- port direction and type compatibility;
- flow contract consistency;
- type, effect, failure, and capability constraints;
- reachable-hole closure for deployment entrypoints;
- policy isolation and authority non-amplification;
- canonicalization and content identity.
