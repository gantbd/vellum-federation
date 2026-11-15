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
Canonical replica frames bind the exact workspace, disclosure profile, sequence,
and bounded payload with fixed tags and big-endian fields. Decoders reject
unknown disclosure values, cross-type frames, zero identities/sequences,
oversized payloads, truncation, and trailing bytes.
The event payload vocabulary is also closed: document presence, monotone source
revision advancement with a content digest, and document absence are canonical
content-safe facts. It carries no source bytes and confers no edit, save,
persistence, membership, or retention authority.
Canonical session evidence records both verified homes, trust epochs, service
incarnations, and the negotiated disclosure/schema/limit set. It deliberately
omits the transport exporter and cannot replace current trust revalidation or
grant read, edit, save, persistence, or retention authority.
Remote edit admission uses a separately scoped actor, document, request ID,
expected source revision, and opaque intent. The federation service preflights
exact replay/conflict/capacity before effects and retains the original reply;
its client preserves only the equal uncertain request. An effect-free port keeps
reply, definitely-not-executed, and outcome-unknown distinct. The concrete
adapter must still invoke ordinary current Vellum authority and persistence;
federation never interprets or executes the edit itself.
The fixed post-trust hello additionally binds home, trust epoch, service
incarnation, disclosure and schema ranges, and positive operational ceilings.
Negotiating source disclosure remains a capability selection, never edit,
save, persistence, or workspace authority.

See [`docs/discipline.md`](docs/discipline.md) and
[`docs/roadmap.md`](docs/roadmap.md). Run `make check` for the local gate.
