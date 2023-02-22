.cpu "w65c02"

straddr  = $30

sprint
        .block
        pla
        sta straddr
        pla
        sta straddr+1
; prepare mmu
        lda $0001
        pha
        lda #$02
        sta $0001
; do actual print

        ldy #1
loop    lda (straddr),y
        beq end
        sta $c000,y
        iny
        jmp loop
end
; restore MMU
        pla
        sta $0001
        
; calculate new return address
        tya
        clc
        adc straddr
        sta straddr
        lda #0
        adc straddr+1

; restore return address
        pha
        lda straddr
        pha
        rts
        .bend

settextmode
        .block
        lda $0001
        pha
        stz $0001
        lda #1
        sta $d000
        stz $d001
        pla
        sta $0001
        rts
        .bend

nl      .macro
        lda #13
        ;jsr $ffd2
        .endm


prl     .macro
        jsr sprint
        .null \1
      ;  .byte 13,0
        .endm

cpr     .macro
        jsr sprint
;        .byte \1
        .null \2
        .endm

cprl    .macro
        jsr sprint
;        .byte \1
        .null \2
        .endm


