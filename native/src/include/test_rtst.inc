TEST_RTST_INC :?= false
.if !TEST_RTST_INC
TEST_RTST_INC := true

; -------------------------------------------------------------------------------------
; Runtime Test Stream (RTST) helper macros for 64tass tests.
; These helpers mirror the `runtime_sdk::rtst` Rust module and emit record streams that
; match the documented protocol. Define `RTST_BASE` before invoking `RTST_BEGIN`.
; -------------------------------------------------------------------------------------

RTST_VERSION        := $01
RTST_HEADER_SIZE    := $10

RTST_HEADER_MAGIC0  := $00
RTST_HEADER_MAGIC1  := $01
RTST_HEADER_MAGIC2  := $02
RTST_HEADER_MAGIC3  := $03
RTST_HEADER_VERSION := $04
RTST_HEADER_STATE   := $05
RTST_HEADER_WPOS    := $06
RTST_HEADER_TOTAL   := $08
RTST_HEADER_PASS    := $0A
RTST_HEADER_FAIL    := $0C

RTST_STATE_PENDING  := $00
RTST_STATE_RUNNING  := $01
RTST_STATE_DONE     := $02
RTST_STATE_ABORTED  := $03

RTST_RECORD_CASE_BEGIN := $01
RTST_RECORD_CASE_OK    := $02
RTST_RECORD_CASE_FAIL  := $03
RTST_RECORD_ASSERT     := $04
RTST_RECORD_MSG        := $05
RTST_RECORD_ACT_KV     := $10
RTST_RECORD_ACT_MEM    := $11
RTST_RECORD_ACT_HASH   := $12
RTST_RECORD_ACT_REGS   := $13
RTST_RECORD_ACT_TIME   := $14
RTST_RECORD_END        := $FF

RTST_ZP_BASE :?= $40
RTST_WRITE_PTR      := RTST_ZP_BASE
RTST_WPOS           := RTST_WRITE_PTR + 2
RTST_PAYLOAD_START  := RTST_WPOS + 2
RTST_LENGTH_PTR     := RTST_PAYLOAD_START + 2
RTST_ARG_KEY_PTR    := RTST_LENGTH_PTR + 2
RTST_ARG_BLOCK_PTR  := RTST_ARG_KEY_PTR + 2
RTST_ARG_BLOCK_LEN  := RTST_ARG_BLOCK_PTR + 2
RTST_VALUE32        := RTST_ARG_BLOCK_LEN + 2
RTST_TMP32          := RTST_VALUE32 + 4
RTST_TMPBYTE        := RTST_TMP32 + 4

rtst .namespace

begin .proc
    lda #'R'
    sta RTST_BASE + RTST_HEADER_MAGIC0
    lda #'T'
    sta RTST_BASE + RTST_HEADER_MAGIC1
    lda #'S'
    sta RTST_BASE + RTST_HEADER_MAGIC2
    lda #'T'
    sta RTST_BASE + RTST_HEADER_MAGIC3
    lda #RTST_VERSION
    sta RTST_BASE + RTST_HEADER_VERSION
    lda #RTST_STATE_RUNNING
    sta RTST_BASE + RTST_HEADER_STATE
    lda #0
    sta RTST_BASE + RTST_HEADER_WPOS
    sta RTST_BASE + RTST_HEADER_WPOS + 1
    sta RTST_BASE + RTST_HEADER_TOTAL
    sta RTST_BASE + RTST_HEADER_TOTAL + 1
    sta RTST_BASE + RTST_HEADER_PASS
    sta RTST_BASE + RTST_HEADER_PASS + 1
    sta RTST_BASE + RTST_HEADER_FAIL
    sta RTST_BASE + RTST_HEADER_FAIL + 1
    sta RTST_BASE + RTST_HEADER_SIZE - 2
    sta RTST_BASE + RTST_HEADER_SIZE - 1
    lda #<(RTST_BASE + RTST_HEADER_SIZE)
    sta RTST_WRITE_PTR
    lda #>(RTST_BASE + RTST_HEADER_SIZE)
    sta RTST_WRITE_PTR + 1
    lda #0
    sta RTST_WPOS
    sta RTST_WPOS + 1
    rts
.endproc

emit_byte .proc
    ldy #0
    sta (RTST_WRITE_PTR), y
    inc RTST_WRITE_PTR
    bne rtst_write_ptr_ok
    inc RTST_WRITE_PTR + 1
rtst_write_ptr_ok:
    inc RTST_WPOS
    bne rtst_wpos_ok
    inc RTST_WPOS + 1
rtst_wpos_ok:
    rts
.endproc

start_record .proc
    jsr emit_byte
    lda #0
    jsr emit_byte
    jsr emit_byte
    lda RTST_WRITE_PTR
    sta RTST_PAYLOAD_START
    lda RTST_WRITE_PTR + 1
    sta RTST_PAYLOAD_START + 1
    lda RTST_WRITE_PTR
    sec
    sbc #2
    sta RTST_LENGTH_PTR
    lda RTST_WRITE_PTR + 1
    sbc #0
    sta RTST_LENGTH_PTR + 1
    rts
