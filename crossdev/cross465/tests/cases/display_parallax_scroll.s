rtst.BASE = $C000
* = $2200
    jmp start
.include "test_rtst.h"

start:
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
    .ctest.logKV SCROLL_X_KEY, $12
    .ctest.logKV SCROLL_Y_KEY, $03
    .ctest.logHash FB_HASH_KEY, FRAMEBUFFER, 32
    .ctest.OK 0, OK_MSG
    .rtst.end

SCROLL_X_KEY: .null "scroll_x"
SCROLL_Y_KEY: .null "scroll_y"
FB_HASH_KEY: .null "fb_hash"
CASE_NAME: .null "display::parallax_scroll"
OK_MSG: .null "display metrics captured"
FRAMEBUFFER:
    .byte $00,$01,$02,$03,$04,$05,$06,$07
    .byte $08,$09,$0A,$0B,$0C,$0D,$0E,$0F
    .byte $10,$11,$12,$13,$14,$15,$16,$17
    .byte $18,$19,$1A,$1B,$1C,$1D,$1E,$1F
