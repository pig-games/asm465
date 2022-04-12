.cpu _45gs02
.encoding "screencode_mixed"

#import "./include/m65macros.s"
#import "./include/utils.s"

// consts

.const am_imp   = 0
.const am_acc   = 1
.const am_imm   = 2
.const am_imw   = 3
.const am_bp    = 4
.const am_bpq   = 5
.const am_bpx   = 6 
.const am_abs   = 7
.const am_abx   = 8
.const am_aby   = 9
.const am_rel   = 10
.const am_rlw   = 11
.const am_xin   = 12 // ($nn, X)
.const am_iny   = 13 // ($nn), Y
.const am_inz   = 14 // ($nn), Z
.const am_ind   = 15
.const am_bpr   = 16

.const ext_ea   = %0001
.const ext_4242 = %0110
.const ext_4242ea = %0111

// base page pointers

.const bin_search_ixl = $02
.const bin_search_ixr = $03
.const bin_search_ixmid = $04
.const bin_search_ptr = $05

.const mnemsize = (mn_end - mnemonics) / 6

BasicUpstart65(entry)

    *=$2020
entry:

    enable40Mhz()
    enableVIC4Registers()
    disableC65ROM()

    sei

	// Disable CIA and IRQ interrupts
	lda #$7f
	sta $DC0D 
	sta $DD0D 

	lda #$00
	sta $D01A

    lda #$70
	sta $D640
    nop

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

    lda #$26
    tab

    jmp parse
    cli
    rts

parse:
// initialise binary search vars


        // init x to start pos in input
        ldx #0

parse_segment:
        ldy #0              // reset y to start pos in candidate mnemonic

parse_loop: {
        lda input_line,x
        cmp #$FF
        bne not_end
        jmp end_parse       // found end of line
    not_end:
        cmp #' '            // test for space and skip if found
        bne no_space
        inx
        jmp parse_loop
    no_space:
        cmp #';'
        bne not_comment
        jsr get_comment
        jmp end_parse
    not_comment:
}

// binary search, first set mid-point

        setJSRAddress(bin_search.addr_calc, calc_mnem_addr)

bin_search_init:
        // set left and right index
        lda #0
        sta bin_search_ixl
        lda #mnemsize - 1
        sta bin_search_ixr       // last row of table

 bin_search: {
        // if new index bin_search_ixmid >= ixl we're done

        lda bin_search_ixr
        cmp bin_search_ixl      // compare ix r to l
        bcs not_end             // if (r >= l) we search on
        jmp end_parse
    not_end:
        sec
        sbc bin_search_ixl      // r - l
        lsr                     // (r - l) / 2
        clc
        adc bin_search_ixl      // l + (r - l) / 2
        sta bin_search_ixmid    // mid = l + (r - l) / 2

    addr_calc:
        jsr $0000               // needs to be code modified to the address of the address calc routine, use: setJSRAddress

    // add start address of mnemonics table to (mid * 6) 
        lda bin_search_ptr      // load low byte of ptr
        clc
        adc #<mnemonics         // add low byte of mnemonics table start
        sta bin_search_ptr      // store back to low byte of ptr
        lda bin_search_ptr+1    // load high byte of ptr
        adc #>mnemonics         // add high byte of mnemonics table start 
        sta bin_search_ptr+1    // store back to high byte of ptr
}

// start comparing current input with candidate mnemonic
compare_loop: { 
        // DEBUG OUTPUT
        lda (bin_search_ptr),y  // print current candidate character
        sta $0800+11*80,x
        // END DEBUG OUTPUT

        lda input_line,x        // get character from input
        sta $0800+10*80,x       // print it
        cmp #$FF
        beq end_parse           // found end of line
        cmp #' '
        bne no_space
        jmp found
    no_space:
        // sta label_buf,x    // store possible label char in label_buf, if turns out an instruction we'll $FF the first char
        // scan for mnemonic

        cmp (bin_search_ptr),y  // compare current candidate mnemonic char with input char
        bcc is_lower            // if input is lower than candidate
        beq is_equal            // if input is equal to candidate
    is_higher:                  // else is higher
        lda bin_search_ixmid
        
        inc

        sta bin_search_ixl
        ldx #0
        ldy #0
        jmp bin_search
    is_lower:
        lda bin_search_ixmid

        dec

        sta bin_search_ixr
        ldx #0
        ldy #0
        jmp bin_search
    is_equal:

        lda (bin_search_ptr),y
        beq found               // should be 'found'
        cpy #3
        beq found               // should be 'found'

        inx                     // input char index to next character
        iny                     // candidate char index to next character

        jmp compare_loop        // go compare next characters
        //TODO: write FF to label_buf,0 when found mnemonic instead of label
        jmp parse_loop
}
    end_parse:
        rts

