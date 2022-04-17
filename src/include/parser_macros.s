

.macro checkIfAlpha(notAlpha, isAlpha) {
        cmp #'a'
        bcc notAlpha
        cmp #'['
        bcc isAlpha
        cmp #'A'
        bcc notAlpha
        cmp #$5B
        bcs notAlpha
}

.macro checkIfComment(inputLinePtr, notComment) {
        cmp #'/'
        bne notComment
        iny
        lda (inputLinePtr),y
        cmp #'/'
        bne notComment
}

.macro checkLineType(lineType, lineTypeFlags, okLabel) {
        // check on line type
        lda #lineTypeFlags
        eor #$ff
        and lineType
        beq okLabel

    // not allowed to have multiple label defs on one line
        lda #'e'            // DEBUG OUTPUT
        sta $0800+10*80,y   // DEBUG OUTPUT
        iny
        //TODO: handle error
}