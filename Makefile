export PATH := $(HOME)/.cargo/bin:$(PATH)

BIND ?= 127.0.0.1:8080
BASE ?= http://localhost:8080
DB ?= ./data/shortener.db

.PHONY: dev run test fmt lint release clean

## dev: run locally with a throwaway key (Ctrl-C to stop)
dev:
	@SHORTENER_API_KEYS=$${SHORTENER_API_KEYS:-$$(openssl rand -hex 24)} \
	 SHORTENER_BIND=$(BIND) SHORTENER_PUBLIC_BASE_URL=$(BASE) SHORTENER_DB=$(DB) \
	 RUST_LOG=shortener=info,tower_http=info,warn \
	 cargo run

## run: run the release binary
run:
	SHORTENER_BIND=$(BIND) SHORTENER_PUBLIC_BASE_URL=$(BASE) SHORTENER_DB=$(DB) \
	 ./target/release/shortener

test:
	cargo test

fmt:
	cargo fmt

lint:
	cargo clippy --all-targets -- -D warnings

release:
	cargo build --release

clean:
	cargo clean && rm -rf data
