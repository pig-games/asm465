rtst.BASE = $C000
TARGET_CROSS465 :?= 0
TARGET_ASM465_NATIVE :?= 0
TARGET_ASM465_WASM :?= 0
TARGET_ULTIMATE64 :?= 0
DEBUG_ :?= 1
DEBUG_RTST_ONLY := 0
DEBUG_RTST_ENABLED := 1

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"
.include "debug_macros.h"

start
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
.if TARGET_CROSS465 | TARGET_ASM465_NATIVE | TARGET_ASM465_WASM | TARGET_ULTIMATE64
    .dbg.info "CONSOLE LOGGING FROM ASM"
.endif
    .ctest.OK 0, OK_MSG
    .rtst.end

CASE_NAME .null "logging::console_output"
OK_MSG    .null "console output emitted"
