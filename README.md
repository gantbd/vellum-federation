# Vellum Federation

`vellum-federation` is the separately owned native federation composition for
Vellum. The initial profile preserves one authoritative workspace home and
read-only remote replicas. Remote edits return to ordinary current Vellum
authority; replicas cannot allocate revisions, save, persist, rebind, or mutate
membership locally.

The first protocol package closes home-scoped workspace/document/source identity
and exact bounded negotiation. Disclosure is negotiated independently:
metadata-only support cannot silently become source-bearing replication.

See [`docs/discipline.md`](docs/discipline.md) and
[`docs/roadmap.md`](docs/roadmap.md). Run `make check` for the local gate.
