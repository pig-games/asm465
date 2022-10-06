straddr  = $0a

sprint
         .block
         pla
         sta straddr
         pla
         sta straddr+1
         ldy #0
loop     lda (straddr),y
         beq end
         jsr $ffd2
         iny
         jmp loop
end      tya
         clc
         adc straddr
         sta straddr
         lda #0
         adc straddr+1
         pha
         lda straddr
         pha
         rts
         .bend

nl       .macro
         lda #13
         jsr $ffd2
         .endm


prl      .macro
         jsr sprint
         .text "@1"
         .byte 13,0
         .endm

cpr      .macro
         jsr sprint
         .byte \1
         .text "@2"
         .byte 0
         .endm

cprl     .macro
         jsr sprint
         .byte \1
         .text "@2"
         .byte 13,0
         .endm

