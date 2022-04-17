#import "include/parser_macros.s"
#import "include/utils.s"

// consts

.const AM_IMP   = 0
.const AM_ACC   = 1
.const AM_IMM   = 2
.const AM_IMW   = 3
.const AM_BP    = 4
.const AM_BPQ   = 5
.const AM_BPX   = 6 
.const AM_ABS   = 7
.const AM_ABX   = 8
.const AM_ABY   = 9
.const AM_REL   = 10
.const AM_RLW   = 11
.const AM_XIN   = 12 // ($nn, X)
.const AM_INY   = 13 // ($nn), Y
.const AM_INZ   = 14 // ($nn), Z
.const AM_IND   = 15
.const AM_BPR   = 16

.const EXT_EA   = %0001
.const EXT_4242 = %0110
.const EXT_4242EA = %0111

.const LT_EMPTY = %0000_0000
.const LT_LBDEF = %0000_0001
.const LT_INST  = %0000_0010
.const LT_LBUSE = %0000_0100
.const LT_DIR   = %0000_1000
.const LT_MCDEF = %0001_0000
.const LT_MCUSE = %0010_0000
.const LT_COMM  = %0100_0000

.const VP_HEX   = 1 << 5
.const VP_DEC   = 2 << 5
.const VP_BIN   = 3 << 5
.const VP_OCT   = 4 << 5
.const VP_EXP   = 5 << 5
.const VP_LAB   = 6 << 5

// base page pointers

.const ParsePos         = $07
.const ParseLineType    = $08
.const ParseLineLen     = $09
.const ParseLabelID0    = $0A
.const ParseLabelID1    = $0B
.const ParseLabelID2    = $0C
.const ParseLabelID3    = $0D
.const ParseLabelID4    = $0E
.const ParseLabelID5    = $0F
.const ParseLabelID6    = $10
.const ParseLabelID7    = $11
.const ParseSize        = $12
.const ParsePC          = $13
.const InputLinePtr     = $15

.const mnemsize = (mn_end - mnemonics) / 6

// Parse the line of code at (BasePage) InputLinePtr
parseLine: {
        ldy #0          // init x to start pos in input
        ldx #4          // start of content of parsed/tokenised line

        lda #0
        sta ParseBuf    // reset the line type byte
        lda #4
        sta ParsePos

    loop:
        lda (InputLinePtr),y
        cmp #$FF
        bne notLineEnd
        jmp endParse       // found end of line
    notLineEnd:
        cmp #' '            // test for space and skip if found
        bne notSpace
        jsr skipWhiteSpace
        jmp loop
    notSpace:
        cmp #'.'
        bne notDirectiveOrMacroDef
        jmp parseDirectiveOrMacroDef
        jmp loop
    notDirectiveOrMacroDef:
        cmp #'!'
        bne notMultiLabel
        jmp parseMultiLabel
    notMultiLabel:
        checkIfAlpha(notAlpha, isAlpha)
    isAlpha:
        jsr parseSymbolOrInstruction
        jmp loop
    notAlpha:
        checkIfComment(InputLinePtr, notComment)
        jmp parseComment
    notComment:
        jmp endParse //TODO: add error handling        
}

endParse:
    lda #'d'            // DEBUG OUTPUT
    sta $0800+10*80,y   // DEBUG OUTPUT
    rts

parseDirectiveOrMacroDef: {
    lda #'p'            // DEBUG OUTPUT
    sta $0800+10*80,y   // DEBUG OUTPUT


    rts
}

// Process a symbol (label, macro use) or instruction
parseSymbolOrInstruction: {
    lda #'s'            // DEBUG OUTPUT
    sta $0800+10*80,y   // DEBUG OUTPUT

    setParsePC(ParsePC)

    // store input line character column
    tya
    sta ParseBuf,x
    inx

    // TODO: check for valid characters

    // first determine if this is a symbol or potential instruction
    loop:
        lda (InputLinePtr),y
        cmp #':'
        bne notColon
        sta ParseBuf,x
        jmp processLabelDef
    notColon:
        cmp #'('
        bne notParen
        jmp parseMacroUse
    notParen:
        cmp #' '
        bne notSpace
        jmp parseInstruction
    notSpace:
        iny
        inx
        jmp loop
    end:
        rts
}

