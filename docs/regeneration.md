# The goal: code as a derived artifact

## The core does not move

Colloq is a language for the typed conversation between servers, and its compiler,
identities and runtime exist to make that agreement checkable. That is the whole product.
Everything on this page is the reason the core is built the way it is, never a second
direction for it.

Read the rest as motivation and as an honest scoreboard. If a line on this page ever
argues for weakening the conversation, the identity or the gates, the line loses.

## The bet

Software is becoming something you *regenerate* rather than something you maintain. If a
component can be produced again cheaply and correctly, then accumulating decisions inside
it is a liability, not an asset. The valuable thing is the contract it satisfies, and the
evidence that a new version still satisfies it.

That shift has five consequences, and each one is a claim that can be checked.

## The five claims, and where Colloq stands

### 1. Code is a derived artifact, not an accumulation of decisions

**Partly true today.** The protocol machine for each role is derived. Nobody writes an
endpoint: `colloq project` produces it from the conversation, and the runtime refuses a
peer whose machine came from a different plan.

What is not derived is the work itself. What a role *does* with a message is still
hand-written, and the conversation is still hand-written source. Colloq derives the
coordination layer, not the application.

### 2. Provenance replaces version control as the source of truth

**Not yet.** The foundations are here: conversation identity and plan identity are content
addressed, a rename does not change them, and a mismatched peer is refused before frame
zero. That is the hard half of provenance, because it makes "the same meaning" a
computable question.

The other half is missing. Lineage, meaning parent, transform, generator, inputs and
compiler version, is specified in [governed evolution](evolution.md) and in RFC-0008 and
implemented nowhere. Git is still the source of truth.

### 3. AI agents are first-class developers

**Partly, and mostly on paper.** The typed graph exists, the Automerge draft store exists,
and the promotion gate rejects a draft that does not validate.

Missing: typed transactions, so an agent proposes structure instead of characters
(RFC-0007), and any enforcement of capability and budget limits (RFC-0008). Until those
land, an agent can edit, but the system cannot bound what it is allowed to edit.

### 4. Regeneration is evaluated against stability in production

**No.** There is no production system here, and no regeneration loop to evaluate. What
exists is the instrument: the [evidence protocol](../rfcs/0004-evidence-protocol.md)
pre-registers workloads, baselines and gates before a run, and the
[benchmark](benchmark.md) publishes the numbers including the unflattering ones. An
instrument with no measurements is still better than measurements with no instrument, but
it is not the claim.

### 5. The principles can be adopted incrementally in existing systems

**No, and this one is not on the roadmap.** Colloq today is greenfield: it replaces the
protocol layer between two services or it does nothing. There is no bridge from an
existing gRPC or REST service, and no way to put one conversation inside a running system
without rewriting both sides.

This is the most commercially dangerous gap. A language that only works on a blank page
gets admired and not adopted. It is listed here so that the omission is a decision rather
than an oversight.

## What Colloq adds that the list leaves out

Those five claims are about one codebase and its history. They assume the hard part is
remembering why the code looks like it does.

Colloq's bet is that the hard part is the agreement *between* processes. A component you
can regenerate is only safe to regenerate if the other side can prove the replacement
still means the same thing. That proof is what conversation identity, plan identity and
the session preface already do, and it is the part of the problem a single-codebase view
does not see.

Regeneration without a checkable contract across the network is just a faster way to break
a distributed system.

## What Colloq will not become

Three tempting adjacent products, and why each stays out:

- **A general code generator.** Colloq derives coordination from a contract. It does not
  write application logic, and adding that would make the contract a side effect of a
  generator instead of the source of truth.
- **A replacement for version control.** Content identity answers "is this the same
  meaning". It does not replace history, review or blame, and pretending otherwise would
  trade a solved problem for an unsolved one.
- **An agent framework.** An optional binding lets a model pick a declared branch. Nothing
  in the core depends on a model, and nothing should.

Each of these would grow the surface and weaken the one claim Colloq can actually defend:
that two processes can prove they still mean the same thing.

## How to read this page later

Each claim above should move from *no* to *partly* to *yes* with a link to the evidence,
not with a rewrite of the sentence. If a claim is still *no* a year from now, that is
information, and it stays on the page.
