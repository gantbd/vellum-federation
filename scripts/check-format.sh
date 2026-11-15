#!/usr/bin/env bash
set -euo pipefail

while IFS= read -r package; do
  cargo fmt --package "${package}" -- --check
done < <(
  rg --no-filename '^name = "vellum-federation-' adapters ports protocol service -g Cargo.toml \
    | sed -E 's/^name = "([^"]+)"/\1/' \
    | sort
)
