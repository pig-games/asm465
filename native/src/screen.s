
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
.endsection

.section data

PrtRow      .byte 0
PrtColumn   .byte 0
PrtColour   .byte 0

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
        ldqa math.MULTOUT1
        adqa ScreenPtr
        stqa CurScreenPosPtr
        clc
        ldqa math.MULTOUT1
        adqa ColPtr
        stqa CurColourPosPtr
        rts
    .endproc

    ; A: character
    putC .proc
        #PutC
        rts
    .endproc

    ; A: character
    cPutC .proc
        #PutC
        lda PrtColour
        ldz PrtColumn
        stabpqz CurColourPosPtr
        rts
    .endproc

    ; A: character
    ; Z: colour
    setCPutC .proc
        stz PrtColour
        jmp cPutC
    .endproc

    ; A: character
    printC .proc
        #PutC
        inc PrtColumn
        rts
    .endproc

    ; A: character
    cPrintC .proc
        #PutC
        lda PrtColour
        stabpqz CurColourPosPtr
        inc PrtColumn
        rts
    .endproc

    ; A: character
    ; Z: colour
    setCPrintC .proc
        stz PrtColour
        #PutC
        lda PrtColour
        stabpqz CurColourPosPtr
        inc PrtColumn
        rts
    .endproc

    ; X: str ptr lo
    ; Y: str ptr hi
    print .proc
        #stxy Ptr
        
        ldz PrtColumn
        ldy #0
        loop
            lda (Ptr),y
            beq end
        
            stabpqz CurScreenPosPtr
            lda PrtColour
            stabpqz CurColourPosPtr

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
        
        #incxy
        
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
    ; Z: colour
    cPrint .proc
        #stxy Ptr
        stz PrtColour
        
        ldz PrtColumn
        ldy #0
        loop
            lda (Ptr),y
            beq end
        
            stabpqz CurScreenPosPtr
            lda PrtColour
            stabpqz CurColourPosPtr

            inz
            iny
        jmp loop
    end
        stz PrtColumn
        rts
    .endproc

    sCPrint .proc
        plx
        ply
        ; do actual print

        #incxy

        #stxy Ptr
        phy
        ldy #0
        lda (Ptr),y
        taz
        ply
        #incxy
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

    printNL .proc
        ldz #0
        stz PrtColumn

        lda #80
        ldx #0
        ldy #0
        ldz #0
        adqa CurScreenPosPtr
        stqa CurScreenPosPtr
        lda #80
        ldx #0
        ldy #0
        ldz #0
        adqa CurColourPosPtr
        stqa CurColourPosPtr
        rts
    .endproc

.endsection ; screen