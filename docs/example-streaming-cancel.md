# Worked example: regenerating both ends of a stream

Colloq starts from one bet: software is becoming something you
[regenerate rather than maintain](regeneration.md). An agent rewrites a service, a compiler
re-places it on other hardware, a client is produced again for a new platform. If code can be
produced again cheaply, the code is not the asset. The contract is, together with the evidence
that each new version still keeps it.

This page shows that with one small, concrete case. Every output below is a real run of the
prototype.

## Who this is for

Colloq is for systems where **you own every end of the contract**, because that is where you
can regenerate every end. You write the conversation once. Each side's local state machine is
derived from it, and nobody edits a derived side by hand.

If you do not control the other end, Colloq does not help you today. There is no bridge to an
existing gRPC or REST peer yet. That gap is recorded in [the roadmap](roadmap.md#known-gap-adoption-inside-existing-systems).

## The contract

A client sends a prompt to a model server. The server streams tokens back. After any token the
client may ask for more or cancel. The server may finish on its own. The network may fail at any
point. A text view:

```colloq
conversation Generate(prompt: Prompt) -> stream<Token> {
    roles client, server

    client -> server: prompt within 20ms

    loop {
        choice server {
            done  { end }
            token {
                server -> client: Token
                choice client {
                    continue { }
                    cancel   { cancel client -> server generation; end }
                }
            }
        }
    }
}
```

The executable version is [`examples/generate.colloqconv.json`](../examples/generate.colloqconv.json).
It also declares every transport failure (`transport.closed`, `transport.timeout`,
`transport.reset`, `transport.unreachable`, `transport.uncertain`) as a named branch, not as an
untyped error. Both sides are derived from it:

```bash
cargo run -- project examples/generate.colloqconv.json
# wrote build/endpoints/client.endpoint.json
# wrote build/endpoints/server.endpoint.json
```

## Why a schema is not enough to regenerate against

With gRPC you would describe the same messages:

```protobuf
service Generator {
  rpc Generate(stream ClientMsg) returns (stream ServerMsg);
}
message ClientMsg { oneof kind { Prompt prompt = 1; Continue continue = 2; Cancel cancel = 3; } }
message ServerMsg { oneof kind { Token token = 1; Done done = 2; } }
```

The schema fixes the **shape** of every message. The **order** lives in two hand-written state
machines: prompt first, a decision before each token, nothing after the cancel. When an agent
regenerates the server, the schema still passes. Whether the new server keeps the order is
something you find out in production.

## Four concrete situations

### 1. An agent rewrites the server. Does it still keep the contract?

The regenerated server is checked against the contract, not against the old code. Here a new
server sends a token without first choosing the `token` branch. The message has exactly the
right type, so a schema accepts it. Colloq does not:

```bash
cargo run -- verify-trace examples/generate.colloqconv.json \
  examples/traces/generate-wrong-order.invalid.json
```

```text
Error: TraceError { step: 1, state: "decide",
  expected: "select by server: done | token",
  received: "data server -> client: token" }
```

The same check catches a regenerated server that keeps streaming after the client cancels. Add
one more token to the valid cancel trace
([`generate-cancel.valid.json`](../examples/traces/generate-cancel.valid.json)) and it fails:

```text
Error: TraceError { step: 5, state: "end",
  expected: "end of conversation",
  received: "data server -> client: token" }
```

This is the evidence a regeneration loop needs: a new version is accepted because it keeps the
contract, not because it resembles the old code.

### 2. The code changes, the meaning does not

An agent regenerates the conversation and only changes its title. The identity stays exactly the
same, so every deployed peer still accepts it:

```text
promoted example.generate as sha256:92c198d0...   (unchanged)
```

The agent changes the prompt deadline from 20 ms to 200 ms. That is a change of meaning, and the
identity changes with it:

```text
promoted example.generate as sha256:4d852b08...   (new contract)
```

"Is this still the same contract?" becomes a computed question, not a code review.

### 3. Regenerated for other infrastructure

The same plan runs over memory, TCP and QUIC. The wiring is produced again for each transport.
The meaning is not touched, and all three runs report the same semantic trace:

```text
memory  plan sha256:016d5975...  trace sha256:b833fa2f...  semantic_trace_equivalent: true
tcp     plan sha256:016d5975...  trace sha256:b833fa2f...  semantic_trace_equivalent: true
quic    plan sha256:016d5975...  trace sha256:b833fa2f...  semantic_trace_equivalent: true
```

When the network fails, the regenerated sides do not pretend to know more than they do. With a
server timeout injected, the server ends in `transport.timeout` and the client in
`transport.uncertain`, because the client cannot know how far the server got:

```bash
cargo run -- fault-demo --fault-role server --fault-operation send --fault-at 2 \
  --failure transport.timeout --peer-failure transport.uncertain
```

### 4. An agent changes the contract itself

The agent decides the server should answer repeated prompts from a cache. It does not open two
repositories and patch two state machines. It edits one thing, the conversation.

Its first edit adds a `cached` branch that points to a state that does not exist:

```bash
cargo run -- draft-create examples/generate.colloqconv.json
cargo run -- draft-patch build/generate.colloqdraft \
  --pointer /states/1/branches/cached --value '"answer-from-cache"' --out build/bad.colloqdraft
cargo run -- draft-promote build/bad.colloqdraft
```

```text
Error: Validation(... code: "CLQ0016", path: "$.states[1].cached",
  message: "target state answer-from-cache does not exist" ...)
```

The gate refuses it. Nothing is generated and nothing reaches a server. The fixed edit lets
`cached` end the conversation, and the gate accepts it:

```bash
cargo run -- draft-patch build/generate.colloqdraft \
  --pointer /states/1/branches/cached --value '"end"' --out build/ok.colloqdraft
cargo run -- draft-promote build/ok.colloqdraft
# promoted example.generate as sha256:5e5be47f...
```

The new contract has a new identity. Both sides are derived again from it. Over TCP, QUIC and
Iroh the peers exchange a session preface that binds the conversation, the plan and the roles
before any data moves, so a client still on the old plan is refused at connect time. It never
receives a `cached` choice it does not understand. See [Colloq Wire](wire.md) and
[plans and identity](plan.md).

## What is not regenerated yet

Colloq derives the coordination layer, not the application. The code that decides *when* to
answer from the cache, or how to produce a token, is still written by hand. The record of who
changed the contract and why (lineage, [RFC-0008](../rfcs/0008-evolution-loop.md)) is specified
but not built. [The goal page](regeneration.md) keeps the honest scoreboard.

## When a schema is enough

If the exchange is one request and one response, and nobody regenerates either side, a schema
already covers it. Colloq earns its cost when the order of messages carries meaning and the code
on each side is expected to be produced again.