found:
        lda #0
        sta $0800+20*80
        lda input_line,x
        cmp #' '
        bne found_no_space
        inx
        ldy #4
        lda (bin_search_ptr),y
        sta instruction
        sta $0801+20*80
        jmp parse_segment
found_no_space:
        ldy #4
        lda (bin_search_ptr),y
        sta instruction
        sta $0801+20*80
        rts

get_comment: {
        inx                 // skip ';'
    !:
        lda input_line,x
        cmp #$FF
        beq end     // found end of line
        sta comment_buf,y
        inx
        iny
        cpy #80
        bne !-
    end:
        rts
}

calc_mnem_addr:
    // mid * 6 (record size) and use as offset from table start: mnemonics to get next candidate mnemonic row
        sta bin_search_tmp      // store mid index in 16 bit ' tmp register'
        lda #0
        sta bin_search_tmp+1    // mid index is always one byte, so set high byte of tmp to 0

        asw bin_search_tmp      // multiply by 4
        asw bin_search_tmp   

        lda bin_search_tmp      // load low byte of tmp (needed as asw shifts in memory, so a will still be #0)
        sta bin_search_ptr      // store in low byte of ptr
        lda bin_search_tmp+1    // load high byte of tmp
        sta bin_search_ptr+1    // store in high byte of ptr

        lda bin_search_ixmid    // load mid again for second multiply (by 2)
        sta bin_search_tmp      // store in tmp
        lda #0
        sta bin_search_tmp+1    // high byte is again 0 (see above)

        asw bin_search_tmp      // multiply by 2

        lda bin_search_tmp      // load again into a as asw shits in mem
        clc
        adc bin_search_ptr      // add to ptr (result from mult by 4 above)
        sta bin_search_ptr      // store back in ptr

        lda bin_search_tmp+1    // load high byte from result of mult by 2
        adc bin_search_ptr+1    // add to ptr high byte
        sta bin_search_ptr+1    // store back in ptr high byte, now we have 16 bit result of multiply by 6
        rts

// temporary current line buffer for parsed line
white_l:     .byte $00
white_i:     .byte $00
white_lo:    .byte $00
white_ro:    .byte $00
white_c:     .byte $00
label_buf:   .fill 16, $FF
instruction: .byte $00
addrmode:    .byte $00
comment_buf: .fill 81, $FF
 
input_line: .text  "clv  ; comment@" 
            .byte $FF

// the first opcode for each mnemonic is the token for the editor, combined with the id of the specific addressing mode

datasize:   .word lookup_end-mnemonics
tokensize:  .word lookup_end-tok_to_mnem
addrmsize:  .word tok_to_mnem-addrm_groups
bin_search_tmp: .word 0

mnemonics:
mn_adc:     .text "adc@"
            .byte $61, gr01-addrm_groups
mn_and:     .text "and@" 
            .byte $21, gr01-addrm_groups
mn_asl:     .text "asl@"
            .byte $06, gr02-addrm_groups
mn_asr:     .text "asr@"
            .byte $43, gr03-addrm_groups
mn_asw:     .text "asw@"
            .byte $CB, gr04-addrm_groups
mn_br0:     .text "bbr0"
            .byte $0F, gr05-addrm_groups
mn_br1:     .text "bbr1"
            .byte $1F, gr05-addrm_groups
mn_br2:     .text "bbr2"
            .byte $2F, gr05-addrm_groups
