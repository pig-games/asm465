.cpu "4510"
.enc "screen"

.section main

    #ClearScreen 1
    jsr setLowerCase

    #SetBGBColors 11, 0 
 
    #cprl 3, "Screen functions tests"
    
    ; set location to 4,2
    #SetLocation 4,2

    ; setCPrintC
    lda #'a'
    ldz #4
    jsr setCPrintC

    ; cPrintC
    lda #'b'
    jsr printC

    ; setLocation 2, 10 + putc
    #SetLocation 2, 5
    lda #'c'
    jsr putC
    
    ; sPrint
    #SetLocation 10,4
    jsr sPrint
    .null "10,4 sPrint, "

    ; sCPrint
    jsr sCPrint
    .byte 7
    .null "7 sCPrint"

    #SetLocation 10,30
    #prl "10,30 prl,"
    #cpr 3 | CA_BLINK | CA_REV,"3, cpr "
    #prl "prl"
    #pr "newline"
    #cprl 5,"5, cprl"
    #pr "newline"
    #nl
    #pr "newline"

    #nl
    #ldxy str
    jsr print

    #nl
    #ldxy str
    ldz #1
    jsr cPrint
    #nl

    #cprl 3, "before proc"
    jsr inside

    #prl "outside proc again"

    jmp *

inside .proc
    #prl "test inside proc"
    #prl "more inside proc"
    rts
.endproc
str .null "StringPtr"

.endsection ; main