// process the label definition
// only needs to update the line type and skip the colon
processLabelDef: {
        lda #'l'            // DEBUG OUTPUT
        sta $0800+10*80,y   // DEBUG OUTPUT

    // check on line type
        checkLineType(ParseBuf, 0, firstLabelDef)
        rts

    firstLabelDef:
        lda ParseBuf
        ora #LT_LBDEF
        sta ParseBuf

        iny
        inx                 // skip colon from input

        // update start pos
        txa
        sta ParsePos

        rts
}

parseInstruction: {
        lda #'i'            // DEBUG OUTPUT
        sta $0800+10*80,y   // DEBUG OUTPUT
        lda (InputLinePtr),y

    // check on line type
        checkLineType(ParseBuf, LT_LBDEF, firstInstruction)
        rts

    firstInstruction:
        dex                 // x points to the ' ' after the candidate instruction so decrease

        // check if longer than 4 chars (can't be an instruction)
        txa
        sec
        sbc ParsePos
        cmp #5
        bcs tooLong
        // do binary search to find instruction

        //TODO: use binary search to find instruction (or not)

        iny
        inx                 // skip colon from input

        // update start pos
        txa
        sta ParsePos

        // update line type with instruction flag
        lda ParseBuf
        ora #LT_INST
        sta ParseBuf

        rts
    tooLong:
        lda #'e'            // DEBUG OUTPUT
        sta $0800+10*80,y   // DEBUG OUTPUT
        rts
}

parseMacroUse: {
    lda #'u'            // DEBUG OUTPUT
    sta $0800+10*80,y   // DEBUG OUTPUT
    lda (InputLinePtr),y

    iny
    rts
}

parseMultiLabel: {
    lda #'m'            // DEBUG OUTPUT
    sta $0800+10*80,y   // DEBUG OUTPUT
    lda (InputLinePtr),y


    rts
}

parseComment: {
        lda #'c'            // DEBUG OUTPUT
        sta $0800+10*80,y   // DEBUG OUTPUT

        lda ParseBuf        // load line type byte
        bne notOnlyComment

        // we're responsibble for setting the current PC
        setParsePC(ParsePC)

    notOnlyComment:
        ora #LT_COMM
        sta ParseBuf

        tya
        sta ParseBuf,x      // store position of comment
        iny                 // skip size
        inx                 // skip second '/'

    !:
        lda (InputLinePtr),y
        cmp #$FF
        beq end             // found end of line

        sta $0800+11*80,y   // DEBUG OUTPUT

        sta ParseBuf,x
        inx
        iny
        cpy #80
        bne !-
    end:
        // we need to set the length of the line
        txa                 // load last ParseBuf pos
        sta ParseBuf + 1
        rts
}


// x: character column, increments x to first non-space character column
skipWhiteSpace: {
    loop:
        iny
        lda (InputLinePtr),y
        cmp #' '
        beq loop
        rts
}

ParseBuf: .fill $FF, 0

/*
Design sketches:

do_inst                     // read operand and determine addressing mode
    read char
    if char == '#'          // immediate mode
        check for allowed mode
        inc ParsePos
        do immediate
    if char == '$'
        check for allowed mode
        inc ParsePos
        do absolute
    if char == '('
        check for allowed mode
        inc ParsePos
        do indirect
    if char == 'a' || char == 'A'
        check for allowed mode
        inc ParsePos
        do accumulator
    
    ....

    else
        do implicit
    

do_immediate
    read char
    if char == "<" || char == '>'
        if byte_select > 0
            error
        set 'byte select'
        loop
    if char == '$'
        do hexvalue
    if char == '%'
        do binvalue
    if char == '&'
        do octvalue
    if char == '('
        do expression
    if char == '0-9'
        do decimal
        
    if char is alpha
        do label

    if char == ' '
        process whitespace
    if char == '/'
        read char
        if char == '/'
            do comment
        dec ParsePos    // get back to first '/'
    
    do expression           // first implmenet simple addition, subtraction
    


do_hexvalue

*/

