.enc "ascii"
.cdef " ~", 32

TEST_RTST_INC :?= false
.if !TEST_RTST_INC
TEST_RTST_INC := true

; -------------------------------------------------------------------------------------
; Runtime Test Stream (RTST) helper macros for 64tass tests.
; These helpers mirror the `runtime_sdk::rtst` Rust module and emit record streams that
; match the documented protocol. Define `rtst.BASE` before invoking `rtst.begin`.
; -------------------------------------------------------------------------------------

rtst .namespace

VERSION        := $01
HEADER_SIZE    := $10
HEADER_MAGIC0  := $00
HEADER_MAGIC1  := $01
HEADER_MAGIC2  := $02
HEADER_MAGIC3  := $03
HEADER_VERSION := $04
HEADER_STATE   := $05
HEADER_WPOS    := $06
HEADER_TOTAL   := $08
HEADER_PASS    := $0A
HEADER_FAIL    := $0C
MAGIC_R := $52
MAGIC_T := $54
MAGIC_S := $53
STATE_PENDING  := $00
STATE_RUNNING  := $01
STATE_DONE     := $02
STATE_ABORTED  := $03
RECORD_CASE_BEGIN := $01
RECORD_CASE_OK    := $02
RECORD_CASE_FAIL  := $03
RECORD_ASSERT     := $04
RECORD_MSG        := $05
RECORD_ACT_KV     := $10
RECORD_ACT_MEM    := $11
RECORD_ACT_HASH   := $12
RECORD_ACT_REGS   := $13
RECORD_ACT_TIME   := $14
RECORD_END        := $FF
ZP_BASE :?= $40
WRITE_PTR      := rtst.ZP_BASE
WPOS           := rtst.WRITE_PTR + 2
PAYLOAD_START  := rtst.WPOS + 2
LENGTH_PTR     := rtst.PAYLOAD_START + 2
ARG_KEY_PTR    := rtst.LENGTH_PTR + 2
ARG_BLOCK_PTR  := rtst.ARG_KEY_PTR + 2
ARG_BLOCK_LEN  := rtst.ARG_BLOCK_PTR + 2
VALUE32        := rtst.ARG_BLOCK_LEN + 2
TMP32          := rtst.VALUE32 + 4
TMPBYTE        := rtst.TMP32 + 4

_begin .proc
    lda #'R'
    sta rtst.BASE + rtst.HEADER_MAGIC0
    lda #rtst.MAGIC_T
    sta rtst.BASE + rtst.HEADER_MAGIC1
    lda #rtst.MAGIC_S
    sta rtst.BASE + rtst.HEADER_MAGIC2
    lda #rtst.MAGIC_T
    sta rtst.BASE + rtst.HEADER_MAGIC3
    lda #rtst.VERSION
    sta rtst.BASE + rtst.HEADER_VERSION
    lda #rtst.STATE_RUNNING
    sta rtst.BASE + rtst.HEADER_STATE
    lda #0
    sta rtst.BASE + rtst.HEADER_WPOS
    sta rtst.BASE + rtst.HEADER_WPOS + 1
    sta rtst.BASE + rtst.HEADER_TOTAL
    sta rtst.BASE + rtst.HEADER_TOTAL + 1
    sta rtst.BASE + rtst.HEADER_PASS
    sta rtst.BASE + rtst.HEADER_PASS + 1
    sta rtst.BASE + rtst.HEADER_FAIL
    sta rtst.BASE + rtst.HEADER_FAIL + 1
    sta rtst.BASE + rtst.HEADER_SIZE - 2
    sta rtst.BASE + rtst.HEADER_SIZE - 1
    lda #<(rtst.BASE + rtst.HEADER_SIZE)
    sta rtst.WRITE_PTR
    lda #>(rtst.BASE + rtst.HEADER_SIZE)
    sta rtst.WRITE_PTR + 1
    lda #0
    sta rtst.WPOS
    sta rtst.WPOS + 1
    rts
.endproc

emitByte .proc
    ldy #0
    sta (rtst.WRITE_PTR), y
    inc rtst.WRITE_PTR
    bne write_ptr_ok
    inc rtst.WRITE_PTR + 1
write_ptr_ok
    inc rtst.WPOS
    bne wpos_ok
    inc rtst.WPOS + 1
wpos_ok
    rts
.endproc

startRecord .proc
    jsr emitByte
    lda #0
    jsr emitByte
    jsr emitByte
    lda rtst.WRITE_PTR
    sta rtst.PAYLOAD_START
    lda rtst.WRITE_PTR + 1
    sta rtst.PAYLOAD_START + 1
    lda rtst.WRITE_PTR
    sec
    sbc #2
    sta rtst.LENGTH_PTR
    lda rtst.WRITE_PTR + 1
    sbc #0
    sta rtst.LENGTH_PTR + 1
    rts
