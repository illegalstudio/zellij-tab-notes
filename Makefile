.PHONY: build test release relese

build:
	cargo build -p tab-notes --release --target wasm32-wasip1

test:
	cargo test -p tab-notes-core

# Propose the next version, confirm, then tag and push to origin.
release:
	@bash scripts/release.sh

relese: release
