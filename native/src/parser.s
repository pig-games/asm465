.enc "screen"

parser .namespace
; constants

    AM_IMP   = 00
    AM_ACC   = 01   ; A
    AM_QPR   = 02   ; Q
    AM_IMM   = 03   ; #$nn
    AM_IMW   = 04   ; #$nnnn
    AM_BP    = 05   ; $nn
    AM_BPQ   = 06   ; $nn
    AM_BPX   = 07   ; $nn,x
    AM_BQX   = 08   ; $nn,x
    AM_BPY   = 09   ; $nn,y
    AM_ABS   = 10   ; $nnnn
    AM_ABQ   = 11   ; $nnnn
    AM_ABX   = 12   ; $nnnn,x
    AM_ABQX  = 13   ; $nnnn,x
    AM_ABY   = 14   ; $nnnn,y
    AM_AIN   = 15   ; ($nnnn)
    AM_AIX   = 16   ; ($nnnn,x)
    AM_BIX   = 17   ; ($nn,x)
    AM_BIY   = 18   ; ($nn),y
    AM_BIZ   = 19   ; ($nn),z
    AM_BQIZ  = 20   ; ($nn),z
    AM_BIZ32 = 21   ; [$nn],z
    AM_BQIZ32= 22   ; [$nn],z
    AM_BQI   = 23   ; ($nn)
    AM_BQI32 = 24   ; [$nn]
    AM_SRI   = 25   ; ($nn+sp),y
    AM_REL   = 26   ; $nnnn
    AM_RLW   = 27   ; $nnnn
    AM_BPR   = 28   ; $nn,$rr

    VD_HEX   = 1 << 5
    VD_DEC   = 2 << 5
    VD_BIN   = 3 << 5
    VD_OCT   = 4 << 5
    VD_EXP   = 5 << 5
    VD_LAB   = 6 << 5

    LT_EMPTY = %0000_0000
    LT_LBDEF = %0000_0001
    LT_INST  = %0000_0010
    LT_LBUSE = %0000_0100
    LT_DIR   = %0000_1000
    LT_MCDEF = %0001_0000
    LT_MCUSE = %0010_0000
    LT_COMM  = %0100_0000

    EXT_EA   = 1
    EXT_4242 = 2
    EXT_4242EA = 3

; base page pointers
.section bp
    InputLinePtr    .word 0
    Ptr             .word 0
.endsection ; bp

.section data
    ParsePos        .byte 0
    ParseLineType   .byte 0
    ParseLineLen    .byte 0
    ParseLabelID0   .byte 0
    ParseLabelID1   .byte 0
    ParseLabelID2   .byte 0
    ParseLabelID3   .byte 0
    ParseLabelID4   .byte 0
    ParseLabelID5   .byte 0
    ParseLabelID6   .byte 0
    ParseLabelID7   .byte 0
    ParseSize       .byte 0
    ParseBufPos     .byte 0
    ParsePC         .word 0
.endsection ; data

    mnemsize = (mn_end - mnemonics) / 6

.section parser

; Parse the line of code at (BasePage) InputLinePtr
parseLine .proc
        .dbg.info "[parseLine]!n"
        ldy #0          ; init y to start pos in input
        ldx #4          ; start of content of parsed/tokenised line
        stx ParseBufPos
        sty ParsePos
        lda #0
        sta ParseBuf    ; reset the line type byte
    loop
        lda (InputLinePtr),y
        cmp #$FF
        bne notLineEnd
        jmp endParse       ; found end of line
    notLineEnd
        cmp #' '            ; test for space and skip if found
        bne notSpace
        jsr skipWhiteSpace
        bra loop
    notSpace
        cmp #'.'
        bne notDirectiveOrMacroDef
        jsr parseDirectiveOrMacroDef
        jmp loop
    notDirectiveOrMacroDef
        .checkIfAlpha notAlpha, isAlpha
    isAlpha
        jsr parseSymbolOrInstruction
        jmp loop
    notAlpha
        .checkIfComment InputLinePtr, notComment
        jmp parseComment
    notComment
        jmp endParse          ; TODO: add error handling
