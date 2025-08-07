#*******************************************************************************************
# Makefile
#
# Created by: Erik van der Tier
# on: 2025-05-31
#*******************************************************************************************
TARGET ?= Mega65
# Force lowercase TARGET just in case
TARGET := $(shell echo $(TARGET) | tr A-Z a-z)

# Validate target
VALID_TARGETS := mega65 u64
ifeq (,$(filter $(TARGET),$(VALID_TARGETS)))
$(error Unknown TARGET '$(TARGET)'. Valid targets: $(VALID_TARGETS))
endif

ULTIMATE_IP ?= 192.168.0.188

NAME = 	asm465
ADDR = 	0000
OUTDIR = build

PLATFORM_INCLUDE = src/platform/$(TARGET)/include
COMMON_INCLUDE = src/include
INCLUDE_FLAGS = -I $(COMMON_INCLUDE) -I $(PLATFORM_INCLUDE)

BLD_NAME = $(OUTDIR)/$(NAME).prg

PLATFORM_SRC = platform/$(TARGET)

CORE_SRC = \
		src/$(PLATFORM_SRC)/layout.s \
		src/$(PLATFORM_SRC)/boot.s \
		src/$(PLATFORM_SRC)/init.s \
		src/$(PLATFORM_SRC)/screen.s \
		src/util.s \

PTEST_SRC = \
		src/tests/parsertest.s \
		src/parser.s 

STEST_SRC = \
		src/tests/screentest.s

DTEST_SRC = \
		src/tests/debugtest.s

UTEST_SRC = \
		src/unittest.s \
		src/tests/unittest.s

OPTS = -C -a -B $(INCLUDE_FLAGS)

# Platform-specific options
ifeq ($(TARGET),mega65)
	OPTS += --m45gs02 -D MEGA65:=true
	UPLOAD = etherload.osx -r
endif

ifeq ($(TARGET),run_unittest64)
	OPTS += --m64 -D U64:=true
	UPLOAD = curl --http1.1 -s -X POST http://$(ULTIMATE_IP)/v1/runners:run_prg \
		-H "Content-Type: application/octet-stream" \
		-H "Expect:" \
		-H "Content-Length: $$(stat -f%z $(OUTDIR)/$$*)" \
		--data-binary @$(OUTDIR)/$$*
endif

run_%: %
ifeq ($(TARGET),mega65)
	etherload.osx -r $(OUTDIR)/$*
else ifeq ($(TARGET),u64)
	curl --http1.1 -s -X POST http://$(ULTIMATE_IP)/v1/runners:run_prg \
		-H "Content-Type: application/octet-stream" \
		-H "Expect:" \
		-H "Content-Length: $(shell stat -f%z $(OUTDIR)/$*)" \
		--data-binary @$(OUTDIR)/$*
endif

parsertest: $(PTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(PTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

screentest: $(STEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(STEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

debugtest: $(DTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(DTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

unittest: $(UTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(UTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

all: parsertest screentest debugtest unittest

run_ptest: parsertest run_parsertest

run_stest: screentest run_screentest

run_dtest: debugtest run_debugtest

run_utest: unittest run_unittest

clean:
		rm -r $(OUTDIR)/*

.PHONY: run_ptest run_stest all clean