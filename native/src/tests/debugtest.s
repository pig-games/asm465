.include "debug_macros.h"

.enc "screen"

.section main
    .dbg.setupDebugStats

    .ClearScreen 1
    jsr setLowerCase

    .dbg.info "Debug functions tests!n"

    .dbg.info "[.dbg.info {shift-2}...!!n{shift-2}]!n"
    .dbg.info "[.dbg.info {shift-2}...{shift-2}], "
    .dbg.infoC 4, "[.dbg.InfoC 4, {shift-2}...{shift-2}], "

    .dbg.info "[.dbg.info {shift-2}...!!n{shift-2}] before proc!n"
    jsr inside
    .dbg.info "[.dbg.info {shift-2}...!!n{shift-2}] after proc!n"

    .dbg.infoPtr InfoPtrStr
    .dbg.infoPtr InfoPtrLnStr

    .dbg.warningPtr WarningPtrStr
    .dbg.warningPtr WarningPtrLnStr

    .dbg.error "[.dbg.error {shift-2}...{shift-2}], "
    .dbg.error "[.dbg.error {shift-2}...!!n{shift-2}]!n"

    .dbg.errorPtr ErrorPtrStr
    .dbg.errorPtr ErrorPtrLnStr

    .dbg.info "[.dbg.infoRegDec] A=1: "
    lda #1
    .dbg.infoRegDec
    .nl
    .dbg.info "[.dbg.infoRegDec {shift-2}a{shift-2}] A=2: "
    lda #2
    .dbg.infoRegDec "a"
    .nl
    .dbg.info "[.dbg.infoRegDec {shift-2}x{shift-2}] X=3: "
    ldx #3
    .dbg.infoRegDec "x"
    .nl
    .dbg.info "[.dbg.infoRegDec {shift-2}y{shift-2}] Y=4: "
    ldy #4
    .dbg.infoRegDec "y"
    .nl
    .if MEGA65
        .dbg.info "[.dbg.infoRegDec {shift-2}z{shift-2}, {shift-2}pre:{shift-2}, {shift-2}:post{shift-2}] Z=5: "
        ldz #5
        .dbg.infoRegDec "z", "pre:", ":post!n"
    .endif
    .dbg.info "[.dbg.infoRegHex] A=1: "
    lda #1
    .dbg.infoRegHex
    .nl
    .dbg.info "[.dbg.infoRegHex {shift-2}a{shift-2}] A=$1a: "
    lda #$1a
    .dbg.infoRegHex "a"
    .nl
    .dbg.info "[.dbg.infoRegHex {shift-2}x{shift-2}] X=$9f: "
    ldx #$9f
    .dbg.infoRegHex "x"
    .nl
    .dbg.info "[.dbg.infoRegHex {shift-2}y{shift-2}] Y=$a0: "
    ldy #$a0
    .dbg.infoRegHex "y"
    .nl
    .if MEGA65
        .dbg.info "[.dbg.infoRegHex {shift-2}z{shift-2}] Z=$f9: "
        ldz #$f9
        .dbg.infoRegHex "z"
        .nl
    .endif

    .dbg.info "[.dbg.infoCReg 4] A=1: "
    lda #1
    .dbg.infoCReg 4
    .nl
    .dbg.info "[.dbg.infoCReg 4, {shift-2}a{shift-2}] A=2: "
    lda #2
    .dbg.infoCReg 4, "a"
    .nl
    .dbg.info "[.dbg.infoCReg 4, {shift-2}x{shift-2}] X=3: "
    ldx #3
    .dbg.infoCReg 4, "x"
    .nl
    .dbg.info "[.dbg.infoCReg 4, {shift-2}y{shift-2}] Y=4: "
    ldy #4
    .dbg.infoCReg 4, "y"
    .nl
    .if MEGA65
    .dbg.info "[.dbg.infoCReg 4, {shift-2}z{shift-2}] Z=5: "
        ldz #5
        .dbg.infoCReg 4, "z"
        .nl
    .endif
    .dbg.info "[.dbg.warningReg] A=1: "
    lda #1
    .dbg.warningReg
    .nl
    .dbg.info "[.dbg.errorReg] A=2: "
    lda #2
    .dbg.errorReg
    .nl

    .dbg.info "[.dbg.warningRegDec] A=1: "
    lda #1
    .dbg.warningRegDec
    .nl
    .dbg.info "[.dbg.errorRegDec] A=2: "
    lda #2
    .dbg.errorRegDec
    .nl

    .dbg.info "[.dbg.warningRegHex] A=$ae: "
    lda #$ae
    .dbg.warningRegHex
    .nl
    .dbg.info "[.dbg.errorRegHex] A=1f: "
    lda #$1f
    .dbg.errorRegHex
    .nl

    lda #$fe
    sta toDec.In
    lda #0
    sta toDec.In+1
    jsr toDec
    .dbg.info "decimal converted $fe: "
    #ldbcd24 toDec.Out
    jsr cPrintBCD24

    .nl
    .nl
    .dbg.Stats
    rts

inside .proc
    .dbg.warning "[.dbg.warning {shift-2}...{shift-2}] inside, "
    .dbg.warning "[.dbg.warning {shift-2}...!!n{shift-2}] inside!n"
    rts
.endproc

InfoPtrStr     .null "[.dbg.infoPtr {shift-2}...{shift-2}], "
InfoPtrLnStr   .null "[.dbg.infoPtr {shift-2}.../b{shift-2}]!n"
WarningPtrStr     .null "[.dbg.warningPtr {shift-2}...{shift-2}], "
WarningPtrLnStr   .null "[.dbg.warningPtr {shift-2}...!!n{shift-2}]!n"
ErrorPtrStr     .null "[.dbg.errorPtr {shift-2}...{shift-2}], "
ErrorPtrLnStr   .null "[.dbg.errorPtr {shift-2}...!!n{shift-2}]!n"

.endsection ; main
