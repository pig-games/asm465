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
; computes CurScreenPosPtr = ScreenPtr + row*40
; computes CurColourPosPtr = ColPtr + row*40
setLocation .proc
    ; store logical cursor
    stx PrtColumn
    sty PrtRow

    ; add 40 * row to CurScreenPosPtr
    tya

    jsr mul40               ; Ptr = row*40

    ; CurScreenPosPtr = ScreenPtr + offset
    clc
    lda ScreenPtr
    adc Ptr
    sta CurScreenPosPtr
    lda ScreenPtr+1
    adc Ptr+1
    sta CurScreenPosPtr+1

    ; CurColourPosPtr = ColPtr + offset
    clc
    lda ColPtr
    adc Ptr
    sta CurColourPosPtr
    lda ColPtr+1
    adc Ptr+1
    sta CurColourPosPtr+1

    rts
.endproc

; A: character
cPutC .proc
    .PutC

    lda PrtColour
    ldy PrtColumn
    ; write colour at colour RAM
    sta (CurColourPosPtr),y
    rts
.endproc

; A: character
cPrintC .proc
    .PutC
    lda PrtColour
    ldy PrtColumn
    sta (CurColourPosPtr),y
    inc PrtColumn
    rts
.endproc

; A: character
; X: colour
setCPrintC .proc
    stx PrtColour
    .PutC
    lda PrtColour
    ldy PrtColumn
    sta (CurColourPosPtr),y
    inc PrtColumn
    rts
.endproc

; X: str ptr lo
; Y: str ptr hi
print .proc
    .stxy Ptr

    ldy #0            ; Y = string index

loop
    sty YStore
    lda (Ptr),y
    beq end
    cmp #'!'
    bne noCmd
    iny
    lda (Ptr),y
    cmp #'!'
    beq noCmd
    cmp #'n'
    bne noN
    phy
    jsr printNL
    ply

    iny
    jmp loop
noN
    dey
    sty YStore
    lda (Ptr),y
noCmd
    ldy PrtColumn
    sta (CurScreenPosPtr),y
    lda PrtColour
    sta (CurColourPosPtr),y
next
    inx
    iny
    sty PrtColumn
    ldy YStore
    iny
    jmp loop

end
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
    lda #0
    sta PrtColumn
    clc
    lda #40
    adc CurScreenPosPtr
    sta CurScreenPosPtr
    lda #0
    adc CurScreenPosPtr+1
    sta CurScreenPosPtr+1
    clc
    lda #40
    adc CurColourPosPtr
    sta CurColourPosPtr
    lda #0
    adc CurColourPosPtr+1
    sta CurColourPosPtr+1
    rts
.endproc

.endsection
