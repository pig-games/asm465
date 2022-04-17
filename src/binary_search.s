#import "include/utils.s"

//TODO: refactor into reusable routine. Does not work currently.

// base page pointers

.const BinSearchIxL     = $02
.const BinSearchIxR     = $03
.const BinSearchIxMid   = $04
.const BinSearchPtr     = $05

// binary search, first set mid-point

        setJSRAddress(bin_search.addr_calc, calc_mnem_addr)

//TODO: refactor into separate routine
bin_search_init:
        // set left and right index
        lda #0
        sta BinSearchIxL
        lda #mnemsize - 1
        sta BinSearchIxR     
        // last row of table

 bin_search: {
        // if new index BinSearchIxMid   >= ixl we're done

        lda BinSearchIxR
    
        cmp BinSearchIxL        // compare ix r to l
        bcs not_end             // if (r >= l) we search on
        jmp end_parse
    not_end:
        sec
        sbc BinSearchIxL        // r - l
        lsr                     // (r - l) / 2
        clc
        adc BinSearchIxL        // l + (r - l) / 2
        sta BinSearchIxMid      // mid = l + (r - l) / 2

    addr_calc:
        jsr $0000               // needs to be code modified to the address of the address calc routine, use: setJSRAddress

    // add start address of mnemonics table to (mid * 6) 
        lda BinSearchPtr        // load low byte of ptr
        clc
        adc #<mnemonics         // add low byte of mnemonics table start
        sta BinSearchPtr        // store back to low byte of ptr
        lda BinSearchPtr+  1    // load high byte of ptr
        adc #>mnemonics         // add high byte of mnemonics table start 
        sta BinSearchPtr+  1    // store back to high byte of ptr
}

// start comparing current input with candidate mnemonic
compare_loop: { 
        // DEBUG OUTPUT
        lda (BinSearchPtr)  ,y  // print current candidate character
        sta $0800+11*80,x
        // END DEBUG OUTPUT

        lda inputLine,x        // get character from input
        sta $0800+10*80,x       // print it
        cmp #$FF
        beq end_parse           // found end of line
        cmp #' '
        bne no_space
        jmp found
    no_space:
        // sta label_buf,x    // store possible label char in label_buf, if turns out an instruction we'll $FF the first char
        // scan for mnemonic

        cmp (BinSearchPtr)  ,y  // compare current candidate mnemonic char with input char
        bcc is_lower            // if input is lower than candidate
        beq is_equal            // if input is equal to candidate
    is_higher:                  // else is higher
        lda BinSearchIxMid
          
        inc

        sta BinSearchIxL
          ldx #0
        ldy #0
        jmp bin_search
    is_lower:
        lda BinSearchIxMid
  
        dec

        sta BinSearchIxR
    
      ldx #0
        ldy #0
        jmp bin_search
    is_equal:

        lda (BinSearchPtr)  ,y
        beq found               // should be 'found'
        cpy #3
        beq found               // should be 'found'

        inx                     // input char index to next character
        iny                     // candidate char index to next character

        jmp compare_loop        // go compare next characters
        //TODO: write FF to label_buf,0 when found mnemonic instead of label
        jmp end_parse
}
    end_parse:
        rts

found:
        lda #0
        sta $0800+20*80
        lda inputLine,x
        cmp #' '
        bne found_no_space
        inx
        ldy #4
        lda (BinSearchPtr)  ,y
        sta instruction
        sta $0801+20*80
        jmp endParse
found_no_space:
        ldy #4
        lda (BinSearchPtr)  ,y
        sta instruction
        sta $0801+20*80
        rts

// temporary current line buffer for parsed line
instruction: .byte $00
addrmode:    .byte $00
bin_search_tmp: .word 0
