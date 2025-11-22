; U64 screen macros mirroring Mega65 names/usage, but 6502/VIC-II

SCREENMACROS :?= false
.if !SCREENMACROS
SCREENMACROS := true

DEBUG_RTST_ONLY :?= 0

.include "platformdefs.h"
.include "platformmacros.h"
.include "gen_screen_macros.h"

; runtime vars expected from screen.s:
;   PrtRow, PrtColumn, PrtColour (bytes)
;   ScreenPtr, ColPtr, CurScreenPosPtr, CurColourPosPtr (words)

; Clear screen and color RAM, then set cursor to 0,0
ClearScreen .macro colour
    .if !DEBUG_RTST_ONLY
        ; clear chars
        sta cross465.console.CLR
    .endif
.endmacro

SetBGColor .macro col
    .if !DEBUG_RTST_ONLY
        lda #\col
        sta cross465.console.SETCOL
    .endif
.endmacro

SetBColor .macro col
    .if !DEBUG_RTST_ONLY
        lda #\col
        sta cross465.console.SETBGCOL
    .endif
.endmacro

PutC .macro
    .if !DEBUG_RTST_ONLY
        sta cross465.console.PUTC
    .endif
.endmacro

.endif ; SCREENMACROS
