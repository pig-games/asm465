.cpu "4510"
.enc "screen"

.section main
    #debug.setupDebugStats

    #ClearScreen 1
    jsr setLowerCase

    #debug.infoLn "Debug functions tests"
    #nl

    #debug.info "[#debug.info {shift-2}...{shift-2}], "
    #debug.infoC 4, "[#debug.InfoC 4, {shift-2}...{shift-2}, "
    #debug.infoLn "[#debug.infoLn {shift-2}...{shift-2}]"

    #debug.infoLn "[#debug.infoLn {shift-2}...{shift-2}] before proc"
    jsr inside
    #debug.infoLn "[#debug.infoLn {shift-2}...{shift-2}] after proc"

    #debug.error "[#debug.error {shift-2}...{shift-2}], "
    #debug.errorLn "[#debug.errorLn {shift-2}...{shift-2}]"

    #nl
    #debug.info "[#infoRegDec] A=1: "
    lda #1
    #debug.infoRegDec
    #nl
    #debug.info "[#infoRegDec {shift-2}a{shift-2}] A=2: "
    lda #2
    #debug.infoRegDec "a"
    #nl
    #debug.info "[#infoRegDec {shift-2}x{shift-2}] X=3: "
    ldx #3
    #debug.infoRegDec "x"
    #nl
    #debug.info "[#infoRegDec {shift-2}y{shift-2}] Y=4: "
    ldy #4
    #debug.infoRegDec "y"
    #nl
    #debug.info "[#infoRegDec {shift-2}z{shift-2}] Z=5: "
    ldz #5
    #debug.infoRegDec "z"
    #nl

    #debug.info "[#infoCReg 4] A=1: "
    lda #1
    #debug.infoCReg 4
    #nl
    #debug.info "[#infoCReg 4, {shift-2}a{shift-2}] A=2: "
    lda #2
    #debug.infoCReg 4, "a"
    #nl
    #debug.info "[#infoCReg 4, {shift-2}x{shift-2}] X=3: "
    ldx #3
    #debug.infoCReg 4, "x"
    #nl
    #debug.info "[#infoCReg 4, {shift-2}y{shift-2}] Y=4: "
    ldy #4
    #debug.infoCReg 4, "y"
    #nl
    #debug.info "[#infoCReg 4, {shift-2}z{shift-2}] Z=5: "
    ldz #5
    #debug.infoCReg 4, "z"
    #nl

    #debug.info "[#warningReg] A=1: "
    lda #1
    #debug.warningReg
    #nl
    #debug.info "[#errorReg] A=2: "
    lda #2
    #debug.errorReg
    #nl

    #debug.info "[#warningRegDec] A=1: "
    lda #1
    #debug.warningRegDec
    #nl
    #debug.info "[#errorRegDec] A=2: "
    lda #2
    #debug.errorRegDec
    #nl

    lda #$fe
    sta toDec.In
    lda #0
    sta toDec.In+1
    jsr toDec
    #ldbcd24 toDec.Out
    #debug.info "decimal converted $b: "
    jsr cPrintBCD24

    #nl
    #nl
    #debug.Stats
    jmp *

inside .proc
    #debug.warning "[#debug.warning {shift-2}...{shift-2}] inside, "
    #debug.warningLn "[#debug.warningLn {shift-2}...{shift-2}] inside, "
    rts
.endproc

str .null "StringPtr"

.endsection ; main
