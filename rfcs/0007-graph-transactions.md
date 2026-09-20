# RFC-0007: Typed composite graph transactions

- **Status:** Draft
- **Date:** 2026-09-20
- **Scope:** Structural editing, the draft gate, generator-facing APIs

## Summary

The draft store accepts optimistic scalar patches at a JSON pointer, then validates the
materialised graph and promotes it. A scalar patch is the wrong unit for the edits that
matter: adding a role, adding a branch with its states, or renaming a message all require
several coordinated changes that are only meaningful together. This RFC defines a
transaction: an ordered set of typed operations that either applies whole or not at all,
against a stated base identity.

## Motivation and concrete workloads

**Add a branch.** A new choice branch needs the branch entry, the target state, the
failure edges on that state and the message type it sends. Applied one scalar at a time,
every intermediate graph is invalid, so the validator cannot help until the last write
lands.

**Rename a message type.** Every reference must move together. With scalar patches, a
concurrent editor can interleave and produce a graph that references a name that no longer
exists.

**Machine-authored edits.** A generator proposes structure, not characters. Giving it
`replace pointer with scalar` forces it to serialise its intent into a sequence that loses
the intent, and that no gate can read back.

## Non-goals

This RFC does not define a text syntax for edits, does not introduce merge heuristics that
guess a resolution, and does not change what a valid conversation is. A transaction that
produces an invalid graph is rejected by the existing checker, unchanged.

## Decision and terminology

- **Transaction.** `{ base: <conversation identity>, ops: [...], intent: <string> }`.
  Applies only to that base. A different base is a conflict, not a merge.
- **Typed operation.** One of `add_role`, `remove_role`, `add_state`, `remove_state`,
  `add_branch`, `remove_branch`, `retarget`, `add_type`, `rename`, `set_deadline`,
  `add_failure_edge`. Each carries typed fields, not pointers.
- **Atomic application.** Either every operation applies and the result validates, or the
  draft is untouched.
- **Intent.** A short free-text field, carried into the promotion record. It is evidence
  for a human reviewer, never an input to validation.

## Static semantics

A transaction is checked in three stages. First, each operation is well formed on its own.
Second, the sequence is coherent: an operation cannot reference something a later
operation creates, and cannot remove something a later operation needs. Third, the
resulting graph passes the ordinary conversation checker. Only then does the draft advance.

`rename` is defined as a whole-graph substitution of an identifier, not a text
replacement, so it cannot change semantics. Renaming therefore produces the same
conversation identity, which is the property RFC-0001 asks for and which this RFC makes
testable.

## Runtime semantics

The Automerge draft store gains `apply_transaction`. Concurrent transactions against the
same base are ordered by the store; the first one to apply wins, and the second is
returned to its author with the new base identity and the conflicting operations named.
The authenticated draft-sync conversation carries transactions instead of scalar patches;
the promotion gate is unchanged, because it already validates the materialised graph.

## Failure and security consequences

- A transaction is a bigger unit of authority than a scalar. The gate must therefore keep
  rejecting a transaction whose result is invalid, which it does today.
- A generator that can apply transactions can still only produce graphs the checker
  accepts. Capability and budget limits stay where RFC-0003 and `docs/evolution.md` put
  them.
- Rejected transactions are recorded with their intent, so a generator that repeatedly
  proposes invalid structure is visible rather than silent.

## Canonical representation

New `spec/colloq-transaction-v0.schema.json`. The draft artifact gains a transaction log
with, per entry: base identity, operations, intent, result identity, and the outcome.

## Compatibility and migration

Scalar patches keep working and are defined as single-operation transactions with an empty
intent. `draft-patch` stays for one version. The draft file format gains the log and stays
readable by the current reader.

## Alternatives and prior art

- **Operational transformation on text.** Rejected: the graph is the source of truth,
  and text is a projection.
- **Patch sets of scalars with a validity barrier.** Simpler, but it cannot express
  intent and cannot report which operation conflicted.
- **Full replacement of the graph per edit.** Correct but useless for collaboration, since
  every edit conflicts with every other edit.

## Testable acceptance criteria

1. Adding a branch with its state, failure edges and message type applies as one
   transaction, and no intermediate graph is ever validated.
2. A transaction whose result fails the checker leaves the draft byte-identical.
3. Two concurrent transactions on one base: one applies, the other is returned with the
   new base and the names of the conflicting operations.
4. A property test over generated graphs: `rename` never changes the conversation
   identity, and never changes the projected endpoints.
5. A transaction against a stale base is refused with a typed error that carries both
   identities.
6. The transaction log round-trips through the draft-sync conversation unchanged.

## Open questions

- Does `remove_role` belong in v0 at all, given that it invalidates every plan that used
  it?
- Should a transaction be able to carry a typed hole, so a generator can propose structure
  it cannot yet fill?
- Is `intent` free text, or a small enumeration plus free text, so a gate can route
  proposals by kind?

## Which claim this advances

**Agents as first-class developers** moves from *partly* toward *yes*: an agent proposes typed structure with a stated intent instead of characters, and a gate can read what was intended. See [the goal](../docs/regeneration.md) for the full scoreboard. The core does not
change: this work exists to make the conversation and its identity stronger, not to add a
second direction.
