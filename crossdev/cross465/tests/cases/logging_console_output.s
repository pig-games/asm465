rtst.BASE = $C000
TARGET_CROSS465 :?= 0
TARGET_ASM465_NATIVE :?= 0
TARGET_ASM465_WASM :?= 0
TARGET_ULTIMATE64 :?= 0

.if TARGET_ULTIMATE64
.include "platformmacros.h"
* = $0801
    BasicUpstart(start)
.endif

* = $2000
    jmp start
.include "test_rtst.h"
.include "platformdefs.h"
.include "screen_macros.h"

printConsoleMessage .proc
.if TARGET_CROSS465 | TARGET_ASM465_NATIVE | TARGET_ASM465_WASM
    .SetBColor $0E
    .SetBGColor $06
.endif
    ldy #0
write_loop
    lda LOG_TEXT, y
    beq done
.if TARGET_CROSS465 | TARGET_ASM465_NATIVE | TARGET_ASM465_WASM
    .PutC
.else
    .if TARGET_ULTIMATE64
        jsr CHROUT
    .endif
.endif
    iny
    bne write_loop
done
.if TARGET_CROSS465 | TARGET_ASM465_NATIVE | TARGET_ASM465_WASM
    lda #0
    sta cross465.console.NL
.else
    .if TARGET_ULTIMATE64
        lda #$0D
        jsr CHROUT
    .endif
.endif
    rts
.endproc

start
    sei
    cld
    .rtst.begin
    .ctest.begin CASE_NAME
.if TARGET_CROSS465 | TARGET_ASM465_NATIVE | TARGET_ASM465_WASM | TARGET_ULTIMATE64
    jsr printConsoleMessage
.endif
    .ctest.OK 0, OK_MSG
    .rtst.end

CASE_NAME .null "logging::console_output"
OK_MSG    .null "console output emitted"
LOG_TEXT  .null "CONSOLE LOGGING FROM ASM"
