# Writing code you expect to throw away

A few months ago I deleted a service I had written and regenerated it from its interface
in about twenty minutes. The new one was better, because the old one carried two years of
decisions that were correct when they were made and had stopped being correct since.

That experience is becoming common, and it quietly breaks a habit most of us learned
early: treat code as an asset, maintain it, keep its history, be careful with it. If a
component can be produced again cheaply, the accumulated decisions inside it are not an
asset. They are sediment.

The interesting question is what you keep when you stop keeping the code.

## Two kinds of parts

Everything in a running system falls into one of two piles, and the trouble starts when
you treat them the same.

The first pile is what the system promises. Which roles exist. Who may speak when. What a
choice means. Which failures are possible. When an exchange is finished. This is what a
team argues about in a design review, what an auditor asks about, and what breaks a system
when two sides read it differently.

The second pile is everything underneath. The handler. The retry policy. The serialisation
code. The placement of a process on a machine. The caching. Almost all of your lines of
code are in this pile, and almost none of your meaning is.

For seventy years we kept both piles in the same place, because there was no reason not
to. Writing was the expensive part, so preserving what you wrote was obviously right.

## What changes when writing gets cheap

If the second pile can be regenerated, then keeping it becomes a choice rather than a
necessity, and it is often the wrong choice. Old implementations accumulate workarounds for
conditions that no longer exist. They encode performance decisions made against a different
load. They contain the compromise someone made at 2am in 2023.

But regeneration only works if you can answer one question afterwards: does the new version
still do what the old one promised?

If you cannot answer that mechanically, regeneration is not a capability. It is a way of
losing behaviour you did not know you had. Everyone who has rewritten a service they did
not fully understand knows this feeling. You ship the clean version and find out what the
old one did, one incident at a time.

## The four things worth keeping

Working this way for a while, four things turn out to carry the weight.

**A contract precise enough to regenerate against.** Not a description. Something a
machine can check a replacement against, including the order of the exchange and the
failure cases, not just the shape of the payloads. If your contract is a schema file plus
a wiki page, you cannot regenerate safely, because the wiki page is where all the
interesting requirements live.

**An identity that only changes when meaning changes.** This is the one people skip and it
matters more than it sounds. If your notion of "changed" is a file hash or a git diff, then
reformatting looks like a change and a renamed field looks like a change, so every
regeneration is a false alarm. False alarms train people to ignore alarms. You want the
opposite: something that stays identical when a generator reformats everything, and changes
loudly when a branch is added.

**Provenance.** Which version produced this, from what parent, with which inputs. Not for
compliance theatre. For the Tuesday when something is wrong and the only useful question
is what changed and where it came from. Git tells you which human committed the bytes. It
does not tell you which generator produced them, from which prompt, against which contract
version. That gap is getting wider.

**Evidence.** A fixed workload, a baseline, and a number, recorded before you started so
you cannot move the goalposts afterwards. Regeneration produces plausible code, and
plausible is exactly the failure mode that a test suite written by the same generator will
not catch.

Everything else, including the code itself, can go.

## This is not new advice, exactly

The individual pieces are old. Content-addressed storage is old. Interface-first design is
old. Provenance is an entire research field. Pre-registration came from science, which
learned the hard way what happens when you decide what counts as success after seeing the
result.

What is new is the pressure. All four of those disciplines used to be optional because the
volume of change was human-sized. You could hold a system in your head, or find the person
who could. That is ending, not because models are magic, but because the cost of producing
a plausible change fell by an order of magnitude while the cost of verifying one did not
move.

When production gets cheaper and verification does not, verification becomes the whole job.

## What I would do on Monday

Concretely, if you want to work this way before your stack supports it:

1. Pick one interface between two services. Write down the order of the exchange, not just
   the payloads. Include what each side does when the other goes quiet. This document will
   be embarrassing, because you will find three cases nobody agreed on.
2. Make that document the input to both sides rather than a description of them. Generate
   what you can. Hand-write the rest against it.
3. Give it a version identity that ignores formatting. Even a hash of a canonical form is
   better than nothing. Have both sides refuse to talk when the identity differs, in
   staging first, because the first time you turn that on you will find a mismatch you did
   not know about.
4. Before you regenerate anything, write down the number you expect and where you will
   read it. Then regenerate.

You do not need a new language for any of that. You need the discipline, and the discipline
is easier if the tools push you toward it, which is the argument for building better tools
rather than better intentions.

## The part I am less sure about

I do not know how far up the stack this goes. Regenerating a protocol adapter is one thing.
Regenerating a system's core domain logic is a different claim, and I have not seen it work
on anything with a decade of hard-won behaviour in it.

I also do not know what happens to the craft. A lot of what I know is pattern recognition
from having read a lot of code that I did not write and could not regenerate. If the code
is disposable, some of that learning has nowhere to happen. That might be fine. It might
not. Anyone who tells you confidently either way is guessing.

What I am sure about is narrower. The parts of a system worth protecting are the promise,
the identity of that promise, where a change came from, and the evidence that it still
holds. Protect those four and you can throw the rest away as often as you like. Skip them
and you are not regenerating software. You are rewriting it and hoping.
