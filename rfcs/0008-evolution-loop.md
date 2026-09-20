# RFC-0008: The governed evolution loop

- **Status:** Draft
- **Date:** 2026-09-20
- **Scope:** Candidate proposal, gating, canary promotion, lineage

## Summary

`docs/evolution.md` states the invariants a machine-authored change must satisfy. Nothing
executes them. This RFC defines the loop that does: how a candidate is proposed, what is
checked before it is allowed to run, how it is evaluated in isolation, how it reaches
production traffic, and how it is withdrawn. The loop is a state machine, and it is
written as a Colloq conversation, so the system that governs evolution is subject to the
same checking as the systems it governs.

## Motivation and concrete workloads

**Routing thresholds.** A router's cache-versus-model decision has a threshold. A search
can propose a better one, which is a small, bounded, measurable change, and a good first
target.

**Placement.** Which role runs on which machine, proposed from observed load. The contract
does not change, only the plan's placement constraints.

**Endpoint implementation.** A generated implementation of one role that must satisfy the
same projected endpoint. The interface is fixed by the conversation, so the candidate has
somewhere to be wrong in public.

## Non-goals

No open-ended self-modification. A candidate may not edit its own gate, widen its own
budget, acquire a capability, or change a public contract. Those require a separate
authority and a human decision, and this RFC keeps that line.

## Decision and terminology

- **Region.** The declared part of a system a generator may edit, named in the policy. The
  complement is constitutional and outside the loop.
- **Candidate.** A proposal within a region: a graph transaction (RFC-0007), a placement
  change, or an implementation artifact, always with a parent and a generator identity.
- **Gate.** An independent check the candidate did not author. At minimum: type and effect
  check, capability and budget check, the conformance suite for the projected endpoint,
  and one metric with a pre-registered threshold from RFC-0004.
- **Canary.** A bounded share of real traffic for a bounded time, with an automatic
  withdraw condition fixed before the canary starts.
- **Lineage.** An append-only record: parent, transform, generator, inputs, compiler
  version, gate results, canary outcome, decision.

## Static semantics

`colloq-evolution-policy-v0` declares the region, the capability ceiling, the resource
budget, the gates in order, the canary shape and the withdraw condition. A policy that
places a gate inside the editable region is rejected by the checker. A candidate that
touches anything outside the region is rejected before evaluation.

## Runtime semantics

The loop is the conversation `Evolve`, with roles `proposer`, `gatekeeper`, `evaluator`
and `operator`:

```text
proposed -> checked -> evaluated -> decided -> canary -> promoted
                 \-> rejected      \-> rejected       \-> withdrawn
```

Every edge is a declared transition, and every terminal is declared. `rejected`,
`withdrawn` and `promoted` all carry the lineage record. Because it is a conversation, the
progress of an evolution run is inspectable with the same trace tooling as anything else.

## Failure and security consequences

This is where a language for machine-authored change either earns trust or loses it.

- A candidate cannot supply its own gate, and the policy checker enforces that statically.
- A withdraw condition that is never evaluated is a silent failure, so the operator role
  must report the canary outcome on every run, including "no signal".
- A promoted candidate is not trusted afterwards: the next proposal restarts the loop.
- Lineage is append-only. A run that cannot write lineage does not promote.
- The most dangerous failure is a plausible improvement that optimises the metric and
  breaks something unmeasured. The mitigation is pre-registration, plus at least one gate
  the generator cannot see the definition of.

## Canonical representation

New `spec/colloq-evolution-policy-v0.schema.json` and `spec/colloq-lineage-v0.schema.json`.
The evolution conversation ships as `examples/evolve.colloqconv.json`, so the loop itself is
checkable, projectable and runnable like any other conversation.

## Compatibility and migration

Entirely additive. A system without an evolution policy has no editable region, which is
the correct default: nothing may be machine-edited until someone declares what may.

## Alternatives and prior art

- **A hosted control plane that owns promotion.** Faster to build, but it moves the
  invariants out of the language and into an operator's codebase.
- **CI-only promotion.** Good for the check stage, wrong for the canary, which needs live
  traffic and an automatic withdraw.
- **Let the model decide.** Rejected. The point of the loop is that the decision has a
  gate the proposer does not control.

## Testable acceptance criteria

1. A candidate that improves the pre-registered metric passes the gates and is promoted,
   with a lineage record that reproduces the decision.
2. A candidate that reaches for a capability outside its region is rejected at the gate,
   before evaluation, in a test that asserts nothing was evaluated.
3. A candidate that authors its own gate is rejected by the policy checker.
4. A canary that trips the withdraw condition is withdrawn automatically, and the system
   returns to the parent artifact with no manual step.
5. Lineage is append-only under a property test: no run can rewrite an earlier record.
6. The evolution conversation passes `colloq check`, projects, and runs over memory and
   over QUIC with the same semantic trace.
7. A run with an unavailable evaluator ends in a declared failure, never in a promotion.

## Open questions

- What is the smallest useful first region: a threshold constant, or a whole endpoint
  implementation?
- Does the evaluator need its own identity and authorization, given that it sees candidate
  code?
- How is "no signal" distinguished from "no harm" when a canary is too short, and who is
  allowed to extend it?
