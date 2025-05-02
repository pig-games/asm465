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
PTEST_NAME = $(OUTDIR)/parsertest

CORE_SRC = \
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

OPTS = 	-C -a -B -i --m4510

$(PTEST_NAME): $(PTEST_SRC) $(CORE_SRC) 
		64tass $(OPTS) $(CORE_SRC) $(PTEST_SRC) -o $@ --list $(basename $@).lst --labels=$(basename $@).lbl

runtest: $(PTEST_NAME)
		etherload.osx -r $(PTEST_NAME)

clean:
		rm -r $(OUTDIR)/*

.PHONY: up clean