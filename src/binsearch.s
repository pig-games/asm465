
;---------------------------------------
; Perform bin search in a block of data
; in:
;   prspos:  start column of input text
;   bschixr: size of data (columns)
;
; out:
;   prspos:  pos behind last chr of mnem
;   bschptr: addr of match (if c)
;   c:       set if not found
;
; internal:
;   zptmp1:  original prspos
;   zptmp2:  temp y backup
;   prspos:  position in input
;   y:       position in candidate mnem
;   bsctos:  jsr to 16bit mnem table
;                 offset calculation
;---------------------------------------
binsearch
         ; set left and right index
         ; last row of table

         ; back up prspos
         lda prspos
         sta zptmp1

         ; init
         ldy #0
         sty bschixl

bsloop   ; if new ix bschixm>=ixl: done
         lda bschixr
         cmp bschixl ; compare ix r to l
         bcs bsnotend  ; if (r>=l) cont
         jmp bsnotfound
bsnotend
         sec
         sbc bschixl ; r - l
         lsr a       ; (r - l) / 2
         clc
         adc bschixl ; l + (r - l) / 2
         sta bschixm ; mid = l+(r-l) / 2

bsctos   ; modify addr: #setjsraddr
         jsr $ffff

; add start addr of mnem tab to (mid*6)
         lda bschptr ; ld l byte of ptr
         clc
         adc bschdptr
         sta bschptr
         lda bschptr+1
         adc bschdptr+1
         sta bschptr+1

; start comp cur inp with candidate mnem

bscomploop
         .block

         lda (bschptr),y
         beq bsnotfound      ; found '@'

         sty zptmp2 ; backup y
         ldy prspos

         lda (bschiptr),y   ; get i char
         cmp #$ff
         beq bsnotfound; fnd end of line
         cmp #$20 ; " "
         bne nospace
         jmp bsfound
nospace
         ; scan for mnemonic
         ldy zptmp2 ; restore y
         cmp (bschptr),y

         bcc islower
         beq isequal
ishigher               ; else is higher
         ldx bschixm
         inx
         stx bschixl

         lda zptmp1 ; re-init prspos
         sta prspos
         ldy #0
         jmp bsloop
islower
         ldx bschixm
         dex
         stx bschixr

         lda zptmp1 ; re-init prspos
         sta prspos
         ldy #0
         jmp bsloop
isequal
         lda (bschptr),y
         cpy #3
         beq bsfound

         inc prspos
         iny ; cand chr ix to next char

         jmp bscomploop

;TODO: write FF to label_buf,0 when
; found mnemonic instead of label
         ; jmp bsnotfound

         .bend ; compareloop
bsfound
         .block
         ldy prspos
         lda (bschiptr),y
         cmp #$20   ;" "
         bne nospace
         inc prspos
nospace
         clc
         rts
         .bend ; bsfound

bsnotfound
         ; report non-found
         ; value of a is undefined
         sec
         rts

