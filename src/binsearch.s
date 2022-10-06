

; Perform bin search in a block of data
; in:
;   x = start column of mnemonic text
;   bschixr = size of data (columns)
; Sets C-flag if not found (else unset)
; out:
;   x = pos behind last char of mnem
;   bschptr = addr of match (if c)
binsearch
         ;.block
         ; set left and right index
         ; last row of table

         lda #0
         sta bschixl

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

bsacalc  ; modify addr: #setjsraddr
         jsr $ffff

; add start addr of mnem tab to (mid*6)
         lda bschptr ; ld l byte of ptr
         clc
; add low byte of mnemonics table start
         adc bschdptr
; store back to low byte of ptr
         sta bschptr
; load high byte of ptr
         lda bschptr+1
; add high byte of mnemonics table start

         adc bschdptr+1
; store back to high byte of ptr
         sta bschptr+1

; start comp cur inp with candidate mnem

bscomploop
         .block

         lda (bschptr),y
         beq bsnotfound      ; found '@'

         lda (bschiptr),y   ; get i char
         cmp #$ff
         beq bsnotfound; fnd end of line
         cmp #$20 ; " "
         bne nospace
         jmp bsfound
nospace
         ; scan for mnemonic

         cmp (bschptr),y

         bcc islower
         beq isequal
ishigher               ; else is higher
         ldx bschixm
         inx
         stx bschixl

         ldx $09 ; reset to start pos
         ldy #0
         jmp bsloop
islower
         ldx bschixm
         dex
         stx bschixr

         ldx $09
         ldy #0
         jmp bsloop
isequal
         lda (bschptr),y
         cpy #3
         beq bsfound

         inx ; input chr ix to next char
         iny ; cand chr ixx to next char

         jmp bscomploop

;TODO: write FF to label_buf,0 when
; found mnemonic instead of label
         ; jmp bsnotfound

         .bend ; compareloop
bsfound
         .block
         lda (bschiptr),y
         cmp #$20   ;" "
         bne nospace
         inx
nospace
         clc
         rts
         .bend     ; found

bsnotfound
         ; report non-found
         ; value of a is undefined
         sec
         rts

  ;       .bend ; binsearch

