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
    lda #0
    sta cross465.SPRYLO
    sta cross465.SPRXLO
    lda #1
    sta cross465.SPRNUM

    lda #2
    sta cross465.SPRSEL
    lda #(255/2)
    sta cross465.SPRYHI
    sta cross465.SPRXHI
    lda #0
    sta cross465.SPRANIM
    sta cross465.SPRYLO
    lda #(255/2)
    sta cross465.SPRXLO
    lda #2
    sta cross465.SPRNUM

    lda #3
    sta cross465.SPRSEL
    lda #255
    sta cross465.SPRYHI
    sta cross465.SPRXHI
    lda #0
    sta cross465.SPRANIM
    sta cross465.SPRYLO
    lda #255
    sta cross465.SPRXLO
    lda #3
    sta cross465.SPRNUM

    brk
.endsection
