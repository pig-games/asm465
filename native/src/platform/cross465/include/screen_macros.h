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
    sta cross465.console.CLR
.endmacro

SetBGColor .macro col
    lda #\col
    sta cross465.console.SETCOL
.endmacro

SetBColor .macro col
    lda #\col
    sta cross465.console.SETBGCOL
.endmacro

PutC .macro
    sta cross465.console.PUTC
.endmacro

.endif ; SCREENMACROS
