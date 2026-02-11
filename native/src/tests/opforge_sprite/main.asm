; opForge sprite demo for C64 and Cross465

.module main
    .cpu 6502
    .meta
        .output
            .name "opforge_sprite"
            .list
            .bin
        .endoutput
    .endmeta
 
    ; Select the platform module via -D CROSS465 or -D C64 (defaults to C64).
    .ifdef CROSS465
        .use platform.cross465 (platformInit, platformSetSpriteX)
    .else
        .use platform.c64 (platformInit, platformSetSpriteX)
    .endif

    .org $0801
basic_start:
    .word basic_line_end
    .word 10
    .byte $9E, $20, '2', '0', '6', '4', 0
basic_line_end:
    .word 0

    .org $0810
start:
    jsr platformInit

    lda #40
    sta spriteX
    lda #1
    sta spriteDir
    lda spriteX
    jsr platformSetSpriteX

main_loop:
    jsr waitFrame
    jsr stepSprite
    jmp main_loop

waitFrame:
    ldx #$20
wait_outer:
    ldy #$00
wait_inner:
    dey
    bne wait_inner
    dex
    bne wait_outer
    rts

stepSprite:
    lda spriteX
    clc
    adc spriteDir
    sta spriteX

    cmp #24
    bcs check_right
    lda #1
    sta spriteDir
    lda #24
    sta spriteX
    bne update_position

check_right:
    cmp #216
    bcc update_position
    lda #$FF
    sta spriteDir
    lda #216
    sta spriteX

update_position:
    lda spriteX
    jsr platformSetSpriteX
    rts

spriteX:
    .byte 40
spriteDir:
    .byte 1
.endmodule
