
SetColour .macro colour
    ldz #\colour
    stz PrtColour
.endmacro

ClearScreen .macro colour
    #SetColour \colour
    sta dma.ETRIGINLINE
    #dma.FillJob $0020,   $000800, 4000, true
    #dma.FillJob \colour, $ff80000, 4000, false
    #SetLocation 0,0
.endmacro

SetLocation .macro col, row
    ldx #\col
    ldy #\row
    jsr setLocation
.endmacro

SetBGColor .macro col
    lda #\col
    sta vic4.SCREENCOL
.endmacro

SetBColor .macro col
    lda #\col
    sta vic4.BORDERCOL
.endmacro

SetBGBColors .macro bg, b
    #SetBGColor \bg
    #SetBColor \b
.endmacro

PutC .macro
    ldz PrtColumn
    stabpqz CurScreenPosPtr
.endmacro

nl .macro
    phq
    jsr printNL
    plq
.endmacro

pr .macro str
    phq
    jsr sPrint
    .null \str
    plq
.endmacro

prl .macro str
    phq
    jsr sPrint
    .null \str
    jsr printNL
    plq
.endmacro

cpr .macro colour, str
    phq
    jsr sCPrint
    .byte \colour
    .null \str
    plq
.endmacro

cprl .macro colour, str
    phq
    jsr sCPrint
    .byte \colour
    .null \str
    jsr printNL
    plq
.endmacro