.endproc

finishRecord .proc
    lda rtst.WRITE_PTR
    sec
    sbc rtst.PAYLOAD_START
    sta rtst.ARG_BLOCK_LEN
    lda rtst.WRITE_PTR + 1
    sbc rtst.PAYLOAD_START + 1
    sta rtst.ARG_BLOCK_LEN + 1
    ldy #0
    lda rtst.ARG_BLOCK_LEN
    sta (rtst.LENGTH_PTR), y
    iny
    lda rtst.ARG_BLOCK_LEN + 1
    sta (rtst.LENGTH_PTR), y
    jsr storeWPos
    rts
.endproc

storeWPos .proc
    lda rtst.WPOS
    sta rtst.BASE + rtst.HEADER_WPOS
    lda rtst.WPOS + 1
    sta rtst.BASE + rtst.HEADER_WPOS + 1
    rts
.endproc

copyCStr .proc
    ldy #0
copy_cstr_loop
    lda (rtst.ARG_KEY_PTR), y
    sta rtst.TMPBYTE
    jsr emitByte
    lda rtst.TMPBYTE
    beq copy_cstr_done
    inc rtst.ARG_KEY_PTR
    bne copy_cstr_loop
    inc rtst.ARG_KEY_PTR + 1
    bne copy_cstr_loop
copy_cstr_done
    rts
.endproc

emitArgLen .proc
    lda rtst.ARG_BLOCK_LEN
    jsr emitByte
    lda rtst.ARG_BLOCK_LEN + 1
    jsr emitByte
    rts
.endproc

copyBlock .proc
    ldy #0
copy_block_loop
    lda rtst.ARG_BLOCK_LEN
    ora rtst.ARG_BLOCK_LEN + 1
    beq copy_block_done
    lda (rtst.ARG_BLOCK_PTR), y
    jsr emitByte
    inc rtst.ARG_BLOCK_PTR
    bne copy_block_ptr_ok
    inc rtst.ARG_BLOCK_PTR + 1
copy_block_ptr_ok
    lda rtst.ARG_BLOCK_LEN
    bne copy_block_dec_only
    dec rtst.ARG_BLOCK_LEN + 1
    lda #$FF
    sta rtst.ARG_BLOCK_LEN
    jmp copy_block_loop
copy_block_dec_only
    dec rtst.ARG_BLOCK_LEN
    jmp copy_block_loop
copy_block_done
    rts
.endproc

emitValue32 .proc
    ldx #0
emit_value32_loop
    lda rtst.VALUE32, x
    jsr emitByte
    inx
    cpx #4
    bne emit_value32_loop
    rts
.endproc

computeHash .proc
    lda #$05
    sta rtst.VALUE32
    lda #$15
    sta rtst.VALUE32 + 1
    lda #0
    sta rtst.VALUE32 + 2
    sta rtst.VALUE32 + 3
    ldy #0
compute_hash_loop
    lda rtst.ARG_BLOCK_LEN
    ora rtst.ARG_BLOCK_LEN + 1
    beq compute_hash_done
    lda (rtst.ARG_BLOCK_PTR), y
    sta rtst.TMPBYTE
    inc rtst.ARG_BLOCK_PTR
    bne compute_hash_ptr_ok
    inc rtst.ARG_BLOCK_PTR + 1
compute_hash_ptr_ok
    lda rtst.ARG_BLOCK_LEN
    bne compute_hash_dec_only
    dec rtst.ARG_BLOCK_LEN + 1
    lda #$FF
    sta rtst.ARG_BLOCK_LEN
    jsr hashStep
    jmp compute_hash_loop
compute_hash_dec_only
    dec rtst.ARG_BLOCK_LEN
    jsr hashStep
    jmp compute_hash_loop
compute_hash_done
    rts
.endproc

hashStep .proc
    lda rtst.VALUE32
    sta rtst.TMP32
    lda rtst.VALUE32 + 1
    sta rtst.TMP32 + 1
    lda rtst.VALUE32 + 2
    sta rtst.TMP32 + 2
    lda rtst.VALUE32 + 3
    sta rtst.TMP32 + 3
    ldx #5
