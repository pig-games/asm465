#*******************************************************************************************
# Makefile — asm465
# Per-target builds for MEGA65 and Ultimate64 (64tass)
#*******************************************************************************************

# ---- Platform selection ----------------------------------------------------
TARGET ?= mega65
TARGET := $(shell echo $(TARGET) | tr A-Z a-z)

VALID_TARGETS := mega65 ultimate64 cross465 cross465web c64os
ifeq (,$(filter $(TARGET),$(VALID_TARGETS)))
  $(error Unknown TARGET '$(TARGET)'. Valid targets: $(VALID_TARGETS))
endif

# ---- Network config (for Ultimate64 uploads) --------------------------------
ULTIMATE_IP ?= 192.168.0.214
-include native/config.mk   # optional local overrides (legacy path)
-include config.mk          # optional local overrides (git-ignored)

# ---- Project basics --------------------------------------------------------
NAME    := asm465
ADDR    := 0000

# Tooling defaults
PYTHON ?= python3

# Service configuration for asm465 GUI / bridge
CROSS465_PORT ?= 7465
CROSS465_HOST ?= 127.0.0.1
CROSS465_NATIVE_PORT ?= $(CROSS465_PORT)
CROSS465_NATIVE_HOST ?= $(CROSS465_HOST)
CROSS465_BRIDGE_PORT ?= 7565
CROSS465_BRIDGE_HOST ?= $(CROSS465_HOST)
CROSS465_WS_PORT ?= 8800
CROSS465_WS_HOST ?= 127.0.0.1
CROSS465_BRIDGE_WS_PORT ?= $(CROSS465_WS_PORT)
CROSS465_BRIDGE_WS_HOST ?= $(CROSS465_WS_HOST)
CROSS465_WAIT ?= 2
CROSS465_MAX_CYCLES   ?= 5000000
CROSS465_RETRIES ?= 10
CROSS465_DELAY ?= 1
ASM465_NATIVE_PID_FILE ?= native/build/cross465/asm465_native.pid
ASM465_NATIVE_LOG ?= native/build/cross465/asm465_native.log
ASM465_BRIDGE_PID_FILE ?= native/build/cross465/asm465_bridge.pid
ASM465_BRIDGE_LOG ?= native/build/cross465/asm465_bridge.log
WASM_HTTP_HOST ?= 127.0.0.1
WASM_HTTP_PORT ?= 8000
WASM_HTTP_PID_FILE ?= native/build/wasm_http_server.pid
WASM_HTTP_LOG ?= native/build/wasm_http_server.log

ifeq ($(origin CROSS465_MODE),undefined)
  ifeq ($(TARGET),cross465)
    CROSS465_MODE := native
  endif
  ifeq ($(TARGET),cross465web)
    CROSS465_MODE := bridge
  endif
endif

ifeq ($(TARGET),cross465web)
  ASM465_SEND_HOST ?= $(CROSS465_BRIDGE_HOST)
  ASM465_SEND_PORT ?= $(CROSS465_BRIDGE_PORT)
else ifeq ($(TARGET),cross465)
  ASM465_SEND_HOST ?= $(CROSS465_NATIVE_HOST)
  ASM465_SEND_PORT ?= $(CROSS465_NATIVE_PORT)
endif

