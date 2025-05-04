
SetColour .macro colour
    ldz #\colour
    stz PrtColour
.endmacro

ClearScreen .macro colour
    #SetColour \colour
    sta dma.ETRIGINLINE
    #DMAFillJob $0020,   $000800, 4000, true
    #DMAFillJob \colour, $ff80000, 4000, false
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
    pha
    phx
    phy
    phz
    jsr printNL
    plz
    ply
    plx
    pla
.endmacro

pr .macro str
    pha
    phx
    phy
    phz
    jsr sPrint
    .null \str
    plz
    ply
    plx
    pla
.endmacro

prl .macro str
    pha
    phx
    phy
    phz
    jsr sPrint
    .null \str
    jsr printNL
    plz
    ply
    plx
    pla
.endmacro

cpr .macro colour, str
    pha
    phx
    phy
    phz
    jsr sCPrint
    .byte \colour
    .null \str
    plz
    ply
    plx
    pla
.endmacro

cprl .macro colour, str
    pha
    phx
    phy
    phz
    jsr sCPrint
    .byte \colour
    .null \str
    jsr printNL
    plz
    ply
    plx
    pla
.endmacro