;parseLine_end

    endParse
        .dbg.info "[endParse]!n"
        rts

    parseDirectiveOrMacroDef
        .dbg.info "p"
        rts
.endproc

; Process a symbol (label, macro use) or instruction
parseSymbolOrInstruction .proc
    .dbg.info "[parseSymbolOrInstruction]!n"

    ; store input line character column
        tya
        sta ParseBuf,x
        inx

    ; TODO: check for valid characters

    ; first determine if this is a symbol or potential instruction
    loop
        lda (InputLinePtr),y ; read char from input
        .dbg.infoCReg 1
        cmp #':'
        bne notColon
        dex
        jmp processLabelDef
    notColon
        cmp #'('
        bne notParen
        jmp parseMacroUse
    notParen
        cmp #' '
        bne notSpace
        jmp parseInstruction
    notSpace
        cmp #$FF
        beq end
        sta ParseBuf,x       ; store in parse buf
        iny
        inx
        jmp loop
    end
        ;stx ParseBufPos
        rts
.endproc

; process the label definition
; only needs to update the line type and skip the colon 
processLabelDef .proc
        .dbg.info "[processLabelDef]!n"
        .dbg.infoRegDec "x", "ParseBufPos: ", " (x)!n"
    ; check on line type
        .checkLineType ParseBuf, LT_DIR, firstLabelDef
        .dbg.error "[LabelDef LT Error]!n"
        rts

    firstLabelDef
        lda ParseBuf
        ora #LT_LBDEF
        sta ParseBuf

        iny ; skip colon from input
        ; update start pos
        sty ParsePos

        inx ; increase past last label char
        lda #$ff
        sta ParseBuf,x  ; store TEND for label
        inx
        stx ParseBufPos
        rts
.endproc

