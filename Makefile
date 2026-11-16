.PHONY: check

check:
	./scripts/check-boundaries.sh
	./scripts/check-format.sh
	cargo clippy --workspace --all-targets --locked --offline -- -D warnings
	cargo test --workspace --all-targets --locked --offline
	cargo test --workspace --doc --locked --offline
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
	cargo fmt --manifest-path qualification/transport-session/Cargo.toml -- --check
	cargo clippy --manifest-path qualification/transport-session/Cargo.toml --all-targets --locked --offline -- -D warnings
	cargo test --manifest-path qualification/transport-session/Cargo.toml --locked --offline
	git diff --check