define ENSURE_ASM465_SERVICE
        @mode="$(strip $(CROSS465_MODE))"; \
        [ -z "$$mode" ] && mode="native"; \
        needs_native=0; needs_bridge=0; \
        case " $$mode " in *" native "*) needs_native=1;; esac; \
        case " $$mode " in *" bridge "*) needs_bridge=1;; esac; \
        if [ $$needs_native -eq 1 ]; then \
                host="$(CROSS465_NATIVE_HOST)"; \
                port="$(CROSS465_NATIVE_PORT)"; \
                if [ "$$host" = "127.0.0.1" ] || [ "$$host" = "localhost" ] || [ "$$host" = "0.0.0.0" ] || [ "$$host" = "::1" ]; then \
                        if [ ! -f $(ASM465_NATIVE_PID_FILE) ] || ! kill -0 $$(cat $(ASM465_NATIVE_PID_FILE)) 2>/dev/null; then \
                                mkdir -p $$(dirname $(ASM465_NATIVE_PID_FILE)); \
				echo ">> Starting asm465 native UI on $$host:$$port"; \
				nohup cargo run --manifest-path crossdev/asm465/Cargo.toml -- --service-port $$port --service-host $$host --max-cycles $(CROSS465_MAX_CYCLES) >$(ASM465_NATIVE_LOG) 2>&1 & \
                                echo $$! > $(ASM465_NATIVE_PID_FILE); \
                                sleep $(CROSS465_WAIT); \
                        fi; \
                fi; \
                if ! $(PYTHON) native/tools/wait_for_port.py --host "$$host" --port "$$port" --retries $(CROSS465_RETRIES) --delay $(CROSS465_DELAY); then \
                        echo 'ERROR: asm465 native TCP service did not become ready' >&2; \
                        $(MAKE) --no-print-directory asm465-service-stop >/dev/null 2>&1 || true; \
                        exit 1; \
                fi; \
        fi; \
        if [ $$needs_bridge -eq 1 ]; then \
                tcp_host="$(CROSS465_BRIDGE_HOST)"; \
                tcp_port="$(CROSS465_BRIDGE_PORT)"; \
                ws_host="$(CROSS465_BRIDGE_WS_HOST)"; \
                ws_port="$(CROSS465_BRIDGE_WS_PORT)"; \
                if [ "$$tcp_host" = "127.0.0.1" ] || [ "$$tcp_host" = "localhost" ] || [ "$$tcp_host" = "0.0.0.0" ] || [ "$$tcp_host" = "::1" ]; then \
                        if [ ! -f $(ASM465_BRIDGE_PID_FILE) ] || ! kill -0 $$(cat $(ASM465_BRIDGE_PID_FILE)) 2>/dev/null; then \
                                mkdir -p $$(dirname $(ASM465_BRIDGE_PID_FILE)); \
                                echo ">> Starting asm465 bridge on tcp $$tcp_host:$$tcp_port (ws $$ws_host:$$ws_port)"; \
                                nohup cargo run --manifest-path crossdev/asm465-server/Cargo.toml -- --tcp-host $$tcp_host --tcp-port $$tcp_port --ws-host $$ws_host --ws-port $$ws_port >$(ASM465_BRIDGE_LOG) 2>&1 & \
                                echo $$! > $(ASM465_BRIDGE_PID_FILE); \
                                sleep $(CROSS465_WAIT); \
                        fi; \
                fi; \
                if ! $(PYTHON) native/tools/wait_for_port.py --host "$$tcp_host" --port "$$tcp_port" --retries $(CROSS465_RETRIES) --delay $(CROSS465_DELAY); then \
                        echo 'ERROR: asm465 bridge TCP service did not become ready' >&2; \
                        $(MAKE) --no-print-directory asm465-service-stop >/dev/null 2>&1 || true; \
                        exit 1; \
                fi; \
                if ! $(PYTHON) native/tools/wait_for_port.py --host "$$ws_host" --port "$$ws_port" --retries $(CROSS465_RETRIES) --delay $(CROSS465_DELAY); then \
                        echo 'ERROR: asm465 websocket bridge did not become ready' >&2; \
                        $(MAKE) --no-print-directory asm465-service-stop >/dev/null 2>&1 || true; \
                        exit 1; \
                fi; \
        fi
endef

define STOP_ASM465_SERVICE
	if [ -f $(ASM465_NATIVE_PID_FILE) ]; then \
		PID=$$(cat $(ASM465_NATIVE_PID_FILE)); \
		if kill -0 $$PID 2>/dev/null; then \
			echo ">> Stopping asm465 native UI ($$PID)"; \
			kill $$PID >/dev/null 2>&1 || true; \
		fi; \
		rm -f $(ASM465_NATIVE_PID_FILE); \
	fi; \
	if [ -f $(ASM465_BRIDGE_PID_FILE) ]; then \
		PID=$$(cat $(ASM465_BRIDGE_PID_FILE)); \
		if kill -0 $$PID 2>/dev/null; then \
			echo ">> Stopping asm465 bridge ($$PID)"; \
			kill $$PID >/dev/null 2>&1 || true; \
		fi; \
		rm -f $(ASM465_BRIDGE_PID_FILE); \
	fi
endef

