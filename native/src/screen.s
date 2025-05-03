; constants

    CA_BLINK = %0001_0000
    CA_REV = %0010_0000
    CA_ULINE = %1000_0000

.section bp
    ScreenPtr       .dword 0
    ColPtr          .dword 0
    CurScreenPosPtr .dword 0
    CurColourPosPtr .dword 0
.endsection

.section data

PrtRow      .byte 0
PrtColumn   .byte 0
PrtColour   .byte 0

.endsection

.section screen

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

    ; X: String Ptr lo
    ; Y: String Ptr hi
    Print .proc

        rts
    .endproc

.endsection ; screen