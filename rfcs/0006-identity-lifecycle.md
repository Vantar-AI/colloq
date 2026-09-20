# RFC-0006: Identity rotation and revocation

- **Status:** Draft
- **Date:** 2026-09-20
- **Scope:** Node identities, endpoint tickets, authorization policy, session admission

## Summary

Colloq binds a conversation to exact peers: a node identity proves who a process is, a
ticket says where it can be reached, and an authorization policy says which identity may
take which role under which plan. All three are written once and never change. A key
cannot be rotated without hand-editing every peer, and a compromised key cannot be
withdrawn at all. This RFC defines rotation with overlap, and revocation that takes effect
without a coordinated restart.

## Motivation and concrete workloads

Three situations already break the current model.

**A key is compromised.** Today the only remedy is to edit every policy file by hand and
restart every process. Until the last one restarts, the compromised identity is still
authorised.

**A host is replaced.** A new machine means a new identity, so every peer's policy must be
edited before the replacement can speak, which makes a routine operation a coordinated
change.

**Scheduled rotation.** Good practice is to rotate a long-lived key on a schedule. With no
overlap window, rotation is an outage.

## Non-goals

No certificate authority, no external PKI and no revocation network. Colloq stays
peer-to-peer with local policy. This RFC does not add roaming identity, does not define
key storage hardware, and does not change what a role means.

## Decision and terminology

- **Identity.** An Iroh keypair, as today.
- **Identity set.** A named principal holding an ordered list of identities, each with a
  state: `active`, `retiring` or `revoked`. A policy grants a role to a principal, not to
  a raw key.
- **Overlap window.** A period in which a principal has two `active` identities, so peers
  may accept either while the new one propagates.
- **Revocation record.** A signed statement that an identity is `revoked` from a given
  sequence number, carrying a reason and a timestamp, and signed by the principal's
  current active key or by a declared recovery key.

## Static semantics

`colloq-authorization-v0` gains principals. A rule binds `principal` + `role` + `plan`
instead of `endpoint_id` + `role` + `plan`. An identity set is a new artifact,
`colloq-principal-v0`, listing identities with states and a monotonic sequence number. The
checker rejects a set with two identities in `active` beyond the declared overlap window,
a revoked identity that reappears as active, and a sequence number that goes backwards.

## Runtime semantics

Admission gains one step. After the transport authenticates the peer key and before the
role is granted, the server resolves the key to a principal and checks the identity state.
An identity in `retiring` is accepted and produces a warning in the trace. An identity in
`revoked` is refused with a new declared failure, `identity.revoked`, which terminates the
session before frame zero.

A running session whose peer identity is revoked mid-conversation is terminated at the
next transition, not silently continued. The declared failure is `identity.revoked`, and
the trace records the sequence number that caused it.

## Failure and security consequences

- Revocation is only as fast as distribution. This RFC defines the local rule; how a
  record reaches a peer is deployment-specific, and the runbook must say so.
- A recovery key that never rotates is a standing risk, so a principal may declare
  `recovery: none` and accept that a lost key means a new principal.
- Rotation without overlap is a self-inflicted outage, so the checker warns when an
  overlap window is zero.
- Two new denial paths exist. Both fail closed, and both are typed failures rather than
  transport errors, so a caller can distinguish "you are not allowed" from "the network
  broke".

## Canonical representation

New `spec/colloq-principal-v0.schema.json`. `colloq-authorization-v0` gains
`principal` alongside the existing `endpoint_id`, with the raw form kept for one version
and marked deprecated.

## Compatibility and migration

A policy with raw endpoint IDs keeps working: the loader treats each raw ID as a
single-identity principal. `colloq policy-migrate` rewrites a policy into principal form.
Nothing in the conversation, the plan or the wire changes.

## Alternatives and prior art

- **Short-lived certificates.** Solves revocation by expiry, but needs an issuer, which
  contradicts the peer-to-peer model.
- **Revocation lists distributed over Colloq itself.** Attractive, and probably the right
  answer later, but it makes admission depend on a conversation that itself needs
  admission. Deferred deliberately.
- **Do nothing and rebuild the cluster.** Honest for a research prototype, dishonest for
  anything that holds data.

## Testable acceptance criteria

1. A principal with two active identities is accepted by both during the overlap window,
   and one of them is refused after it ends.
2. A revoked identity is refused at admission with `identity.revoked`, in a test that
   asserts the failure ID and that no frame was exchanged.
3. A session in progress ends with `identity.revoked` when the peer is revoked mid-run.
4. A policy in the old raw form loads unchanged, and `policy-migrate` produces an
   equivalent principal policy, proven by a property test over generated policies.
5. A sequence number that goes backwards is rejected by the checker.
6. The two-node smoke script covers rotate-then-revoke end to end.

## Open questions

- Does a revocation record need a signature from a second identity to resist a stolen key
  revoking the legitimate one?
- Should `retiring` be a separate state, or is a deadline on the identity enough?
- Where does the overlap window live: in the principal artifact, or in local policy?
