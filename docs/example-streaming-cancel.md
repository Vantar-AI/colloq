# Worked example: streaming with cancel

This page answers one question with one example: what problem does Colloq solve that a schema
does not?

## Who this is for

Colloq is for systems where **you own every end of the contract**. You write the conversation
once. The compiler derives each side's local state machine from it. The client and the server
cannot drift apart, because neither one is written by hand.

If you do not control the other end, Colloq does not help you today. There is no bridge to an
existing gRPC or REST peer yet. That gap is recorded in [the roadmap](roadmap.md#known-gap-adoption-inside-existing-systems).

## The flow

A client sends a prompt to a model server. The server streams tokens back. After any token the
client may ask for more or cancel. The server may finish on its own. The network may fail at any
point.

## The usual way: a schema and hand-written state

With gRPC you would describe the messages:

```protobuf
service Generator {
  rpc Generate(stream ClientMsg) returns (stream ServerMsg);
}

message ClientMsg {
  oneof kind {
    Prompt prompt = 1;
    Continue continue = 2;
    Cancel cancel = 3;
  }
}

message ServerMsg {
  oneof kind {
    Token token = 1;
    Done done = 2;
  }
}
```

This is a good schema. It tells each side the **shape** of every message. It says nothing about
the **order**:

- The prompt must come first, and only once.
- The server must decide "token" or "done" before it sends a token.
- After the client cancels, no further token is part of the conversation.
- If the server times out mid-stream, the client may not know how far the server got.

Those rules live in two hand-written state machines, one in the client and one in the server.
Usually they are written by different people, in different repositories, at different times.
The bugs live in the gap between them: a token that arrives after the cancel, a retry that
replays half a stream, a client that believes the stream ended while the server believes it
timed out.

## The Colloq way: one conversation

In Colloq the order is the program. A text view of the same flow:

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
`transport.reset`, `transport.unreachable`, `transport.uncertain`) as a named branch of the
conversation, not as an untyped error.

From that one file the compiler derives both sides:

```bash
cargo run -- project examples/generate.colloqconv.json
# wrote build/endpoints/client.endpoint.json
# wrote build/endpoints/server.endpoint.json
```

## What it catches

Each case below is a real run of the prototype.

### 1. A correctly typed message at the wrong moment

The server sends a token without first choosing the `token` branch. In the protobuf version this
message is valid: it has the right type. Colloq rejects it:

```bash
cargo run -- verify-trace examples/generate.colloqconv.json \
  examples/traces/generate-wrong-order.invalid.json
```

```text
Error: TraceError { step: 1, state: "decide",
  expected: "select by server: done | token",
  received: "data server -> client: token" }
```

### 2. A token after the cancel

Take the valid cancel trace in
[`examples/traces/generate-cancel.valid.json`](../examples/traces/generate-cancel.valid.json)
(prompt, token, cancel) and add one more token from the server. Colloq rejects it, because after
the cancel the conversation has ended:

```text
Error: TraceError { step: 5, state: "end",
  expected: "end of conversation",
  received: "data server -> client: token" }
```

### 3. Two honest views of one failure

The server times out on its second send. The fault demo injects exactly that:

```bash
cargo run -- fault-demo --fault-role server \
  --fault-operation send --fault-at 2 \
  --failure transport.timeout \
  --peer-failure transport.uncertain
```

The server ends in `transport.timeout`. The client ends in `transport.uncertain`, because it
cannot know how far the server got. Both are declared outcomes of the same conversation. The
report says `"outcome_equivalent": false` instead of pretending both sides know the same thing.

### 4. Drift between versions

Change the conversation and the plan identity changes with it. Over TCP, QUIC and Iroh, the two
peers exchange a session preface that binds the conversation, the plan and the roles before any
data moves. A client built from an old version and a server built from a new one fail closed at
the preface. They do not exchange half-compatible messages. See [Colloq Wire](wire.md) and
[plans and identity](plan.md).

## A second example: an agent changes the contract

This is where Colloq is heading: [software you regenerate instead of maintain](regeneration.md).

Say an AI agent decides the server should answer repeated prompts from a cache. It does not
open two repositories and patch two state machines. It edits one thing, the conversation: it
adds a `cached` branch next to `token` and `done`. Today that edit goes through a draft
(`draft-create`, `draft-patch`), and `draft-promote` refuses it unless the new conversation
still validates. The promoted version gets a new plan identity, and both endpoints are derived
again from it. A client that still runs the old plan is refused at the preface, so it cannot
receive a `cached` message it does not understand.

The contract is what you keep. The endpoints are what you throw away and regenerate. What is
not derived yet: the code that decides *when* to answer from the cache is still written by
hand, and the record of who changed the contract and why (lineage, [RFC-0008](../rfcs/0008-evolution-loop.md))
is specified but not built.

## When a schema is enough

If the exchange is one request and one response, a schema already covers it. Colloq earns its
cost when the order of messages carries meaning: streams, cancellation, choices that change what
the other side may send next, and failures that leave the two sides with different knowledge.
