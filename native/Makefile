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
ULTIMATE_IP ?= 192.168.0.194
-include config.mk   # optional local overrides (git-ignored)

# ---- Project basics --------------------------------------------------------
NAME    := asm465
ADDR    := 0000

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
OPTS := -C -a -B $(INCLUDE_FLAGS) -q

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
		-o $(OUTDIR)/$@ \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

screentest: $(STEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(STEST_SRC) \
		-o $(OUTDIR)/$@ \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

debugtest: $(DTEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(DTEST_SRC) \
		-o $(OUTDIR)/$@ \
		--list $(OUTDIR)/$@.lst \
		--labels=$(OUTDIR)/$@.lbl

unittest: $(UTEST_SRC) $(CORE_SRC) | $(OUTDIR)
	64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(UTEST_SRC) \
		-o $(OUTDIR)/$@ \
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
	cargo run --manifest-path ../crossdev/cross465/runner/Cargo.toml -- $(OUTDIR)/$*
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
	@set -e; \
	for P in $(PROGRAMS); do \
		for T in $(BUILD_TARGETS); do \
			$(MAKE) --no-print-directory TARGET=$$T $$P; \
			$(MAKE) --no-print-directory TARGET=$$T run_$$P; \
		done; \
		echo; \
		if [ -z "$$NOPAUSE" ]; then \
			read -p "Press [Enter] to continue..." dummy; \
		fi; \
	done

ci: matrix_build matrix_run

.PHONY: all clean run_% run_ptest run_stest run_dtest run_utest matrix_build matrix_run ci