; Ultimate64 platform macros (6502-safe equivalents to M65 ones)
PLATFORMMACROS :?= false
.if !PLATFORMMACROS
PLATFORMMACROS := true

.include "platformdefs.h"

; ---- push/pop “quad” (A,X,Y only on 6502) ---------------------------------
phq .function
    sta AStore
    pha         ; save A
    txa
    pha         ; save X via A
    tya
    pha         ; save Y via A
    lda AStore
.endfunction

plq .function
    pla         ; restore Y into A
    tay         ; store into Y
    pla         ; restore X into A
    tax         ; store into X
    pla         ; restore A
.endfunction

plx .function
    sta AStore
    pla         ; restore X into A
    tax         ; store into X
    lda AStore
.endfunction

ply .function
    sta AStore
    pla         ; restore Y into A
    tay         ; store into Y
    lda AStore
.endfunction

phx .function
    sta AStore
    txa         ; save X into A
    pha         ; save A
    lda AStore
.endfunction

phy .function
    lda AStore
    tya         ; save Y into A
    pha         ; save A
    sta AStore
.endfunction

adq .function ptr
    clc
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    txa
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    tya
    adc \ptr
    sta \ptr
    bcc *+2
    inc \ptr+1
    rts
.endfunction

; ---- simple timing ---------------------------------------------------------
RASTER_WAIT .macro line
w1
    lda RASTER
    cmp #\line
    bne w1
w2
    lda RASTER
    cmp #\line
    beq w2
.endmacro

CIA_DELAY .macro approx_ms
    ldx #\approx_ms
d1
    ldy #$FF
d2
    dey
    bne d2
    dex
    bne d1
.endmacro

; ---- border helpers --------------------------------------------------------
BORDER_FLASH .macro count
    ldx #\count
    lda BORDERCOL
    pha
loop
    lda #$02
    sta BORDERCOL
    jsr CLRSCN
    pla
    pha
    sta BORDERCOL
    jsr CLRSCN
    dex
    bne loop
    pla
    sta BORDERCOL
.endmacro

; ---- IRQ helpers (vector install, pro/epi) --------------------------------
IRQSTAT = $D019
IRQMASK = $D01A
K_IRQLO = $0314
K_IRQHI = $0315

; ------------------------------------------------------------
; BasicUpstart — C64/Ultimate64 variant
; Emits a single BASIC line: 10 SYS <addr>
; Use at $0801 (your layout already places .dsection boot there)
; ------------------------------------------------------------
BasicUpstart .macro addr
    .word bu_next          ; link to next BASIC line
    .word 10                ; line number
    .byte $9e               ; token for SYS
    .text format("%d", \addr) ; decimal address, no padding
    .byte 0                 ; end of line
bu_next
    .word 0                 ; end of program
.endmacro

INSTALL_IRQ .macro handler
    sei
    lda #<\handler
    sta K_IRQLO
    lda #>\handler
    sta K_IRQHI
    lda IRQSTAT
    sta IRQSTAT
    lda #$01
    sta IRQMASK
    cli
.endmacro

IRQ_BEGIN .macro
    pha
    txa
    pha
    tya
    pha
.endmacro

IRQ_END .macro 
    pla
    tay
    pla
    tax
    pla
    lda IRQSTAT
    sta IRQSTAT
    rti
.endmacro

.endif
