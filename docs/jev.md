# Jev as a choice chooser

Status: v0 experiment. One binding file, one choice state, one decision per call.

## The gap it fills

A `choice` state names the role that selects a branch:

```json
{ "kind": "choice", "id": "route", "chooser": "router",
  "branches": { "cached": "reply-cached", "infer": "reply-infer", "review": "reply-review" } }
```

Colloq checks that the chooser emits a declared label (`EndpointMachine::emit_select`), but Colloq
does not decide *which* label. Today that is hand-written code in the chooser. A Jev binding
lets [TypeSafe](https://typesafe.ai)'s Jev System One model make that selection as a typed
Choice judgment.

Colloq keeps the protocol. Jev only proposes one of the declared labels. It never writes to the
wire, never adds a branch, and never changes the conversation identity.

## The binding

A binding is a separate file, so the conversation schema and its content identity do not
change. See [`examples/route.colloqjev.json`](../examples/route.colloqjev.json) and the
[schema](../spec/colloq-jev-v0.schema.json).

```json
{
  "colloq_jev": "0.1.0",
  "conversation": "example.route",
  "state": "route",
  "model": "jev-latest",
  "instructions": "How should the router handle this incoming request?",
  "criteria": {
    "cached": "A common question with a fixed answer ...",
    "infer": "A new question that needs a model to write a specific answer",
    "review": "A request a person must handle ..."
  },
  "threshold": 0.95,
  "escalate": "review"
}
```

## Fail-closed validation

`jev::bind` validates the conversation first, then the binding. Nothing is sent to Jev
unless every check passes.

| Code | Rule |
| --- | --- |
| `CLQJ001` | Unsupported binding format. |
| `CLQJ002` | The binding names a different conversation module. |
| `CLQJ003` | `model` is empty. |
| `CLQJ004` | `threshold` is not within 0..=1. There is no default. |
| `CLQJ005` | Fewer than two criteria. |
| `CLQJ006` | The state is not a `choice`. |
| `CLQJ007` | The state does not exist. |
| `CLQJ008` | A criterion is not a branch of the state. |
| `CLQJ009` | A branch other than `escalate` has no criterion, so Jev could never select it. |
| `CLQJ010` | `escalate` is not a branch of the state. |

## Decision rule

| Jev answer | Label emitted |
| --- | --- |
| `confidence >= threshold` | Jev's `choice` |
| `confidence < threshold` | the `escalate` branch |
| choice outside the criteria, or confidence outside 0..=1 | typed failure `jev.invalid_answer` |
| missing key, transport error, non-2xx response | typed failure `jev.unavailable` |

Colloq never picks a branch silently when Jev fails. The caller decides what a Jev failure means
for the session, for example by observing a declared failure. `429` and `529` responses are
retried twice with backoff, as the TypeSafe API reference asks.

Every decision returns the evidence behind it: `label`, Jev's `choice`, `confidence`,
`threshold`, `escalated`, the full `probabilities`, and the `model`.

## Try it

```bash
# Validate only. No network call.
cargo run -- jev-check examples/route.colloqconv.json examples/route.colloqjev.json

# One live decision. The key stays in the environment and is never logged.
export TYPESAFE_API_KEY=...
cargo run -- jev-decide --state-json '{"text": "I want my money back for last month."}'
```

From Rust, implement or use a `jev::Decider` and pass the decision's `label` to
`emit_select`:

```rust
let bound = colloq::jev::bind(&conversation, binding)?;
let client = colloq::jev::TypeSafeClient::from_env(Duration::from_secs(10))?;
let decision = bound.decide(&client, &state)?;
machine.emit_select(&decision.label)?;
```

## Observed smoke run

Three live calls to `jev-latest` on 2026-09-19 with the example binding (threshold 0.95).
This is a smoke test of the integration, not an accuracy measurement.

| Request | Jev choice | Confidence | Label emitted |
| --- | --- | ---: | --- |
| "What are your opening hours on Saturday?" | `cached` | 0.99 | `cached` |
| "Can you explain how Colloq projects a conversation into endpoint machines?" | `infer` | 0.94 | `review` (escalated) |
| "I was charged twice and I want a refund now or I will contact my lawyer." | `review` | 1.00 | `review` |

The second row is the threshold doing its job: a correct but less certain answer takes the
declared escalation branch instead of advancing on its own.

## What this does not claim

- Typed output guarantees the interface, not the truth of the judgment.
- Confidence is a property of Jev's distribution, not of the workflow. The right threshold
  depends on the workload and the cost of a wrong branch. Measure it on representative data
  before relying on it. With a high threshold and an unfamiliar domain, most traffic will take
  the escalation branch; that is the intended safe behavior, not a success metric.
- The call is a blocking HTTPS request to a hosted service. Its latency is outside Colloq's
  microsecond transition measurements and is not included in any benchmark.

## Deferred

- Declaring Jev bindings inside the conversation (changes the schema and content identity).
- Recording Jev decisions in Colloq semantic traces and evidence reports.
- Jev inside the runtime demos, batching several choice states into one request, and
  a private or self-hosted model endpoint.
