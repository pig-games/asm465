.include "screen_macros.h"
.include "utils.h"
.enc "screen"

; constants

    CA_BLINK = %0001_0000
    CA_REV = %0010_0000
    CA_ULINE = %1000_0000

.section bp
    ScreenPtr       .dword 0
    ColPtr          .dword 0
    CurScreenPosPtr .dword 0
    CurColourPosPtr .dword 0
    Ptr             .dword 0
    YStore          .byte 0
.endsection

.section screen

    setLowerCase .proc
        lda #00
        sta vic4.CHARPTRLO
        lda #$D8
        sta vic4.CHARPTRHI
        lda #$02
        sta vic4.CHARPTRBN
        rts
    .endproc

    setUpperCase .proc
        lda #00
        sta vic4.CHARPTRLO
        lda #$D0
        sta vic4.CHARPTRHI
        lda #$02
        sta vic4.CHARPTRBN
        rts
    .endproc

    ; X: column
    ; Y: row
    setLocation .proc
        stx PrtColumn
        lda #0
        sta math.IN_B4
        sta math.IN_B3
        sta math.IN_B2
        sta math.IN_A4
        sta math.IN_A3
        sta math.IN_A2
        lda #80
        sta math.IN_A1
        sty PrtRow
        sty math.IN_B1
        clc
        ldq math.MULTOUT1
        adq ScreenPtr
        stq CurScreenPosPtr
        clc
        ldq math.MULTOUT1
        adq ColPtr
        stq CurColourPosPtr
        rts
    .endproc

    ; A: character
    cPutC .proc
        .PutC
        lda PrtColour
        ldz PrtColumn
        sta [CurColourPosPtr],z
        rts
    .endproc

    ; A: character
    cPrintC .proc
        .PutC
        lda PrtColour
        ldz PrtColumn
        sta [CurColourPosPtr],z
        inc PrtColumn
        rts
    .endproc

    ; A: character
    ; X: colour
    setCPrintC .proc
        stx PrtColour
        .PutC
        lda PrtColour
        ldx PrtColumn
        sta [CurColourPosPtr],z
        inc PrtColumn
        rts
    .endproc

    ; X: str ptr lo
    ; Y: str ptr hi
    print .proc
        .stxy Ptr
        
        ldz PrtColumn
        ldy #0
        loop
            lda (Ptr),y
            beq end
            cmp #'!'
            bne noCmd
            iny
            lda (Ptr),y
            cmp #'!'
            beq noCmd
            cmp #'n'
            bne noN
            phy
            jsr printNL
            ply
            iny
            ldz #0
            bra loop
        noN
            dey
            lda (Ptr),y
        noCmd
            sta [CurScreenPosPtr],z
            lda PrtColour
            sta [CurColourPosPtr],z
        next
            inz
            iny
        jmp loop
    end
        stz PrtColumn
        rts
    .endproc

    printSC .proc
        pha
        clc
        adc #48
        phz
        jsr printC
        plz
        pla
        rts
    .endproc

    cPrintSC .proc
        pha
        clc
        adc #48
        phz
        jsr cPrintC
        plz
        pla

        rts
    .endproc

    printNL .proc
        ldz #0
        stz PrtColumn
        clc
        lda #80
        ldx #0
        ldy #0
        ldz #0
        adq CurScreenPosPtr
        stq CurScreenPosPtr
        clc
        lda #80
        ldx #0
        ldy #0
        ldz #0
        adq CurColourPosPtr
        stq CurColourPosPtr
        rts
    .endproc

.endsection ; screen