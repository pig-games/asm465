.include "debug_macros.h"

.enc "screen"

.section main
    .dbg.setupDebugStats

    .ClearScreen 1
    jsr setLowerCase

    .SetBGBColors 11, 0 
 
    .cpr 3, "Screen functions tests!n"

    ; set location to 4,2
    .SetLocation 4,2

    ; setCPrintC
    lda #'a'
    ldx #4
    jsr setCPrintC

    ; cPrintC
    lda #'b'
    jsr printC

    ; setLocation 2, 10 + putc
    .SetLocation 2, 5
    lda #'c'
    jsr putC

    ; sPrint
    .SetLocation 10,4
    jsr sPrint
    .null "10,4 sPrint, "

    ; sCPrint
    jsr sCPrint
    .byte 7
    .null "7 sCPrint"

    .SetLocation 10,8
    .pr "10,8 prl,!n"

    .cpr 3 | CA_BLINK | CA_REV,"3, cpr "
    .pr "pr!n"
    .pr "newline"
    .cpr 5,"5, cpr!n"
    .pr "newline"
    .nl
    .pr "newline!n"

    .ldxy teststr
    jsr print

    .nl
    .ldxy teststr
    lda #1
    jsr cPrint
    .nl

    .cpr 3, "before proc!n"
    jsr inside

    .pr "outside proc again!n"
    .dbg.warning "test warning1!n"
    .dbg.warning "test warning2!n"

    .dbg.error "test error!n"

    .dbg.Stats
    jmp *

inside .proc
    .pr "test inside proc!n"
    .pr "more inside proc!n"
    rts
.endproc

teststr .null "StringPtr"

.endsection ; main