//label token: label_id, parent_id, (parent_id)* (0 if no parent): a $00 at the first label byte means the following $ff terminated string is the unresolved label name 
//line_type flags: opcode, label def, label use, directive, macro def, macro use, comment
//valuespec: bin, hex, dec, oct, expr, label (0, 1, 2, 3, 4, 5) (bit 7-5)

//ex text:                            |ab:  adc #10    // abcd
            //    linetype    length, addr     pos, label decl          pos, opt, pos, addrm,  oper, pos, comment
//example1:  .byte %00000011, $12    ,$00,$21, $00, $01, $00,         , $05, $61, $09, AM_IMM, $a,   $10, $20, $01, $02, $03, $04
//example2:  .byte %00000011, $14    ,$00,$21, $00, $00, $01, $02, $ff, $05, $61, $09, AM_IMM, $a,   $10, $20, $01, $02, $03, $04

// the first opcode for each mnemonic is the token for the editor, combined with the id of the specific addressing mode

datasize:   .word lookup_end-mnemonics
tokensize:  .word lookup_end-tok_to_mnem
addrmsize:  .word tok_to_mnem-addrm_groups

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
gr01:       .byte AM_XIN, $00, $50
            .byte AM_BP,  $04, $30 
            .byte AM_IMM, $04, $20
            .byte AM_ABS, $04, $40
            .byte AM_INY, $04, $50
            .byte AM_INZ, $01, $50
            .byte AM_BPX, $03, $30
            .byte AM_ABY, $04, $40
            .byte AM_ABX, $04, $40
gr02:       .byte AM_BP,  $00, $40
            .byte AM_ACC, $04, $10
            .byte AM_ABS, $04, $50
            .byte AM_BPX, $08, $40
            .byte AM_ABX, $08, $40
gr03:       .byte AM_ACC, $00, $10
            .byte AM_BP,  $01, $40
            .byte AM_BPX, $0A, $40
gr04:       .byte AM_ABS, $00, $00
gr05:       .byte AM_BPR, $00, $50
gr06:       .byte AM_REL, $00, $20
            .byte AM_RLW, $03, $30
gr07:       .byte AM_BP,  $00, $40
            .byte AM_ABS, $04, $50
            .byte AM_BPX, $08, $40
            .byte AM_ABX, $08, $40
            .byte AM_IMM, $4A, $00
gr08:       .byte AM_IMP, $00, $70
gr09:       .byte AM_RLW, $00, $30
gr0A:       .byte AM_IMM, $00, $00
            .byte AM_BP,  $04, $00
            .byte AM_ABS, $08, $00
gr0B:       .byte AM_ACC, $00, $10
            .byte AM_BP,  $8C, $00
            .byte AM_ABS, $08, $00
            .byte AM_BPX, $08, $00
            .byte AM_ABX, $08, $00
gr0C:       .byte AM_BP,  $00, $00
gr0D:       .byte AM_ABS, $00, $30
            .byte AM_IND, $20, $50
            .byte AM_XIN, $10, $00
gr0E:       .byte AM_IMM, $00, $00
            .byte AM_BP,  $04, $00
            .byte AM_ABS, $08, $00
            .byte AM_BPX, $08, $00
            .byte AM_ABX, $08, $00
gr0F:       .byte AM_IMM, $00, $00
            .byte AM_BP,  $08, $00
            .byte AM_ABS, $10, $00
gr10:       .byte AM_ACC, $00, $10
gr11:       .byte AM_IMW, $00, $00
            .byte AM_ABS, $08, $00

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