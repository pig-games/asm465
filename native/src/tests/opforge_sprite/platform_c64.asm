; C64 VIC-II sprite backend

.module platform.c64
    .cpu 6502

    .pub
    .use assets.sprite (SPRITE_ADDR, SPRITE_PTR)

VIC_BASE        .const $D000
SPRITE0_X       .const VIC_BASE + $00
SPRITE0_Y       .const VIC_BASE + $01
SPRITE_X_MSB    .const VIC_BASE + $10
SPRITE_ENABLE   .const VIC_BASE + $15
SPRITE_COLOR0   .const VIC_BASE + $27
BORDER_COLOR    .const VIC_BASE + $20
BG_COLOR        .const VIC_BASE + $21
SPRITE_PTR0     .const $07F8

platformInit:
    lda #$0E
    sta BORDER_COLOR
    lda #$06
    sta BG_COLOR

    lda #SPRITE_PTR
    sta SPRITE_PTR0

    lda #1
    sta SPRITE_ENABLE

    lda #1
    sta SPRITE_COLOR0

    lda #100
    sta SPRITE0_Y
    rts

platformSetSpriteX:
    sta SPRITE0_X
    lda #0
    sta SPRITE_X_MSB
    rts
.endmodule
