OPFORGE ?= opForge
OUTDIR ?= build

C64_OUT := $(OUTDIR)/opforge_sprite_c64
CROSS_OUT := $(OUTDIR)/opforge_sprite_cross465

.PHONY: all c64 cross465 clean

all: c64

$(OUTDIR):
	mkdir -p $(OUTDIR)

c64: $(OUTDIR)
	$(OPFORGE) -i . -D C64 -l -x -o $(C64_OUT)

cross465: $(OUTDIR)
	$(OPFORGE) -i . -D CROSS465 -l -x -o $(CROSS_OUT)

clean:
	rm -rf $(OUTDIR)
