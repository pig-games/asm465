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
        sta $d777
        sta $d776
        sta $d775
        sta $d773
        sta $d772
        sta $d771
        lda #80
        sta $d770
        sty PrtRow
        sty $d774
        clc
        ldqa $d778
        adqa ScreenPtr
        stqa CurScreenPosPtr
        clc
        ldqa $d778
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