; C64/Ultimate64 screen implementation (ported from Mega65 version)
; - Uses zero-page indirect indexed addressing for screen/color writes
; - Assumes Ptr and PtrScr/PtrCol live in bp as project expects
; - Expects ScreenPtr / ColPtr to be initialized by platform init

.include "screen_macros.h"
.include "utils.h"
.enc "screen"

; constants
CA_BLINK = %0001_0000
CA_REV   = %0010_0000
CA_ULINE = %1000_0000

.section bp
    ScreenPtr       .word 0
    ColPtr          .word 0
    CurScreenPosPtr .word 0
    CurColourPosPtr .word 0
    Ptr             .word 0
    AStore          .byte 0
    XStore          .byte 0
    YStore          .byte 0
.endsection

.section screen

; multiply A * 40 -> Ptr (lo/hi). Preserves A.
mul40 .proc
    pha
    sta Ptr
    lda #0
    sta Ptr+1          ; save A

    ; A*32
    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1

    .rdxy Ptr   ; backup Ptr in X/Y
    ; + A*8

    pla
    sta Ptr
    lda #0
    sta Ptr+1

    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1
    asl Ptr
    rol Ptr+1

    clc
    txa
    adc Ptr
    sta Ptr
    tya
    adc #0
    sta Ptr+1
    rts
.endproc

setLowerCase .proc
    rts
.endproc

setUpperCase .proc
    rts
.endproc

; X: column
; Y: row
setLocation .proc
    ; store logical cursor
    stx PrtColumn
    sty PrtRow
    stx cross465.SETX
    sty cross465.SETY
    sty cross465.SETLOC
    rts
.endproc

; A: character
cPutC .proc
    sty YStore
    ldy PrtColour
    sty cross465.SETCOL
    ldy YStore
    .PutC
    rts
.endproc

; A: character
cPrintC .proc
    sty YStore
    ldy PrtColour
    sty cross465.SETCOL
    ldy YStore
    .PutC
    rts
.endproc

; A: character
; X: colour
setCPrintC .proc
    stx PrtColour
    stx cross465.SETCOL
    .PutC
    rts
.endproc

; X: str ptr lo
; Y: str ptr hi
print .proc
    .stxy Ptr
    sty YStore
    ldy PrtColour
    sty cross465.SETCOL
    ldy YStore
    stx cross465.SETLPTR
    sty cross465.PRINT
    ldy cross465.PRINT
    rts
.endproc

printSC .proc
    pha
    clc
    adc #48
    jsr printC
    pla
    rts
.endproc

cPrintSC .proc
    pha
    clc
    adc #48
    jsr cPrintC
    pla
    rts
.endproc

; Print a newline: reset cursor to 0,0 and update pointers
; (also updates current screen/color pointers)
printNL .proc
    sta cross465.NL
    rts
.endproc

.endsection
