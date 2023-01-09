         .include "prsrmacros.asm"

;nemsize = mnend-mnemonics/6

         .include "binsearch.asm"
; Parse the line of code at (ZeroPage)
; inlnptr
parseline
         .block
         #setjsraddr addrcalc,clcmnmaddr

         ldy #0; set y to input strt pos
         ldx #4; start tokenised content

         lda #0
         sta prsbuf; reset the line type
         lda #4
         sta prspos
loop
         lda (inlnptr),y
         cmp #$ff
         bne notlnend
         jmp endparse; found end of line
notlineend
         cmp #" "    ; test for space
         bne notspace; and skip if found
         jsr skipwhitespace
         jmp loop
notspace
         cmp #"."
         bne notdirormdf
         jmp prsdirormdf
         jmp loop
notdirormdf
         cmp #"!"
         bne notmultilbl
         jmp prsmultilbl
notmultilbl
         #chk{CBM-@}if{CBM-@}alpha nota,isa
isa
         jsr prssymorinst
         jmp loop
nota
         #chkifcom inlnptr,notcomment
         jsr prscomm
notcomment
         jmp endparse
         .bend
endparse
         txa
         sta prsbuf+1
         rts

prsdirormdf
         .block
        ;TODO: implement
         rts
         .bend

; Parse symbol (label,macrouse) or instr
prssymorinst
         .block


    ; store input line character column
         tya
         sta prsbuf,x
         inx

    ; TODO: check for valid characters

    ; first determine if this is a
    ; symbol or potential instruction
loop
         lda (inlnptr),y
         cmp #":"
         bne notcolon
         jmp proclbldf
notcolon
         cmp #"("
         jmp prsmuse
notparen
         cmp #" "
         bne notspace
         jmp prsinst
notspace
         sta prsbuf,x
         iny
         inx
         jmp loop
end
         rts
         .bend

; Process the label definition
; As we already have the label in the
; prsbuf,this only needs to update the
; line type and skip the colon
proclbldf
         .block

    ; check on line type
         #chklntype prsbuf,0,firstlbldf
         rts
firstlbldf
         lda prsbuf
         ora #ltlbdef
         sta prsbuf
         iny
         lda #$ff
         sta prsbuf,x
         inx

        ; update start pos
         txa
         sta prspos

         rts
         .bend

prsinst
         .block
        ;lda (inlnptr),y

; check on line type
         #chklntype prsbuf,ltlbdef,finst
         rts

finst

; x points to the " " after the
; candidate instruction so decrease
         dex

; check if longer than 4 chars
; cannot be an instruction

         txa
         sec
         sbc prspos
         cmp #5
         bcs toolong
; do binary search to find instruction
         stx $f3; backup x
         ldx #12; prspos
         lda #mnemsize-1
         sta bschixr
         lda #<mnemonics
         sta bschdptr
         lda #>mnemonics
         sta bschdptr+1

         sty $f2; backup y
         ldy #0

         jsr binsearch
         bcs notfound

         ldy $f2; restore y

         stx prspos
         ldx #4
         lda (bschptr),y
         ldx $f3 ; restore x
         sta prsbuf,x

; update line type with instruction flag
         lda prsbuf
         ora #ltinst
         sta prsbuf

         rts

notfound
         ldy $f2; restore y
         ldx $f3; restore x

         iny
         inx

        ; update start pos
        ; txa
        ; sta prspos


         rts
toolong
         ldy $f2
         rts
         .bend

prsmuse
         .block
         lda (inlnptr),y

         iny
         rts
         .bend

prsmultlbl
         .block
         lda (inlnptr),y
         rts
         .bend

prscomm
         .block

         lda prsbuf; load line type byte
         bne notonlycomm
; we're must for setting the current PC
         #setprspc prspc

notonlycomm
         ora #ltcomm
         sta prsbuf

         tya
; pos of comm
         inx         ; skip size
         iny         ; skip second "/"

loop
         lda (inlnptr),y
         cmp #$ff
         beq end     ; found end of line

         sta prsbuf,x
         inx
         iny
         cpy #80
         bne loop
end

;TODO: move to appropriate place
; we need to set the length of the line
; txa             ; load last prsbuf pos
         sta prsbuf+1
         rts
         .bend


; x: character column,increments x to
; first non-space character column
skipwhitespace
         .block
loop
         iny
         lda (inlnptr),y
         cmp #" "
         beq loop
         rts
         .bend
;prsbuf  .fill $ff,0

