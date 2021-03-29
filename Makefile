check:
	cargo check --all --features "roa/full"
build: 
	cargo build --all --features "roa/full"
test: 
	cargo test --all --features "roa/full"
fmt:
	cargo +nightly fmt
check-all:
	cargo +nightly check --all --all-features
test-all:
	cargo +nightly test --all --all-features
