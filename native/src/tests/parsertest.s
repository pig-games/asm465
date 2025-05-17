.cpu "4510"
.enc "screen"

.section main
    .dbg.setupDebugStats
    .with parser
    
    .ClearScreen 1
    jsr setLowerCase

    .dbg.info "Parser functions tests!n!n"
    .setParsePC $0000
    .setInputLine inputLine

    .cpr 4,"Testing: ["
    .ldxy inputLine
    ldz #1
    jsr cPrint
    .cpr 4, "]!n"

    .dbg.setFilter "searchInstruction", "firstInstruction", "SIInOut"
    jsr parseLine

    .nl
    .dbg.info "end of parse!n"

    .nl
    .nl
    .dbg.Stats

    jmp *
    .endwith
.endsection ; main

.section data
.align
inputLine   .text  "label:  and    ";"#10    ; abcd"
            .byte $FF, 0

;TODO: write test routines that validate the line above with expected results below
.namespace parser
expected    .byte LT_LBDEF | LT_INST | LT_COMM  ; line type
            .byte end_expected - expected       ; line length (of tokenised line)
            .word $0000                         ; line address (absolute or relative)
            .byte $00                           ; start column of label
            .text "label"                       ; unresolved label text
            .byte 08                            ; start column of instruction
            .byte $61                           ; instruction token
            .byte 12                            ; start column of operand
            .byte AM_IMM | VD_DEC               ; addressing mode | value spec (hex, dec, label, ...)
            .byte $a                            ; value
            .byte 19                            ; start column of comment
            .text " abcd"                       ; comment text including leading space
end_expected 
            .byte $FF

.endnamespace ; parser
.endsection ; data

