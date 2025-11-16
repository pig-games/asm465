rtst.BASE = $C000
* = $2200
    jmp start
.include "test_rtst.h"

start:
    sei
    cld
    .rtst.begin
    .ctest.begin case_name
    .ctest.logKV scroll_x_key, $12
    .ctest.logKV scroll_y_key, $03
    .ctest.logHash fb_hash_key, framebuffer, 32
    .ctest.OK 0, ok_msg
    .rtst.end

scroll_x_key: .null "scroll_x"
scroll_y_key: .null "scroll_y"
fb_hash_key: .null "fb_hash"
case_name: .null "display::parallax_scroll"
ok_msg: .null "display metrics captured"
framebuffer:
    .byte $00,$01,$02,$03,$04,$05,$06,$07
    .byte $08,$09,$0A,$0B,$0C,$0D,$0E,$0F
    .byte $10,$11,$12,$13,$14,$15,$16,$17
    .byte $18,$19,$1A,$1B,$1C,$1D,$1E,$1F
