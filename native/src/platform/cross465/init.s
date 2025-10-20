.include "platformmacros.h"
.include "debug_macros.h"

.section init
    ; No VIC‑IV setup here; keep it minimal so Mega65/U64 stay parallel.
    ; Initialize screen/color base pointers and cursor defaults,
    ; mirroring semantics from the Mega65 side (but 16‑bit).

    .cpr 3, "Sprite tests!n"
    lda #$e
    sta cross465.display.BRCOL
    lda #6
    sta cross465.display.BGCOL

    lda #1
    sta cross465.sprite.SEL
    lda #0
    sta cross465.sprite.YHI
    sta cross465.sprite.XHI
    sta cross465.sprite.ANIM
    lda #20 ;40
    sta cross465.sprite.XLO
    lda #26 ;53
    sta cross465.sprite.YLO

    lda #1
    sta cross465.sprite.NUM

    lda #2
    sta cross465.sprite.SEL
    lda #0
    sta cross465.sprite.YHI
    sta cross465.sprite.XHI
    lda #0
    sta cross465.sprite.ANIM
    lda #(160+20)   ; 160 + left margin of 20/2
    sta cross465.sprite.XLO
    lda #(120+26)   ; 120 + top margin of 20/2
    sta cross465.sprite.YLO
    lda #2
    sta cross465.sprite.NUM

    lda #3
    sta cross465.sprite.SEL
    lda #1
    sta cross465.sprite.XHI
    lda #1
    sta cross465.sprite.YHI
    sta cross465.sprite.ANIM
    lda #(320-256+20)
    sta cross465.sprite.XLO
    lda #(240-256+26)
    sta cross465.sprite.YLO

    lda #3
    sta cross465.sprite.NUM

    brk
.endsection
