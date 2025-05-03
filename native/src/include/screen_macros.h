
SetColour .macro colour
    ldz \colour
    stz PrtColour
.endmacro

ClearScreen .macro colour
    #SetColour \colour
    sta dma.ETRIGINLINE
    #DMAFillJob $0020,   $000800, 4000, true
    #DMAFillJob \colour, $ff8000, 4000, false
    #SetLocation 0,0
.endmacro

SetLocation .macro row, col
    lda #\col
    sta PrtColumn
    lda #0
    sta math.IN_B4
    sta math.IN_B3
    sta math.IN_B2
    sta math.IN_A4
    sta math.IN_A3
    sta math.IN_A2
    lda #80
    sta math.IN_A1
    lda #\row
    sta PrtRow
    sta math.IN_B1
    clc
    ldqa math.MULTOUT1
    adqa ScreenPtr
    stqa CurScreenPosPtr
    clc
    ldqa math.MULTOUT1
    adqa ColPtr
    stqa CurColourPosPtr
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
