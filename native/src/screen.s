.include "screen_macros.h"
.include "utils.h"
.enc "screen"


.section data
    PrtRow      .byte 0
    PrtColumn   .byte 0
    PrtColour   .byte 0
.endsection

.section screen

    ; A: character
    putC .proc
        .PutC
        rts
    .endproc

    ; A: character
    ; x: colour
    setCPutC .proc
        stx PrtColour
        jmp cPutC
    .endproc

    ; A: character
    printC .proc
        .PutC
        inc PrtColumn
        rts
    .endproc

    printBCD24 .proc ; a, x, y
        pha
        clc
        tya
        lsr
        lsr
        lsr
        lsr
        beq noDec6
        jsr printSC
    noDec6
        tya
        and #$f
        beq noDec5
        jsr printSC
    noDec5
        txa
        lsr
        lsr
        lsr
        lsr
        beq noDec4
        jsr printSC
    noDec4
        txa
        and #$f
        beq noDec3
        jsr printSC
    noDec3
        pla
        pha
        lsr
        lsr
        lsr
        lsr
        beq noDec2
        jsr printSC
    noDec2
        pla
        and #$f
        jsr printSC

        rts
    .endproc

    cPrintBCD24 .proc ; a, x, y, z
        pha
        clc
        tya
        lsr
        lsr
        lsr
        lsr
        beq noDec6
        jsr cPrintSC
    noDec6
        tya
        and #$f
        beq noDec5
        jsr cPrintSC
    noDec5
        txa
        lsr
        lsr
        lsr
        lsr
        beq noDec4
        jsr cPrintSC
    noDec4
        txa
        and #$f
        beq noDec3
        jsr cPrintSC
    noDec3
        pla
        pha
        lsr
        lsr
        lsr
        lsr
        beq noDec2
        jsr cPrintSC
    noDec2
        pla
        and #$f
        jsr cPrintSC

        rts
    .endproc

    sPrint .proc
        plx
        ply
        ; do actual print
        
        .incxy
        
        jsr print

        ; calculate new return address
        tya
        clc
        adc Ptr
        sta Ptr
        lda #0
        adc Ptr+1

        ; restore return address
        pha
        lda Ptr
        pha
        rts
    .endproc

    ; X: str ptr lo
    ; Y: str ptr hi
    ; A: colour
    cPrint .proc
        .stxy Ptr
        sta PrtColour
        jmp print
    .endproc

    sCPrint .proc
        plx
        ply
        ; do actual print

        .incxy

        .stxy Ptr
        phy
        ldy #0
        lda (Ptr),y     ;read colour
        ply
        .incxy
        jsr cPrint

        ; calculate new return address
        tya
        clc
        adc Ptr
        sta Ptr
        lda #0
        adc Ptr+1

        ; restore return address
        pha
        lda Ptr
        pha
        rts
    .endproc

.endsection ; screen