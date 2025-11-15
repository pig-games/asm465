RTST_BASE = $4000
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

scroll_x_key: .byte $73,$63,$72,$6f,$6c,$6c,$5f,$78,0
scroll_y_key: .byte $73,$63,$72,$6f,$6c,$6c,$5f,$79,0
fb_hash_key: .byte $66,$62,$5f,$68,$61,$73,$68,0
case_name: .byte $64,$69,$73,$70,$6c,$61,$79,$3a,$3a,$70,$61,$72,$61,$6c,$6c,$61,$78,$5f,$73,$63,$72,$6f,$6c,$6c,0
ok_msg: .byte $64,$69,$73,$70,$6c,$61,$79,$20,$6d,$65,$74,$72,$69,$63,$73,$20,$63,$61,$70,$74,$75,$72,$65,$64,0
framebuffer:
    .byte $00,$01,$02,$03,$04,$05,$06,$07
    .byte $08,$09,$0A,$0B,$0C,$0D,$0E,$0F
    .byte $10,$11,$12,$13,$14,$15,$16,$17
    .byte $18,$19,$1A,$1B,$1C,$1D,$1E,$1F
