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
		src/layout.s \
		src/boot.s \
		src/init.s \
		src/screen.s \

PTEST_SRC = \
		src/include/parser_macros.h \
		src/parser.s \
		src/tests/parsertest.s

STEST_SRC = \
		src/tests/screentest.s

OPTS = 	-C -a -B -i --m4510

parsertest: $(PTEST_SRC) $(CORE_SRC) 
		64tass $(OPTS) $(CORE_SRC) $(PTEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

screentest: $(STEST_SRC) $(CORE_SRC) 
		64tass $(OPTS) $(CORE_SRC) $(STEST_SRC) -o $(OUTDIR)/$@ --list $(OUTDIR)/$(basename $@).lst --labels=$(OUTDIR)/$(basename $@).lbl

all: parsertest screentest

run_ptest: parsertest
		etherload.osx -r $(OUTDIR)/parsertest

run_stest: screentest
		etherload.osx -r $(OUTDIR)/screentest

clean:
		rm -r $(OUTDIR)/*

.PHONY: run_ptest run_stest all clean