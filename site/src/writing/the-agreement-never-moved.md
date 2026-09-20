# Thirty years of making two computers agree

Here is a bug I have now seen at four companies, in four languages, with four different
stacks. I will describe it once and you can map it onto your own.

A service gets a new response case. The team that owns it adds the case, ships it, and
tells the other team in a message that gets read and forgotten. The other team's client
has a switch statement. The new case falls through to the default branch. The default
branch was written three years ago by somebody who has left, and it logs a warning and
returns an empty result.

Nothing crashes. The dashboards stay green. Six weeks later someone notices a number is
wrong.

Every generation of distributed tooling has claimed to solve this. It is worth walking
through what each one actually solved, because the pattern is clear once you line them up.

## Remote procedure calls

The first serious attempt was to make the network invisible. You call a function, and it
happens to run somewhere else. Sun RPC, then a long line of successors.

The appeal is obvious. Programmers already know how to call functions, so make the remote
case look identical. The problem is equally obvious in hindsight, and it was written down
early and repeatedly by people who had watched it fail: a local call and a remote call are
not the same thing. One of them can be slow, can be partial, and can be uncertain in a way
that has no local equivalent.

What RPC gave us was the argument list. Both sides agreed on the shape of the call. What
it did not give us was any way to talk about what happens when the call does not come back.

## Interface definition languages

CORBA and its relatives made the interface explicit. You write an IDL file, you generate a
stub for each language, and both sides are generated from the same definition.

That is a real improvement and it is the ancestor of everything we use now. The generated
stub cannot disagree about the field order or the type of the third argument.

But notice what an IDL describes. It describes a set of operations and the types of their
arguments and results. It does not describe when you may call them, in what order, what
must have happened first, or what the other side is allowed to do while you wait. The
agreement covered the shape and left the order to a document, if one existed.

## REST and the schema era

REST arrived partly as a rejection of all of that, and it traded a formal interface for a
convention. Resources, verbs, status codes. In practice, the contract moved into
documentation, and then, when documentation proved inadequate, into a schema file that
described the documentation.

The schema era is still where most systems live. It is an improvement and it has the same
hole. A schema file says what a response looks like. It says nothing about the sequence.
You can read every schema a service publishes and still not know whether you are allowed
to call this endpoint before that one, whether a retry is safe, or what you should do if
the stream stops halfway.

## gRPC and streaming

Modern RPC added the parts people kept building by hand: streams, deadlines, cancellation
propagation. This is genuinely better, and the deadline in particular fixed a whole class
of hanging systems.

Still, look at where the order lives. A `.proto` file declares that a method takes a stream
and returns a stream. It does not declare that the client must send a configuration message
first, that the server will send exactly one acknowledgement, that after the acknowledgement
either side may send until one of them sends a terminator, and that a cancellation before
the acknowledgement means something different from a cancellation after it.

All of that is real, all of it is load-bearing, and all of it lives in two hand-written
implementations and possibly a comment.

## The thing that never moved

Line the generations up and the pattern is not subtle.

| Generation | Agreed on | Left out |
| --- | --- | --- |
| RPC | argument shape | failure, order |
| IDL | operations and types | order, state, failure branches |
| REST plus schemas | payload shape | everything else |
| Modern RPC | shape, deadlines, streaming | the order of the exchange |

Every generation formalised a little more of the shape and left the order to prose. The
order is where the bug at the top of this essay lives. It is where almost every integration
outage I have debugged has lived.

There is a reason for the omission, and it is not stupidity. Describing shape is easy and
composable. Describing order means describing a state machine that two parties advance
together, including what each one may do when the other goes quiet, and that is harder to
write down and much harder to make ergonomic.

It has also been done. Session types have been in the literature for decades. Protocol
verification tools exist and find real bugs. The ideas are not new. What has been missing
is a version of them that a working team would actually adopt on a Tuesday.

## Why it matters more now than it did

For most of this history you could survive with prose, because the rate of change was
human. A protocol changed when a team decided to change it, which meant a meeting, a
document, a deprecation window, and a person who remembered the history.

That rate is going up, and the reason is obvious. Code is being generated faster than it
is being reviewed, on both sides of every interface, and the memory of why the protocol
looks like it does is not in anyone's head.

Generating both sides of an unspecified agreement does not produce agreement. It produces
two confident implementations of slightly different protocols, faster than before, with
plausible code and green tests.

The default branch still logs a warning. The number is still wrong six weeks later. It just
arrives sooner.

## What I would want instead

I want the order to be a thing I can hold. One artifact that says: these are the roles,
this is who speaks first, these are the branches, this is what happens when a message does
not arrive in time, and this is how it ends. I want each side derived from that artifact
rather than written against it. And I want the artifact to have an identity, so that when
it changes, the other side notices by refusing rather than by logging a warning.

That is not a new idea. It is the oldest missing piece in the list above, and the reason to
build it now is that the rate of change finally outran the prose.

If you want to see what that looks like as a language, the [quickstart](/docs/quickstart/)
takes ten minutes. If you want the argument rather than the tool,
[RFC-0002](/rfcs/0002-conversation/) is the honest version, including what it does not
solve yet.
