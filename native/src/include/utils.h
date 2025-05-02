
.cpu "4510"

setJSRAddress .macro addr_jsr, addr_calc
        lda #<\addr_calc
        sta \addr_jsr + 1
        lda #>\addr_calc
        sta \addr_jsr + 2
.endmacro


setBasePage .macro addr
        lda #>\addr
        tab
.endmacro
