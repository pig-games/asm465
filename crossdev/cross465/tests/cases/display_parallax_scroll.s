RTST_BASE = $C000
* = $2200
    jmp start
.include "test_rtst.inc"

start:
    sei
    cld
    RTST_BEGIN
    TEST_CASE_BEGIN case_name
    LOG_KV scroll_x_key, $12
    LOG_KV scroll_y_key, $03
    LOG_HASH fb_hash_key, framebuffer, 32
    TEST_CASE_OK 0, ok_msg
    RTST_END

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
