WASM_TARGET := wasm32-unknown-unknown
WASM_OUT := web-dist
WASM_BIN := target/$(WASM_TARGET)/release/asm465_wasm.wasm
CARGO_FLAGS := --manifest-path Cargo.toml --target $(WASM_TARGET) --release --no-default-features

.PHONY: build
build:
	cargo build $(CARGO_FLAGS)
	wasm-bindgen --out-dir $(WASM_OUT) --target web $(WASM_BIN)

.PHONY: build-dev
build-dev:
	cargo build $(CARGO_FLAGS) --features dev-loopback
	wasm-bindgen --out-dir $(WASM_OUT) --target web $(WASM_BIN)

.PHONY: clean
clean:
	rm -rf $(WASM_OUT)
	cargo clean --manifest-path Cargo.toml
