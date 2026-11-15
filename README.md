# Vellum Federation

`vellum-federation` is the separately owned native federation composition for
Vellum. The initial profile preserves one authoritative workspace home and
read-only remote replicas. Remote edits return to ordinary current Vellum
authority; replicas cannot allocate revisions, save, persist, rebind, or mutate
membership locally.

The protocol package closes home-scoped workspace/document/source identity and
exact bounded negotiation. Disclosure is negotiated independently: metadata-
only support cannot silently become source-bearing replication. Injected trust
composition additionally requires exact configured homes and trust epochs,
certificate-verified provenance, one exporter, and current authorization for the
selected disclosure profile before binding both service incarnations.

The replica package adds a bounded, disclosure-bound direct-home journal for
authoritative event envelopes and snapshots. It detects gaps and divergence,
supports exact replay and checkpoint recovery, and refuses both non-home input
and authoritative re-export. A replica acknowledgement means only that the
named sequence has been incorporated into that replica; it is not evidence of
read, edit, save, persistence, review, attention, or semantic freshness.

See [`docs/discipline.md`](docs/discipline.md) and
[`docs/roadmap.md`](docs/roadmap.md). Run `make check` for the local gate.
