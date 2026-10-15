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

## Where this bites in real products

- **The stop button in a chat app.** The user clicks stop. The UI stops drawing, but the
  server keeps generating and the tokens are still billed. Nothing in the schema says
  "no token after cancel", so nothing checks it. In Colloq that rule is case 2 above.
- **A voice agent that gets interrupted.** The caller talks over the bot. The speech service
  must stop sending audio for the old answer and start on the new one. A late audio chunk from
  the old answer is a correctly typed message at the wrong moment, which is case 1.
- **Failover between model servers.** A gateway sends a prompt, the server goes quiet, the
  gateway retries on a second server. Did the first one already stream half the answer? The
  gateway cannot know. Colloq makes it say so (`transport.uncertain`, case 3) instead of
  guessing, so the retry logic must handle that branch.
- **A mobile app that updates slowly.** The server ships a new message type. Half the users
  still run last month's app. With a schema they receive a message they do not understand. In
  Colloq the old app and the new server refuse each other at connect time (case 4).

## A second example: an agent changes the contract

This is where Colloq is heading: [software you regenerate instead of maintain](regeneration.md).

An AI agent decides the server should answer repeated prompts from a cache. It does not open
two repositories and patch two state machines. It edits one thing, the conversation. This is a
real run of the prototype.

First the agent makes a mistake. It adds a `cached` branch that points to a state that does not
exist:

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

The gate refuses it. Nothing is generated and nothing reaches a server. The agent fixes the edit
so that `cached` ends the conversation, and the gate accepts it:

```bash
cargo run -- draft-patch build/generate.colloqdraft \
  --pointer /states/1/branches/cached --value '"end"' --out build/ok.colloqdraft
cargo run -- draft-promote build/ok.colloqdraft
# promoted example.generate as sha256:5e5be47f... to build/promoted.colloqconv.json
#   and build/promoted.colloqplan.json
```

The new contract has a new identity (`sha256:5e5be47f...`, was `sha256:92c198d0...`). Both
endpoints are derived again from the new plan. A client that still runs the old plan is refused
at the preface, so it never receives a `cached` choice it does not understand.

The contract is what you keep. The endpoints are what you throw away and regenerate. What is
not derived yet: the code that decides *when* to answer from the cache is still written by
hand, and the record of who changed the contract and why (lineage, [RFC-0008](../rfcs/0008-evolution-loop.md))
is specified but not built.

## When a schema is enough

If the exchange is one request and one response, a schema already covers it. Colloq earns its
cost when the order of messages carries meaning: streams, cancellation, choices that change what
the other side may send next, and failures that leave the two sides with different knowledge.
