# RFC-0003: Pluggable collaboration, connectivity, and deployment substrates

- **Status:** Experimental
- **Authors:** Vantar AI
- **Created:** 2026-08-09

## Motivation

Eve needs collaborative machine-authored graphs, authenticated server-to-server connectivity,
and multi-node deployment. Reimplementing mature mechanisms inside the language would blur Eve's
novel claim and make it impossible to tell whether the conversation model adds value.

This RFC defines an architectural boundary: Eve owns portable semantics and may use replaceable
substrates for draft history, transport reachability, and infrastructure lifecycle.

## Decision and terminology

Three interfaces are distinct:

- a **draft substrate** stores concurrent edit history and exchanges changes;
- a **transport substrate** creates authenticated byte streams between identified peers;
- a **deployment substrate** builds, places, restarts, and connects endpoint processes.

An Eve **promotion gate** converts draft state into executable meaning. It must reject unresolved
conflicts, deserialize the declared Eve representation, validate the global conversation, and
compile a verified plan. Substrate identities never substitute for Eve conversation or plan
identities.

The first experimental adapters are Automerge, Iroh, and Miren. They are implementations of these
interfaces, not required parts of a future Eve standard.

## Static semantics

No draft change is executable merely because the draft converged. Promotion must produce a single
conflict-free Eve graph that passes all representation, semantic, and plan-verification checks.

A deployment artifact must declare the expected conversation and plan identities. A runtime that
receives those declarations must fail before accepting a connection when its locally compiled
identities differ.

## Runtime semantics

An authenticated transport may carry either reference or compact Eve Wire. Before frame zero,
both endpoints must agree on session version, conversation identity, plan identity, roles, and
encoding. When the substrate exposes authenticated peer and channel identities, the preface must
bind them to the connection rather than treating them as untrusted application claims.

Deployment placement and restart do not advance an Eve endpoint machine. Miren lifecycle events
remain infrastructure observations until a future Eve conversation explicitly models them.

Automerge sync payloads are transport-independent control data. They are not Eve semantic frames
unless a future conversation type explicitly declares them.

## Failure and security consequences

The current Iroh adapter authenticates the expected endpoint keys and binds the preface to a TLS
exporter. Authentication does not confer authorization. Key admission, allowed roles, plan policy,
rotation, revocation, and application freshness remain required before untrusted deployment.

Automerge resolves concurrent document operations but can retain same-property conflicts. Eve
must enumerate and reject those conflicts during promotion. A deterministic Automerge winner is
not an acceptable implicit language decision.

Miren-generated identity environment values guard against stale or substituted build inputs. They
are not secrets, signatures, or workload identity proofs.

## Canonical representation

Automerge history, actor IDs, deployment manifests, network addresses, relay choices, and endpoint
keys are excluded from the portable Eve conversation identity unless an Eve program explicitly
declares a policy that makes one of them semantic.

The optional Iroh fields in Eve Session Preface v0 are connection evidence, not part of the
conversation or plan digest.

## Compatibility and migration

The adapters are additive. Existing memory, TCP, and pinned-certificate QUIC plans remain valid.
Session Preface v0 keeps Iroh fields optional so existing fixtures and peers remain representable;
an Iroh transport nevertheless requires and verifies them.

Replacing Automerge, Iroh, or Miren must not change a successful endpoint trace for the same plan.

## Alternatives and prior art

- Put collaborative history into Eve's canonical graph: rejected because authoring history and
  executable meaning have different identity and conflict requirements.
- Define a new transport immediately: rejected until existing authenticated QUIC substrates fail a
  representative benchmark or semantic requirement.
- Treat deployment configuration as Eve source: rejected because infrastructure lifecycle and
  portable computation semantics need an explicit interface, not an accidental merger.

See [the substrate experiment](../docs/substrates.md) and [prior art](../docs/prior-art.md).

## Testable acceptance criteria

1. Independent Automerge replicas converge under ordered sync; unresolved same-field conflicts
   cannot be promoted.
2. A non-conflicting draft promotes through Eve validation into a verified, identified plan.
3. Reference and compact Eve plans retain equivalent successful traces over Iroh.
4. An unexpected Iroh peer identity or mismatched channel binding is rejected before frame zero.
5. Generated Miren TOML parses, pins build/runtime inputs, and binds both Eve identities.
6. A deployed server refuses an environment identity that differs from its compiled plan.
