

;---------------------------------------
; Translate A into hex digits A, X
; (screen code)
; In: A = number
; Out: A = high nib, X = low nib
;---------------------------------------
to{CBM-@}hex{CBM-@}scrc
         pha
         and #$0f
         cmp #$0a
         bcs ge{CBM-@}0a1
         adc #$3a
ge{CBM-@}0a1   sbc #$09
         tax
         pla
         lsr a
         lsr a
         lsr a
         lsr a
         cmp #$0a
         bcs ge{CBM-@}0a2
         adc #$3a
ge{CBM-@}0a2   sbc #$09
         rts

