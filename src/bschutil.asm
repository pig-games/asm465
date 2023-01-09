asw      .macro
         asl \1
         rol \1+1
         .endm

;---------------------------------------
; clcmnmaddr
; calculates table lookup addr from ix
;
; input:
;   a: index
;---------------------------------------

clcmnmaddr
         pha ; backup ix
         sta bschptr
         lda #0
         sta bschptr+1

         ; bschptr * 4
         #asw bschptr
         #asw bschptr
         pla ; restore ix
         sta bschtmp
         lda #0
         sta bschtmp+1

         ; * 2 (completes *6)
         #asw bschtmp
         lda bschtmp
         clc
         adc bschptr
         sta bschptr
         lda bschtmp+1
         adc bschptr+1
         sta bschptr+1
         rts

