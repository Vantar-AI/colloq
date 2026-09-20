# Pluggable substrates

Colloq owns the portable conversation, its validation rules, endpoint projection, plan identity,
and runtime state transitions. Existing systems can supply collaboration, connectivity, and
deployment without becoming part of those semantics.

```text
Automerge draft history
        │ conflict-free collaboration and ordered sync
        ▼
Colloq promotion gate
        │ reject conflicts → validate graph → compile identified plan
        ▼
projected Colloq endpoints
        ├── Iroh: mutually authenticated, policy-authorized peer transport
        └── Miren: TCP testbed or Iroh UDP node-port deployment
```

The boundary is deliberate: a converged document is not automatically a valid Colloq program, a
connected peer is not automatically authorized to execute a plan, and a successfully deployed
container is not evidence that its conversation is semantically compatible.

## Automerge: collaborative draft plane

[`AutomergeDraft`](../src/graph.rs) stores an Colloq Conversation v0 document and its collaborative
history. It supports optimistic scalar graph patches, forks and merges, transport-independent
ordered synchronization, and persistence as an `.colloqdraft` file.

Promotion is fail closed:

1. reject every unresolved Automerge conflict;
2. deserialize through Colloq's strict representation;
3. run Colloq semantic validation;
4. compile a verified Colloq Plan;
5. calculate Colloq's conversation and plan identities from the promoted semantics.

Automerge change hashes name draft history. They do not become Colloq conversation identities. The
current patch API deliberately accepts only existing JSON-pointer locations and scalar values;
typed structural insert, move, delete, and schema-aware edit operations remain future work.

```bash
cargo run -- draft-create examples/generate.colloqconv.json \
  --out build/generate.colloqdraft

cargo run -- draft-patch build/generate.colloqdraft \
  --pointer /module/semantic_version --value '"0.2.0"'

cargo run -- draft-promote build/generate.colloqdraft \
  --conversation-out build/promoted.colloqconv.json \
  --plan-out build/promoted.colloqplan.json
```

The Rust API exposes one `DraftSyncSession` per peer. `examples/draft-sync.colloqconv.json` makes that
ordered exchange an ordinary Colloq control conversation: projected client and server endpoints carry
incremental Automerge messages as typed `sync` frames and explicitly select `message`, `idle`, or
`done`. `draft-serve-iroh` and `draft-connect-iroh` run it over the same authenticated, plan-bound
Iroh transport and save both converged drafts. Convergence still does not imply promotion.

## Iroh: authenticated peer data plane

The Iroh adapter uses the application protocol identifier `colloq/0.1` and a persistent Ed25519
endpoint key. A caller supplies a local authorization policy; Colloq rejects a remote identity unless
it is granted the exact peer role and compiled plan before accepting a semantic frame.

The Colloq Session Preface adds two optional Iroh bindings:

- `endpoint_identity` declares the endpoint key authenticated by Iroh;
- `channel_binding` is a TLS exporter bound to the concrete connection and plan identity.

Both sides verify the remote identity, local authorization, exporter, conversation, plan, roles,
and exact encoding before frame zero. This prevents a valid preface copied from one Iroh connection
from authenticating another. Node identity files, public endpoint tickets, and authorization files
are versioned separately. Rotation, revocation distribution, and replay-resistant application
freshness remain open.

```bash
cargo run -- demo --transport iroh --tokens 3
cargo run -- run-plan build/generate.colloqplan.json \
  --transport iroh --wire compact --tokens 3

cargo run -- bootstrap-two-node --out build/two-node
cargo run -- serve-iroh --listen 127.0.0.1:7880
cargo run -- connect-iroh \
  --server build/two-node/server.colloqendpoint.json
```

The reference demo uses direct loopback endpoints with relay disabled. Separate-process commands
load persistent identities and direct-address tickets for routable data-center networks. The
library also exposes a default Iroh endpoint with discovery, NAT traversal, and relay fallback.
See the [two-node runbook](two-node.md).

## Miren: deployment adapter

Miren stays outside Colloq's meaning. The adapter validates and compiles the source conversation,
then emits a Miren manifest and a reproducible container for the current `client`/`server`
Generate reference workload:

```bash
cargo run -- emit-miren examples/generate.colloqconv.json
```

This writes:

- `.miren/app.toml`, containing the service, TCP port, instance count, and expected Colloq identities;
- `.miren/Dockerfile.colloq`, pinning Rust 1.96 and building with `--locked`;
- environment bindings that make `colloq serve` reject a stale or substituted conversation.

The default generated server listens on TCP through Miren's routable cluster network. With
`--transport iroh`, the adapter instead emits a UDP node port and required sensitive environment
bindings for the node identity and authorization JSON, plus a required advertised address. Miren
owns build, placement, restart, and UDP forwarding; Iroh and Colloq own endpoint authentication,
authorization, and the session. Miren workload identity could later replace static key delivery,
but that requires an explicit attestation and rotation design.

## What this proves—and what it does not

The integrations establish replaceable interfaces around Colloq's core:

- draft edits can merge before passing one deterministic promotion gate;
- a plan can run over a mutually authenticated peer connection without changing its trace;
- an identified plan can be packaged for a multi-node deployment system.

They do not yet provide a general scheduler, arbitrary-role deployment, production key lifecycle,
durable execution recovery, or a complete collaborative editor. Those are the next validation
targets described in the [roadmap](roadmap.md) and [RFC-0003](../rfcs/0003-pluggable-substrates.md).
