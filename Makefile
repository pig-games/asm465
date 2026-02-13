#*******************************************************************************************
# Makefile — opFoundry
# Cross-development GUI IDE for 65xx retro computers
#*******************************************************************************************

.PHONY: all build build-gui build-server build-wasm clean

all: build

build: build-gui build-server

build-gui:
	cargo build --manifest-path opfoundry-gui/Cargo.toml

build-server:
	cargo build --manifest-path opfoundry-server/Cargo.toml

build-wasm:
	$(MAKE) -C opfoundry-wasm build

clean:
	cargo clean --manifest-path opfoundry-gui/Cargo.toml
	cargo clean --manifest-path opfoundry-server/Cargo.toml
	$(MAKE) -C opfoundry-wasm clean

# ---- Run targets -----------------------------------------------------------
.PHONY: run run-server

OPFOUNDRY_PORT ?= 7465
OPFOUNDRY_WS_PORT ?= 8800

run:
	cargo run --manifest-path opfoundry-gui/Cargo.toml -- --service-port $(OPFOUNDRY_PORT)

run-server:
	cargo run --manifest-path opfoundry-server/Cargo.toml -- \
		--tcp-port $(OPFOUNDRY_PORT) --ws-port $(OPFOUNDRY_WS_PORT)
