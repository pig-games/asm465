; Cross465 sprite backend

.module platform.cross465
    .pub
    .cpu 6502

SPRITE_SEL      .const $DF30
SPRITE_NUM      .const $DF31
SPRITE_ANIM     .const $DF32
SPRITE_XHI      .const $DF33
SPRITE_XLO      .const $DF34
SPRITE_YHI      .const $DF35
SPRITE_YLO      .const $DF36

DISP_BORDER     .const $DF20
DISP_BG         .const $DF21

SPRITE_ID       .const 1

platformInit:
    lda #$0E
    sta DISP_BORDER
    lda #$06
    sta DISP_BG

    lda #SPRITE_ID
    sta SPRITE_SEL
    lda #0
    sta SPRITE_XHI
    sta SPRITE_YHI
    sta SPRITE_ANIM

    lda #100
    sta SPRITE_YLO

    lda #SPRITE_ID
    sta SPRITE_NUM
    rts

platformSetSpriteX:
    tax
    lda #SPRITE_ID
    sta SPRITE_SEL
    lda #0
    sta SPRITE_XHI
    txa
    sta SPRITE_XLO
    lda #SPRITE_ID
    sta SPRITE_NUM
    rts
.endmodule
