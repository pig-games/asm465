
SetColour .macro colour
    ldz \colour
    stz PrtColour
.endmacro

ClearScreen .macro colour
    #SetColour \colour
    sta $d707
    #DMAFillJob $0020, $000800, 4000, true
    #DMAFillJob \colour, $01f800, 4000, false
    #SetLocation 0,0
.endmacro

SetLocation .macro row, col
    lda #\col
    sta PrtColumn
    lda #0
    sta $d777
    sta $d776
    sta $d775
    sta $d773
    sta $d772
    sta $d771
    lda #80
    sta $d770
    lda #\row
    sta PrtRow
    sta $d774
    clc
    ldqa $d778
    adqa ScreenPtr
    stqa CurScreenPosPtr
    clc
    ldqa $d778
    adqa ColPtr
    stqa CurColourPosPtr
.endmacro

PutC .macro
    ldz PrtColumn
    stabpqz CurScreenPosPtr
.endmacro


