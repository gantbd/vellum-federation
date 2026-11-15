# Native federation roadmap

1. Freeze home-scoped workspace/document/source identities and exact bounded
   protocol, disclosure, event, snapshot, edit, and receipt negotiation.
2. Implemented foundation: verified-home trust composition binds both
   identities, trust epochs, service incarnations, exporter, and selected
   disclosure/profile set. Certificate provenance, identity/exporter
   substitution, encrypted-only downgrade, self-session, and live disclosure
   revocation fail closed with content-safe diagnostics.
3. Implemented replica foundation: authoritative event/snapshot envelopes and
   a disclosure-bound direct-home journal provide exact replay/divergence,
   explicit gaps, bounded snapshots, checkpoint recovery, authenticated-home
   enforcement, and imported-event non-export. The closed content-safe workspace
   fact vocabulary now distinguishes document presence, monotone source-revision
   advancement with a redacted digest, and document absence without carrying
   source bytes or granting edit/save authority. Canonical session evidence now
   binds both homes/epochs/incarnations plus the negotiated disclosure, schemas,
   and limits while deliberately excluding the exporter and any authority.
   Durable replica storage/recovery remains open. Canonical bounded event,
   snapshot, acknowledgement, session-evidence, and fixed disclosure-aware
   hello frames and fixtures are implemented.
4. Remote edit admission foundation implemented: home-scoped actor/document and
   request identities, opaque nonempty edit intent, revision-safe closed reply,
   pre-effect scope/replay/conflict/capacity checks, exact retained receipts,
   recovery validation, and client-side exact uncertainty fencing. The separate
   effect-free edit port exposes reply, definitely-not-executed, and
   outcome-unknown without defining Vellum edit semantics. Canonical bounded
   request/reply frames bind the exact actor, document, expected revision,
   opaque intent, request identity, and closed outcome while refusing unknown,
   zero, truncated, trailing, and oversized values. Concrete Vellum authority/
   persistence composition, transport adapter, and durable receipt publication
   remain open.
5. Compose Vellum authentication/current authority, durable publication,
   bounded transport, disclosure revocation, and irreversible session close.
6. Qualify partitions, reconnect, revocation, lost replies, receipt recovery,
   imported-event non-export, human/agent symmetry, source cleanup obligations,
   and synchronization-acknowledgement meaning.
7. Close package/supply hygiene, canonical fixtures, corruption/model/fuzz/soak,
   supported platforms, reproducible source artifacts, and commit-pinned
   integrated qualification.
