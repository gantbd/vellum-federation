.PHONY: check

check:
	./scripts/check-boundaries.sh
	./scripts/check-format.sh
	cargo clippy --workspace --all-targets --locked --offline -- -D warnings
	cargo test --workspace --all-targets --locked --offline
	cargo test --workspace --doc --locked --offline
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
	git diff --check
