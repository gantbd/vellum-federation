# Federation identity namespaces and SPIFFE boundary

## Canonical application namespace

Vellum federation identities are interpreted under
`gantbd.vellum.federation/v1`. Protocol types and canonical frame kinds carry
that namespace inside the typed federation boundary; generic stores, policy
inputs, registries, and telemetry must preserve the explicit namespace label.

An identity is never a bare byte string:

- a home is `(gantbd.vellum.federation/v1, VellumHomeIdV1)`;
- a workspace is `(namespace, workspace, home, HomeLocalWorkspaceIdV1)`;
- a document adds its distinct document kind and local document ID to the
  complete workspace identity;
- an actor retains its home and distinct actor kind;
- edit request IDs are replay keys scoped by authenticated sender, actor,
  document, expected source revision, and operation profile.

Equal local bytes in different homes, kinds, engine namespaces, workspaces, or
request scopes do not identify the same thing. Source revisions are monotone
state coordinates under a document identity, not standalone global IDs.

## SPIFFE

SPIFFE can authenticate a Vellum service workload and distribute trust-domain
bundles across administrative boundaries. It does not identify workspaces,
documents, source revisions, authors, humans, agents, or edit authority.

A future SPIFFE trust adapter belongs under `adapters/` and may produce a
verified home only after validating the SVID with the exact bundle for its
trust domain and applying an explicit configured mapping:

```text
(full SPIFFE ID, trust-domain bundle, application namespace)
    -> (VellumHomeIdV1, current trust epoch, allowed disclosure/peer profile)
```

The complete SPIFFE ID participates in the mapping; a path alone is
insufficient. Trust-domain names are operator-selected and not guaranteed
globally unique, so the configured trust-domain-to-bundle association must be
preserved. SVID rotation must not rotate the stable Vellum home. Workload
authentication remains provenance; the ordinary Vellum Host rechecks current
workspace, actor, disclosure, edit, and persistence authority.

Vellum remains the writing engine. Humans and agents retain Vellum-owned actor
identity and use the same edit laws; neither becomes a SPIFFE workload merely
by participating in a document.

References:

- <https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE-ID.md>
- <https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE_Trust_Domain_and_Bundle.md>
- <https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE_Federation.md>
