; U64 screen macros mirroring Mega65 names/usage, but 6502/VIC-II

SCREENMACROS :?= false
.if !SCREENMACROS
SCREENMACROS := true

.include "platformdefs.h"
.include "platformmacros.h"
.include "gen_screen_macros.h"

; runtime vars expected from screen.s:
;   PrtRow, PrtColumn, PrtColour (bytes)
;   ScreenPtr, ColPtr, CurScreenPosPtr, CurColourPosPtr (words)

; Clear screen and color RAM, then set cursor to 0,0
ClearScreen .macro colour
    ; clear chars
    jsr CLRSCN
    ; fill color RAM
    lda #\colour
    ldx #0
loop
    sta COLR_BASE,x
    sta COLR_BASE+256,x
    sta COLR_BASE+512,x
    sta COLR_BASE+768,x
    inx
    bne loop
    .SetLocation 0,0
.endmacro

SetBGColor .macro col
    lda #\col
    sta vic2.SCREENCOL
.endmacro

SetBColor .macro col
    lda #\col
    sta vic2.BORDERCOL
.endmacro

PutC .macro
    ldy PrtColumn
    sta (CurScreenPosPtr),y
.endmacro

.endif ; SCREENMACROS
