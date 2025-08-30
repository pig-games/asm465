.include "platformmacros.h"

.section init
    ; No VIC‑IV setup here; keep it minimal so Mega65/U64 stay parallel.
    ; Initialize screen/color base pointers and cursor defaults,
    ; mirroring semantics from the Mega65 side (but 16‑bit).

    ; Screen base / color base
    lda #<SCRN_BASE
    sta ScreenPtr
    lda #>SCRN_BASE
    sta ScreenPtr+1

    lda #<COLR_BASE
    sta ColPtr
    lda #>COLR_BASE
    sta ColPtr+1

    ; default colour: white
    lda #$01
    sta PrtColour

    .SetLocation 0,0
.endsection
