;---------------------------------------
; bschtest.s
;---------------------------------------

         .include "setup.s"
         .include "zeropage.s"
         .include "assert.s"
         .include "prsrmacros.s"
         .include "prsrconst.s"

         jmp start

inputln  .text "adc"
         .byte $ff

         .include "io.s"
         .include "bschutil.s"
         .include "binsearch.s"
         .include "prsrdefs.s"

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

