; Sprite data for C64 (24x21, 63 bytes + 1 padding byte)

.module assets.sprite
    .cpu 6502

    .pub

SPRITE_ADDR .const $2000
SPRITE_PTR  .const (SPRITE_ADDR / 64)

    .org SPRITE_ADDR
spriteData:
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $FF, $FF, $FF
    .byte $00
.endmodule
