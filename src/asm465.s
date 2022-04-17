.cpu _45gs02
.encoding "screencode_mixed"

#import "include/m65macros.s"
#import "include/utils.s"

.const mnemsize = (mn_end - mnemonics) / 6

BasicUpstart65(entry)

    *=$2020

    #import "parser.s"

entry:

    enable40Mhz()
    enableVIC4Registers()
    disableC65ROM()
    disableCIAandIRQ()
    
    // set 80x50
	lda #80
	sta $D058
	lda #80
	sta $D05E
    lda #50
    sta $D07B

    // set 640x400 mode
    lda $d031
    ora #%10001000
    sta $d031

    setBasePage(ZeroPage) 

    sei 

    //TEMP set ParsePC to $0000
    lda #0
    sta ParsePC
    sta ParsePC+1
    //TEMP

    lda #<inputLine
    sta InputLinePtr
    lda #>inputLine
    sta InputLinePtr + 1

    jsr parseLine

    cli
    rts

inputLine:  .text  "label:  adc #10    // abcd"
            .byte $FF

//TODO: write test routines that validate the line above with expected results below
expected:   .byte LT_LBDEF | LT_INST | LT_COMM  // line type
            .byte end_expected - expected       // line length (of tokenised line)
            .word $0000                         // line address (absolute or relative)
            .byte $00                           // start column of label
            .text "label"                       // unresolved label text
            .byte 08                            // start column of instruction
            .byte $61                           // instruction token
            .byte 12                            // start column of operand
            .byte AM_IMM | VP_DEC               // addressing mode | value spec (hex, dec, label, ...)
            .byte $a                            // value
            .byte 19                            // start column of comment
            .text " abcd"                       // comment text including leading space
end_expected: 
            .byte $FF                           


// #import "binary_search.s"
// calc_mnem_addr:
//     // mid * 6 (record size) and use as offset from table start: mnemonics to get next candidate mnemonic row
//         sta bin_search_tmp      // store mid index in 16 bit ' tmp register'
//         lda #0
//         sta bin_search_tmp+1    // mid index is always one byte, so set high byte of tmp to 0

//         asw bin_search_tmp      // multiply by 4
//         asw bin_search_tmp   

//         lda bin_search_tmp      // load low byte of tmp (needed as asw shifts in memory, so a will still be #0)
//         sta BinSearchPtr        // store in low byte of ptr
//         lda bin_search_tmp+1    // load high byte of tmp
//         sta BinSearchPtr+  1    // store in high byte of ptr

//         lda BinSearchIxMid      // load mid again for second multiply (by 2)
//         sta bin_search_tmp      // store in tmp
//         lda #0
//         sta bin_search_tmp+1    // high byte is again 0 (see above)

//         asw bin_search_tmp      // multiply by 2

//         lda bin_search_tmp      // load again into a as asw shits in mem
//         clc
//         adc BinSearchPtr        // add to ptr (result from mult by 4 above)
//         sta BinSearchPtr        // store back in ptr

//         lda bin_search_tmp+1    // load high byte from result of mult by 2
//         adc BinSearchPtr+  1    // add to ptr high byte
//         sta BinSearchPtr+  1    // store back in ptr high byte, now we have 16 bit result of multiply by 6
//         rts


setBasePagePC()
ZeroPage:
        .fill $100, 0
