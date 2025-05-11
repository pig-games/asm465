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

    #debug.infoPtr InfoPtrStr
    #debug.infoLnPtr InfoPtrLnStr

    #debug.warningPtr WarningPtrStr
    #debug.warningLnPtr WarningPtrLnStr

    #debug.error "[#debug.error {shift-2}...{shift-2}], "
    #debug.errorLn "[#debug.errorLn {shift-2}...{shift-2}]"

    #debug.errorPtr ErrorPtrStr
    #debug.errorLnPtr ErrorPtrLnStr

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

    #nl
    #debug.info "[#infoRegHex] A=1: "
    lda #1
    #debug.infoRegHex
    #nl
    #debug.info "[#infoRegHex {shift-2}a{shift-2}] A=$1a: "
    lda #$1a
    #debug.infoRegHex "a"
    #nl
    #debug.info "[#infoRegHex {shift-2}x{shift-2}] X=$9f: "
    ldx #$9f
    #debug.infoRegHex "x"
    #nl
    #debug.info "[#infoRegHex {shift-2}y{shift-2}] Y=$a0: "
    ldy #$a0
    #debug.infoRegHex "y"
    #nl
    #debug.info "[#infoRegHex {shift-2}z{shift-2}] Z=$f9: "
    ldz #$f9
    #debug.infoRegHex "z"
    #nl

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

    #debug.info "[#warningRegHex] A=$ae: "
    lda #$ae
    #debug.warningRegHex
    #nl
    #debug.info "[#errorRegHex] A=1f: "
    lda #$1f
    #debug.errorRegHex
    #nl

    lda #$fe
    sta toDec.In
    lda #0
    sta toDec.In+1
    jsr toDec
    #debug.info "decimal converted $fe: "
    #ldbcd24 toDec.Out
    jsr cPrintBCD24

    #nl
    #nl
    #debug.Stats
    jmp *

inside .proc
    #debug.warning "[#debug.warning {shift-2}...{shift-2}] inside, "
    #debug.warningLn "[#debug.warningLn {shift-2}...{shift-2}] inside"
    rts
.endproc

InfoPtrStr     .null "[#debug.infoPtr {shift-2}...{shift-2}], "
InfoPtrLnStr   .null "[#debug.infoLnPtr {shift-2}...{shift-2}]"
WarningPtrStr     .null "[#debug.warningPtr {shift-2}...{shift-2}], "
WarningPtrLnStr   .null "[#debug.warningLnPtr {shift-2}...{shift-2}]"
ErrorPtrStr     .null "[#debug.errorPtr {shift-2}...{shift-2}], "
ErrorPtrLnStr   .null "[#debug.errorLnPtr {shift-2}...{shift-2}]"

.endsection ; main