define ENSURE_WASM_HTTP_SERVER
        @host="$(WASM_HTTP_HOST)"; \
        port="$(WASM_HTTP_PORT)"; \
        pid_file="$(WASM_HTTP_PID_FILE)"; \
        if [ -f "$$pid_file" ] && kill -0 $$(cat "$$pid_file") 2>/dev/null; then \
                :; \
        else \
                mkdir -p $$(dirname "$$pid_file"); \
                echo ">> Starting wasm HTTP server on $$host:$$port"; \
                cd crossdev/asm465-wasm/web-dist && \
                        nohup python3 -m http.server $$port --bind $$host >$(WASM_HTTP_LOG) 2>&1 & \
                        echo $$! > "$$pid_file"; \
                sleep $(CROSS465_WAIT); \
        fi
endef

define STOP_WASM_HTTP_SERVER
	if [ -f $(WASM_HTTP_PID_FILE) ]; then \
		PID=$$(cat $(WASM_HTTP_PID_FILE)); \
		if kill -0 $$PID 2>/dev/null; then \
			echo ">> Stopping wasm HTTP server ($$PID)"; \
			kill $$PID >/dev/null 2>&1 || true; \
		fi; \
		rm -f $(WASM_HTTP_PID_FILE); \
	fi
endef

# Per-target output dir (watch out for tabs here!)
OUTDIR := native/build/$(TARGET)

# Fail fast if OUTDIR is somehow empty (whitespace/tab issues etc.)
ifeq ($(strip $(OUTDIR)),)
  $(error OUTDIR is empty; expected something like native/build/$(TARGET))
endif

# Create OUTDIR on demand (order-only prereq will trigger this)
$(OUTDIR):
	@mkdir -p $(OUTDIR)

# ---- Include paths for headers/macros -------------------------------------
COMMON_INCLUDE   := native/src/include
TARGET_ALIAS := $(TARGET)
ifeq ($(TARGET),cross465web)
  TARGET_ALIAS := cross465
endif

PLATFORM_INCLUDE := native/src/platform/$(TARGET_ALIAS)/include

# Use --include-dir (portable). These only affect `.include "..."` resolution.
INCLUDE_FLAGS := -I $(COMMON_INCLUDE) -I $(PLATFORM_INCLUDE)

# ---- Assembler options -----------------------------------------------------
OPTS := -C -a -B -q $(INCLUDE_FLAGS)

ifeq ($(TARGET_ALIAS),mega65)
  OPTS += --m45gs02 -D MEGA65:=true
endif
ifeq ($(TARGET_ALIAS),ultimate64)
  OPTS += --m6502 -D ULTIMATE64:=true
endif
ifeq ($(TARGET_ALIAS),cross465)
  OPTS += --m6502 -D CROSS465:=true
endif

# ---- Sources ---------------------------------------------------------------
PLATFORM_SRC := platform/$(TARGET_ALIAS)

# If you add a platform_prelude.s, put it first in CORE_SRC.
CORE_SRC = \
        native/src/include/core.h \
        native/src/$(PLATFORM_SRC)/layout.s \
        native/src/$(PLATFORM_SRC)/boot.s \
        native/src/$(PLATFORM_SRC)/init.s \
        native/src/$(PLATFORM_SRC)/screen.s \
        native/src/screen.s \
        native/src/util.s

# ---- RTST test runner shortcuts --------------------------------------------
RTST_TARGET ?= cross465
RUNNER_ARGS ?=
RUNNER_INCLUDE_TARGETS ?=
RUNNER_EXCLUDE_TARGETS ?=
USE_FIXTURES ?= 
UPDATE_FIXTURES ?=
FIXTURES ?=

ifdef CASE
RUNNER_CASE_ARGS := $(foreach c,$(CASE),--case $(c))
endif

ifdef PERSONALITY
RUNNER_PERSONALITY_ARG := --personality $(PERSONALITY)
endif

CROSS465_TEST_RUNNER = cargo run --quiet --manifest-path crossdev/cross465/Cargo.toml --bin cross465-test-runner --

RUNNER_FIXTURE_ARGS :=
ifeq ($(UPDATE_FIXTURES),true)
RUNNER_FIXTURE_ARGS += --update-fixtures
ifneq ($(strip $(FIXTURES)),)
RUNNER_FIXTURE_ARGS += --fixtures $(FIXTURES)
else
RUNNER_FIXTURE_ARGS += --fixtures
endif
else
ifneq ($(strip $(FIXTURES)),)
RUNNER_FIXTURE_ARGS += --fixtures $(FIXTURES)
else ifeq ($(USE_FIXTURES),true)
RUNNER_FIXTURE_ARGS += --fixtures
endif
endif