mn_br3:     .text "bbr3"
            .byte $3F, gr05-addrm_groups
mn_br4:     .text "bbr4"
            .byte $4F, gr05-addrm_groups
mn_br5:     .text "bbr5"
            .byte $5F, gr05-addrm_groups
mn_br6:     .text "bbr6"
            .byte $6F, gr05-addrm_groups
mn_br7:     .text "bbr7"
            .byte $7F, gr05-addrm_groups
mn_bs0:     .text "bbs0"
            .byte $8F, gr05-addrm_groups
mn_bs1:     .text "bbs1"
            .byte $9F, gr05-addrm_groups
mn_bs2:     .text "bbs2"
            .byte $AF, gr05-addrm_groups
mn_bs3:     .text "bbs3"
            .byte $BF, gr05-addrm_groups
mn_bs4:     .text "bbs4"
            .byte $CF, gr05-addrm_groups
mn_bs5:     .text "bbs5"
            .byte $DF, gr05-addrm_groups
mn_bs6:     .text "bbs6"
            .byte $EF, gr05-addrm_groups
mn_bs7:     .text "bbs7"
            .byte $FF, gr05-addrm_groups
mn_bcc:     .text "bcc@"
            .byte $90, gr06-addrm_groups
mn_bcs:     .text "bcs@"
            .byte $B0, gr06-addrm_groups
mn_beq:     .text "beq@"
            .byte $F0, gr06-addrm_groups
mn_bit:     .text "bit@"
            .byte $24, gr07-addrm_groups
mn_bmi:     .text "bmi@"
            .byte $30, gr06-addrm_groups
mn_bne:     .text "bne@"
            .byte $D0, gr06-addrm_groups
mn_bpl:     .text "bpl@"
            .byte $10, gr06-addrm_groups
mn_bra:     .text "bra@"
            .byte $80, gr06-addrm_groups
mn_brk:     .text "brk@"
            .byte $00, gr08-addrm_groups
mn_bsr:     .text "bsr@"
            .byte $65, gr09-addrm_groups
mn_bvc:     .text "bvc@"
            .byte $50, gr06-addrm_groups
mn_bvs:     .text "bvs@"
            .byte $70, gr06-addrm_groups
mn_clc:     .text "clc@"
            .byte $18, gr08-addrm_groups
mn_cld:     .text "cld@"
            .byte $D8, gr08-addrm_groups
mn_cle:     .text "cle@"
            .byte $02, gr08-addrm_groups
mn_cli:     .text "cli@"
            .byte $58, gr08-addrm_groups
mn_clv:     .text "clv@"
            .byte $B8, gr08-addrm_groups
mn_cmp:     .text "cmp@"
            .byte $C1, gr01-addrm_groups
mn_cpx:     .text "cpx@"
            .byte $E0, gr0A-addrm_groups
mn_cpy:     .text "cpy@"
            .byte $C0, gr0A-addrm_groups
mn_cpz:     .text "cpz@"
            .byte $C2, gr0A-addrm_groups
mn_dec:     .text "dec@"
            .byte $3A, gr0B-addrm_groups
mn_dew:     .text "dew@"
            .byte $C3, gr0C-addrm_groups
mn_dex:     .text "dex@"
            .byte $CA, gr08-addrm_groups
mn_dey:     .text "dey@"
            .byte $88, gr08-addrm_groups
mn_dez:     .text "dez@"
            .byte $3B, gr08-addrm_groups
mn_eom:     .text "dez@"
            .byte $EA, gr08-addrm_groups
mn_eor:     .text "eor@"
            .byte $41, gr01-addrm_groups
mn_inc:     .text "inc@"
            .byte $1A, gr0B-addrm_groups
mn_inw:     .text "inw@"
            .byte $E3, gr0C-addrm_groups
mn_inx:     .text "inx@"
            .byte $E8, gr08-addrm_groups
mn_iny:     .text "iny@"
            .byte $C8, gr08-addrm_groups
mn_inz:     .text "inz@"
            .byte $1B, gr08-addrm_groups
mn_jmp:     .text "jmp@"
            .byte $4C, gr0D-addrm_groups
