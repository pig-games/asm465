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

.namespace rtst

.section zeropage
WritePtr:       .word 0
Wpos:           .word 0
PayloadStart:   .word 0
LengthPtr:      .word 0
ArgKeyPtr:      .word 0
ArgBlockPtr:    .word 0
ArgBlockLen:    .word 0
Value32:        .dword 0
Tmp32:          .dword 0
TmpByte:        .byte 0
.endsection

.section runtime_rtst

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
    sta rtst.WritePtr
    lda #>(RTST_BASE + RTST_HEADER_SIZE)
    sta rtst.WritePtr + 1
    lda #0
    sta rtst.Wpos
    sta rtst.Wpos + 1
    rts
.endproc

emit_byte .proc
    ldy #0
    sta (rtst.WritePtr), y
    inc rtst.WritePtr
    bne :+ 
    inc rtst.WritePtr + 1
:
    inc rtst.Wpos
    bne :+
    inc rtst.Wpos + 1
:
    rts
.endproc

start_record .proc
    jsr emit_byte
    lda #0
    jsr emit_byte
    jsr emit_byte
    lda rtst.WritePtr
    sta rtst.PayloadStart
    lda rtst.WritePtr + 1
    sta rtst.PayloadStart + 1
    lda rtst.WritePtr
    sec
    sbc #2
    sta rtst.LengthPtr
    lda rtst.WritePtr + 1
    sbc #0
    sta rtst.LengthPtr + 1
    rts
.endproc

finish_record .proc
    lda rtst.WritePtr
    sec
    sbc rtst.PayloadStart
    sta rtst.ArgBlockLen
    lda rtst.WritePtr + 1
    sbc rtst.PayloadStart + 1
    sta rtst.ArgBlockLen + 1
    ldy #0
    lda rtst.ArgBlockLen
    sta (rtst.LengthPtr), y
    iny
    lda rtst.ArgBlockLen + 1
    sta (rtst.LengthPtr), y
    jsr store_wpos
    rts
.endproc

store_wpos .proc
    lda rtst.Wpos
    sta RTST_BASE + RTST_HEADER_WPOS
    lda rtst.Wpos + 1
    sta RTST_BASE + RTST_HEADER_WPOS + 1
    rts
.endproc

copy_cstr .proc
    ldy #0
:loop
    lda (rtst.ArgKeyPtr), y
    jsr emit_byte
    beq :done
    iny
    bne :loop
    inc rtst.ArgKeyPtr + 1
    bne :loop
:done
    rts
.endproc

emit_arg_len .proc
    lda rtst.ArgBlockLen
    jsr emit_byte
    lda rtst.ArgBlockLen + 1
    jsr emit_byte
    rts
.endproc

copy_block .proc
    ldy #0
:loop
    lda rtst.ArgBlockLen
    ora rtst.ArgBlockLen + 1
    beq :done
    lda (rtst.ArgBlockPtr), y
    jsr emit_byte
    inc rtst.ArgBlockPtr
    bne :+
    inc rtst.ArgBlockPtr + 1
:
    lda rtst.ArgBlockLen
    bne :dec_only
    dec rtst.ArgBlockLen + 1
    lda #$FF
    sta rtst.ArgBlockLen
    jmp :loop
:dec_only
    dec rtst.ArgBlockLen
    jmp :loop
:done
    rts
.endproc

emit_value32 .proc
    ldx #0
:loop
    lda rtst.Value32, x
    jsr emit_byte
    inx
    cpx #4
    bne :loop
    rts
.endproc

compute_hash .proc
    lda #$05
    sta rtst.Value32
    lda #$15
    sta rtst.Value32 + 1
    lda #0
    sta rtst.Value32 + 2
    sta rtst.Value32 + 3
    ldy #0
:loop
    lda rtst.ArgBlockLen
    ora rtst.ArgBlockLen + 1
    beq :done
    lda (rtst.ArgBlockPtr), y
    sta rtst.TmpByte
    inc rtst.ArgBlockPtr
    bne :+
    inc rtst.ArgBlockPtr + 1
:
    lda rtst.ArgBlockLen
    bne :dec_only
    dec rtst.ArgBlockLen + 1
    lda #$FF
    sta rtst.ArgBlockLen
    jsr hash_step
    jmp :loop
:dec_only
    dec rtst.ArgBlockLen
    jsr hash_step
    jmp :loop
:done
    rts
.endproc

hash_step .proc
    lda rtst.Value32
    sta rtst.Tmp32
    lda rtst.Value32 + 1
    sta rtst.Tmp32 + 1
    lda rtst.Value32 + 2
    sta rtst.Tmp32 + 2
    lda rtst.Value32 + 3
    sta rtst.Tmp32 + 3
    ldx #5
