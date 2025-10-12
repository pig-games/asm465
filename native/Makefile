#*******************************************************************************************
# Makefile — asm465
# Per-target builds for MEGA65 and Ultimate64 (64tass)
#*******************************************************************************************

# ---- Platform selection ----------------------------------------------------
TARGET ?= mega65
TARGET := $(shell echo $(TARGET) | tr A-Z a-z)

VALID_TARGETS := mega65 ultimate64 cross465
ifeq (,$(filter $(TARGET),$(VALID_TARGETS)))
  $(error Unknown TARGET '$(TARGET)'. Valid targets: $(VALID_TARGETS))
endif

# ---- Network config (for Ultimate64 uploads) --------------------------------
ULTIMATE_IP ?= 192.168.0.188
-include config.mk   # optional local overrides (git-ignored)

# ---- Project basics --------------------------------------------------------
NAME    := asm465
ADDR    := 0000

# Tooling defaults
PYTHON ?= python3

# Service configuration for asm465 GUI integration
CROSS465_PORT ?= 7465
CROSS465_WAIT ?= 2
CROSS465_MAX_CYCLES   ?= 5000000
CROSS465_HOST ?= 127.0.0.1
CROSS465_WS_HOST ?= 127.0.0.1
CROSS465_WS_PORT ?= 8800
CROSS465_RETRIES ?= 10
CROSS465_DELAY ?= 1
ASM465_SERVICE_PID_FILE ?= build/cross465/asm465_service.pid
ASM465_SERVICE_LOG ?= build/cross465/asm465_service.log

define ENSURE_ASM465_SERVICE
	@if [ "$(CROSS465_HOST)" = "127.0.0.1" ] || [ "$(CROSS465_HOST)" = "localhost" ] || [ "$(CROSS465_HOST)" = "0.0.0.0" ] || [ "$(CROSS465_HOST)" = "::1" ]; then \
		if [ ! -f $(ASM465_SERVICE_PID_FILE) ] || ! kill -0 $$(cat $(ASM465_SERVICE_PID_FILE)) 2>/dev/null; then \
			mkdir -p $$(dirname $(ASM465_SERVICE_PID_FILE)); \
			echo ">> Starting asm465 bridge on tcp $(CROSS465_HOST):$(CROSS465_PORT) (ws $(CROSS465_WS_HOST):$(CROSS465_WS_PORT))"; \
			nohup cargo run --manifest-path ../crossdev/asm465-server/Cargo.toml -- --tcp-host $(CROSS465_HOST) --tcp-port $(CROSS465_PORT) --ws-host $(CROSS465_WS_HOST) --ws-port $(CROSS465_WS_PORT) >$(ASM465_SERVICE_LOG) 2>&1 & \
			echo $$! > $(ASM465_SERVICE_PID_FILE); \
		fi; \
	fi; \
	if ! $(PYTHON) tools/wait_for_port.py --host "$(CROSS465_HOST)" --port "$(CROSS465_PORT)" --retries $(CROSS465_RETRIES) --delay $(CROSS465_DELAY); then \
		echo 'ERROR: asm465 service did not become ready' >&2; \
		if [ "$(CROSS465_HOST)" = "127.0.0.1" ] || [ "$(CROSS465_HOST)" = "localhost" ] || [ "$(CROSS465_HOST)" = "0.0.0.0" ] || [ "$(CROSS465_HOST)" = "::1" ]; then \
			if [ -f $(ASM465_SERVICE_PID_FILE) ]; then \
				PID=$$(cat $(ASM465_SERVICE_PID_FILE)); \
				if kill -0 $$PID 2>/dev/null; then kill $$PID >/dev/null 2>&1 || true; fi; \
				rm -f $(ASM465_SERVICE_PID_FILE); \
			fi; \
		fi; \
		exit 1; \
	fi
endef

define STOP_ASM465_SERVICE
	@if [ "$(CROSS465_HOST)" = "127.0.0.1" ] || [ "$(CROSS465_HOST)" = "localhost" ] || [ "$(CROSS465_HOST)" = "0.0.0.0" ] || [ "$(CROSS465_HOST)" = "::1" ]; then \
		if [ -f $(ASM465_SERVICE_PID_FILE) ]; then \
			PID=$$(cat $(ASM465_SERVICE_PID_FILE)); \
			if kill -0 $$PID 2>/dev/null; then \
				echo ">> Stopping asm465 service ($$PID)"; \
				kill $$PID >/dev/null 2>&1 || true; \
			fi; \
			rm -f $(ASM465_SERVICE_PID_FILE); \
		fi; \
	fi
endef

# Per-target output dir (watch out for tabs here!)
OUTDIR := build/$(TARGET)

# Fail fast if OUTDIR is somehow empty (whitespace/tab issues etc.)
ifeq ($(strip $(OUTDIR)),)
  $(error OUTDIR is empty; expected something like build/$(TARGET))
endif

# Create OUTDIR on demand (order-only prereq will trigger this)
$(OUTDIR):
	@mkdir -p $(OUTDIR)

# ---- Include paths for headers/macros -------------------------------------
COMMON_INCLUDE   := src/include
PLATFORM_INCLUDE := src/platform/$(TARGET)/include

