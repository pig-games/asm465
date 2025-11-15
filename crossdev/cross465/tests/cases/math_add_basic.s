RTST_BASE = $C000
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

case_name: .null "math::add_basic"
ok_msg: .null "math is correct"
fail_msg: .null "math mismatch"
