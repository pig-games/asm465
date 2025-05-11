#*******************************************************************************************
# Makefile
#
# Created by: Erik van der Tier
# on: 2023-02-02
#*******************************************************************************************
NAME = 	asm465
ADDR = 	0000
OUTDIR = build
BLD_NAME = $(OUTDIR)/$(NAME).prg

CORE_SRC = \
		src/include/m65defs.h \
		src/include/m65macros.h \
		src/include/utils.h \
		src/include/screen_macros.h \
		src/include/debug_macros.h \
		src/layout.s \
		src/boot.s \
		src/init.s \
		src/screen.s \
		src/util.s \

PTEST_SRC = \
		src/include/parser_macros.h \
		src/parser.s \
		src/tests/parsertest.s

STEST_SRC = \
		src/tests/screentest.s

DTEST_SRC = \
		src/tests/debugtest.s

UTEST_SRC = \
		src/include/unittest_macros.h \
		src/unittest.s \
		src/tests/unittest.s


OPTS = 	-C -a -B -i --m4510

parsertest: $(PTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(PTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

screentest: $(STEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(STEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

debugtest: $(DTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(DTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

unittest: $(UTEST_SRC) $(CORE_SRC) 
		64tass -D DEBUG_:=true $(OPTS) $(CORE_SRC) $(UTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

all: parsertest screentest debugtest unittest

run_ptest: parsertest
		etherload.osx -r $(OUTDIR)/parsertest

run_stest: screentest
		etherload.osx -r $(OUTDIR)/screentest

run_dtest: debugtest
		etherload.osx -r $(OUTDIR)/debugtest

run_utest: unittest
		etherload.osx -r $(OUTDIR)/unittest

clean:
		rm -r $(OUTDIR)/*

.PHONY: run_ptest run_stest all clean