.endproc

finish_record .proc
    lda RTST_WRITE_PTR
    sec
    sbc RTST_PAYLOAD_START
    sta RTST_ARG_BLOCK_LEN
    lda RTST_WRITE_PTR + 1
    sbc RTST_PAYLOAD_START + 1
    sta RTST_ARG_BLOCK_LEN + 1
    ldy #0
    lda RTST_ARG_BLOCK_LEN
    sta (RTST_LENGTH_PTR), y
    iny
    lda RTST_ARG_BLOCK_LEN + 1
    sta (RTST_LENGTH_PTR), y
    jsr store_wpos
    rts
.endproc

store_wpos .proc
    lda RTST_WPOS
    sta RTST_BASE + RTST_HEADER_WPOS
    lda RTST_WPOS + 1
    sta RTST_BASE + RTST_HEADER_WPOS + 1
    rts
.endproc

copy_cstr .proc
    ldy #0
copy_cstr_loop:
    lda (RTST_ARG_KEY_PTR), y
    jsr emit_byte
    beq copy_cstr_done
    iny
    bne copy_cstr_loop
    inc RTST_ARG_KEY_PTR + 1
    bne copy_cstr_loop
copy_cstr_done:
    rts
.endproc

emit_arg_len .proc
    lda RTST_ARG_BLOCK_LEN
    jsr emit_byte
    lda RTST_ARG_BLOCK_LEN + 1
    jsr emit_byte
    rts
.endproc

copy_block .proc
    ldy #0
copy_block_loop:
    lda RTST_ARG_BLOCK_LEN
    ora RTST_ARG_BLOCK_LEN + 1
    beq copy_block_done
    lda (RTST_ARG_BLOCK_PTR), y
    jsr emit_byte
    inc RTST_ARG_BLOCK_PTR
    bne copy_block_ptr_ok
    inc RTST_ARG_BLOCK_PTR + 1
copy_block_ptr_ok:
    lda RTST_ARG_BLOCK_LEN
    bne copy_block_dec_only
    dec RTST_ARG_BLOCK_LEN + 1
    lda #$FF
    sta RTST_ARG_BLOCK_LEN
    jmp copy_block_loop
copy_block_dec_only:
    dec RTST_ARG_BLOCK_LEN
    jmp copy_block_loop
copy_block_done:
    rts
.endproc

emit_value32 .proc
    ldx #0
emit_value32_loop:
    lda RTST_VALUE32, x
    jsr emit_byte
    inx
    cpx #4
    bne emit_value32_loop
    rts
.endproc

compute_hash .proc
    lda #$05
    sta RTST_VALUE32
    lda #$15
    sta RTST_VALUE32 + 1
    lda #0
    sta RTST_VALUE32 + 2
    sta RTST_VALUE32 + 3
    ldy #0
compute_hash_loop:
    lda RTST_ARG_BLOCK_LEN
    ora RTST_ARG_BLOCK_LEN + 1
    beq compute_hash_done
    lda (RTST_ARG_BLOCK_PTR), y
    sta RTST_TMPBYTE
    inc RTST_ARG_BLOCK_PTR
    bne compute_hash_ptr_ok
    inc RTST_ARG_BLOCK_PTR + 1
compute_hash_ptr_ok:
    lda RTST_ARG_BLOCK_LEN
    bne compute_hash_dec_only
    dec RTST_ARG_BLOCK_LEN + 1
    lda #$FF
    sta RTST_ARG_BLOCK_LEN
    jsr hash_step
    jmp compute_hash_loop
compute_hash_dec_only:
    dec RTST_ARG_BLOCK_LEN
    jsr hash_step
    jmp compute_hash_loop
compute_hash_done:
    rts
.endproc

hash_step .proc
    lda RTST_VALUE32
    sta RTST_TMP32
    lda RTST_VALUE32 + 1
    sta RTST_TMP32 + 1
    lda RTST_VALUE32 + 2
    sta RTST_TMP32 + 2
    lda RTST_VALUE32 + 3
    sta RTST_TMP32 + 3
    ldx #5
hash_step_shift:
    asl RTST_TMP32
    rol RTST_TMP32 + 1
    rol RTST_TMP32 + 2
    rol RTST_TMP32 + 3
    dex
    bne hash_step_shift
    clc
    lda RTST_TMP32
    adc RTST_VALUE32
    sta RTST_TMP32
    lda RTST_TMP32 + 1
    adc RTST_VALUE32 + 1
    sta RTST_TMP32 + 1
    lda RTST_TMP32 + 2
    adc RTST_VALUE32 + 2
    sta RTST_TMP32 + 2
    lda RTST_TMP32 + 3
    adc RTST_VALUE32 + 3
    sta RTST_TMP32 + 3
    lda RTST_TMP32
    eor RTST_TMPBYTE
    sta RTST_VALUE32
    lda RTST_TMP32 + 1
    sta RTST_VALUE32 + 1
    lda RTST_TMP32 + 2
    sta RTST_VALUE32 + 2
    lda RTST_TMP32 + 3
    sta RTST_VALUE32 + 3
    rts