parseInstruction .proc
        .dbg.setTag "firstInstruction"
        .dbg.info "[parseInstruction]!n"

    ; check on line type
        .checkLineType ParseBuf, LT_DIR | LT_LBDEF, firstInstruction
        .dbg.error "[Instruction LT Error]!n"
        sec
        rts

    firstInstruction
        .dbg.info "[first instruction]!n"
        .dbg.infoRegDec "y", "[end pos of candidate: ", "]!n"

        ; check if longer than 4 chars (can't be an instruction)
        tya
        sec
        sbc ParsePos
        cmp #5
        bcs errTooLong
        jsr searchInstruction
        bcs errNoInstruction
        ldx ParseBufPos
        .dbg.infoRegDec "x", "ParseBufPos: ", "!n"

        lda ParsePos
        sta ParseBuf,x
        inx
        ; update parse pos
        sty ParsePos
        ; update line type with instruction flag
        lda ParseBuf
        ora #LT_INST
        sta ParseBuf

        .dbg.info "[found instruction]!n"
        clc
        lda #4
        adc Ptr
        sta Ptr
        lda #0
        adc Ptr
        sty YStore
        ldy #0
        lda (Ptr),y
        .dbg.infoRegHex "a", "Opcode base: $", "!n"
        sta ParseBuf,x ; store instruction
        inx
        stx ParseBufPos

        ; calc ptr in addr mode table
        iny
        lda (Ptr),y
        ldy YStore
        .dbg.infoRegDec "a", "Addr mode offset: ", "!n"
        phx
        phy
        clc
        adc #<addrm_groups
        tax
        lda #0
        adc #>addrm_groups
        tay
        .stxy Ptr
        plx
        ldy #0
        lda (Ptr),y
        ply
        .dbg.infoRegDec "a", "First addr mode: ", "!n"

        ; parse 

        rts
    errTooLong
        .dbg.error "[mnemonic too long!]!n"
        sec
        rts
    errNoInstruction
        .dbg.error "[invalid instruction!]!n"
        sec
        rts
        .dbg.resetTag
.endproc

; -> A: 
searchInstruction .proc
    .dbg.setTag "searchInstruction"
    phx
    
    .dbg.info "[searchInstruction]!n"
    .dbg.only lda ParsePos
    .dbg.infoRegDec "a", "[start pos of candidate: ", "]!n"
    .dbg.only .rdxy InputLinePtr
    .dbg.infoXYHex "InputLine address: $", " (InputLinePtr)!n"

    .ldxy mnemonics
    .dbg.infoXYHex "mnemonics address: $", " (Ptr)!n"
    .ldxy mn_end
    .dbg.infoXYHex "mn end address:    $", " (Ptr)!n"
    .dbg.resetTag

    .ldxy mnemonics
    .stxy Ptr

    ;ldy ParsePos
    ;ldz #0
    ldx #0
    stx YStore  ; conveniently setting YStore to 0
loop
    .dbg.setTag "SIInOut"
    .dbg.only ldy ParsePos
    .dbg.only lda (InputLinePtr), y
    .dbg.infoReg "a", "input: ", "!n"
    .dbg.only ldy YStore
    .dbg.only lda (Ptr),y
    .dbg.only ldy ParsePos
    .dbg.infoReg "a", "mnem:  ", "!n"
    .dbg.resetTag
    .dbg.setTag "searchInstruction"

    ldy YStore
    lda (Ptr),y
    beq notFound
    ldy ParsePos
    cmp (InputLinePtr),y
    bcc noMatch
    bne notFound
    iny
    sty ParsePos
    lda (InputLinePtr),y
    cmp #' '
    beq found

    lda Ptr
    cmp #<mn_end
    bcc +
    lda Ptr+1
    cmp #>mn_end
    bcs notFound
+
    inc YStore
    ldy YStore
    cpy #5
    beq found

    bra loop
noMatch
    clc
    lda Ptr
    adc #6
    sta Ptr
    lda #0
    adc Ptr+1
    sta Ptr+1
    
    dec YStore
    ldy YStore
    bmi +
    lda (Ptr),y
    dec ParsePos
    ldy ParsePos
    cmp (InputLinePtr),y
    bne +
    inc YStore
    inc ParsePos
    bra loop
+
    ldy #0
    sty YStore
    ldy ParsePos
    bra loop
found
    .dbg.info "[instruction found]!n"

    plx
    clc
    rts
notFound
    .dbg.info "[instruction not found]!n"
    .dbg.only .rdxy Ptr
    .dbg.infoXYHex "Ptr address: $", " (Ptr)!n"

    plx
    sec
    rts
    .dbg.resetTag
.endproc

parseMacroUse .proc
    .dbg.info "[parseMacroUse]!n"
    lda (InputLinePtr),y

    iny
    rts
.endproc

parseComment .proc
        .dbg.info "[parseComment]!n"

        lda ParseBuf        ; load line type byte
        bne notOnlyComment

        ; we're responsibble for setting the current PC
        .setParsePC ParsePC

    notOnlyComment
        ora #LT_COMM
        sta ParseBuf

        tya
        sta ParseBuf,x      ; store position of comment
        inx                 ; skip size
        iny

    parseCmt1
        lda (InputLinePtr),y
        cmp #$FF
        beq end             ; found end of line
        .dbg.infoReg "a", "comment: ", "!n"
        
        sta ParseBuf,x
        inx
        iny
        cpy #80
        bne parseCmt1
    end
        ; we need to set the length of the line
        sta ParseBuf,x
        inx
        txa                 ; load last ParseBuf pos
        sta ParseBuf + 1
        rts
.endproc


; x: character column, increments x to first non-space character column
skipWhiteSpace .proc
    .dbg.info "[skipWhiteSpace]!n"
    loop
        iny
        lda (InputLinePtr),y
        cmp #' '
        beq loop
        sty ParsePos
        rts
.endproc
.endsection ; parser

.section data
            .align
ParseBuf    .fill $100, 0

            .align
mnemonics
mn_adc      .text "adc@"
            .byte $61, gr01-addrm_groups
mn_adcq     .text "adcq"
            .byte $61, gr12-addrm_groups
mn_and      .text "and@" 
            .byte $21, gr01-addrm_groups
mn_andq     .text "andq"
            .byte $21, gr12-addrm_groups
mn_asl      .text "asl@"
            .byte $06, gr02-addrm_groups
mn_aslq     .text "aslq"
            .byte $06, gr13-addrm_groups
mn_asr      .text "asr@"
            .byte $43, gr03-addrm_groups
mn_asrq     .text "asrq"
            .byte $43, gr14-addrm_groups
mn_asw      .text "asw@"
            .byte $CB, gr04-addrm_groups
mn_br0      .text "bbr0"
            .byte $0F, gr05-addrm_groups
mn_br1      .text "bbr1"
            .byte $1F, gr05-addrm_groups
mn_br2      .text "bbr2"
            .byte $2F, gr05-addrm_groups
mn_br3      .text "bbr3"
            .byte $3F, gr05-addrm_groups
mn_br4      .text "bbr4"
            .byte $4F, gr05-addrm_groups
mn_br5      .text "bbr5"
            .byte $5F, gr05-addrm_groups
mn_br6      .text "bbr6"
            .byte $6F, gr05-addrm_groups
mn_br7      .text "bbr7"
            .byte $7F, gr05-addrm_groups
mn_bs0      .text "bbs0"
            .byte $8F, gr05-addrm_groups
mn_bs1      .text "bbs1"
            .byte $9F, gr05-addrm_groups
mn_bs2      .text "bbs2"
            .byte $AF, gr05-addrm_groups
mn_bs3      .text "bbs3"
            .byte $BF, gr05-addrm_groups
mn_bs4      .text "bbs4"
            .byte $CF, gr05-addrm_groups
mn_bs5      .text "bbs5"
            .byte $DF, gr05-addrm_groups
mn_bs6      .text "bbs6"
            .byte $EF, gr05-addrm_groups
mn_bs7      .text "bbs7"
            .byte $FF, gr05-addrm_groups
mn_bcc      .text "bcc@"
            .byte $90, gr06-addrm_groups
mn_bcs      .text "bcs@"
            .byte $B0, gr06-addrm_groups
mn_beq      .text "beq@"
            .byte $F0, gr06-addrm_groups
mn_bit      .text "bit@"
            .byte $24, gr07-addrm_groups
mn_bitq     .text "bitq"
            .byte $24, gr15-addrm_groups
mn_bmi      .text "bmi@"
            .byte $30, gr06-addrm_groups
mn_bne      .text "bne@"
            .byte $D0, gr06-addrm_groups
mn_bpl      .text "bpl@"
            .byte $10, gr06-addrm_groups
mn_bra      .text "bra@"
            .byte $80, gr06-addrm_groups
mn_brk      .text "brk@"
            .byte $00, gr08-addrm_groups
mn_bsr      .text "bsr@"
            .byte $65, gr09-addrm_groups
mn_bvc      .text "bvc@"
            .byte $50, gr06-addrm_groups
mn_bvs      .text "bvs@"
            .byte $70, gr06-addrm_groups
mn_clc      .text "clc@"
            .byte $18, gr08-addrm_groups
mn_cld      .text "cld@"
            .byte $D8, gr08-addrm_groups
mn_cle      .text "cle@"
            .byte $02, gr08-addrm_groups
mn_cli      .text "cli@"
            .byte $58, gr08-addrm_groups
mn_clv      .text "clv@"
            .byte $B8, gr08-addrm_groups
mn_cmp      .text "cmp@"
            .byte $C1, gr01-addrm_groups
mn_cmpq     .text "cmpq"
            .byte $C1, gr12-addrm_groups
mn_cpx      .text "cpx@"
            .byte $E0, gr0a-addrm_groups
mn_cpy      .text "cpy@"
            .byte $C0, gr0a-addrm_groups
mn_cpz      .text "cpz@"
            .byte $C2, gr0a-addrm_groups
mn_dec      .text "dec@"
            .byte $3A, gr0b-addrm_groups
mn_deq      .text "deq@"
            .byte $3A, gr13-addrm_groups
mn_dew      .text "dew@"
            .byte $C3, gr0c-addrm_groups
mn_dex      .text "dex@"
            .byte $CA, gr08-addrm_groups
mn_dey      .text "dey@"
            .byte $88, gr08-addrm_groups
mn_dez      .text "dez@"
            .byte $3B, gr08-addrm_groups
mn_eom      .text "dez@"
            .byte $EA, gr08-addrm_groups
mn_eor      .text "eor@"
            .byte $41, gr01-addrm_groups
mn_eorq     .text "eorq"
            .byte $41, gr12-addrm_groups
mn_inc      .text "inc@"
            .byte $1A, gr0b-addrm_groups
mn_inq       .text "inq@"
            .byte $1A, gr13-addrm_groups
mn_inw      .text "inw@"
            .byte $E3, gr0c-addrm_groups
mn_inx      .text "inx@"
            .byte $E8, gr08-addrm_groups
mn_iny      .text "iny@"
            .byte $C8, gr08-addrm_groups
mn_inz      .text "inz@"
            .byte $1B, gr08-addrm_groups
mn_jmp      .text "jmp@"
            .byte $4C, gr0d-addrm_groups
mn_jsr      .text "jsr@"
            .byte $20, gr0d-addrm_groups
mn_lda      .text "lda@"
            .byte $A1, gr01-addrm_groups
mn_ldq      .text "ldaq"
            .byte $A1, gr12-addrm_groups
mn_ldx      .text "ldx@"
            .byte $A2, gr0e-addrm_groups
mn_ldy      .text "ldy@"
            .byte $A0, gr0e-addrm_groups
mn_ldz      .text "ldz@"
            .byte $A3, gr0f-addrm_groups
mn_lsr      .text "lsr@"
            .byte $46, gr02-addrm_groups
mn_lsrq     .text "lsrq"
            .byte $46, gr13-addrm_groups
mn_map      .text "map@"
            .byte $5C, gr08-addrm_groups
mn_neg      .text "neg@"
            .byte $42, gr10-addrm_groups
mn_ora      .text "ora@"
            .byte $01, gr01-addrm_groups
mn_orq      .text "orq@"
            .byte $01, gr12-addrm_groups
mn_pha      .text "pha@"
            .byte $48, gr08-addrm_groups
mn_php      .text "php@"
            .byte $08, gr08-addrm_groups
mn_phw      .text "phw@"
            .byte $F4, gr11-addrm_groups
mn_phx      .text "phx@"
            .byte $DA, gr08-addrm_groups
mn_phy      .text "phy@"
            .byte $5A, gr08-addrm_groups
mn_phz      .text "phz@"
            .byte $DB, gr08-addrm_groups
mn_pla      .text "pla@"
            .byte $68, gr08-addrm_groups
mn_plp      .text "plp@"
            .byte $28, gr08-addrm_groups
mn_plx      .text "plx@"
            .byte $FA, gr08-addrm_groups
mn_ply      .text "ply@"
            .byte $7A, gr08-addrm_groups 
mn_plz      .text "plz@"
            .byte $FB, gr08-addrm_groups
mn_rmb0     .text "rmb0"
            .byte $07, gr0c-addrm_groups
mn_rmb1     .text "rmb1"
            .byte $07, gr0c-addrm_groups
mn_rmb2     .text "rmb2"
            .byte $17, gr0c-addrm_groups
mn_rmb3     .text "rmb3"
            .byte $27, gr0c-addrm_groups
mn_rmb4     .text "rmb4"
            .byte $37, gr0c-addrm_groups
mn_rmb5     .text "rmb5"
            .byte $47, gr0c-addrm_groups
mn_rmb6     .text "rmb6"
            .byte $57, gr0c-addrm_groups
mn_rmb7     .text "rmb7"
            .byte $67, gr0c-addrm_groups
mn_rol      .text "rol@"
            .byte $26, gr02-addrm_groups
mn_ror      .text "ror@"
            .byte $66, gr02-addrm_groups
mn_row      .text "row@"
            .byte $eb, gr04-addrm_groups
mn_rti      .text "rti@"
            .byte $40, gr08-addrm_groups
mn_rts      .text "rts@"
            .byte $60, gr16-addrm_groups
mn_sbc      .text "sbc@"
            .byte $e1, gr01-addrm_groups
mn_sbcq     .text "sbcq"
            .byte $e1, gr12-addrm_groups
mn_sec      .text "sec@"
            .byte $38, gr08-addrm_groups
mn_sed      .text "sed@"
            .byte $f8, gr08-addrm_groups
mn_see      .text "see@"
            .byte $03, gr08-addrm_groups
mn_sei      .text "sei@"
            .byte $78, gr08-addrm_groups
mn_smb0     .text "rmb0"
            .byte $87, gr0c-addrm_groups
mn_smb1     .text "rmb1"
            .byte $97, gr0c-addrm_groups
mn_smb2     .text "rmb2"
            .byte $a7, gr0c-addrm_groups
mn_smb3     .text "rmb3"
            .byte $b7, gr0c-addrm_groups
mn_smb4     .text "rmb4"
            .byte $c7, gr0c-addrm_groups
mn_smb5     .text "rmb5"
            .byte $d7, gr0c-addrm_groups
mn_smb6     .text "rmb6"
            .byte $e7, gr0c-addrm_groups
mn_smb7     .text "rmb7"
            .byte $f7, gr0c-addrm_groups
mn_sta      .text "sta@"
            .byte $81, gr17-addrm_groups
mn_stx      .text "stx@"
            .byte $86, gr17-addrm_groups
mn_sty      .text "sty@"
            .byte $84, gr17-addrm_groups
mn_stz      .text "stz@"
            .byte $64, gr17-addrm_groups
mn_tab      .text "tab@"
            .byte $5b, gr08-addrm_groups
mn_tax      .text "tax@"
            .byte $aa, gr08-addrm_groups
mn_tay      .text "tay@"
            .byte $a8, gr08-addrm_groups
mn_taz      .text "taz@"
            .byte $4b, gr08-addrm_groups
mn_tba      .text "tba@"
            .byte $7b, gr08-addrm_groups
mn_trb      .text "trb@"
            .byte $14, gr18-addrm_groups
mn_tsb      .text "tsb@"
            .byte $04, gr18-addrm_groups
mn_tsx      .text "tsx@"
            .byte $ba, gr08-addrm_groups
mn_tsy      .text "tsy@"
            .byte $0b, gr08-addrm_groups
mn_txa      .text "txa@"
            .byte $8a, gr08-addrm_groups
mn_txs      .text "txs@"
            .byte $9a, gr08-addrm_groups
mn_tya      .text "tya@"
            .byte $98, gr08-addrm_groups
mn_tys      .text "tys@"
            .byte $2b, gr08-addrm_groups
mn_tza      .text "tza@"
            .byte $6b, gr08-addrm_groups
mn_end 

.align
addrm_groups
gr01        .byte AM_BIX,   $00, $50
            .byte AM_BP,    $04, $30 
            .byte AM_IMM,   $08, $20
            .byte AM_ABS,   $0c, $40
            .byte AM_BIY,   $10, $50
            .byte AM_BIZ,   $11, $50
            .byte AM_BIZ32, $11, $70 | EXT_EA
            .byte AM_BPX,   $14, $30
            .byte AM_ABY,   $18, $40
            .byte AM_ABX,   $1c, $40
gr02        .byte AM_ACC,   $04, $10
            .byte AM_BP,    $00, $40
            .byte AM_ABS,   $08, $50
            .byte AM_BPX,   $10, $40
            .byte AM_ABX,   $18, $40
gr03        .byte AM_ACC,   $00, $10
            .byte AM_BP,    $01, $40
            .byte AM_BPX,   $0b, $40
gr04        .byte AM_ABS,   $00, $40
gr05        .byte AM_BPR,   $00, $50
gr06        .byte AM_REL,   $00, $20
            .byte AM_RLW,   $03, $30
gr07        .byte AM_BP,    $00, $40
            .byte AM_ABS,   $04, $50
            .byte AM_BPX,   $0c, $40
            .byte AM_ABX,   $14, $40
            .byte AM_IMM,   $5e, $00
gr08        .byte AM_IMP,   $00, $70
gr09        .byte AM_RLW,   $00, $30
gr0a        .byte AM_IMM,   $00, $00
            .byte AM_BP,    $04, $00
            .byte AM_ABS,   $0c, $00
gr0b        .byte AM_ACC,   $00, $10
            .byte AM_BP,    $8C, $00
            .byte AM_ABS,   $94, $00
            .byte AM_BPX,   $9c, $00
            .byte AM_ABX,   $a4, $00
gr0c        .byte AM_BP,    $00, $40
gr0d        .byte AM_ABS,   $00, $30
            .byte AM_AIN,   $20, $50
            .byte AM_BIX,   $30, $00
gr0e        .byte AM_IMM,   $00, $00
            .byte AM_BP,    $04, $00
            .byte AM_ABS,   $0c, $00
            .byte AM_BPX,   $14, $00
            .byte AM_ABX,   $1c, $00
gr0f        .byte AM_IMM,   $00, $00
            .byte AM_BP,    $08, $00
            .byte AM_ABS,   $18, $00
gr10        .byte AM_ACC,   $00, $10
gr11        .byte AM_IMW,   $00, $00
            .byte AM_ABS,   $08, $00
gr12        .byte AM_BPQ,   $04, $80 | EXT_4242
            .byte AM_ABQ,   $0c, $90 | EXT_4242
            .byte AM_BQI,   $11, $a0 | EXT_4242
            .byte AM_BQI32, $11, $d0 | EXT_4242EA
gr13        .byte AM_BPQ,   $00, $c0 | EXT_4242
            .byte AM_QPR,   $04, $30 | EXT_4242
            .byte AM_ABQ,   $08, $d0 | EXT_4242
            .byte AM_BQX,   $10, $c0 | EXT_4242
            .byte AM_ABQX,  $18, $d0 | EXT_4242
gr14        .byte AM_QPR,   $00, $30 | EXT_4242
            .byte AM_BPQ,   $01, $c0 | EXT_4242
            .byte AM_BQX,   $11, $c0 | EXT_4242
gr15        .byte AM_BPQ,   $00, $80 | EXT_4242
            .byte AM_ABQ,   $08, $90 | EXT_4242
gr16        .byte AM_IMM,   $00, $60
            .byte AM_IMW,   $02, $40
gr17        .byte AM_BIX,   $00, $50
            .byte AM_SRI,   $01, $60 
            .byte AM_BP,    $04, $30
            .byte AM_ABS,   $0c, $40
            .byte AM_BIY,   $10, $50
            .byte AM_BIZ,   $11, $50
            .byte AM_BIZ32, $11, $70 | EXT_EA
            .byte AM_BPX,   $14, $30
            .byte AM_ABY,   $18, $40
            .byte AM_ABX,   $1c, $40
gr18        .byte AM_BP,    $00, $50
            .byte AM_ABS,   $08, $40 

tok_to_mnem
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
            ; !byte   $FB, <mn_plz, >mn_plz
lookup_end

; the first opcode for each mnemonic is the token for the editor, combined with the id of the specific addressing mode
datasize    .word lookup_end-mnemonics
tokensize   .word lookup_end-tok_to_mnem
addrmsize   .word tok_to_mnem-addrm_groups

.endsection ; data

.endnamespace ; parser