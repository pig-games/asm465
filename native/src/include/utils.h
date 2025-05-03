
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

ldxy .macro ptr
    ldx #<\ptr
    ldy #>\ptr
.endmacro

stxy .macro ptr
        stx \ptr
        sty \ptr+1
.endmacro

incxy .macro
        inx
        bcc *+3
        iny
.endmacro