.endproc

emit_end_record .proc
    lda #RTST_RECORD_END
    jsr start_record
    jsr finish_record
    lda #RTST_STATE_DONE
    sta RTST_BASE + RTST_HEADER_STATE
    rts
.endproc
.endnamespace ; rtst

RTST_LOAD_PTR .macro slot, ptr
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

RTST_STORE_LEN .macro value
    lda #((\value) & $FF)
    sta RTST_ARG_BLOCK_LEN
    lda #((\value >> 8) & $FF)
    sta RTST_ARG_BLOCK_LEN + 1
.endmacro

RTST_STORE_VALUE32 .macro value
    lda #((\value) & $FF)
    sta RTST_VALUE32
    lda #((\value >> 8) & $FF)
    sta RTST_VALUE32 + 1
    lda #((\value >> 16) & $FF)
    sta RTST_VALUE32 + 2
    lda #((\value >> 24) & $FF)
    sta RTST_VALUE32 + 3
.endmacro

RTST_INC_FIELD .macro offset
    inc RTST_BASE + \offset
    bne rtst_inc_field_ok
    inc RTST_BASE + \offset + 1
rtst_inc_field_ok:
.endmacro

RTST_COPY_STRING_OR_ZERO .macro ptr
    .if \ptr = 0
        lda #0
        jsr rtst.emit_byte
    .else
        RTST_LOAD_PTR RTST_ARG_KEY_PTR, \ptr
        jsr rtst.copy_cstr
    .endif
.endmacro

RTST_BEGIN .macro
    jsr rtst.begin
.endmacro

RTST_END .macro
    jsr rtst.emit_end_record
\@wait:
    jmp \@wait
.endmacro

TEST_CASE_BEGIN .macro name_ptr
    lda #RTST_RECORD_CASE_BEGIN
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \name_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
    RTST_INC_FIELD RTST_HEADER_TOTAL
.endmacro

TEST_CASE_OK .macro code, message_ptr
    lda #RTST_RECORD_CASE_OK
    jsr rtst.start_record
    lda #((\code) & $FF)
    jsr rtst.emit_byte
    RTST_COPY_STRING_OR_ZERO \message_ptr
    jsr rtst.finish_record
    RTST_INC_FIELD RTST_HEADER_PASS
.endmacro

TEST_CASE_FAIL .macro code, message_ptr
    lda #RTST_RECORD_CASE_FAIL
    jsr rtst.start_record
    lda #((\code) & $FF)
    jsr rtst.emit_byte
    RTST_COPY_STRING_OR_ZERO \message_ptr
    jsr rtst.finish_record
    RTST_INC_FIELD RTST_HEADER_FAIL
.endmacro

LOG_MSG .macro message_ptr
    lda #RTST_RECORD_MSG
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \message_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
.endmacro

LOG_ASSERT .macro message_ptr
    lda #RTST_RECORD_ASSERT
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \message_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
.endmacro

LOG_KV .macro key_ptr, value
    lda #RTST_RECORD_ACT_KV
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \key_ptr
    jsr rtst.copy_cstr
    RTST_STORE_VALUE32 \value
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

LOG_TIME .macro key_ptr, value
    lda #RTST_RECORD_ACT_TIME
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \key_ptr
    jsr rtst.copy_cstr
    RTST_STORE_VALUE32 \value
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

LOG_MEM .macro key_ptr, data_ptr, length
    lda #RTST_RECORD_ACT_MEM
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \key_ptr
    jsr rtst.copy_cstr
    RTST_STORE_LEN \length
    jsr rtst.emit_arg_len
    RTST_LOAD_PTR RTST_ARG_BLOCK_PTR, \data_ptr
    jsr rtst.copy_block
    jsr rtst.finish_record
.endmacro

LOG_HASH .macro key_ptr, data_ptr, length
    lda #RTST_RECORD_ACT_HASH
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \key_ptr
    jsr rtst.copy_cstr
    RTST_STORE_LEN \length
    RTST_LOAD_PTR RTST_ARG_BLOCK_PTR, \data_ptr
    jsr rtst.compute_hash
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

LOG_REGS .macro key_ptr, regs_ptr
    lda #RTST_RECORD_ACT_REGS
    jsr rtst.start_record
    RTST_LOAD_PTR RTST_ARG_KEY_PTR, \key_ptr
    jsr rtst.copy_cstr
    RTST_STORE_LEN 7
    RTST_LOAD_PTR RTST_ARG_BLOCK_PTR, \regs_ptr
    jsr rtst.copy_block
    jsr rtst.finish_record
.endmacro

.endif ; include guard
