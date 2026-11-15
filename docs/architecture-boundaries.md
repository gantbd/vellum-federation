# Federation placement and dependency boundary

Native Vellum federation is an enclosing composition, not a feature of
`vellum-engine`.

## Ownership

| Layer | Owns | Must not own |
| --- | --- | --- |
| Vellum engine/host | workspaces, membership, documents, source revisions, edit laws, history, saves, persistence, and ordinary authorized commands | peer trust, federation sessions, federation wire values, replica cursors, remote receipts, or transport uncertainty |
| Federation protocol/ports/services | home-scoped peer identity, disclosure negotiation, trust/session evidence, canonical remote edits, replicas, gaps, repair, replay, and remote uncertainty | edit semantics, revision allocation, saving, persistence authority, membership, or semantic interpretation |
| Federation-to-Vellum adapter | translation from a verified federation request into the ordinary current Vellum host API and translation of its result back into a federation receipt | bypass authority, allocate revisions, save directly, or reinterpret document state |
| Transport/storage adapters | bounded byte movement and durable checkpoint mechanisms | Vellum or federation authority |

The allowed dependency direction is:

```text
federation protocol <- federation ports/services <- federation adapter -> Vellum host API -> Vellum engine
```

`vellum-engine` must never depend on a `vellum-federation-*` package. Core
protocol, port, and service packages in this repository must never depend on
`vellum-engine`; only a specifically classified
`adapters/vellum-federation-vellum` composition may do so.

Vellum remains the writing engine. Federation may carry scoped identities,
opaque edit intent, authorized projections, and exact outcomes; it does not
become a second document model, persistence owner, semantic engine, agent
harness, or task orchestrator.

## Enforcement

`scripts/check-boundaries.sh` rejects an engine dependency anywhere except the
named external composition adapter, rejects foreign engine semantics, and pins
the separation statements above. Integrated qualification must additionally
prove that the adapter calls the ordinary current-authority Host path.
