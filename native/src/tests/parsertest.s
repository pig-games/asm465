.cpu "4510"

.section main
.namespace parser

    #ClearScreen 1
    #setParsePC $0000
    #setInputLine inputLine

    jsr parseLine

    cli
    rts

.endnamespace ; parser
.endsection ; main

.section data

inputLine   .text  "label:  adc #10    ; abcd"
            .byte $FF

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
            .byte AM_IMM | VP_DEC               ; addressing mode | value spec (hex, dec, label, ...)
            .byte $a                            ; value
            .byte 19                            ; start column of comment
            .text " abcd"                       ; comment text including leading space
end_expected 
            .byte $FF

.endnamespace ; parser
.endsection ; data

