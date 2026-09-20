# Security

Colloq is a research prototype. It is not hardened, it has had no external audit, and it
should not hold anything you cannot afford to lose. That said, a report is welcome and
will be answered.

## Reporting

Email **hello@vantar.xyz** with "colloq security" in the subject, or open a
[private advisory](https://github.com/Vantar-AI/colloq/security/advisories/new).

Please do not open a public issue for a vulnerability that has not been fixed yet.

Include what you have: the revision, the transport, a reproduction if you have one, and
what you think the impact is. A report with a broken reproduction is still worth sending.

## What to expect

| Step | Timing |
| --- | --- |
| Acknowledgement | Within 3 working days |
| First assessment | Within 10 working days |
| Fix or public statement of the risk | As fast as the fix allows, and the advisory says so if it is slow |

There is no bounty programme. Credit is given in the advisory unless you ask otherwise.

## Scope

In scope: the checker, the plan compiler, the wire encodings, the session preface, the
identity and authorization artifacts, the draft gate, and anything that lets a peer take a
role it was not granted or advance a conversation it should not.

Out of scope: denial of service by an authorised peer, the marketing site, and anything
that requires local write access to a node identity file, because that file is the secret.

## Known weak points

These are already public, so reporting them is unnecessary:

- The compact and reference wire encodings are JSON, and the decoder has not been fuzzed.
- TCP is plaintext and unauthenticated; it exists as a correctness instrument only.
- The standalone QUIC adapter authenticates the server, not the client.
- Identities cannot be rotated or revoked yet. See RFC-0006.
