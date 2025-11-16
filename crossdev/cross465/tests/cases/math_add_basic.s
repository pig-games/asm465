rtst.BASE = $C000
TARGET_ULTIMATE64 :?= 0

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"

start
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
    lda #2
    clc
    adc #3
    cmp #5
    bne fail
    .ctest.OK 0, OK_MSG
    jmp done
fail
    .ctest.FAIL 1, FAIL_MSG
    jmp done
done
    .rtst.end

CASE_NAME .null "math::add_basic"
OK_MSG    .null "math is correct"
FAIL_MSG  .null "math mismatch"
