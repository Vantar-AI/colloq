# RFC-0005: Binary Colloq Wire codec

- **Status:** Draft
- **Date:** 2026-09-20
- **Scope:** Wire representation, plan-derived codecs, benchmark methodology

## Summary

Colloq Wire is JSON in both encodings. The reference envelope is self-describing; the
compact envelope replaces the state name with a transition ID but still serialises a JSON
object and still reconstructs owned strings before checking. This RFC proposes a third
encoding derived from the compiled plan: a binary frame whose transition ID, sequence and
payload layout come from the plan's transition table rather than from field names on the
wire.

## Motivation and concrete workloads

The checked-in benchmark on an Apple M4, 500 iterations, three tokens:

| Measurement | Median |
| --- | ---: |
| Reference checked transition | 2.417 µs |
| Compact checked transition | 1.833 µs |
| Baseline JSON transition | 0.250 µs |
| Compact warm exchange | 83.6 µs |
| Hand-written whole exchange | 55.2 µs |

The compact encoding removed 1.32× from the isolated transition and 1.11× from the warm
exchange, leaving 1.51× against the hand-written baseline, short of the 1.25× target in
`docs/benchmark.md`. The remaining cost is representation, not protocol checking: the
compact path still allocates, parses and validates UTF-8 for every field.

Two workloads make this concrete. A token stream sends one small frame per token, so
per-frame cost dominates. A mixture-of-experts router sends many small frames across a
fan-out, where allocation per frame turns into allocator pressure on the hot path.

## Non-goals

This RFC does not choose a general interchange format for Colloq artifacts. Plans,
conversations and evidence stay JSON, because they are read by people and by validators.
It does not promise Colloq will beat a conventional RPC stack. It does not remove the
reference encoding, which stays the debugging and conformance representation.

## Decision and terminology

- **Plan codec.** A codec built from one `PreparedPlan`. Both peers derive it from the
  same plan identity, so neither side sends field names or type tags.
- **Frame.** `[transition_id: varint][sequence: varint][payload_len: varint][payload]`.
- **Payload layout.** Derived from the message type's field order in the conversation.
  Scalars are fixed-width little-endian, strings are length-prefixed UTF-8 slices, and
  optional fields use a leading presence bitmap.
- **Borrowed decode.** The decoder yields slices into the received buffer. Checking a
  transition must not require an owned `String`.

## Static semantics

The compiler already produces a canonical transition table for the compact encoding. This
RFC adds a canonical *payload schedule* per message type: an ordered list of field
descriptors with fixed or variable width. The schedule is part of the plan artifact and
therefore part of the plan identity, so a layout change produces a different plan and a
mismatched peer is refused at the session preface, exactly as today.

## Runtime semantics

Frame zero is unchanged: the session preface stays JSON, because it is exchanged once and
must remain readable when two peers disagree. After acceptance, a `wire: "binary"` session
exchanges binary frames. The state machine, the sequence checks, the typed failures and
the semantic trace are unchanged. A decoded frame produces the same `Frame` value as the
reference path, so `verify_trace` accepts traces from all three encodings.

## Failure and security consequences

- A malformed frame is a typed failure, not a panic. Length prefixes are bounds-checked
  against the remaining buffer before any slice is taken.
- Payload length has an explicit ceiling per message type, derived from the schedule.
- A peer that sends a transition ID outside the table fails the session, as today.
- Binary framing removes the accidental protection of JSON parsing, so the decoder is the
  new trust boundary and needs fuzzing before this leaves Draft.

## Canonical representation

`spec/colloq-plan-v0.schema.json` gains an optional `wire.payload_schedules` object. A plan
without it is a v0 plan and supports the reference and compact encodings only. Nothing in
the conversation schema changes.

## Compatibility and migration

`WireEncoding` gains a third variant. The preface already carries the encoding, and a peer
that does not know `binary` refuses the session before frame zero rather than
misinterpreting bytes. Older plans keep working untouched.

## Alternatives and prior art

- **Protobuf or Cap'n Proto payloads.** Rejected for v0: both reintroduce a second schema
  language, and the field numbering would live outside the conversation identity.
- **CBOR.** Smaller than JSON but still self-describing, so it keeps the per-field cost
  this RFC exists to remove.
- **Keep JSON, optimise the parser.** A real option, and the honest baseline to measure
  against: if a zero-copy JSON reader closes most of the gap, this RFC should be rejected.

## Testable acceptance criteria

1. The binary codec reproduces the same semantic trace as the reference encoding for every
   checked-in trace fixture, including the deliberately invalid ones.
2. `benchmark` reports the binary variant beside the reference, compact and hand-written
   variants, in one report, on one machine, in one run.
3. The whole warm exchange reaches **1.25× or better** against the hand-written baseline.
   A result above 1.25× is published as a failed gate, per RFC-0004, and this RFC stays
   Draft.
4. The isolated checked transition beats the compact transition by at least 1.5×.
5. A fuzz target for the decoder runs 10 million cases with no panic and no out-of-bounds
   read.
6. A plan whose payload schedule differs is refused at the preface, with a test.

## Open questions

- Does the payload schedule belong in the plan, or in a separate codec artifact with its
  own identity?
- Are varints worth it for a protocol whose transition IDs are small and dense, or is a
  fixed `u16` simpler and faster?
- Should the preface negotiate the best mutually supported encoding instead of failing?
