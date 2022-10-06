;---------------------------------------
; macrotest
;---------------------------------------
         .include "setup.s"

         jmp start

         .include "prsrconst.s"
         .include "prsrmacros.s"
         .include "io.s"
         .include "assert.s"
start
         #cprl 5,"Unit Test: macrotest"
         #nl

test1
         .block
         lda #"a"-$40
         #chkifalpha nota,isa
         #abra2 "test1","nota","isa"
         .bend

test2
         .block
         lda #"["-$40
         #chkifalpha nota,isa
         #abra2 "test2","isa","nota"
         .bend

test3
         .block
         lda #"z"-$40
         #chkifalpha nota,isa
         #abra2 "test3","nota","isa"
         .bend

test4
         .block
         lda #"{SHIFT-*}"-$c0
         #chkifalpha isa,nota
         #abra2 "test4","nota","isa"
         .bend

test5
         .block
         lda #"A"-$c0
         #chkifalpha isa,nota
         #abra2 "test5","isa","nota"
         .bend

test6
         .block
         lda #"{SHIFT-+}"-$c0
         #chkifalpha isa,nota
         #abra2 "test6","nota","isa"
         .bend

         #nl
         #cprl 5,"Done"

         jmp *