WASM_READY_WAIT ?= 20

TEST_DEPS :=
ifeq ($(RTST_TARGET),asm465)
TEST_DEPS += build-asm465 build-asm465-server
endif
ifeq ($(RTST_TARGET),asm465-wasm)
TEST_DEPS += build-asm465 build-asm465-server build-asm465-wasm
endif

all: test

.PHONY: test
test: $(TEST_DEPS)
	@target="$(RTST_TARGET)"; \
	if [ "$$target" = "asm465" ]; then \
		$(MAKE) CROSS465_MODE=native asm465-service-start || exit $$?; \
	elif [ "$$target" = "asm465-wasm" ]; then \
		$(MAKE) CROSS465_MODE=bridge asm465-service-start || exit $$?; \
		$(MAKE) ensure-wasm-http-server || exit $$?; \
		echo ">> Waiting $(WASM_READY_WAIT)s for asm465-wasm viewer to connect at http://$(WASM_HTTP_HOST):$(WASM_HTTP_PORT)/"; \
		sleep $(WASM_READY_WAIT); \
	fi; \
	$(CROSS465_TEST_RUNNER) --mode run --target $(RTST_TARGET) $(RUNNER_PERSONALITY_ARG) $(RUNNER_CASE_ARGS) $(RUNNER_FIXTURE_ARGS) $(RUNNER_ARGS); \
	status=$$?; \
	if [ "$$target" = "asm465" ]; then \
		$(MAKE) asm465-service-stop; \
	fi; \
	if [ "$$target" = "asm465-wasm" ]; then \
		$(MAKE) asm465-service-stop; \
		$(MAKE) wasm-http-server-stop; \
	fi; \
	exit $$status

.PHONY: test-ci
test-ci: build-asm465 build-asm465-server build-asm465-wasm
	@$(MAKE) CROSS465_MODE="native bridge" asm465-service-start || exit $$?; \
	$(MAKE) ensure-wasm-http-server || exit $$?; \
	echo ">> Waiting $(WASM_READY_WAIT)s for asm465-wasm viewer to connect at http://$(WASM_HTTP_HOST):$(WASM_HTTP_PORT)/"; \
	sleep $(WASM_READY_WAIT); \
	$(CROSS465_TEST_RUNNER) --mode run --ci-matrix $(foreach t,$(RUNNER_INCLUDE_TARGETS),--ci-target $(t)) $(foreach t,$(RUNNER_EXCLUDE_TARGETS),--ci-skip-target $(t)) $(RUNNER_FIXTURE_ARGS) $(RUNNER_ARGS); \
	status=$$?; \
	$(MAKE) asm465-service-stop; \
	$(MAKE) wasm-http-server-stop; \
	exit $$status

.PHONY: ci
ci: test-ci

.PHONY: fixtures
fixtures:
	$(MAKE) test-ci USE_FIXTURES=true

.PHONY: update-fixtures
update-fixtures:
	$(MAKE) test-ci USE_FIXTURES=true UPDATE_FIXTURES=true

# ---- Housekeeping ----------------------------------------------------------
clean:
	@rm -rf native/build/*

.PHONY: asm465-service-start asm465-service-stop
asm465-service-start:
	$(ENSURE_ASM465_SERVICE)

asm465-service-stop:
	$(STOP_ASM465_SERVICE)

.PHONY: wasm wasm-clean
wasm:
	@$(MAKE) -C crossdev/asm465-wasm build

wasm-clean:
	@$(MAKE) -C crossdev/asm465-wasm clean

.PHONY: ensure-wasm-http-server wasm-http-server-stop
ensure-wasm-http-server:
	$(ENSURE_WASM_HTTP_SERVER)

wasm-http-server-stop:
	$(STOP_WASM_HTTP_SERVER)

.PHONY: build-asm465 build-asm465-server build-asm465-wasm
build-asm465:
	cargo build --manifest-path crossdev/asm465/Cargo.toml

build-asm465-server:
	cargo build --manifest-path crossdev/asm465-server/Cargo.toml

build-asm465-wasm:
	@$(MAKE) -C crossdev/asm465-wasm build

.PHONY: all clean test test-ci ci fixtures update-fixtures asm465-service-start asm465-service-stop wasm wasm-clean ensure-wasm-http-server wasm-http-server-stop build-asm465 build-asm465-server build-asm465-wasm
