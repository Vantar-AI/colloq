# Why Colloq exists

Most programming languages assume the expensive part of software is writing it. That
assumption is now wrong. Code is cheap to produce and getting cheaper. What stayed
expensive is knowing whether a new version still means the same thing as the old one,
and whether the other side of the network agrees.

Colloq is a language built for that world.

## Durable meaning, disposable implementation

A running system has two kinds of parts, and treating them the same is the mistake.

**The durable part is the agreement.** Which roles exist, who may speak when, what a
choice means, which failures are possible, what ends the exchange. This is what a team
argues about, what an audit asks for, and what breaks a system when two sides read it
differently.

**The disposable part is everything else.** The endpoint implementation, the encoding on
the wire, the placement of a role on a machine, the retry policy, the code that serialises
a message. A generator can write all of it. A better generator can rewrite it next month.

Conventional stacks mix the two. The agreement is spread across a schema file, a client
library and a server handler, so replacing an implementation risks changing the meaning
without anyone noticing. Colloq separates them by construction: the conversation carries
the meaning and has an identity, and everything under it is derived, replaceable and
verifiable against that identity.

## Replaceable, not merely maintainable

If a component can be regenerated cheaply, maintenance stops being the goal. The goal is
to make replacement safe, frequent and boring. That needs four things the usual toolchain
does not give you:

- **An identity that survives a rewrite.** Colloq identities are content addressed and
  independent of names and formatting. A rename is not a change. A new branch is.
- **A check that runs before deployment.** A projected endpoint either satisfies the
  conversation or it does not, and the compiler answers that question without a cluster.
- **A boundary a generated part cannot cross.** A replacement inherits no capability from
  the thing it replaces, and cannot widen its own budget or weaken its own gate.
- **A record of where a part came from.** Parent, transform, generator and compiler
  version, kept with the artifact, so a rollback is a decision rather than an excavation.

## Written by people, and increasingly not

Models already propose changes to routing, placement and whole subgraphs. Evolutionary
search will propose more. The question is not whether machine-authored change happens. It
is whether the substrate can tell a good change from a dangerous one.

A pile of text files cannot. Formatting noise looks like meaning, meaning looks like
formatting noise, and the only test is production. A typed, content-addressed graph can:
a proposal is a transaction that either type-checks, fits its budget, passes an
independent gate and survives a canary, or it does not reach production.

That is why Colloq starts from the conversation rather than from a syntax. The point is
not a nicer way to write network code. The point is a substrate where meaning is
checkable, so implementations can be replaced as fast as machines can write them.

## The scoreboard

The core is the typed conversation and the proof that two sides still mean the same thing.
Everything below is the motivation for building it that way, scored honestly.

| The claim | Colloq today |
| --- | --- |
| Code is a derived artifact | **Partly.** Each role's protocol machine is derived and nobody writes it. What a role *does* with a message is still hand-written, and so is the conversation. |
| Provenance replaces version control | **Not yet.** Content identity makes "the same meaning" computable, which is the hard half. Lineage is specified and implemented nowhere. Git is still the source of truth. |
| Agents as first-class developers | **Partly.** Typed graph, draft store and promotion gate exist. Typed transactions and enforced capability limits are drafts. |
| Regeneration evaluated against stability | **No.** The instrument exists, the pre-registered protocol and the published benchmark. The measurements do not. |
| Adoption inside existing systems | **No, and not on the roadmap.** Colloq replaces the protocol layer or does nothing. That gap is recorded rather than hidden. |

The list above is about one codebase and its history. Colloq's bet is that the hard part is
the agreement *between* processes: a component is only safe to regenerate if the other side
can prove the replacement still means the same thing. Regeneration without a checkable
contract across the network is a faster way to break a distributed system.

What Colloq will not become, no matter how tempting: a general code generator, a
replacement for version control, or an agent framework. Each would grow the surface and
weaken the one claim it can defend. The full reasoning is in
[the goal](/docs/regeneration/).

## Where the project actually is

Colloq is a v0.1 research prototype under Apache 2.0, and it is honest about the line
between what runs and what is planned.

**Working today:** the conversation checker, endpoint projection, the reusable compiled
plan with content identity, a reference runtime over shared memory, TCP, authenticated
QUIC and Iroh, a session preface that refuses a mismatched peer, an Automerge draft gate
for collaborative structural edits, and a published benchmark with a hand-written
baseline.

**Next:** a binary codec, execution across more physical hosts, identity rotation and
revocation, typed composite graph transactions, and then the governed evolution loop where
a system adopts an improvement it found and rejects one that reaches for authority it was
not given.

The [roadmap](/docs/roadmap/) states the exit criterion for each phase before the work
starts, and the [evidence protocol](/rfcs/0004-evidence/) pre-registers what a result must
contain to count. The [benchmark](/docs/benchmark/) publishes where Colloq is slower than
hand-written code, by how much, and against which target.

## Who builds it

Colloq is developed in the open by Vantar Group LLC, a Wyoming company, and released
under Apache 2.0. Design decisions land as [RFCs](/rfcs/) before they land as code, so the
reasoning is reviewable and so is the disagreement.

If you are building a system where several servers must agree on a protocol, the
[quickstart](/docs/quickstart/) takes about ten minutes. Questions, objections and
benchmark disputes are welcome at
[hello@vantar.xyz](mailto:hello@vantar.xyz) or in
[the issues](https://github.com/Vantar-AI/colloq/issues).
