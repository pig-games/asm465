;---------------------------------------
; parser macros
;---------------------------------------

         ;.include "prsrconst.s"

;---------------------------------------
; chkifalpha(\1, \2)
; check if alpha and jmp to appropriate
; address
;
; \1 = notAlpha: address
; \2 = isAlpha : address
;---------------------------------------
chkifalpha .macro

        cmp #"A"
        bcc \1     ; notAlpha
        cmp #"{"
        bcc \2     ; isAlpha
        cmp #"a"
        bcc \1
        cmp #"["
        bcs \1     ; notAlpha

        .endm

;---------------------------------------
; chkifcomment(\1, \2)
; bra to \2 if not comment or fall thr
;
; \1 = inputln{CBM-@}ptr : address
; \2 = notComment  : address
;---------------------------------------
chkifcomment .macro

        cmp #"/"
        bne \2     ; notComment
        iny        ; next input char
        lda (\1),y
        cmp #"/"
        bne \2     ; notComment

        .endm

;---------------------------------------
; chklinetype(\1, \2, \3)
; if lineType fits the lineTypeFlags
; branch to okLabel or fall through
;
; \1 = lineType      : byte
; \2 = lineTypeFlags : byte
; \3 = okLabel       : address
;---------------------------------------
chklinetype .macro

        lda #\2  ; lineTypeFlags
        eor #$ff
        and \1   ; lineType
        beq \3   ; okLabel

        .endm

;---------------------------------------
; setprspc(\1)
; set PC for line of code
;
; \1 = parsePC : address
;---------------------------------------
setprspc .macro

        lda #<\1  ; parsePC
        sta prsbuf+2
        lda #>\1  ; paserPC
        sta prsbuf+3

        .endm

;---------------------------------------
; setinputln(\1)
; set input line ptr
;
; \1 = inputLine
;---------------------------------------
setinputln .macro

        lda #<\1  ; inputLine
        sta ilnptr
        lda #>\1  ; inputLine
        sta ilnptr+1

        .endm

;---------------------------------------
; setjsraddr(\1, \2)
; self-mod jsr target address
;
; \1 = addrJsr: address
; \2 = targetAddr: address
;---------------------------------------
setjsraddr .macro

        lda #<\2 ; targetAddr
        sta \1+1
        lda #>\2 ; targetAddr
        sta \1+1

        .endm

