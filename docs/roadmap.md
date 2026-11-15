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
   enforcement, and imported-event non-export. Still open: freeze the closed
   workspace event vocabulary, session-evidence wire encoding, and durable
   replica storage/recovery. Canonical bounded event, snapshot, acknowledgement,
   and fixed disclosure-aware hello frames and fixtures are implemented.
4. Add remote edit admission and persistence reconciliation ports with scoped
   receipts and three-way execution certainty.
5. Compose Vellum authentication/current authority, durable publication,
   bounded transport, disclosure revocation, and irreversible session close.
6. Qualify partitions, reconnect, revocation, lost replies, receipt recovery,
   imported-event non-export, human/agent symmetry, source cleanup obligations,
   and synchronization-acknowledgement meaning.
7. Close package/supply hygiene, canonical fixtures, corruption/model/fuzz/soak,
   supported platforms, reproducible source artifacts, and commit-pinned
   integrated qualification.
