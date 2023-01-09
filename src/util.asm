

;---------------------------------------
; Translate A into hex digits A, X
; (screen code)
; In: A = number
; Out: A = high nib, X = low nib
;---------------------------------------
to_hex_scrc
         pha
         and #$0f
         cmp #$0a
         bcs ge_0a1
         adc #$3a
ge_0a1   sbc #$09
         tax
         pla
         lsr a
         lsr a
         lsr a
         lsr a
         cmp #$0a
         bcs ge_0a2
         adc #$3a
ge_0a2   sbc #$09
         rts

