.cpu "4510"
.enc "screen"

.section main

    #ClearScreen 1
    jsr setLowerCase

    #cprl 5,"Screen functions tests"

    ; set location to 4,2
    ldx #4
    ldy #2
    #SetLocation 4,2

    ; setCPrintC
    lda #'a'
    ldz #2
    jsr setCPrintC

    ; cPrintC
    lda #'b'
    jsr cPrintC

    ; setLocation 2, 10 + putc
    #SetLocation 2, 10
    lda #'c'
    jsr putC

    ; sPrint
    #SetLocation 10,20
    jsr sPrint
    .null "10,10 sPrint, "

    ; sCPrint
    jsr sCPrint
    .byte 4
    .null "sCPrint"

    #SetLocation 10,30

    #pr "10,30 pr, "
    #cpr 3,"3, cpr "
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

    jmp *

str .null "StringPtr"

.endsection ; main
