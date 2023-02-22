;---------------------------------------
; macrotest
;---------------------------------------

         .include "prsrconst.asm"
         .include "prsrmacros.asm"
         .include "assert.asm"

*=$8000
        jmp start
        .include "io.asm"

start 
        jsr settextmode
        #cprl 5, "Unit Test: macrotest"
     ;    #nl

test1
        .block
        lda #"a"
        #chkifalpha nota, isa
        #abra2 "test1",nota,isa
        .bend

        jmp *

test2
        .block
        lda #"["
        #chkifalpha nota,isa
        #abra2 "test2",isa,nota
        .bend

test3
        .block
        lda #"z"
        #chkifalpha nota,isa
        #abra2 "test3",nota,isa
        .bend

test4
        .block
        lda #"_"
        #chkifalpha isa,nota
        #abra2 "test4",nota,isa
        .bend

test5
        .block
        lda #"A"
        #chkifalpha isa,nota
        #abra2 "test5",isa,nota
        .bend

test6
        .block
        lda #"_"
        #chkifalpha isa,nota
        #abra2 "test6",nota,isa
        .bend

        #nl
        #cprl 5,"Done"

        jmp *

