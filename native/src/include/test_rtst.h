.enc "ascii"
.cdef " ~", 32

TEST_RTST_INC :?= false
.if !TEST_RTST_INC
TEST_RTST_INC := true

; -------------------------------------------------------------------------------------
; Runtime Test Stream (RTST) helper macros for 64tass tests.
; These routines mirror the `runtime_sdk::rtst` Rust module so the host parser and
; the 6502 side share a byte-for-byte compatible protocol.
;
; Usage basics
; ------------
; 1. Choose a buffer base address in RAM (set `rtst.BASE = $C000`, for example).
; 2. At the start of your test, call `rtst.begin` to write the RTST header and
;    reset the internal pointers.
; 3. Emit records using the provided macros (`.rtst.testCaseBegin`,
;    `.rtst.testCaseOk`, `.rtst.logKv`, etc.). Each macro takes care of writing
;    the record ID, payload length, and payload bytes.
; 4. End the stream by calling `rtst.end`, which writes the END record and parks
;    the CPU in a `wait_loop` so the host has time to read the buffer.
;
; Key concepts
; ------------
; - `rtst.BASE` must point at a contiguous chunk of RAM large enough to hold the
;   header (16 bytes) and all emitted records. The default layout reserves ZP
;   workspace starting at `rtst.ZP_BASE` ($40 by default) for pointers and
;   temporary values.
; - Strings must be declared with `.null` (e.g., `case_name .null "math::add"`).
;   Pass the label to the macros; they copy the string and automatically append
;   the trailing NUL byte in the RTST stream.
; - All macros follow the naming conventions described in `native/AGENTS.md`:
;   lowerCamelCase names, explicit labels before `.macro`, and `.namespace`
;   blocks with `; namespace` trailing comments.
;
; Available macros (prefixed with `.rtst.` when included via `.include"test_rtst.h"`):
; - `begin` / `end`: initialise/finish a stream.
; - `testCaseBegin namePtr`
; - `testCaseOk code, msgPtr`
; - `testCaseFail code, msgPtr`
; - `logMsg msgPtr`
; - `logAssert msgPtr`
; - `logKv keyPtr, value`
; - `logTime keyPtr, cycles`
; - `logMem keyPtr, dataPtr, length`
; - `logHash keyPtr, dataPtr, length`
; - `logRegs keyPtr, regsPtr`
;
; Each helper assumes the test has already disabled interrupts or otherwise
; ensured deterministic execution while the RTST buffer is being written.
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

; Initialise the RTST header and reset pointers. Call once per test start.
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

; Write A to the buffer and advance the write cursor + header WPOS.
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

; Reserve space for a new record header (id + len) and remember payload start.
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

; Patch the record length field and update header WPOS based on write ptr.
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

; Mirror the ZP WPOS cursor into the on-stream header fields.
storeWPos .proc
    lda rtst.WPOS
    sta rtst.BASE + rtst.HEADER_WPOS
    lda rtst.WPOS + 1
    sta rtst.BASE + rtst.HEADER_WPOS + 1
    rts
.endproc

; Copy a NUL-terminated string from ARG_KEY_PTR into the stream.
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

; Emit the current ARG_BLOCK_LEN as a little-endian length prefix.
emitArgLen .proc
    lda rtst.ARG_BLOCK_LEN
    jsr emitByte
    lda rtst.ARG_BLOCK_LEN + 1
    jsr emitByte
    rts
.endproc

; Copy ARG_BLOCK_LEN bytes from ARG_BLOCK_PTR into the stream.
copyBlock .proc
    ldy #0
copy_block_loop
    lda rtst.ARG_BLOCK_LEN
    ora rtst.ARG_BLOCK_LEN + 1
    beq copy_block_done
    lda (rtst.ARG_BLOCK_PTR), y
    ; Stash byte so hashStep can consume it after pointer/len bookkeeping.
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

; Emit VALUE32 (little-endian u32).
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

; Compute a rolling hash over ARG_BLOCK_LEN bytes starting at ARG_BLOCK_PTR.
; Compute the DJB2-style hash used by logHash: hash = ((hash << 5) + hash) ^ byte
; Seeds VALUE32 with 0x1505, folds ARG_BLOCK_LEN bytes from ARG_BLOCK_PTR, and leaves the result in VALUE32.
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
    ; Loop until ARG_BLOCK_LEN reaches zero (length tracked as u16).
    lda rtst.ARG_BLOCK_LEN
    ora rtst.ARG_BLOCK_LEN + 1
    beq compute_hash_done
    lda (rtst.ARG_BLOCK_PTR), y
    ; Stash byte so hashStep can consume it after pointer/len bookkeeping.
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

; One iteration of the simple rolling hash used by logHash.
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
    ; tmp = hash << 5 performed via 5 ASL/ROL steps.
    asl rtst.TMP32
    rol rtst.TMP32 + 1
    rol rtst.TMP32 + 2
    rol rtst.TMP32 + 3
    dex
    bne hash_step_shift
    clc
    ; tmp += hash (tmp now equals (hash << 5) + hash).
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
    ; XOR the original data byte into the low word (stored in TMPBYTE earlier).
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

