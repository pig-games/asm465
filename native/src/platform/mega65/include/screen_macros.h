SCREENMACROS :?= false
.if !SCREENMACROS
SCREENMACROS := true

.include "platformdefs.h"
.include "platformmacros.h"
.include "gen_screen_macros.h"

ClearScreen .macro colour
    .SetColour \colour
    sta dma.ETRIGINLINE
    .dma.fillJob $0020,   $000800, 4000, true
    .dma.fillJob \colour, $ff80000, 4000, false
    .SetLocation 0,0
.endmacro

SetBGColor .macro col
    lda #\col
    sta vic4.SCREENCOL
.endmacro

SetBColor .macro col
    lda #\col
    sta vic4.BORDERCOL
.endmacro

PutC .macro
    ldz PrtColumn
    sta [CurScreenPosPtr],z
.endmacro

.endif