# Use --include-dir (portable). These only affect `.include "..."` resolution.
INCLUDE_FLAGS := -I $(COMMON_INCLUDE) -I $(PLATFORM_INCLUDE)

# ---- Assembler options -----------------------------------------------------
OPTS := -C -a -B -q $(INCLUDE_FLAGS)

ifeq ($(TARGET),mega65)
  OPTS += --m45gs02 -D MEGA65:=true
endif
ifeq ($(TARGET),ultimate64)
  OPTS += --m6502 -D ULTIMATE64:=true
endif
ifeq ($(TARGET),cross465)
  OPTS += --m6502 -D CROSS465:=true
endif

# ---- Sources ---------------------------------------------------------------
PLATFORM_SRC := platform/$(TARGET)

# If you add a platform_prelude.s, put it first in CORE_SRC.
CORE_SRC = \
	src/include/core.h \
	src/$(PLATFORM_SRC)/layout.s \
	src/$(PLATFORM_SRC)/boot.s \
	src/$(PLATFORM_SRC)/init.s \
	src/$(PLATFORM_SRC)/screen.s \
	src/screen.s \
	src/util.s

PTEST_SRC = src/tests/parsertest.s src/parser.s
STEST_SRC = src/tests/screentest.s
DTEST_SRC = src/tests/debugtest.s
UTEST_SRC = src/unittest.s src/tests/unittest.s

# ---- Build targets ---------------------------------------------------------
parsertest: $(PTEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(PTEST_SRC) \
		-o $(OUTDIR)/$@$(if $(filter cross465,$(TARGET)),.prg) \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

screentest: $(STEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(STEST_SRC) \
		-o $(OUTDIR)/$@$(if $(filter cross465,$(TARGET)),.prg) \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

debugtest: $(DTEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(DTEST_SRC) \
		-o $(OUTDIR)/$@$(if $(filter cross465,$(TARGET)),.prg) \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

unittest: $(UTEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(UTEST_SRC) \
		-o $(OUTDIR)/$@$(if $(filter cross465,$(TARGET)),.prg) \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

all: parsertest screentest debugtest unittest

# ---- Run / Upload (kept separate) -----------------------------------------
run_%: %
ifeq ($(TARGET),mega65)
	etherload.osx --quiet -r $(OUTDIR)/$*
else ifeq ($(TARGET),ultimate64)
	curl --http1.1 -s -X POST http://$(ULTIMATE_IP)/v1/runners:run_prg \
		-H "Content-Type: application/octet-stream" \
		-H "Expect:" \
		-H "Content-Length: $(shell stat -f%z $(OUTDIR)/$*)" \
		--data-binary @$(OUTDIR)/$*
else ifeq ($(TARGET),cross465)
	$(ENSURE_ASM465_SERVICE)
	$(PYTHON) tools/send_prg.py "$(OUTDIR)/$*.prg" "$(CROSS465_PORT)" "$(CROSS465_MAX_CYCLES)" --host "$(CROSS465_HOST)" --embed --name "$*"
endif

# Convenience wrappers (build then run)
run_ptest:   parsertest   run_parsertest
run_stest:   screentest   run_screentest
run_dtest:   debugtest    run_debugtest
run_utest:   unittest     run_unittest

# ---- Housekeeping ----------------------------------------------------------
clean:
	@rm -rf build/*

# ===== Build/Run Matrix ======================================================
BUILD_TARGETS ?= $(VALID_TARGETS)
PROGRAMS ?= parsertest screentest debugtest

.PHONY: matrix_build
matrix_build:
	@echo ">> Building [$(PROGRAMS)] for targets: $(BUILD_TARGETS)"
	@set -e; \
	for P in $(PROGRAMS); do \
		for T in $(BUILD_TARGETS); do \
			$(MAKE) --no-print-directory TARGET=$$T $$P; \
		done; \
	done

.PHONY: matrix_run
matrix_run:
	@echo ">> Building+Running [$(PROGRAMS)] for targets: $(BUILD_TARGETS)"
ifneq (,$(filter cross465,$(BUILD_TARGETS)))
	$(ENSURE_ASM465_SERVICE)
endif
	@set -e; \
	for P in $(PROGRAMS); do \
		for T in $(BUILD_TARGETS); do \
			$(MAKE) --no-print-directory TARGET=$$T run_$$P; \
		done; \
		echo; \
		if [ -z "$$NOPAUSE" ]; then \
			read -p "Press [Enter] to continue..." dummy; \
		fi; \
	done

ci: matrix_build matrix_run

.PHONY: asm465-service-start asm465-service-stop
asm465-service-start:
	$(ENSURE_ASM465_SERVICE)

asm465-service-stop:
	$(STOP_ASM465_SERVICE)

.PHONY: wasm wasm-clean
wasm:
	@$(MAKE) -C ../crossdev/asm465-wasm build

wasm-clean:
	@$(MAKE) -C ../crossdev/asm465-wasm clean

.PHONY: all clean run_% run_ptest run_stest run_dtest run_utest matrix_build matrix_run ci asm465-service-start asm465-service-stop wasm wasm-clean