; Emit the END record and mark state DONE.
emitEndRecord .proc
    lda #rtst.RECORD_END
    jsr startRecord
    jsr finishRecord
    lda #rtst.STATE_DONE
    sta rtst.BASE + rtst.HEADER_STATE
    rts
.endproc

; Helper macro: load a pointer (label) into the requested ZP slot.
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

; Helper macro: store an immediate word into ARG_BLOCK_LEN.
storeLen .macro value
    lda #((\value) & $FF)
    sta rtst.ARG_BLOCK_LEN
    lda #((\value >> 8) & $FF)
    sta rtst.ARG_BLOCK_LEN + 1
.endmacro

; Helper macro: store an immediate dword into VALUE32.
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

; Helper macro: increment a 16-bit field inside the header (TOTAL/PASS/FAIL).
incField .macro offset
    inc rtst.BASE + \offset
    bne inc_field_ok
    inc rtst.BASE + \offset + 1
inc_field_ok
.endmacro

; Helper macro: copy a string payload or emit a single 0 byte if null pointer provided.
copyStringOrZero .macro ptr
    .if \ptr = 0
        lda #0
        jsr rtst.emitByte
    .else
        .rtst.loadPtr rtst.ARG_KEY_PTR, \ptr
        jsr rtst.copyCStr
    .endif
.endmacro

; Public macro: jump into rtst._begin initialiser.
begin .macro
    jsr rtst._begin
.endmacro

; Public macro: emit END record and spin forever to keep memory stable.
end .macro
    jsr rtst.emitEndRecord
\@wait
    jmp \@wait
.endmacro

.endnamespace ; rtst

ctest .namespace

; Public macro: jump into rtst._begin initialiser.
begin .macro namePtr
    lda #rtst.RECORD_CASE_BEGIN
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \namePtr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_TOTAL
.endmacro

OK .macro code, messagePtr
    lda #rtst.RECORD_CASE_OK
    jsr rtst.startRecord
    lda #((\code) & $FF)
    jsr rtst.emitByte
    .rtst.copyStringOrZero \messagePtr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_PASS
.endmacro

FAIL .macro code, messagePtr
    lda #rtst.RECORD_CASE_FAIL
    jsr rtst.startRecord
    lda #((\code) & $FF)
    jsr rtst.emitByte
    .rtst.copyStringOrZero \messagePtr
    jsr rtst.finishRecord
    .rtst.incField rtst.HEADER_FAIL
.endmacro

; Public macro: emit a MSG record for free-form text diagnostics.
logMsg .macro messagePtr
    lda #rtst.RECORD_MSG
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \messagePtr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
.endmacro

; Public macro: emit an ASSERT record to mirror 6502-side assertions.
logAssert .macro messagePtr
    lda #rtst.RECORD_ASSERT
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \messagePtr
    jsr rtst.copyCStr
    jsr rtst.finishRecord
.endmacro

logKV .macro keyPtr, value
    lda #rtst.RECORD_ACT_KV
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \keyPtr
    jsr rtst.copyCStr
    .rtst.storeValue32 \value
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

; Public macro: emit an ACT_TIME record (key/u32 cycles).
logTime .macro keyPtr, value
    lda #rtst.RECORD_ACT_TIME
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \keyPtr
    jsr rtst.copyCStr
    .rtst.storeValue32 \value
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

; Public macro: emit an ACT_MEM record by copying a block from dataPtr.
logMem .macro keyPtr, dataPtr, length
    lda #rtst.RECORD_ACT_MEM
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \keyPtr
    jsr rtst.copyCStr
    .rtst.storeLen \length
    jsr rtst.emitArgLen
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \dataPtr
    jsr rtst.copyBlock
    jsr rtst.finishRecord
.endmacro

; Public macro: emit an ACT_HASH record by hashing a block.
logHash .macro keyPtr, dataPtr, length
    lda #rtst.RECORD_ACT_HASH
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \keyPtr
    jsr rtst.copyCStr
    .rtst.storeLen \length
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \dataPtr
    jsr rtst.computeHash
    jsr rtst.emitValue32
    jsr rtst.finishRecord
.endmacro

; Public macro: emit an ACT_REGS snapshot (A/X/Y/SP/P/PC).
logRegs .macro keyPtr, regsPtr
    lda #rtst.RECORD_ACT_REGS
    jsr rtst.startRecord
    .rtst.loadPtr rtst.ARG_KEY_PTR, \keyPtr
    jsr rtst.copyCStr
    .rtst.storeLen 7
    .rtst.loadPtr rtst.ARG_BLOCK_PTR, \regsPtr
    jsr rtst.copyBlock
    jsr rtst.finishRecord
.endmacro

.endnamespace ; ctest

.endif ; include guard