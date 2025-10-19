.include "platformmacros.h"
.include "debug_macros.h"

.section init
    ; No VIC‑IV setup here; keep it minimal so Mega65/U64 stay parallel.
    ; Initialize screen/color base pointers and cursor defaults,
    ; mirroring semantics from the Mega65 side (but 16‑bit).

    .cpr 3, "Sprite tests!n"
    lda #4
    sta cross465.BRCOL
    lda #6
    sta cross465.BGCOL

    lda #1
    sta cross465.SPRSEL
    lda #0
    sta cross465.SPRYHI
    sta cross465.SPRXHI
    sta cross465.SPRANIM
    lda #20 ;40
    sta cross465.SPRXLO
    lda #26 ;53
    sta cross465.SPRYLO

    lda #1
    sta cross465.SPRNUM

    lda #2
    sta cross465.SPRSEL
    lda #0
    sta cross465.SPRYHI
    sta cross465.SPRXHI
    lda #0
    sta cross465.SPRANIM
    lda #(160+20)   ; 160 + left margin of 20/2
    sta cross465.SPRXLO
    lda #(120+26)   ; 120 + top margin of 20/2
    sta cross465.SPRYLO
    lda #2
    sta cross465.SPRNUM

    lda #3
    sta cross465.SPRSEL
    lda #1
    sta cross465.SPRXHI
    lda #1
    sta cross465.SPRYHI
    sta cross465.SPRANIM
    lda #(320-256+20)
    sta cross465.SPRXLO
    lda #(240-256+26)
    sta cross465.SPRYLO

    lda #3
    sta cross465.SPRNUM

    brk
.endsection
