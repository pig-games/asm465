RTST_BASE = $4000
* = $2000
    jmp start
.include "test_rtst.inc"

start:
    sei
    cld
    RTST_BEGIN
    TEST_CASE_BEGIN case_name
    lda #2
    clc
    adc #3
    cmp #5
    bne fail
    TEST_CASE_OK 0, ok_msg
    jmp done
fail:
    TEST_CASE_FAIL 1, fail_msg
    jmp done
done:
    RTST_END

case_name: .byte $6d,$61,$74,$68,$3a,$3a,$61,$64,$64,$5f,$62,$61,$73,$69,$63,0
ok_msg: .byte $6d,$61,$74,$68,$20,$69,$73,$20,$63,$6f,$72,$72,$65,$63,$74,0
fail_msg: .byte $6d,$61,$74,$68,$20,$6d,$69,$73,$6d,$61,$74,$63,$68,0
