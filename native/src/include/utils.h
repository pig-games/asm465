

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

rdxy .macro ptr
    ldx \ptr
    ldy \ptr+1
.endmacro

stxy .macro ptr
        stx \ptr
        sty \ptr+1
.endmacro

ldbcd24 .macro ptr
        ldy \ptr+2
        ldx \ptr+1
        lda \ptr
.endmacro

stbcd24 .macro ptr
        sty \ptr+2
        stx \ptr+1
        sta \ptr
.endmacro


incxy .macro
        inx
        bne *+3
        iny
.endmacro