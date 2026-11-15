#!/usr/bin/env bash
set -euo pipefail

test "$(rg -l '^name = "vellum-federation-' adapters ports protocol service -g Cargo.toml | wc -l | tr -d ' ')" = 12

if rg -q 'vellum-engine|vellum_engine' ports protocol service -g Cargo.toml -g '*.rs'; then
  echo 'Vellum federation protocol, ports, and services must not import the engine' >&2
  exit 1
fi

engine_importers="$(rg -l 'vellum-engine|vellum_engine' adapters -g Cargo.toml -g '*.rs' || true)"
if test -n "${engine_importers}" && test "${engine_importers}" != 'adapters/vellum-federation-vellum/Cargo.toml'; then
  echo 'Only the classified Vellum composition adapter may import vellum-engine' >&2
  printf '%s\n' "${engine_importers}" >&2
  exit 1
fi

if rg -q 'agora-engine|agora_engine|chronos-engine|chronos_engine|RoomKey|TaskAttempt|lease token' adapters ports protocol service -g Cargo.toml -g '*.rs'; then
  echo 'Foreign engine semantics entered Vellum federation' >&2
  exit 1
fi

rg -q 'Native Vellum federation is an enclosing composition, not a feature of' docs/architecture-boundaries.md
rg -q '`vellum-engine` must never depend on a `vellum-federation-\*` package' docs/architecture-boundaries.md
rg -q 'only a specifically classified' docs/architecture-boundaries.md
rg -q 'ordinary current Vellum host API' docs/architecture-boundaries.md
rg -q 'Vellum remains the writing engine' docs/architecture-boundaries.md
rg -Fq 'redacted_nonzero_id!(VellumHomeIdV1, 32)' protocol/vellum-federation-protocol/src/lib.rs
rg -q '^pub struct FederatedWorkspaceRefV1' protocol/vellum-federation-protocol/src/lib.rs
rg -q '^pub struct FederatedDocumentRefV1' protocol/vellum-federation-protocol/src/lib.rs
rg -q '^pub struct FederatedActorRefV1' protocol/vellum-federation-protocol/src/lib.rs
rg -q 'gantbd\.vellum\.federation/v1' docs/identity-namespaces.md
rg -q 'complete SPIFFE ID participates in the mapping' docs/identity-namespaces.md
rg -q 'neither becomes a SPIFFE workload' docs/identity-namespaces.md

if rg -q 'spiffe://|SPIFFE|SVID' protocol service ports -g '*.rs'; then
  echo 'SPIFFE workload provenance must enter through a trust adapter, not federation protocol or services' >&2
  exit 1
fi

echo 'Vellum federation remains an external composition with one adapter-only engine seam'
