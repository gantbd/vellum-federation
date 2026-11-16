# Contributing

Preserve the boundary described in `docs/architecture-boundaries.md`: Vellum
owns document truth and ordinary authorized commands, while this repository
owns federation protocol, sessions, replication, repair, and uncertainty.

Check out Vellum as the sibling directory `../vellum` at revision
`418f4eeb5522309d9245b6d455085bac5cd1f8cd`, then run:

```sh
make check
```

Keep identities and bounds explicit, failures content-safe, and authority
narrow. This workspace is not published to crates.io.
