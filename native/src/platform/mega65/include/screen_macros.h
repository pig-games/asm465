SCREENMACROS :?= false
.if !SCREENMACROS
SCREENMACROS := true

DEBUG_RTST_ONLY :?= 0

.include "platformdefs.h"
.include "platformmacros.h"
.include "gen_screen_macros.h"

ClearScreen .macro colour
    .if !DEBUG_RTST_ONLY
        .SetColour \colour
        sta dma.ETRIGINLINE
        .dma.fillJob $0020,   $000800, 4000, true
        .dma.fillJob \colour, $ff80000, 4000, false
        .SetLocation 0,0
    .endif
.endmacro

SetBGColor .macro col
    .if !DEBUG_RTST_ONLY
        lda #\col
        sta vic4.SCREENCOL
    .endif
.endmacro

SetBColor .macro col
    .if !DEBUG_RTST_ONLY
        lda #\col
        sta vic4.BORDERCOL
    .endif
.endmacro

PutC .macro
    .if !DEBUG_RTST_ONLY
        ldz PrtColumn
        sta [CurScreenPosPtr],z
    .endif
.endmacro

.endif
