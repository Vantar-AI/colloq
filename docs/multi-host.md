# Multi-host execution

Status: plan. Nothing here is implemented yet.

Every Colloq result so far comes from one machine. Two processes on one laptop share a
clock, a scheduler and a loopback interface that never reorders, never drops and never
partitions. That is enough to prove projection and identity. It is not enough to claim
anything about behaviour on a network.

This page is the plan for the first results that come from real hosts, and the gates those
results must pass. It follows [the evidence protocol](../rfcs/0004-evidence-protocol.md):
the workload, the baseline and the acceptance gate are fixed before the run.

## What must be true before a multi-host claim

1. The same compiled plan runs unchanged on both hosts. No host-specific build.
2. Both hosts report the same semantic trace, checked with `verify_trace`, not by eye.
3. Every failure observed is a declared Colloq failure, not a panic or a hang.
4. The report names the hosts, the distance, the measured baseline RTT and the exact
   revision, per RFC-0004.

## Testbed

Three configurations, in order of difficulty.

| Configuration | Purpose |
| --- | --- |
| Two containers, one host, with a netem qdisc | Deterministic latency, loss and reorder, repeatable in CI |
| Two hosts in one region | Real NIC, real kernel, sub-millisecond RTT |
| Two hosts across regions | Tens of milliseconds RTT, real jitter, real path changes |

The first configuration is the one that belongs in CI. The other two run on demand and
publish an evidence artifact each time.

## Fault matrix

Each transport runs the same matrix: no fault, 1% loss, 5% loss, 50 ms added latency,
reorder, one-way partition during a stream, peer process killed mid-conversation, and
peer host powered off mid-conversation. For each cell the report records which declared
failure fired, whether both sides agree on how far the conversation progressed, and how
long the observation took.

`transport.uncertain` is the interesting cell: it exists precisely because a peer cannot
always know how far the other side got. The matrix should show it firing where the theory
says it must, and not firing where it must not.

## Work items

- [ ] A `colloq bench-host` subcommand: run one side of a plan against a remote peer,
      emit a timing and trace report as JSON.
- [ ] A netem harness with a fixed profile per fault cell.
- [ ] A conventional baseline with the same workload over the same paths: one gRPC
      client and server, and one hand-written TCP pair.
- [ ] An evidence artifact per configuration, validated against
      `spec/colloq-evidence-v0.schema.json`.
- [ ] A CI job for the container configuration only, so the deterministic part stays
      honest without turning a shared runner into a laboratory.
- [ ] `docs/benchmark.md` gains a network section, separate from the in-process numbers,
      so the two are never quoted as one.

## Acceptance gates

1. Trace equivalence across hosts holds for every fault cell that is expected to complete.
2. Added Colloq overhead over the same path, against the hand-written TCP pair, is stated
   with a median and a p95, and is not hidden inside the network time.
3. Every abnormal cell terminates through a declared failure within its deadline. A hang
   is a failed gate.
4. The result reproduces on a second day on the same hardware within the noise band the
   protocol declares in advance.

## Open questions

- Does the plan-bound session preface need a resumption path before cross-region runs are
  meaningful, or is a fresh session per conversation acceptable at that RTT?
- Which of the three configurations, if any, produces a number worth publishing on the
  website, and which stays in the repository as evidence only?

## Which claim this advances

**Regeneration evaluated against stability** moves: every current number comes from one machine, and a claim about distributed software needs results from real hosts under real faults. See [the goal](../docs/regeneration.md) for the full scoreboard. The core does not
change: this work exists to make the conversation and its identity stronger, not to add a
second direction.