:shift
    asl rtst.Tmp32
    rol rtst.Tmp32 + 1
    rol rtst.Tmp32 + 2
    rol rtst.Tmp32 + 3
    dex
    bne :shift
    clc
    lda rtst.Tmp32
    adc rtst.Value32
    sta rtst.Tmp32
    lda rtst.Tmp32 + 1
    adc rtst.Value32 + 1
    sta rtst.Tmp32 + 1
    lda rtst.Tmp32 + 2
    adc rtst.Value32 + 2
    sta rtst.Tmp32 + 2
    lda rtst.Tmp32 + 3
    adc rtst.Value32 + 3
    sta rtst.Tmp32 + 3
    lda rtst.Tmp32
    eor rtst.TmpByte
    sta rtst.Value32
    lda rtst.Tmp32 + 1
    sta rtst.Value32 + 1
    lda rtst.Tmp32 + 2
    sta rtst.Value32 + 2
    lda rtst.Tmp32 + 3
    sta rtst.Value32 + 3
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

.endsection ; runtime_rtst
.endnamespace ; rtst

.macro __RTST_LOAD_PTR slot, ptr
    .if \ptr = 0
        lda #0
        sta rtst.\slot
        sta rtst.\slot + 1
    .else
        lda #<\ptr
        sta rtst.\slot
        lda #>\ptr
        sta rtst.\slot + 1
    .endif
.endmacro

.macro __RTST_STORE_LEN value
    lda #((\value) & $FF)
    sta rtst.ArgBlockLen
    lda #((\value >> 8) & $FF)
    sta rtst.ArgBlockLen + 1
.endmacro

.macro __RTST_STORE_VALUE32 value
    lda #((\value) & $FF)
    sta rtst.Value32
    lda #((\value >> 8) & $FF)
    sta rtst.Value32 + 1
    lda #((\value >> 16) & $FF)
    sta rtst.Value32 + 2
    lda #((\value >> 24) & $FF)
    sta rtst.Value32 + 3
.endmacro

.macro __RTST_INC_FIELD offset
    inc RTST_BASE + \offset
    bne :+
    inc RTST_BASE + \offset + 1
:+
.endmacro

.macro __RTST_COPY_STRING_OR_ZERO ptr
    .if \ptr = 0
        lda #0
        jsr rtst.emit_byte
    .else
        __RTST_LOAD_PTR ArgKeyPtr, \ptr
        jsr rtst.copy_cstr
    .endif
.endmacro

.macro RTST_BEGIN
    jsr rtst.begin
.endmacro

.macro RTST_END
    jsr rtst.emit_end_record
\@wait:
    jmp \@wait
.endmacro

.macro TEST_CASE_BEGIN name_ptr
    lda #RTST_RECORD_CASE_BEGIN
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \name_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
    __RTST_INC_FIELD RTST_HEADER_TOTAL
.endmacro

.macro TEST_CASE_OK code, message_ptr
    lda #RTST_RECORD_CASE_OK
    jsr rtst.start_record
    lda #((\code) & $FF)
    jsr rtst.emit_byte
    __RTST_COPY_STRING_OR_ZERO \message_ptr
    jsr rtst.finish_record
    __RTST_INC_FIELD RTST_HEADER_PASS
.endmacro

.macro TEST_CASE_FAIL code, message_ptr
    lda #RTST_RECORD_CASE_FAIL
    jsr rtst.start_record
    lda #((\code) & $FF)
    jsr rtst.emit_byte
    __RTST_COPY_STRING_OR_ZERO \message_ptr
    jsr rtst.finish_record
    __RTST_INC_FIELD RTST_HEADER_FAIL
.endmacro

.macro LOG_MSG message_ptr
    lda #RTST_RECORD_MSG
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \message_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
.endmacro

.macro LOG_ASSERT message_ptr
    lda #RTST_RECORD_ASSERT
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \message_ptr
    jsr rtst.copy_cstr
    jsr rtst.finish_record
.endmacro

.macro LOG_KV key_ptr, value
    lda #RTST_RECORD_ACT_KV
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \key_ptr
    jsr rtst.copy_cstr
    __RTST_STORE_VALUE32 \value
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

.macro LOG_TIME key_ptr, value
    lda #RTST_RECORD_ACT_TIME
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \key_ptr
    jsr rtst.copy_cstr
    __RTST_STORE_VALUE32 \value
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

.macro LOG_MEM key_ptr, data_ptr, length
    lda #RTST_RECORD_ACT_MEM
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \key_ptr
    jsr rtst.copy_cstr
    __RTST_STORE_LEN \length
    jsr rtst.emit_arg_len
    __RTST_LOAD_PTR ArgBlockPtr, \data_ptr
    jsr rtst.copy_block
    jsr rtst.finish_record
.endmacro

.macro LOG_HASH key_ptr, data_ptr, length
    lda #RTST_RECORD_ACT_HASH
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \key_ptr
    jsr rtst.copy_cstr
    __RTST_STORE_LEN \length
    __RTST_LOAD_PTR ArgBlockPtr, \data_ptr
    jsr rtst.compute_hash
    jsr rtst.emit_value32
    jsr rtst.finish_record
.endmacro

.macro LOG_REGS key_ptr, regs_ptr
    lda #RTST_RECORD_ACT_REGS
    jsr rtst.start_record
    __RTST_LOAD_PTR ArgKeyPtr, \key_ptr
    jsr rtst.copy_cstr
    __RTST_STORE_LEN 7
    __RTST_LOAD_PTR ArgBlockPtr, \regs_ptr
    jsr rtst.copy_block
    jsr rtst.finish_record
.endmacro

.endif ; include guard