hash_step_shift
    asl rtst.TMP32
    rol rtst.TMP32 + 1
    rol rtst.TMP32 + 2
    rol rtst.TMP32 + 3
    dex
    bne hash_step_shift
    clc
    lda rtst.TMP32
    adc rtst.VALUE32
    sta rtst.TMP32
    lda rtst.TMP32 + 1
    adc rtst.VALUE32 + 1
    sta rtst.TMP32 + 1
    lda rtst.TMP32 + 2
    adc rtst.VALUE32 + 2
    sta rtst.TMP32 + 2
    lda rtst.TMP32 + 3
    adc rtst.VALUE32 + 3
    sta rtst.TMP32 + 3
    lda rtst.TMP32
    eor rtst.TMPBYTE
    sta rtst.VALUE32
    lda rtst.TMP32 + 1
    sta rtst.VALUE32 + 1
    lda rtst.TMP32 + 2
    sta rtst.VALUE32 + 2
    lda rtst.TMP32 + 3
    sta rtst.VALUE32 + 3
    rts
.endproc

emitEndRecord .proc
    lda #rtst.RECORD_END
    jsr startRecord
    jsr finishRecord
    lda #rtst.STATE_DONE
    sta rtst.BASE + rtst.HEADER_STATE
    rts
.endproc

loadPtr .macro slot, ptr
    .if \ptr = 0
        lda #0
        sta \slot
        sta \slot + 1
    .else
        lda #<\ptr
        sta \slot
        lda #>\ptr
        sta \slot + 1
    .endif
.endmacro

storeLen .macro value
    lda #((\value) & $FF)
    sta rtst.ARG_BLOCK_LEN
    lda #((\value >> 8) & $FF)
    sta rtst.ARG_BLOCK_LEN + 1
.endmacro

storeValue32 .macro value
    lda #((\value) & $FF)
    sta rtst.VALUE32
    lda #((\value >> 8) & $FF)
    sta rtst.VALUE32 + 1
    lda #((\value >> 16) & $FF)
    sta rtst.VALUE32 + 2
    lda #((\value >> 24) & $FF)
    sta rtst.VALUE32 + 3
.endmacro

incField .macro offset
    inc rtst.BASE + \offset
    bne inc_field_ok
    inc rtst.BASE + \offset + 1
inc_field_ok:
.endmacro

copyStringOrZero .macro ptr
    .if \ptr = 0
        lda #0
        jsr rtst.emitByte
    .else
        .rtst.loadPtr rtst.ARG_KEY_PTR, \ptr
        jsr rtst.copyCStr
    .endif
.endmacro

begin .macro
    jsr rtst._begin
.endmacro

end .macro
    jsr rtst.emitEndRecord
\@wait:
    jmp \@wait
.endmacro

.endnamespace ; rtst

ctest .namespace

begin .macro name_ptr
    lda #rtst.RECORD_CASE_BEGIN
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \name_ptr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_TOTAL
.endmacro

OK .macro code, message_ptr
    lda #rtst.RECORD_CASE_OK
    jsr rtst.startRecord
    lda #((\code) & $FF)
    jsr rtst.emitByte
    .rtst.copyStringOrZero \message_ptr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_PASS
.endmacro

FAIL .macro code, message_ptr
    lda #rtst.RECORD_CASE_FAIL
    jsr rtst.startRecord
    lda #((\code) & $FF)
    jsr rtst.emitByte
    .rtst.copyStringOrZero \message_ptr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_FAIL
.endmacro

logMsg .macro message_ptr
    lda #rtst.RECORD_MSG
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \message_ptr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
.endmacro

logAssert .macro message_ptr
    lda #rtst.RECORD_ASSERT
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \message_ptr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
.endmacro

logKV .macro key_ptr, value
    lda #rtst.RECORD_ACT_KV
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \key_ptr
    jsr rtst.copyCStr
    .rtst.storeValue32 \value
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

logTime .macro key_ptr, value
    lda #rtst.RECORD_ACT_TIME
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \key_ptr
    jsr rtst.copyCStr
    .rtst.storeValue32 \value
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

logMem .macro key_ptr, data_ptr, length
    lda #rtst.RECORD_ACT_MEM
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \key_ptr
    jsr rtst.copyCStr
    .rtst.storeLen \length
    jsr rtst.emitArgLen
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \data_ptr
    jsr rtst.copyBlock
    jsr rtst.finishRecord
.endmacro

logHash .macro key_ptr, data_ptr, length
    lda #rtst.RECORD_ACT_HASH
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \key_ptr
    jsr rtst.copyCStr
    .rtst.storeLen \length
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \data_ptr
    jsr rtst.computeHash
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

logRegs .macro key_ptr, regs_ptr
    lda #rtst.RECORD_ACT_REGS
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \key_ptr
    jsr rtst.copyCStr
    .rtst.storeLen 7
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \regs_ptr
    jsr rtst.copyBlock
    jsr rtst.finishRecord
.endmacro

.endnamespace ;ctest

.endif ; include guard
