.PHONY: build test fmt fmt-check ci watch watch-test bench check clean run release lint

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

check:
	cargo check

lint:
	cargo clippy

bench:
	cargo bench

clean:
	cargo clean

run:
	cargo run

release:
	cargo build --release

ci: fmt-check lint test build

watch:
	watchexec -e rs -c -r -- cargo run

watch-test:
	watchexec -e rs -c -r -- cargo test
