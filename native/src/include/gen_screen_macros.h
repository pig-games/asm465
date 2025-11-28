GENSCREENMACROS :?= false
.if !GENSCREENMACROS
GENSCREENMACROS := true

DEBUG_RTST_ONLY    :?= 0

SetColour .macro colour
    ldx #\colour
    stx PrtColour
.endmacro

SetLocation .macro col, row
    ldx #\col
    ldy #\row
    jsr setLocation
.endmacro

SetBGBColors .macro bg, b
    .SetBGColor \bg
    .SetBColor \b
.endmacro

nl .macro
    phq
    jsr printNL
    plq
.endmacro

; Print raw (inline) string after JSR — same calling as M65 pr
pr .macro str
    phq
    jsr sPrint
    .null \str
    plq
.endmacro

; Color print (inline): first a color byte, then string — same as M65 cpr
cpr .macro colour, str
    phq
    jsr sCPrint
    .byte \colour
    .null \str
    plq
.endmacro

.endif
