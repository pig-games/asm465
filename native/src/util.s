
.section util
;---------------------------------------
; Translate A into hex digits X, Y
; (screen code)
; In: A = number
; Out: X = high nib, Y = low nib
;---------------------------------------
toHexXY .proc
        pha
        and #$0f
        cmp #$0a
        bcs ge_0a1
        adc #$3a
ge_0a1  sbc #$09
        tay
        pla
        pha
        lsr a
        lsr a
        lsr a
        lsr a
        cmp #$0a
        bcs ge_0a2
        adc #$3a
ge_0a2  sbc #$09
        tax
        pha
        rts
.endproc

toDec .proc 
    sed             ; Output gets added up in decimal.
    lda #0
    sta Out         ; Inititalize output as 0.
    sta Out+1   
    sta Out+2

    ldx #$2d        ; 2DH is 45 decimal, or 3x15 bits.
loop
    lda In
    asl In      ; (0 to 15 is 16 bit positions.)
    rol In+1    ; If the next highest bit was 0,
    bcc htd1       ; then skip to the next bit after that.
    lda Out     ; But if the bit was 1,
    clc             ; get ready to
    adc Table+2,x   ; add the bit value in the table to the
    sta Out     ; output sum in decimal--  first low byte,
    lda Out+1   ; then middle byte,
    adc Table+1,x
    sta Out+1
    lda Out+2   ; then high byte,
    adc Table,x     ; storing each byte
    sta Out+2   ; of the summed output in HTD_OUT.

htd1
    dex             ; By taking X in steps of 3, we don't have to
    dex             ; multiply by 3 to get the right bytes fromthe
    dex             ; table.
    bpl loop

    cld
    rts

.section data
In		.word	1	; Low byte first, as is normal for 6502.
Out	    .byte	3   ; Low byte first, highest byte last.

                ; The table below has high byte first just to
                ; make it easier to see the number progression.
Table   .byte    $0, $0, $1, 0, $0, $2, 0, $0, $4, 0, $0, $8
    	.byte    $0, $0,$16, 0, $0,$32, 0, $0,$64, 0, $1,$28
    	.byte    $0, $2,$56, 0, $5,$12, 0,$10,$24, 0,$20,$48
    	.byte    $0,$40,$96, 0,$81,$92, 1,$63,$84, 3,$27,$68
.endsection

.endproc

.endsection ; util