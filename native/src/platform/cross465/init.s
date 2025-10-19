.include "platformmacros.h"
.include "debug_macros.h"

.section init
    ; No VIC‑IV setup here; keep it minimal so Mega65/U64 stay parallel.
    ; Initialize screen/color base pointers and cursor defaults,
    ; mirroring semantics from the Mega65 side (but 16‑bit).

    .cpr 3, "Sprite tests!n"
    lda #4
    sta cross465.display.BRCOL
    lda #6
    sta cross465.display.BGCOL

    lda #1
    sta cross465.sprite.SPRSEL
    lda #0
    sta cross465.sprite.SPRYHI
    sta cross465.sprite.SPRXHI
    sta cross465.sprite.SPRANIM
    lda #20 ;40
    sta cross465.sprite.SPRXLO
    lda #26 ;53
    sta cross465.sprite.SPRYLO

    lda #1
    sta cross465.sprite.SPRNUM

    lda #2
    sta cross465.sprite.SPRSEL
    lda #0
    sta cross465.sprite.SPRYHI
    sta cross465.sprite.SPRXHI
    lda #0
    sta cross465.sprite.SPRANIM
    lda #(160+20)   ; 160 + left margin of 20/2
    sta cross465.sprite.SPRXLO
    lda #(120+26)   ; 120 + top margin of 20/2
    sta cross465.sprite.SPRYLO
    lda #2
    sta cross465.sprite.SPRNUM

    lda #3
    sta cross465.sprite.SPRSEL
    lda #1
    sta cross465.sprite.SPRXHI
    lda #1
    sta cross465.sprite.SPRYHI
    sta cross465.sprite.SPRANIM
    lda #(320-256+20)
    sta cross465.sprite.SPRXLO
    lda #(240-256+26)
    sta cross465.sprite.SPRYLO

    lda #3
    sta cross465.sprite.SPRNUM

    brk
.endsection
