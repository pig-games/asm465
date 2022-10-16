;---------------------------------------
; bschtest.s
;---------------------------------------

         .include "setup.s"
         .include "zeropage.s"
         .include "assert.s"
         .include "prsrmacros.s"
         .include "prsrconst.s"
         jmp start

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



         jmp *