mn_jsr:     .text "jsr@"
            .byte $20, gr0D-addrm_groups
mn_lda:     .text "lda@"
            .byte $A1, gr01-addrm_groups
mn_ldx:     .text "ldx@"
            .byte $A2, gr0E-addrm_groups
mn_ldy:     .text "ldy@"
            .byte $A0, gr0E-addrm_groups
mn_ldz:     .text "ldz@"
            .byte $A3, gr0F-addrm_groups
mn_lsr:     .text "lsr@"
            .byte $46, gr02-addrm_groups
mn_map:     .text "map@"
            .byte $5C, gr08-addrm_groups
mn_neg:     .text "neg@"
            .byte $42, gr10-addrm_groups
mn_ora:     .text "ora@"
            .byte $01, gr01-addrm_groups
mn_pha:     .text "pha@"
            .byte $48, gr08-addrm_groups
mn_php:     .text "php@"
            .byte $08, gr08-addrm_groups
mn_phw:     .text "phw@"
            .byte $F4, gr11-addrm_groups
mn_phx:     .text "phx@"
            .byte $DA, gr08-addrm_groups
mn_phy:     .text "phy@"
            .byte $5A, gr08-addrm_groups
mn_phz:     .text "phz@"
            .byte $DB, gr08-addrm_groups
mn_pla:     .text "pla@"
            .byte $68, gr08-addrm_groups
mn_plp:     .text "plp@"
            .byte $28, gr08-addrm_groups
mn_plx:     .text "plx@"
            .byte $FA, gr08-addrm_groups
mn_ply:     .text "ply@"
            .byte $7A, gr08-addrm_groups 
mn_plz:     .text "plz@"
            .byte $FB, gr08-addrm_groups

mn_end:

addrm_groups:
gr01:       .byte am_xin, $00, $50
            .byte am_bp,  $04, $30 
            .byte am_imm, $04, $20
            .byte am_abs, $04, $40
            .byte am_iny, $04, $50
            .byte am_inz, $01, $50
            .byte am_bpx, $03, $30
            .byte am_aby, $04, $40
            .byte am_abx, $04, $40
gr02:       .byte am_bp,  $00, $40
            .byte am_acc, $04, $10
            .byte am_abs, $04, $50
            .byte am_bpx, $08, $40
            .byte am_abx, $08, $40
gr03:       .byte am_acc, $00, $10
            .byte am_bp,  $01, $40
            .byte am_bpx, $0A, $40
gr04:       .byte am_abs, $00, $00
gr05:       .byte am_bpr, $00, $50
gr06:       .byte am_rel, $00, $20
            .byte am_rlw, $03, $30
gr07:       .byte am_bp,  $00, $40
            .byte am_abs, $04, $50
            .byte am_bpx, $08, $40
            .byte am_abx, $08, $40
            .byte am_imm, $4A, $00
gr08:       .byte am_imp, $00, $70
gr09:       .byte am_rlw, $00, $30
gr0A:       .byte am_imm, $00, $00
            .byte am_bp,  $04, $00
            .byte am_abs, $08, $00
gr0B:       .byte am_acc, $00, $10
            .byte am_bp,  $8C, $00
            .byte am_abs, $08, $00
            .byte am_bpx, $08, $00
            .byte am_abx, $08, $00
gr0C:       .byte am_bp,  $00, $00
gr0D:       .byte am_abs, $00, $30
            .byte am_ind, $20, $50
            .byte am_xin, $10, $00
gr0E:       .byte am_imm, $00, $00
            .byte am_bp,  $04, $00
            .byte am_abs, $08, $00
            .byte am_bpx, $08, $00
            .byte am_abx, $08, $00
gr0F:       .byte am_imm, $00, $00
            .byte am_bp,  $08, $00
            .byte am_abs, $10, $00
gr10:       .byte am_acc, $00, $10
gr11:       .byte am_imw, $00, $00
            .byte am_abs, $08, $00

