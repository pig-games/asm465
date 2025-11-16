rtst.BASE = $C000
* = $2000
    jmp start
.include "test_rtst.h"

start:
    sei
    cld
    .rtst.begin
    .ctest.begin case_name
    lda #2
    clc
    adc #3
    cmp #5
    bne fail
    .ctest.OK 0, ok_msg
    jmp done
fail:
    .ctest.FAIL 1, fail_msg
    jmp done
done:
    .rtst.end

case_name: .null "math::add_basic"
ok_msg: .null "math is correct"
fail_msg: .null "math mismatch"
