

checkIfAlpha .macro notAlpha, isAlpha
        cmp #'a'
        bcc \notAlpha
        cmp #'['
        bcc \isAlpha
        cmp #'A'
        bcc \notAlpha
        cmp #$5B
        bcs \notAlpha
.endmacro

checkIfComment .macro notComment
        cmp #';'
        bne \notComment
.endmacro

checkLineType .macro lineType, lineTypeFlags, okLabel
        ; check on line type
        lda #\lineTypeFlags
        eor #$ff
        and \lineType
        beq \okLabel

    ; not allowed to have multiple label defs on one line
        lda #'e'            ; DEBUG OUTPUT
        sta $0800+10*80,y   ; DEBUG OUTPUT
        iny
        ;TODO: handle error
.endmacro

setParsePC .macro parsePC
    ; set PC for line
    lda #<\parsePC
    sta ParseBuf+2
    lda #>\parsePC
    sta ParseBuf+3
.endmacro

setInputLine .macro inputLine
    lda #<\inputLine
    sta InputLinePtr
    lda #>\inputLine
    sta InputLinePtr + 1
.endmacro