tok_to_mnem:
            .byte $00, <mn_brk, >mn_brk
            .byte $02, <mn_cle, >mn_cle
            .byte $06, <mn_asl, >mn_asl
            .byte $0F, <mn_br0, >mn_br0
            .byte $10, <mn_bpl, >mn_bpl
            .byte $18, <mn_clc, >mn_clc
            .byte $1F, <mn_br1, >mn_br1
            .byte $21, <mn_and, >mn_and
            .byte $24, <mn_bit, >mn_bit
            .byte $2F, <mn_br2, >mn_br2
            .byte $30, <mn_bmi, >mn_bmi
            .byte $3A, <mn_dec, >mn_dec
            .byte $3F, <mn_br3, >mn_br3
            .byte $43, <mn_asr, >mn_asr
            .byte $4F, <mn_br4, >mn_br4
            .byte $50, <mn_bvc, >mn_bvc
            .byte $58, <mn_cli, >mn_cli
            .byte $5F, <mn_br5, >mn_br5
            .byte $61, <mn_adc, >mn_adc
            .byte $65, <mn_bsr, >mn_bsr
            .byte $6F, <mn_br6, >mn_br6
            .byte $70, <mn_bvs, >mn_bvs
            .byte $7F, <mn_br7, >mn_br7
            .byte $80, <mn_bra, >mn_bra
            .byte $8F, <mn_bs0, >mn_bs0
            .byte $90, <mn_bcc, >mn_bcc
            .byte $9F, <mn_bs1, >mn_bs1
            .byte $AF, <mn_bs2, >mn_bs2
            .byte $B0, <mn_bcs, >mn_bcs
            .byte $B8, <mn_clv, >mn_clv
            .byte $BF, <mn_bs3, >mn_bs3
            .byte $C0, <mn_cpy, >mn_cpy
            .byte $C1, <mn_cmp, >mn_cmp
            .byte $C2, <mn_cpz, >mn_cpz
            .byte $C3, <mn_dew, >mn_dew
            .byte $CA, <mn_dex, >mn_dex
            .byte $CB, <mn_asw, >mn_asw
            .byte $CF, <mn_bs4, >mn_bs4
            .byte $D0, <mn_bne, >mn_bne
            .byte $D8, <mn_cld, >mn_cld
            .byte $DF, <mn_bs5, >mn_bs5
            .byte $E0, <mn_cpx, >mn_cpx
            .byte $EF, <mn_bs6, >mn_bs6
            .byte $F0, <mn_beq, >mn_beq
            .byte $FF, <mn_bs7, >mn_bs7
            .byte $88, <mn_dey, >mn_dey
            .byte $3B, <mn_dez, >mn_dez
            .byte $EA, <mn_eom, >mn_eom
            .byte $41, <mn_eor, >mn_eor
            .byte $1A, <mn_inc, >mn_inc
            .byte $E3, <mn_inw, >mn_inw
            .byte $E8, <mn_inx, >mn_inx
            .byte $C8, <mn_iny, >mn_iny
            .byte $1B, <mn_inz, >mn_inz
            .byte $4C, <mn_jmp, >mn_jmp
            .byte $20, <mn_jsr, >mn_jsr
            .byte $A1, <mn_lda, >mn_lda
            .byte $A2, <mn_ldx, >mn_ldx
            .byte $A0, <mn_ldy, >mn_ldy
            .byte $A3, <mn_ldz, >mn_ldz
            .byte $46, <mn_lsr, >mn_lsr
            .byte $5C, <mn_map, >mn_map
            .byte $42, <mn_neg, >mn_neg
            .byte $01, <mn_ora, >mn_ora
            .byte $48, <mn_pha, >mn_pha
            .byte $08, <mn_php, >mn_php
            .byte $F4, <mn_phw, >mn_phw
            .byte $DA, <mn_phx, >mn_phx
            .byte $5A, <mn_phy, >mn_phy
            .byte $DB, <mn_phz, >mn_phz
            .byte $68, <mn_pla, >mn_pla
            .byte $28, <mn_plp, >mn_plp
            .byte $FA, <mn_plx, >mn_plx
            .byte $7A, <mn_ply, >mn_ply
            // !byte   $FB, <mn_plz, >mn_plz

lookup_end: