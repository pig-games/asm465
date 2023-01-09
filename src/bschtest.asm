;---------------------------------------
; bschtest.s
;---------------------------------------

         .include "zeropage.asm"
         .include "assert.asm"
         .include "prsrmacros.asm"
         .include "prsrconst.asm"
* = $8000
         jmp start

inputln  .text "adc"
         .byte $ff

         .include "io.asm"
         .include "bschutil.asm"
         .include "binsearch.asm"
         .include "prsrdefs.asm"

start
         #cprl 5,"Unit test: binsearch"
         #nl

         #setjsraddr bsctos,clcmnmaddr

         lda #0
         sta prspos

         lda #4
         sta bschixr

         lda <mnemonics
         sta bschdptr
         lda >mnemonics
         sta bschdptr+1

         lda <inputln
         sta bschiptr
         lda >inputln
         sta bschiptr+1

         jsr binsearch

         bcs notfound

         #cprl 5,"found!"

notfound
         #cprl 5,"notfound!"
         jmp *

